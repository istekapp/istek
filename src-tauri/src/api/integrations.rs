use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, SecretProvider};
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

impl From<SecretProvider> for IntegrationResponse {
    fn from(p: SecretProvider) -> Self {
        IntegrationResponse {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            enabled: p.enabled,
            config: p.config,
            created_at: p.created_at,
        }
    }
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
        (status = 200, description = "List of integrations", body = PaginatedResponseSchema)
    ),
    tag = "Integrations"
)]
pub async fn list_integrations(
    State(storage): State<Arc<Storage>>,
    Path(_workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = providers.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, providers.len());
    
    let items: Vec<IntegrationResponse> = providers
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(IntegrationResponse::from)
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
    State(storage): State<Arc<Storage>>,
    Path(_workspace_id): Path<String>,
    Json(req): Json<CreateIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    
    let provider = SecretProvider {
        id: id.clone(),
        name: req.name.clone(),
        provider_type: req.provider_type.clone(),
        enabled: req.enabled,
        config: req.config.clone(),
        created_at,
    };
    
    storage.save_secret_provider(&provider)
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let provider = providers.into_iter()
        .find(|p| p.id == integration_id)
        .ok_or_else(|| ApiError::not_found("Integration not found"))?;
    
    Ok(Json(IntegrationResponse::from(provider)))
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(req): Json<UpdateIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let mut provider = providers.into_iter()
        .find(|p| p.id == integration_id)
        .ok_or_else(|| ApiError::not_found("Integration not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        provider.name = name;
    }
    if let Some(provider_type) = req.provider_type {
        provider.provider_type = provider_type;
    }
    if let Some(enabled) = req.enabled {
        provider.enabled = enabled;
    }
    if req.config.is_some() {
        provider.config = req.config;
    }
    
    storage.save_secret_provider(&provider)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(IntegrationResponse::from(provider)))
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    if !providers.iter().any(|p| p.id == integration_id) {
        return Err(ApiError::not_found("Integration not found"));
    }
    
    storage.delete_secret_provider(&integration_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let mut provider = providers.into_iter()
        .find(|p| p.id == integration_id)
        .ok_or_else(|| ApiError::not_found("Integration not found"))?;
    
    // Toggle
    provider.enabled = !provider.enabled;
    
    storage.save_secret_provider(&provider)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(IntegrationResponse::from(provider)))
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(_req): Json<TestIntegrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let provider = providers.into_iter()
        .find(|p| p.id == integration_id)
        .ok_or_else(|| ApiError::not_found("Integration not found"))?;
    
    // TODO: Implement actual integration testing based on provider_type
    // For now, just return a placeholder response
    let response = match provider.provider_type.as_str() {
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
            error: Some(format!("Unknown provider type: {}", provider.provider_type)),
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
    State(storage): State<Arc<Storage>>,
    Path((_workspace_id, integration_id)): Path<(String, String)>,
    Json(req): Json<FetchSecretRequest>,
) -> Result<Json<FetchSecretResponse>, ApiError> {
    let providers = storage.get_secret_providers()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let provider = providers.into_iter()
        .find(|p| p.id == integration_id)
        .ok_or_else(|| ApiError::not_found("Integration not found"))?;
    
    if !provider.enabled {
        return Err(ApiError::bad_request("Integration is disabled"));
    }
    
    // TODO: Implement actual secret fetching based on provider_type and config
    // For now, return a placeholder error
    Err(ApiError::bad_request(format!(
        "Secret fetching not implemented. Requested key: {}",
        req.key
    )))
}
