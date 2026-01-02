use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::database::Database;
use super::{ApiError, PaginatedResponse, PaginationQuery, SuccessResponse};

// Request/Response types for Secret Providers (integrations)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateIntegrationRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub config: Option<Value>,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIntegrationRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub provider_type: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub config: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestIntegrationRequest {
    #[serde(default)]
    pub test_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestIntegrationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetchSecretRequest {
    pub key: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetchSecretResponse {
    pub key: String,
    pub value: String,
}

/// List all integrations (secret providers)
/// Note: Integrations are global, not workspace-scoped, but we include workspace_id for consistency
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/integrations",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID (for consistency, integrations are global)"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of integrations", body = PaginatedResponse<IntegrationResponse>)
    ),
    tag = "Integrations"
)]
pub async fn list_integrations(
    State(db): State<Arc<Database>>,
    Path(_workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count (integrations are global)
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM secret_providers",
            [],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items
    let mut stmt = conn
        .prepare("SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers ORDER BY created_at ASC LIMIT ?1 OFFSET ?2")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<IntegrationResponse> = stmt
        .query_map(rusqlite::params![pagination.limit, pagination.offset], |row| {
            let config_str: Option<String> = row.get(4)?;
            
            Ok(IntegrationResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
                config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| ApiError::internal_error(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    
    Ok(Json(PaginatedResponse {
        items,
        total,
        limit: pagination.limit,
        offset: pagination.offset,
    }))
}

/// Create a new integration (secret provider)
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/integrations",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = CreateIntegrationRequest,
    responses(
        (status = 201, description = "Integration created", body = IntegrationResponse),
        (status = 400, description = "Invalid request", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn create_integration(
    State(db): State<Arc<Database>>,
    Path(_workspace_id): Path<String>,
    Json(req): Json<CreateIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    let config_str = req.config.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default());
    
    conn.execute(
        "INSERT INTO secret_providers (id, name, provider_type, enabled, config, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, req.name, req.provider_type, req.enabled as i32, config_str, created_at],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(IntegrationResponse {
            id,
            name: req.name,
            provider_type: req.provider_type,
            enabled: req.enabled,
            config: req.config,
            created_at,
        })
    ))
}

/// Get an integration by ID
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration found", body = IntegrationResponse),
        (status = 404, description = "Integration not found", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn get_integration(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let integration = conn
        .query_row(
            "SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| {
                let config_str: Option<String> = row.get(4)?;
                
                Ok(IntegrationResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    Ok(Json(integration))
}

/// Update an integration
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    request_body = UpdateIntegrationRequest,
    responses(
        (status = 200, description = "Integration updated", body = IntegrationResponse),
        (status = 404, description = "Integration not found", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn update_integration(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(req): Json<UpdateIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get existing integration
    let mut integration = conn
        .query_row(
            "SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| {
                let config_str: Option<String> = row.get(4)?;
                
                Ok(IntegrationResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        integration.name = name;
    }
    if let Some(provider_type) = req.provider_type {
        integration.provider_type = provider_type;
    }
    if let Some(enabled) = req.enabled {
        integration.enabled = enabled;
    }
    if req.config.is_some() {
        integration.config = req.config;
    }
    
    let config_str = integration.config.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default());
    
    conn.execute(
        "UPDATE secret_providers SET name = ?1, provider_type = ?2, enabled = ?3, config = ?4 WHERE id = ?5",
        rusqlite::params![integration.name, integration.provider_type, integration.enabled as i32, config_str, integration_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(integration))
}

/// Delete an integration
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration deleted", body = SuccessResponse),
        (status = 404, description = "Integration not found", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn delete_integration(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute(
            "DELETE FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("Integration not found"));
    }
    
    Ok(Json(SuccessResponse::ok()))
}

/// Toggle an integration's enabled state
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}/toggle",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    responses(
        (status = 200, description = "Integration toggled", body = IntegrationResponse),
        (status = 404, description = "Integration not found", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn toggle_integration(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get current state
    let current_enabled: bool = conn
        .query_row(
            "SELECT enabled FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| Ok(row.get::<_, i32>(0)? != 0),
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    // Toggle
    let new_enabled = !current_enabled;
    conn.execute(
        "UPDATE secret_providers SET enabled = ?1 WHERE id = ?2",
        rusqlite::params![new_enabled as i32, integration_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Return updated integration
    let integration = conn
        .query_row(
            "SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| {
                let config_str: Option<String> = row.get(4)?;
                
                Ok(IntegrationResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    Ok(Json(integration))
}

/// Test an integration connection
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}/test",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    request_body = TestIntegrationRequest,
    responses(
        (status = 200, description = "Test result", body = TestIntegrationResponse),
        (status = 404, description = "Integration not found", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn test_integration(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(_req): Json<TestIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Verify integration exists
    let (provider_type, config_str): (String, Option<String>) = conn
        .query_row(
            "SELECT provider_type, config FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    // TODO: Implement actual integration testing based on provider_type
    // For now, just return a placeholder response
    let _ = config_str; // Silence unused warning
    
    let response = match provider_type.as_str() {
        "1password" => TestIntegrationResponse {
            success: true,
            message: Some("1Password integration configured (actual test not implemented)".to_string()),
            error: None,
        },
        "hashicorp_vault" => TestIntegrationResponse {
            success: true,
            message: Some("HashiCorp Vault integration configured (actual test not implemented)".to_string()),
            error: None,
        },
        "aws_secrets_manager" => TestIntegrationResponse {
            success: true,
            message: Some("AWS Secrets Manager integration configured (actual test not implemented)".to_string()),
            error: None,
        },
        _ => TestIntegrationResponse {
            success: false,
            message: None,
            error: Some(format!("Unknown provider type: {}", provider_type)),
        },
    };
    
    Ok(Json(response))
}

/// Fetch a secret value from an integration
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/integrations/{integration_id}/fetch",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("integration_id" = String, Path, description = "Integration ID")
    ),
    request_body = FetchSecretRequest,
    responses(
        (status = 200, description = "Secret fetched", body = FetchSecretResponse),
        (status = 404, description = "Integration not found", body = ApiError),
        (status = 400, description = "Failed to fetch secret", body = ApiError)
    ),
    tag = "Integrations"
)]
pub async fn fetch_secret(
    State(db): State<Arc<Database>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(req): Json<FetchSecretRequest>,
) -> Result<Json<FetchSecretResponse>, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Verify integration exists and is enabled
    let (enabled, _provider_type, _config_str): (bool, String, Option<String>) = conn
        .query_row(
            "SELECT enabled, provider_type, config FROM secret_providers WHERE id = ?1",
            rusqlite::params![integration_id],
            |row| Ok((row.get::<_, i32>(0)? != 0, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| ApiError::not_found("Integration not found"))?;
    
    if !enabled {
        return Err(ApiError::bad_request("Integration is disabled"));
    }
    
    // TODO: Implement actual secret fetching based on provider_type and config
    // For now, return a placeholder error
    Err(ApiError::bad_request(format!(
        "Secret fetching not implemented. Requested key: {}",
        req.key
    )))
}
