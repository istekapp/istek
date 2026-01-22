use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

// ============ YAML Export/Import for Collections ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedRequest {
    pub id: String,
    pub name: String,
    pub protocol: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFolder {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<Vec<ExportedRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<ExportedFolder>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedCollection {
    pub version: String,
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<ExportedFolder>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<Vec<ExportedRequest>>,
    pub created_at: i64,
}

/// Remove fields from a JSON object that are already in the ExportedRequest struct
fn strip_request_meta_fields(req: &serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(map) = req {
        let mut new_map = serde_json::Map::new();
        for (key, value) in map {
            // Skip fields that are already defined in ExportedRequest struct
            if key != "id" && key != "name" && key != "protocol" {
                new_map.insert(key.clone(), value.clone());
            }
        }
        serde_json::Value::Object(new_map)
    } else {
        req.clone()
    }
}

/// Convert a collection JSON to the export format
fn convert_to_export_format(collection: &serde_json::Value) -> Result<ExportedCollection, String> {
    let id = collection.get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing collection id")?
        .to_string();
    
    let name = collection.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing collection name")?
        .to_string();
    
    let created_at = collection.get("createdAt")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    
    let settings = collection.get("settings").cloned();
    
    // Convert requests
    let requests = collection.get("requests")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|req| {
                let id = req.get("id").and_then(|v| v.as_str())?.to_string();
                let name = req.get("name").and_then(|v| v.as_str())?.to_string();
                let protocol = req.get("protocol").and_then(|v| v.as_str())?.to_string();
                Some(ExportedRequest {
                    id,
                    name,
                    protocol,
                    data: strip_request_meta_fields(req),
                })
            }).collect()
        });
    
    // Convert folders
    let folders = collection.get("folders")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|folder| convert_folder(folder).ok()).collect()
        });
    
    Ok(ExportedCollection {
        version: "1.0".to_string(),
        id,
        name,
        settings,
        folders,
        requests,
        created_at,
    })
}

fn convert_folder(folder: &serde_json::Value) -> Result<ExportedFolder, String> {
    let id = folder.get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing folder id")?
        .to_string();
    
    let name = folder.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing folder name")?
        .to_string();
    
    let settings = folder.get("settings").cloned();
    
    let requests = folder.get("requests")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|req| {
                let id = req.get("id").and_then(|v| v.as_str())?.to_string();
                let name = req.get("name").and_then(|v| v.as_str())?.to_string();
                let protocol = req.get("protocol").and_then(|v| v.as_str())?.to_string();
                Some(ExportedRequest {
                    id,
                    name,
                    protocol,
                    data: strip_request_meta_fields(req),
                })
            }).collect()
        });
    
    let folders = folder.get("folders")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|f| convert_folder(f).ok()).collect()
        });
    
    Ok(ExportedFolder {
        id,
        name,
        settings,
        requests,
        folders,
    })
}

/// Export a single collection to YAML format
#[tauri::command]
pub async fn export_collection_yaml(collection: serde_json::Value) -> Result<String, String> {
    let exported = convert_to_export_format(&collection)?;
    serde_yaml::to_string(&exported)
        .map_err(|e| format!("Failed to serialize to YAML: {}", e))
}

/// Import a collection from YAML format
#[tauri::command]
pub async fn import_collection_yaml(yaml_content: String) -> Result<serde_json::Value, String> {
    let exported: ExportedCollection = serde_yaml::from_str(&yaml_content)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;
    
    // Convert back to collection format
    let requests: Vec<serde_json::Value> = exported.requests
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.data)
        .collect();
    
    let folders: Option<Vec<serde_json::Value>> = exported.folders.map(|folders| {
        folders.into_iter()
            .map(|f| convert_folder_back(&f))
            .collect()
    });
    
    let mut collection = serde_json::json!({
        "id": exported.id,
        "name": exported.name,
        "requests": requests,
        "createdAt": exported.created_at,
    });
    
    if let Some(settings) = exported.settings {
        collection["settings"] = settings;
    }
    
    if let Some(folders) = folders {
        collection["folders"] = serde_json::json!(folders);
    }
    
    Ok(collection)
}

fn convert_folder_back(folder: &ExportedFolder) -> serde_json::Value {
    let requests: Vec<serde_json::Value> = folder.requests
        .as_ref()
        .map(|reqs| reqs.iter().map(|r| r.data.clone()).collect())
        .unwrap_or_default();
    
    let folders: Option<Vec<serde_json::Value>> = folder.folders.as_ref().map(|folders| {
        folders.iter()
            .map(|f| convert_folder_back(f))
            .collect()
    });
    
    let mut result = serde_json::json!({
        "id": folder.id,
        "name": folder.name,
        "requests": requests,
    });
    
    if let Some(settings) = &folder.settings {
        result["settings"] = settings.clone();
    }
    
    if let Some(folders) = folders {
        result["folders"] = serde_json::json!(folders);
    }
    
    result
}

/// Export all collections to a directory structure
#[tauri::command]
pub async fn export_all_collections_yaml(
    collections: Vec<serde_json::Value>,
    output_dir: String,
) -> Result<Vec<String>, String> {
    let output_path = PathBuf::from(&output_dir);
    
    // Create the directory if it doesn't exist
    fs::create_dir_all(&output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;
    
    let mut exported_files = Vec::new();
    
    for collection in collections {
        let name = collection.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("untitled");
        
        // Sanitize filename
        let safe_name: String = name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        
        let yaml_content = export_collection_yaml(collection).await?;
        let file_path = output_path.join(format!("{}.yaml", safe_name));
        
        fs::write(&file_path, &yaml_content)
            .map_err(|e| format!("Failed to write file: {}", e))?;
        
        exported_files.push(file_path.to_string_lossy().to_string());
    }
    
    Ok(exported_files)
}
