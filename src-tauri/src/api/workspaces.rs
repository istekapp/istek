use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::sync::Arc;

use crate::database::{Database, Workspace};
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
        (status = 200, description = "List of workspaces", body = PaginatedResponse<WorkspaceResponse>)
    ),
    tag = "Workspaces"
)]
pub async fn list_workspaces(
    State(db): State<Arc<Database>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items
    let mut stmt = conn
        .prepare("SELECT id, name, sync_path, is_default, created_at FROM workspaces ORDER BY created_at DESC LIMIT ?1 OFFSET ?2")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<WorkspaceResponse> = stmt
        .query_map([pagination.limit, pagination.offset], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                sync_path: row.get(2)?,
                is_default: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| ApiError::internal_error(e.to_string()))?
        .filter_map(|r| r.ok())
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
    State(db): State<Arc<Database>>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    
    conn.execute(
        "INSERT INTO workspaces (id, name, sync_path, is_default, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
        rusqlite::params![id, req.name, req.sync_path, created_at],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(WorkspaceResponse {
            id,
            name: req.name,
            sync_path: req.sync_path,
            is_default: false,
            created_at,
        })
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
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let workspace = conn
        .query_row(
            "SELECT id, name, sync_path, is_default, created_at FROM workspaces WHERE id = ?1",
            [&workspace_id],
            |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sync_path: row.get(2)?,
                    is_default: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Workspace not found"))?;
    
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
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Check if workspace exists
    let mut workspace: Workspace = conn
        .query_row(
            "SELECT id, name, sync_path, is_default, created_at FROM workspaces WHERE id = ?1",
            [&workspace_id],
            |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sync_path: row.get(2)?,
                    is_default: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Workspace not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        workspace.name = name;
    }
    if req.sync_path.is_some() {
        workspace.sync_path = req.sync_path;
    }
    
    conn.execute(
        "UPDATE workspaces SET name = ?1, sync_path = ?2 WHERE id = ?3",
        rusqlite::params![workspace.name, workspace.sync_path, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
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
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute("DELETE FROM workspaces WHERE id = ?1", [&workspace_id])
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("Workspace not found"));
    }
    
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
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Check if workspace exists
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1)",
            [&workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if !exists {
        return Err(ApiError::not_found("Workspace not found"));
    }
    
    // Update active workspace in settings
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_workspace_id', ?1)",
        [&workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(SuccessResponse::with_message("Workspace activated")))
}
