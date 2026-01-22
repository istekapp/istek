use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, Variable};
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
        (status = 200, description = "List of variables", body = PaginatedResponseSchema)
    ),
    tag = "Variables"
)]
pub async fn list_variables(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let variables = storage.get_global_variables(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = variables.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, variables.len());
    
    let items: Vec<VariableResponse> = variables
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(VariableResponse::from)
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateVariableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    
    let variable = Variable {
        id: id.clone(),
        key: req.key.clone(),
        value: req.value.clone(),
        description: req.description.clone(),
        is_secret: req.is_secret,
        secret_provider: req.secret_provider.clone(),
        enabled: req.enabled,
    };
    
    storage.save_global_variable(&workspace_id, &variable)
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, variable_id)): Path<(String, String)>,
    Json(req): Json<UpdateVariableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let variables = storage.get_global_variables(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let mut variable = variables.into_iter()
        .find(|v| v.id == variable_id)
        .ok_or_else(|| ApiError::not_found("Variable not found"))?;
    
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
    
    storage.save_global_variable(&workspace_id, &variable)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(VariableResponse::from(variable)))
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, variable_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let variables = storage.get_global_variables(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if !variables.iter().any(|v| v.id == variable_id) {
        return Err(ApiError::not_found("Variable not found"));
    }
    
    storage.delete_global_variable(&workspace_id, &variable_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::ok()))
}
