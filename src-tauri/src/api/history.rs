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
pub struct HistoryItemResponse {
    pub id: String,
    pub request: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    pub timestamp: i64,
}

/// List history items in a workspace
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/history",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of history items", body = PaginatedResponse<HistoryItemResponse>)
    ),
    tag = "History"
)]
pub async fn list_history(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count for this workspace
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM history WHERE workspace_id = ?1",
            [&workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items (newest first)
    let mut stmt = conn
        .prepare("SELECT id, request, response, timestamp FROM history WHERE workspace_id = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<HistoryItemResponse> = stmt
        .query_map(rusqlite::params![workspace_id, pagination.limit, pagination.offset], |row| {
            let request_str: String = row.get(1)?;
            let response_str: Option<String> = row.get(2)?;
            
            Ok(HistoryItemResponse {
                id: row.get(0)?,
                request: serde_json::from_str(&request_str).unwrap_or(Value::Object(serde_json::Map::new())),
                response: response_str.and_then(|s| serde_json::from_str(&s).ok()),
                timestamp: row.get(3)?,
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

/// Get a specific history item
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/history/{history_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("history_id" = String, Path, description = "History item ID")
    ),
    responses(
        (status = 200, description = "History item found", body = HistoryItemResponse),
        (status = 404, description = "History item not found", body = ApiError)
    ),
    tag = "History"
)]
pub async fn get_history_item(
    State(db): State<Arc<Database>>,
    Path((workspace_id, history_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let item = conn
        .query_row(
            "SELECT id, request, response, timestamp FROM history WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![history_id, workspace_id],
            |row| {
                let request_str: String = row.get(1)?;
                let response_str: Option<String> = row.get(2)?;
                
                Ok(HistoryItemResponse {
                    id: row.get(0)?,
                    request: serde_json::from_str(&request_str).unwrap_or(Value::Object(serde_json::Map::new())),
                    response: response_str.and_then(|s| serde_json::from_str(&s).ok()),
                    timestamp: row.get(3)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("History item not found"))?;
    
    Ok(Json(item))
}

/// Delete a specific history item
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/history/{history_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("history_id" = String, Path, description = "History item ID")
    ),
    responses(
        (status = 200, description = "History item deleted", body = SuccessResponse),
        (status = 404, description = "History item not found", body = ApiError)
    ),
    tag = "History"
)]
pub async fn delete_history_item(
    State(db): State<Arc<Database>>,
    Path((workspace_id, history_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute(
            "DELETE FROM history WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![history_id, workspace_id],
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("History item not found"));
    }
    
    Ok(Json(SuccessResponse::ok()))
}

/// Clear all history in a workspace
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/history",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    responses(
        (status = 200, description = "History cleared", body = SuccessResponse)
    ),
    tag = "History"
)]
pub async fn clear_history(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    conn.execute(
        "DELETE FROM history WHERE workspace_id = ?1",
        rusqlite::params![workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(SuccessResponse::with_message("History cleared")))
}
