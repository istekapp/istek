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

// Request/Response types
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VariableResponse {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub is_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_provider: Option<Value>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateVariableRequest {
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default)]
    pub secret_provider: Option<Value>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVariableRequest {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default)]
    pub secret_provider: Option<Value>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// List all global variables in a workspace
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/variables",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of variables", body = PaginatedResponse<VariableResponse>)
    ),
    tag = "Variables"
)]
pub async fn list_variables(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count for this workspace
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM global_variables WHERE workspace_id = ?1",
            [&workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items
    let mut stmt = conn
        .prepare("SELECT id, key, value, description, is_secret, secret_provider, enabled FROM global_variables WHERE workspace_id = ?1 LIMIT ?2 OFFSET ?3")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<VariableResponse> = stmt
        .query_map(rusqlite::params![workspace_id, pagination.limit, pagination.offset], |row| {
            let secret_provider_str: Option<String> = row.get(5)?;
            
            Ok(VariableResponse {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                description: row.get(3)?,
                is_secret: row.get::<_, i32>(4)? != 0,
                secret_provider: secret_provider_str.and_then(|s| serde_json::from_str(&s).ok()),
                enabled: row.get::<_, i32>(6)? != 0,
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

/// Create a new global variable
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/variables",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = CreateVariableRequest,
    responses(
        (status = 201, description = "Variable created", body = VariableResponse),
        (status = 400, description = "Invalid request", body = ApiError)
    ),
    tag = "Variables"
)]
pub async fn create_variable(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateVariableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let secret_provider_str = req.secret_provider.as_ref().map(|sp| serde_json::to_string(sp).unwrap_or_default());
    
    conn.execute(
        "INSERT INTO global_variables (id, key, value, description, is_secret, secret_provider, enabled, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, req.key, req.value, req.description, req.is_secret as i32, secret_provider_str, req.enabled as i32, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(VariableResponse {
            id,
            key: req.key,
            value: req.value,
            description: req.description,
            is_secret: req.is_secret,
            secret_provider: req.secret_provider,
            enabled: req.enabled,
        })
    ))
}

/// Update a global variable
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/variables/{variable_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("variable_id" = String, Path, description = "Variable ID")
    ),
    request_body = UpdateVariableRequest,
    responses(
        (status = 200, description = "Variable updated", body = VariableResponse),
        (status = 404, description = "Variable not found", body = ApiError)
    ),
    tag = "Variables"
)]
pub async fn update_variable(
    State(db): State<Arc<Database>>,
    Path((workspace_id, variable_id)): Path<(String, String)>,
    Json(req): Json<UpdateVariableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get existing variable
    let mut variable = conn
        .query_row(
            "SELECT id, key, value, description, is_secret, secret_provider, enabled FROM global_variables WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![variable_id, workspace_id],
            |row| {
                let secret_provider_str: Option<String> = row.get(5)?;
                
                Ok(VariableResponse {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    description: row.get(3)?,
                    is_secret: row.get::<_, i32>(4)? != 0,
                    secret_provider: secret_provider_str.and_then(|s| serde_json::from_str(&s).ok()),
                    enabled: row.get::<_, i32>(6)? != 0,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Variable not found"))?;
    
    // Apply updates
    if let Some(key) = req.key {
        variable.key = key;
    }
    if let Some(value) = req.value {
        variable.value = value;
    }
    if req.description.is_some() {
        variable.description = req.description;
    }
    if let Some(is_secret) = req.is_secret {
        variable.is_secret = is_secret;
    }
    if req.secret_provider.is_some() {
        variable.secret_provider = req.secret_provider;
    }
    if let Some(enabled) = req.enabled {
        variable.enabled = enabled;
    }
    
    let secret_provider_str = variable.secret_provider.as_ref().map(|sp| serde_json::to_string(sp).unwrap_or_default());
    
    conn.execute(
        "UPDATE global_variables SET key = ?1, value = ?2, description = ?3, is_secret = ?4, secret_provider = ?5, enabled = ?6 WHERE id = ?7 AND workspace_id = ?8",
        rusqlite::params![variable.key, variable.value, variable.description, variable.is_secret as i32, secret_provider_str, variable.enabled as i32, variable_id, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(variable))
}

/// Delete a global variable
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/variables/{variable_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("variable_id" = String, Path, description = "Variable ID")
    ),
    responses(
        (status = 200, description = "Variable deleted", body = SuccessResponse),
        (status = 404, description = "Variable not found", body = ApiError)
    ),
    tag = "Variables"
)]
pub async fn delete_variable(
    State(db): State<Arc<Database>>,
    Path((workspace_id, variable_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute(
            "DELETE FROM global_variables WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![variable_id, workspace_id],
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("Variable not found"));
    }
    
    Ok(Json(SuccessResponse::ok()))
}
