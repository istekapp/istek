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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub variables: Vec<VariableResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub variables: Vec<VariableResponse>,
    #[serde(default)]
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub variables: Option<Vec<VariableResponse>>,
    #[serde(default)]
    pub is_default: Option<bool>,
}

/// List all environments in a workspace
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/environments",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of environments", body = PaginatedResponse<EnvironmentResponse>)
    ),
    tag = "Environments"
)]
pub async fn list_environments(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count for this workspace
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM environments WHERE workspace_id = ?1",
            [&workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items
    let mut stmt = conn
        .prepare("SELECT id, name, color, variables, is_default, created_at FROM environments WHERE workspace_id = ?1 ORDER BY created_at ASC LIMIT ?2 OFFSET ?3")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<EnvironmentResponse> = stmt
        .query_map(rusqlite::params![workspace_id, pagination.limit, pagination.offset], |row| {
            let variables_str: String = row.get(3)?;
            let variables: Vec<VariableResponse> = serde_json::from_str(&variables_str).unwrap_or_default();
            
            Ok(EnvironmentResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                variables,
                is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
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

/// Create a new environment
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/environments",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = CreateEnvironmentRequest,
    responses(
        (status = 201, description = "Environment created", body = EnvironmentResponse),
        (status = 400, description = "Invalid request", body = ApiError)
    ),
    tag = "Environments"
)]
pub async fn create_environment(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateEnvironmentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    let variables_str = serde_json::to_string(&req.variables).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    conn.execute(
        "INSERT INTO environments (id, name, color, variables, is_default, created_at, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, req.name, req.color, variables_str, req.is_default.map(|v| v as i32), created_at, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(EnvironmentResponse {
            id,
            name: req.name,
            color: req.color,
            variables: req.variables,
            is_default: req.is_default,
            created_at,
        })
    ))
}

/// Get an environment by ID
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/environments/{environment_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("environment_id" = String, Path, description = "Environment ID")
    ),
    responses(
        (status = 200, description = "Environment found", body = EnvironmentResponse),
        (status = 404, description = "Environment not found", body = ApiError)
    ),
    tag = "Environments"
)]
pub async fn get_environment(
    State(db): State<Arc<Database>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let environment = conn
        .query_row(
            "SELECT id, name, color, variables, is_default, created_at FROM environments WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![environment_id, workspace_id],
            |row| {
                let variables_str: String = row.get(3)?;
                let variables: Vec<VariableResponse> = serde_json::from_str(&variables_str).unwrap_or_default();
                
                Ok(EnvironmentResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    variables,
                    is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Environment not found"))?;
    
    Ok(Json(environment))
}

/// Update an environment
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/environments/{environment_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("environment_id" = String, Path, description = "Environment ID")
    ),
    request_body = UpdateEnvironmentRequest,
    responses(
        (status = 200, description = "Environment updated", body = EnvironmentResponse),
        (status = 404, description = "Environment not found", body = ApiError)
    ),
    tag = "Environments"
)]
pub async fn update_environment(
    State(db): State<Arc<Database>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
    Json(req): Json<UpdateEnvironmentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get existing environment
    let mut environment = conn
        .query_row(
            "SELECT id, name, color, variables, is_default, created_at FROM environments WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![environment_id, workspace_id],
            |row| {
                let variables_str: String = row.get(3)?;
                let variables: Vec<VariableResponse> = serde_json::from_str(&variables_str).unwrap_or_default();
                
                Ok(EnvironmentResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    variables,
                    is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Environment not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        environment.name = name;
    }
    if let Some(color) = req.color {
        environment.color = color;
    }
    if let Some(variables) = req.variables {
        environment.variables = variables;
    }
    if req.is_default.is_some() {
        environment.is_default = req.is_default;
    }
    
    let variables_str = serde_json::to_string(&environment.variables).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    conn.execute(
        "UPDATE environments SET name = ?1, color = ?2, variables = ?3, is_default = ?4 WHERE id = ?5 AND workspace_id = ?6",
        rusqlite::params![environment.name, environment.color, variables_str, environment.is_default.map(|v| v as i32), environment_id, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(environment))
}

/// Delete an environment
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/environments/{environment_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("environment_id" = String, Path, description = "Environment ID")
    ),
    responses(
        (status = 200, description = "Environment deleted", body = SuccessResponse),
        (status = 404, description = "Environment not found", body = ApiError)
    ),
    tag = "Environments"
)]
pub async fn delete_environment(
    State(db): State<Arc<Database>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute(
            "DELETE FROM environments WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![environment_id, workspace_id],
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("Environment not found"));
    }
    
    // Clear active environment if it was the deleted one
    let active_env_key = format!("active_environment_id_{}", workspace_id);
    conn.execute(
        "DELETE FROM app_settings WHERE key = ?1 AND value = ?2",
        rusqlite::params![active_env_key, environment_id],
    ).ok();
    
    Ok(Json(SuccessResponse::ok()))
}

/// Activate an environment (set as active for the workspace)
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/environments/{environment_id}/activate",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("environment_id" = String, Path, description = "Environment ID")
    ),
    responses(
        (status = 200, description = "Environment activated", body = SuccessResponse),
        (status = 404, description = "Environment not found", body = ApiError)
    ),
    tag = "Environments"
)]
pub async fn activate_environment(
    State(db): State<Arc<Database>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Verify environment exists
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM environments WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![environment_id, workspace_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    
    if !exists {
        return Err(ApiError::not_found("Environment not found"));
    }
    
    // Set as active environment for this workspace
    let active_env_key = format!("active_environment_id_{}", workspace_id);
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![active_env_key, environment_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(SuccessResponse::with_message("Environment activated")))
}

/// Deactivate the active environment for a workspace
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/environments/active",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    responses(
        (status = 200, description = "Active environment cleared", body = SuccessResponse)
    ),
    tag = "Environments"
)]
pub async fn deactivate_environment(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let active_env_key = format!("active_environment_id_{}", workspace_id);
    conn.execute(
        "DELETE FROM app_settings WHERE key = ?1",
        rusqlite::params![active_env_key],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(SuccessResponse::with_message("Active environment cleared")))
}
