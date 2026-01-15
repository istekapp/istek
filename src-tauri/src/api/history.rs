use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, HistoryItem};
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

impl From<HistoryItem> for HistoryItemResponse {
    fn from(h: HistoryItem) -> Self {
        HistoryItemResponse {
            id: h.id,
            request: h.request,
            response: h.response,
            timestamp: h.timestamp,
        }
    }
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let history = storage.get_history(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = history.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, history.len());
    
    let items: Vec<HistoryItemResponse> = history
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(HistoryItemResponse::from)
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, history_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let history = storage.get_history(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let item = history.into_iter()
        .find(|h| h.id == history_id)
        .ok_or_else(|| ApiError::not_found("History item not found"))?;
    
    Ok(Json(HistoryItemResponse::from(item)))
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
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, history_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let history = storage.get_history(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if !history.iter().any(|h| h.id == history_id) {
        return Err(ApiError::not_found("History item not found"));
    }
    
    storage.delete_history_item(&workspace_id, &history_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
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
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage.clear_history(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::with_message("History cleared")))
}
