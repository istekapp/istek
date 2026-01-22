use axum::{
    extract::{Path, Query, State},
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use std::sync::Arc;

use crate::storage::{Storage, Collection};
use super::{ApiError, PaginatedResponse, PaginationQuery, SuccessResponse};

// Request/Response types
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub requests: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
    #[serde(default = "default_protocol_type")]
    pub protocol_type: String,
    pub created_at: i64,
}

fn default_protocol_type() -> String {
    "http".to_string()
}

impl From<Collection> for CollectionResponse {
    fn from(c: Collection) -> Self {
        CollectionResponse {
            id: c.id,
            name: c.name,
            requests: c.requests,
            folders: c.folders,
            settings: c.settings,
            protocol_type: c.protocol_type,
            created_at: c.created_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    pub name: String,
    #[serde(default)]
    pub requests: Option<Value>,
    #[serde(default)]
    pub folders: Option<Value>,
    #[serde(default)]
    pub settings: Option<Value>,
    #[serde(default = "default_protocol_type")]
    pub protocol_type: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub requests: Option<Value>,
    #[serde(default)]
    pub folders: Option<Value>,
    #[serde(default)]
    pub settings: Option<Value>,
    #[serde(default)]
    pub protocol_type: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddRequestToCollectionRequest {
    #[serde(default)]
    pub folder_id: Option<String>,
    pub request: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequestRequest {
    pub request: Value,
    #[serde(default)]
    pub folder_id: Option<String>,
}

/// List all collections in a workspace
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/collections",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("limit" = Option<i64>, Query, description = "Number of items to return (default: 50)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default: 0)")
    ),
    responses(
        (status = 200, description = "List of collections", body = PaginatedResponseSchema)
    ),
    tag = "Collections"
)]
pub async fn list_collections(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let collections = storage.get_collections(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    let total = collections.len() as i64;
    let start = pagination.offset as usize;
    let end = std::cmp::min(start + pagination.limit as usize, collections.len());
    
    let items: Vec<CollectionResponse> = collections
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(CollectionResponse::from)
        .collect();
    
    Ok(Json(PaginatedResponse {
        items,
        total,
        limit: pagination.limit,
        offset: pagination.offset,
    }))
}

/// Create a new collection
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/collections",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = CreateCollectionRequest,
    responses(
        (status = 201, description = "Collection created", body = CollectionResponse),
        (status = 400, description = "Invalid request", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn create_collection(
    State(storage): State<Arc<Storage>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    let requests = req.requests.unwrap_or(Value::Array(vec![]));
    
    let collection = Collection {
        id: id.clone(),
        name: req.name.clone(),
        requests: requests.clone(),
        folders: req.folders.clone(),
        settings: req.settings.clone(),
        protocol_type: req.protocol_type.clone(),
        created_at,
    };
    
    storage.save_collection(&workspace_id, &collection)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(CollectionResponse {
            id,
            name: req.name,
            requests,
            folders: req.folders,
            settings: req.settings,
            protocol_type: req.protocol_type,
            created_at,
        })
    ))
}

/// Get a collection by ID
#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    responses(
        (status = 200, description = "Collection found", body = CollectionResponse),
        (status = 404, description = "Collection not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn get_collection(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    
    Ok(Json(CollectionResponse::from(collection)))
}

/// Update a collection
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    request_body = UpdateCollectionRequest,
    responses(
        (status = 200, description = "Collection updated", body = CollectionResponse),
        (status = 404, description = "Collection not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn update_collection(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
    Json(req): Json<UpdateCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    
    // Apply updates
    if let Some(name) = req.name {
        collection.name = name;
    }
    if let Some(requests) = req.requests {
        collection.requests = requests;
    }
    if req.folders.is_some() {
        collection.folders = req.folders;
    }
    if req.settings.is_some() {
        collection.settings = req.settings;
    }
    if let Some(protocol_type) = req.protocol_type {
        collection.protocol_type = protocol_type;
    }
    
    storage.save_collection(&workspace_id, &collection)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(CollectionResponse::from(collection)))
}

/// Delete a collection
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    responses(
        (status = 200, description = "Collection deleted", body = SuccessResponse),
        (status = 404, description = "Collection not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn delete_collection(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if collection exists
    let collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if collection.is_none() {
        return Err(ApiError::not_found("Collection not found"));
    }
    
    storage.delete_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::ok()))
}

/// Add a request to a collection
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}/requests",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    request_body = AddRequestToCollectionRequest,
    responses(
        (status = 201, description = "Request added", body = SuccessResponse),
        (status = 404, description = "Collection not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn add_request(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
    Json(req): Json<AddRequestToCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    
    if let Some(folder_id) = req.folder_id {
        // Add to specific folder
        let mut folders = collection.folders.unwrap_or(Value::Array(vec![]));
        
        if let Value::Array(ref mut folder_array) = folders {
            let mut found = false;
            for folder in folder_array.iter_mut() {
                if folder.get("id").and_then(|v| v.as_str()) == Some(&folder_id) {
                    if let Some(requests) = folder.get_mut("requests") {
                        if let Value::Array(ref mut arr) = requests {
                            arr.push(req.request.clone());
                            found = true;
                        }
                    } else {
                        folder.as_object_mut().unwrap().insert("requests".to_string(), Value::Array(vec![req.request.clone()]));
                        found = true;
                    }
                    break;
                }
            }
            if !found {
                return Err(ApiError::not_found("Folder not found"));
            }
        }
        
        collection.folders = Some(folders);
    } else {
        // Add to root
        if let Value::Array(ref mut arr) = collection.requests {
            arr.push(req.request);
        }
    }
    
    storage.save_collection(&workspace_id, &collection)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok((axum::http::StatusCode::CREATED, Json(SuccessResponse::ok())))
}

/// Update a request in a collection
#[utoipa::path(
    put,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}/requests/{request_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID"),
        ("request_id" = String, Path, description = "Request ID")
    ),
    request_body = UpdateRequestRequest,
    responses(
        (status = 200, description = "Request updated", body = SuccessResponse),
        (status = 404, description = "Request not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn update_request(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id, request_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateRequestRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    
    let mut found = false;
    
    if let Some(folder_id) = req.folder_id {
        // Update in specific folder
        if let Some(ref mut folders) = collection.folders {
            if let Value::Array(ref mut folder_array) = folders {
                for folder in folder_array.iter_mut() {
                    if folder.get("id").and_then(|v| v.as_str()) == Some(&folder_id) {
                        if let Some(Value::Array(ref mut requests)) = folder.get_mut("requests") {
                            for r in requests.iter_mut() {
                                if r.get("id").and_then(|v| v.as_str()) == Some(&request_id) {
                                    *r = req.request.clone();
                                    found = true;
                                    break;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    } else {
        // Update in root requests
        if let Value::Array(ref mut arr) = collection.requests {
            for r in arr.iter_mut() {
                if r.get("id").and_then(|v| v.as_str()) == Some(&request_id) {
                    *r = req.request.clone();
                    found = true;
                    break;
                }
            }
        }
    }
    
    if !found {
        return Err(ApiError::not_found("Request not found"));
    }
    
    storage.save_collection(&workspace_id, &collection)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::ok()))
}

/// Delete a request from a collection
#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}/requests/{request_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID"),
        ("request_id" = String, Path, description = "Request ID")
    ),
    responses(
        (status = 200, description = "Request deleted", body = SuccessResponse),
        (status = 404, description = "Request not found", body = ApiError)
    ),
    tag = "Collections"
)]
pub async fn delete_request(
    State(storage): State<Arc<Storage>>,
    Path((workspace_id, collection_id, request_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let mut collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;
    
    let mut found = false;
    
    // Try to delete from root requests first
    if let Value::Array(ref mut arr) = collection.requests {
        let initial_len = arr.len();
        arr.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&request_id));
        if arr.len() < initial_len {
            found = true;
        }
    }
    
    // If not found in root, try folders
    if !found {
        if let Some(ref mut folders) = collection.folders {
            if let Value::Array(ref mut folder_array) = folders {
                for folder in folder_array.iter_mut() {
                    if let Some(Value::Array(ref mut requests)) = folder.get_mut("requests") {
                        let initial_len = requests.len();
                        requests.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&request_id));
                        if requests.len() < initial_len {
                            found = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    
    if !found {
        return Err(ApiError::not_found("Request not found"));
    }
    
    storage.save_collection(&workspace_id, &collection)
        .map_err(|e| ApiError::internal_error(e))?;
    
    Ok(Json(SuccessResponse::ok()))
}
