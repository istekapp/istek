use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, Workspace};
use super::{ApiError, PaginatedResponse, PaginationQuery, SuccessResponse};

// Request/Response types
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub name: String,
    #[serde(default)]
    pub sync_path: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub sync_path: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: String,
    pub name: String,
    pub sync_path: Option<String>,
    pub is_default: bool,
    pub created_at: i64,
}

impl From<Workspace> for WorkspaceResponse {
    fn from(w: Workspace) -> Self {
        WorkspaceResponse {
            id: w.id,
            name: w.name,
            sync_path: w.sync_path,
            is_default: w.is_default,
            created_at: w.created_at,
        }
    }
}

/// List all workspaces
#[utoipa::path(
    get,
    path = "/api/workspaces",
    params(
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of workspaces", body = PaginatedResponseSchema)
    ),
    tag = "Workspaces"
)]
pub async fn list_workspaces(
    State(storage): State<Arc<Storage>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let workspaces = storage.get_workspaces()
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = workspaces.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, workspaces.len());
    
    let items: Vec<WorkspaceResponse> = workspaces
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(WorkspaceResponse::from)
        .collect();
    
    Ok(Json(PaginatedResponse {
        items,
        total,
        limit: pagination.limit,
        offset: pagination.offset,
    }))
}

/// Create a new workspace
#[utoipa::path(
    post,
    path = "/api/workspaces",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 201, description = "Workspace created", body = WorkspaceResponse),
        (status = 400, description = "Invalid request", body = ApiError)
    ),
    tag = "Workspaces"
)]
pub async fn create_workspace(
    State(storage): State<Arc<Storage>>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let workspace = storage.create_workspace(req.name, req.sync_path)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(WorkspaceResponse::from(workspace))
    ))
}

/// Get a workspace by ID
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    responses(
        (status = 200, description = "Workspace found", body = WorkspaceResponse),
        (status = 404, description = "Workspace not found", body = ApiError)
    ),
    tag = "Workspaces"
)]
pub async fn get_workspace(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let workspace = storage.get_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Workspace not found"))?;
    
    Ok(Json(WorkspaceResponse::from(workspace)))
}

/// Update a workspace
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = UpdateWorkspaceRequest,
    responses(
        (status = 200, description = "Workspace updated", body = WorkspaceResponse),
        (status = 404, description = "Workspace not found", body = ApiError)
    ),
    tag = "Workspaces"
)]
pub async fn update_workspace(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut workspace = storage.get_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Workspace not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        workspace.name = name;
    }
    if req.sync_path.is_some() {
        workspace.sync_path = req.sync_path;
    }
    
    storage.update_workspace(&workspace)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(WorkspaceResponse::from(workspace)))
}

/// Delete a workspace
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    responses(
        (status = 200, description = "Workspace deleted", body = SuccessResponse),
        (status = 404, description = "Workspace not found", body = ApiError)
    ),
    tag = "Workspaces"
)]
pub async fn delete_workspace(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if workspace exists
    let workspace = storage.get_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if workspace.is_none() {
        return Err(ApiError::not_found("Workspace not found"));
    }
    
    storage.delete_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::ok()))
}

/// Set a workspace as active
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/activate",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    responses(
        (status = 200, description = "Workspace activated", body = SuccessResponse),
        (status = 404, description = "Workspace not found", body = ApiError)
    ),
    tag = "Workspaces"
)]
pub async fn activate_workspace(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if workspace exists
    let workspace = storage.get_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if workspace.is_none() {
        return Err(ApiError::not_found("Workspace not found"));
    }
    
    storage.set_active_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::with_message("Workspace activated")))
}
