use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, Environment, Variable};
use super::{ApiError, PaginatedResponse, PaginationQuery, SuccessResponse};

// Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

impl From<Variable> for VariableResponse {
    fn from(v: Variable) -> Self {
        VariableResponse {
            id: v.id,
            key: v.key,
            value: v.value,
            description: v.description,
            is_secret: v.is_secret,
            secret_provider: v.secret_provider,
            enabled: v.enabled,
        }
    }
}

impl From<VariableResponse> for Variable {
    fn from(v: VariableResponse) -> Self {
        Variable {
            id: v.id,
            key: v.key,
            value: v.value,
            description: v.description,
            is_secret: v.is_secret,
            secret_provider: v.secret_provider,
            enabled: v.enabled,
        }
    }
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

impl From<Environment> for EnvironmentResponse {
    fn from(e: Environment) -> Self {
        EnvironmentResponse {
            id: e.id,
            name: e.name,
            color: e.color,
            variables: e.variables.into_iter().map(VariableResponse::from).collect(),
            is_default: e.is_default,
            created_at: e.created_at,
        }
    }
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let environments = storage.get_environments(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = environments.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, environments.len());
    
    let items: Vec<EnvironmentResponse> = environments
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(EnvironmentResponse::from)
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateEnvironmentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    
    let environment = Environment {
        id: id.clone(),
        name: req.name.clone(),
        color: req.color.clone(),
        variables: req.variables.clone().into_iter().map(Variable::from).collect(),
        is_default: req.is_default,
        created_at,
    };
    
    storage.save_environment(&workspace_id, &environment)
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let environments = storage.get_environments(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let environment = environments.into_iter()
        .find(|e| e.id == environment_id)
        .ok_or_else(|| ApiError::not_found("Environment not found"))?;
    
    Ok(Json(EnvironmentResponse::from(environment)))
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
    Json(req): Json<UpdateEnvironmentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let environments = storage.get_environments(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let mut environment = environments.into_iter()
        .find(|e| e.id == environment_id)
        .ok_or_else(|| ApiError::not_found("Environment not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        environment.name = name;
    }
    if let Some(color) = req.color {
        environment.color = color;
    }
    if let Some(variables) = req.variables {
        environment.variables = variables.into_iter().map(Variable::from).collect();
    }
    if req.is_default.is_some() {
        environment.is_default = req.is_default;
    }
    
    storage.save_environment(&workspace_id, &environment)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(EnvironmentResponse::from(environment)))
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if environment exists
    let environments = storage.get_environments(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if !environments.iter().any(|e| e.id == environment_id) {
        return Err(ApiError::not_found("Environment not found"));
    }
    
    storage.delete_environment(&workspace_id, &environment_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    // Clear active environment if it was the deleted one
    let active_env_id = storage.get_active_environment_id(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if active_env_id.as_deref() == Some(&environment_id) {
        storage.set_active_environment(&workspace_id, None)
            .map_err(|e| ApiError::internal_error(e))?;
    }
    
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, environment_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify environment exists
    let environments = storage.get_environments(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if !environments.iter().any(|e| e.id == environment_id) {
        return Err(ApiError::not_found("Environment not found"));
    }
    
    storage.set_active_environment(&workspace_id, Some(&environment_id))
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage.set_active_environment(&workspace_id, None)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::with_message("Active environment cleared")))
}
