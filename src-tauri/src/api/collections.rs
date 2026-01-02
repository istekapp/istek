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
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub requests: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
    pub created_at: i64,
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
        (status = 200, description = "List of collections", body = PaginatedResponse<CollectionResponse>)
    ),
    tag = "Collections"
)]
pub async fn list_collections(
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get total count for this workspace
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collections WHERE workspace_id = ?1",
            [&workspace_id],
            |row| row.get(0),
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get paginated items
    let mut stmt = conn
        .prepare("SELECT id, name, requests, folders, settings, created_at FROM collections WHERE workspace_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3")
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let items: Vec<CollectionResponse> = stmt
        .query_map(rusqlite::params![workspace_id, pagination.limit, pagination.offset], |row| {
            let requests_str: String = row.get(2)?;
            let folders_str: Option<String> = row.get(3)?;
            let settings_str: Option<String> = row.get(4)?;
            
            Ok(CollectionResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                requests: serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![])),
                folders: folders_str.and_then(|s| serde_json::from_str(&s).ok()),
                settings: settings_str.and_then(|s| serde_json::from_str(&s).ok()),
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
    State(db): State<Arc<Database>>,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    let requests = req.requests.unwrap_or(Value::Array(vec![]));
    let requests_str = serde_json::to_string(&requests).map_err(|e| ApiError::internal_error(e.to_string()))?;
    let folders_str = req.folders.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default());
    let settings_str = req.settings.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());
    
    conn.execute(
        "INSERT INTO collections (id, name, requests, folders, settings, created_at, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, req.name, requests_str, folders_str, settings_str, created_at, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok((
        axum::http::StatusCode::CREATED,
        Json(CollectionResponse {
            id,
            name: req.name,
            requests,
            folders: req.folders,
            settings: req.settings,
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let collection = conn
        .query_row(
            "SELECT id, name, requests, folders, settings, created_at FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
            |row| {
                let requests_str: String = row.get(2)?;
                let folders_str: Option<String> = row.get(3)?;
                let settings_str: Option<String> = row.get(4)?;
                
                Ok(CollectionResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    requests: serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![])),
                    folders: folders_str.and_then(|s| serde_json::from_str(&s).ok()),
                    settings: settings_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Collection not found"))?;
    
    Ok(Json(collection))
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
    Json(req): Json<UpdateCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get existing collection
    let mut collection = conn
        .query_row(
            "SELECT id, name, requests, folders, settings, created_at FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
            |row| {
                let requests_str: String = row.get(2)?;
                let folders_str: Option<String> = row.get(3)?;
                let settings_str: Option<String> = row.get(4)?;
                
                Ok(CollectionResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    requests: serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![])),
                    folders: folders_str.and_then(|s| serde_json::from_str(&s).ok()),
                    settings: settings_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|_| ApiError::not_found("Collection not found"))?;
    
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
    
    let requests_str = serde_json::to_string(&collection.requests).map_err(|e| ApiError::internal_error(e.to_string()))?;
    let folders_str = collection.folders.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default());
    let settings_str = collection.settings.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());
    
    conn.execute(
        "UPDATE collections SET name = ?1, requests = ?2, folders = ?3, settings = ?4 WHERE id = ?5 AND workspace_id = ?6",
        rusqlite::params![collection.name, requests_str, folders_str, settings_str, collection_id, workspace_id],
    ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    Ok(Json(collection))
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let affected = conn
        .execute(
            "DELETE FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
        )
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    if affected == 0 {
        return Err(ApiError::not_found("Collection not found"));
    }
    
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id)): Path<(String, String)>,
    Json(req): Json<AddRequestToCollectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    // Get existing collection
    let (requests_str, folders_str): (String, Option<String>) = conn
        .query_row(
            "SELECT requests, folders FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ApiError::not_found("Collection not found"))?;
    
    if let Some(folder_id) = req.folder_id {
        // Add to specific folder
        let mut folders: Value = folders_str
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Array(vec![]));
        
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
        
        let folders_json = serde_json::to_string(&folders).map_err(|e| ApiError::internal_error(e.to_string()))?;
        conn.execute(
            "UPDATE collections SET folders = ?1 WHERE id = ?2 AND workspace_id = ?3",
            rusqlite::params![folders_json, collection_id, workspace_id],
        ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    } else {
        // Add to root
        let mut requests: Value = serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![]));
        if let Value::Array(ref mut arr) = requests {
            arr.push(req.request);
        }
        
        let requests_json = serde_json::to_string(&requests).map_err(|e| ApiError::internal_error(e.to_string()))?;
        conn.execute(
            "UPDATE collections SET requests = ?1 WHERE id = ?2 AND workspace_id = ?3",
            rusqlite::params![requests_json, collection_id, workspace_id],
        ).map_err(|e| ApiError::internal_error(e.to_string()))?;
    }
    
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id, request_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateRequestRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let (requests_str, folders_str): (String, Option<String>) = conn
        .query_row(
            "SELECT requests, folders FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ApiError::not_found("Collection not found"))?;
    
    let mut found = false;
    
    if let Some(folder_id) = req.folder_id {
        // Update in specific folder
        let mut folders: Value = folders_str
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Array(vec![]));
        
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
        
        if found {
            let folders_json = serde_json::to_string(&folders).map_err(|e| ApiError::internal_error(e.to_string()))?;
            conn.execute(
                "UPDATE collections SET folders = ?1 WHERE id = ?2 AND workspace_id = ?3",
                rusqlite::params![folders_json, collection_id, workspace_id],
            ).map_err(|e| ApiError::internal_error(e.to_string()))?;
        }
    } else {
        // Update in root requests
        let mut requests: Value = serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![]));
        if let Value::Array(ref mut arr) = requests {
            for r in arr.iter_mut() {
                if r.get("id").and_then(|v| v.as_str()) == Some(&request_id) {
                    *r = req.request.clone();
                    found = true;
                    break;
                }
            }
        }
        
        if found {
            let requests_json = serde_json::to_string(&requests).map_err(|e| ApiError::internal_error(e.to_string()))?;
            conn.execute(
                "UPDATE collections SET requests = ?1 WHERE id = ?2 AND workspace_id = ?3",
                rusqlite::params![requests_json, collection_id, workspace_id],
            ).map_err(|e| ApiError::internal_error(e.to_string()))?;
        }
    }
    
    if !found {
        return Err(ApiError::not_found("Request not found"));
    }
    
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
    State(db): State<Arc<Database>>,
    Path((workspace_id, collection_id, request_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let conn = db.conn.lock().map_err(|e| ApiError::internal_error(e.to_string()))?;
    
    let (requests_str, folders_str): (String, Option<String>) = conn
        .query_row(
            "SELECT requests, folders FROM collections WHERE id = ?1 AND workspace_id = ?2",
            rusqlite::params![collection_id, workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ApiError::not_found("Collection not found"))?;
    
    let mut found = false;
    
    // Try to delete from root requests first
    let mut requests: Value = serde_json::from_str(&requests_str).unwrap_or(Value::Array(vec![]));
    if let Value::Array(ref mut arr) = requests {
        let initial_len = arr.len();
        arr.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&request_id));
        if arr.len() < initial_len {
            found = true;
            let requests_json = serde_json::to_string(&requests).map_err(|e| ApiError::internal_error(e.to_string()))?;
            conn.execute(
                "UPDATE collections SET requests = ?1 WHERE id = ?2 AND workspace_id = ?3",
                rusqlite::params![requests_json, collection_id, workspace_id],
            ).map_err(|e| ApiError::internal_error(e.to_string()))?;
        }
    }
    
    // If not found in root, try folders
    if !found {
        let mut folders: Value = folders_str
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Array(vec![]));
        
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
        
        if found {
            let folders_json = serde_json::to_string(&folders).map_err(|e| ApiError::internal_error(e.to_string()))?;
            conn.execute(
                "UPDATE collections SET folders = ?1 WHERE id = ?2 AND workspace_id = ?3",
                rusqlite::params![folders_json, collection_id, workspace_id],
            ).map_err(|e| ApiError::internal_error(e.to_string()))?;
        }
    }
    
    if !found {
        return Err(ApiError::not_found("Request not found"));
    }
    
    Ok(Json(SuccessResponse::ok()))
}
