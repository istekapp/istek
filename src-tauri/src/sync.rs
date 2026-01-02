use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use git2::{Repository, Signature, StatusOptions, IndexAddOption};

use crate::database::{Database, Environment, Variable};
use crate::git_export::{export_collection_yaml, ExportedCollection};

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    pub enabled: bool,
    pub sync_path: String,
    pub sync_collections: bool,
    pub sync_environments: bool,
    pub sync_global_variables: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let sync_path = home_dir.join(".istek").join("sync");
        
        SyncConfig {
            enabled: false,
            sync_path: sync_path.to_string_lossy().to_string(),
            sync_collections: true,
            sync_environments: true,
            sync_global_variables: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncChange {
    pub change_type: String,      // "added", "modified", "deleted"
    pub resource_type: String,    // "collection", "environment", "global_variable"
    pub resource_id: String,
    pub resource_name: String,
    pub source: String,           // "local" (app) or "external" (file)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub is_initialized: bool,
    pub sync_path: String,
    pub local_changes: Vec<SyncChange>,
    pub external_changes: Vec<SyncChange>,
    pub last_sync: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub uncommitted_changes: Vec<GitFileChange>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitFileChange {
    pub path: String,
    pub status: String,  // "new", "modified", "deleted", "renamed"
}

// ============ Export Types (for YAML files) ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedVariable {
    pub id: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,  // None for secrets
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub is_secret: bool,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedEnvironment {
    pub version: String,
    pub id: String,
    pub name: String,
    pub color: String,
    pub shareable: bool,
    pub variables: Vec<ExportedVariable>,
    pub is_default: Option<bool>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportedGlobalVariables {
    pub version: String,
    pub variables: Vec<ExportedVariable>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMetadata {
    pub version: String,
    pub last_sync: Option<i64>,
    pub collections: Vec<String>,
    pub environments: Vec<String>,
}

// ============ Helper Functions ============

fn get_sync_path(config: &SyncConfig) -> PathBuf {
    PathBuf::from(&config.sync_path)
}

fn ensure_sync_directories(sync_path: &PathBuf) -> Result<(), String> {
    let collections_path = sync_path.join("collections");
    let environments_path = sync_path.join("environments");
    
    fs::create_dir_all(&collections_path)
        .map_err(|e| format!("Failed to create collections directory: {}", e))?;
    fs::create_dir_all(&environments_path)
        .map_err(|e| format!("Failed to create environments directory: {}", e))?;
    
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Internal sync version of import_collection_yaml
fn import_collection_yaml_sync(yaml_content: &str) -> Result<serde_json::Value, String> {
    let exported: ExportedCollection = serde_yaml::from_str(yaml_content)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;
    
    // Convert back to collection format
    let requests: Vec<serde_json::Value> = exported.requests
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.data)
        .collect();
    
    let folders: Option<Vec<serde_json::Value>> = exported.folders.map(|folders| {
        folders.into_iter()
            .map(|f| convert_exported_folder_back(&f))
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

fn convert_exported_folder_back(folder: &crate::git_export::ExportedFolder) -> serde_json::Value {
    let requests: Vec<serde_json::Value> = folder.requests
        .as_ref()
        .map(|reqs| reqs.iter().map(|r| r.data.clone()).collect())
        .unwrap_or_default();
    
    let folders: Option<Vec<serde_json::Value>> = folder.folders.as_ref().map(|folders| {
        folders.iter()
            .map(|f| convert_exported_folder_back(f))
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

/// Helper function to fetch all collections from database for active workspace
fn fetch_all_collections(app: &tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    
    // Get active workspace ID
    let workspace_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
            [],
            |row| row.get(0),
        )
        .ok();
    
    let mut collections: Vec<serde_json::Value> = Vec::new();
    
    if let Some(ws_id) = workspace_id {
        let mut stmt = conn.prepare("SELECT id, name, requests, folders, settings, created_at FROM collections WHERE workspace_id = ?1")
            .map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map(rusqlite::params![ws_id], |row| {
            let folders_str: Option<String> = row.get(3)?;
            let settings_str: Option<String> = row.get(4)?;
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "requests": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(2)?).unwrap_or(serde_json::json!([])),
                "folders": folders_str.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "settings": settings_str.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "createdAt": row.get::<_, i64>(5)?,
            }))
        }).map_err(|e| e.to_string())?;
        
        for row in rows {
            if let Ok(collection) = row {
                collections.push(collection);
            }
        }
    } else {
        // Fallback: get all collections if no active workspace
        let mut stmt = conn.prepare("SELECT id, name, requests, folders, settings, created_at FROM collections")
            .map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map([], |row| {
            let folders_str: Option<String> = row.get(3)?;
            let settings_str: Option<String> = row.get(4)?;
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "requests": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(2)?).unwrap_or(serde_json::json!([])),
                "folders": folders_str.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "settings": settings_str.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "createdAt": row.get::<_, i64>(5)?,
            }))
        }).map_err(|e| e.to_string())?;
        
        for row in rows {
            if let Ok(collection) = row {
                collections.push(collection);
            }
        }
    }
    
    Ok(collections)
}

fn export_variable(var: &Variable) -> ExportedVariable {
    ExportedVariable {
        id: var.id.clone(),
        key: var.key.clone(),
        // Exclude secret values - only export the key name
        value: if var.is_secret { None } else { Some(var.value.clone()) },
        description: var.description.clone(),
        is_secret: var.is_secret,
        enabled: var.enabled,
    }
}

fn import_variable(exported: &ExportedVariable, existing_value: Option<&str>) -> Variable {
    Variable {
        id: exported.id.clone(),
        key: exported.key.clone(),
        // For secrets, keep existing value if available, otherwise empty
        value: if exported.is_secret {
            existing_value.unwrap_or("").to_string()
        } else {
            exported.value.clone().unwrap_or_default()
        },
        description: exported.description.clone(),
        is_secret: exported.is_secret,
        secret_provider: None,
        enabled: exported.enabled,
    }
}

// ============ Tauri Commands ============

/// Initialize the sync directory structure
#[tauri::command]
pub async fn sync_init(app: tauri::AppHandle) -> Result<SyncConfig, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    
    // Create directory structure
    ensure_sync_directories(&sync_path)?;
    
    // Create .gitignore
    let gitignore_path = sync_path.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore_content = r#"# Local secrets (never commit)
*.local.yaml
.secrets/

# OS files
.DS_Store
Thumbs.db
"#;
        fs::write(&gitignore_path, gitignore_content)
            .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
    }
    
    // Create workspace.yaml
    let workspace_path = sync_path.join("workspace.yaml");
    if !workspace_path.exists() {
        let metadata = WorkspaceMetadata {
            version: "1.0".to_string(),
            last_sync: None,
            collections: vec![],
            environments: vec![],
        };
        let yaml_content = serde_yaml::to_string(&metadata)
            .map_err(|e| format!("Failed to serialize workspace metadata: {}", e))?;
        fs::write(&workspace_path, yaml_content)
            .map_err(|e| format!("Failed to create workspace.yaml: {}", e))?;
    }
    
    // Save config with enabled = true
    let mut new_config = config.clone();
    new_config.enabled = true;
    sync_save_config(app, new_config.clone()).await?;
    
    Ok(new_config)
}

/// Get the current sync configuration
#[tauri::command]
pub async fn sync_get_config(app: tauri::AppHandle) -> Result<SyncConfig, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    
    let config_json: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'sync_config'",
            [],
            |row| row.get(0),
        )
        .ok();
    
    match config_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse sync config: {}", e)),
        None => Ok(SyncConfig::default()),
    }
}

/// Save the sync configuration
#[tauri::command]
pub async fn sync_save_config(app: tauri::AppHandle, config: SyncConfig) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    
    let config_json = serde_json::to_string(&config)
        .map_err(|e| format!("Failed to serialize sync config: {}", e))?;
    
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('sync_config', ?1)",
        [&config_json],
    ).map_err(|e| format!("Failed to save sync config: {}", e))?;
    
    Ok(())
}

/// Get sync status including local and external changes
#[tauri::command]
pub async fn sync_get_status(app: tauri::AppHandle) -> Result<SyncStatus, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    
    if !sync_path.exists() {
        return Ok(SyncStatus {
            is_initialized: false,
            sync_path: config.sync_path,
            local_changes: vec![],
            external_changes: vec![],
            last_sync: None,
        });
    }
    
    // Get last sync time from workspace.yaml
    let workspace_path = sync_path.join("workspace.yaml");
    let last_sync = if workspace_path.exists() {
        let content = fs::read_to_string(&workspace_path)
            .map_err(|e| format!("Failed to read workspace.yaml: {}", e))?;
        let metadata: WorkspaceMetadata = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse workspace.yaml: {}", e))?;
        metadata.last_sync
    } else {
        None
    };
    
    // Detect changes
    let local_changes = detect_local_changes(&app, &config)?;
    let external_changes = detect_external_changes(&app, &config)?;
    
    Ok(SyncStatus {
        is_initialized: true,
        sync_path: config.sync_path,
        local_changes,
        external_changes,
        last_sync,
    })
}

/// Detect changes in the app that haven't been synced to files
fn detect_local_changes(app: &tauri::AppHandle, config: &SyncConfig) -> Result<Vec<SyncChange>, String> {
    let mut changes = Vec::new();
    let sync_path = get_sync_path(config);
    
    if !config.sync_collections {
        return Ok(changes);
    }
    
    // Get collections from database
    let collections: Vec<(String, String)> = {
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, name FROM collections")
            .map_err(|e| e.to_string())?;
        let result: Vec<(String, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    
    let collections_path = sync_path.join("collections");
    
    for (id, name) in collections {
        let safe_name = sanitize_filename(&name);
        let file_path = collections_path.join(format!("{}.yaml", safe_name));
        
        if !file_path.exists() {
            changes.push(SyncChange {
                change_type: "added".to_string(),
                resource_type: "collection".to_string(),
                resource_id: id,
                resource_name: name,
                source: "local".to_string(),
            });
        } else {
            // TODO: Compare file content with database content to detect modifications
            // For now, we'll mark as potentially modified
        }
    }
    
    // Check environments
    if config.sync_environments {
        let environments: Vec<(String, String, bool)> = {
            let db = app.state::<Arc<Database>>();
            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            let mut stmt = conn.prepare("SELECT id, name, shareable FROM environments")
                .map_err(|e| e.to_string())?;
            let result: Vec<(String, String, bool)> = stmt.query_map([], |row| {
                    let shareable: i32 = row.get::<_, Option<i32>>(2)?.unwrap_or(0);
                    Ok((row.get(0)?, row.get(1)?, shareable != 0))
                })
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            result
        };
        
        let environments_path = sync_path.join("environments");
        
        for (id, name, shareable) in environments {
            if !shareable {
                continue; // Skip non-shareable environments
            }
            
            let safe_name = sanitize_filename(&name);
            let file_path = environments_path.join(format!("{}.yaml", safe_name));
            
            if !file_path.exists() {
                changes.push(SyncChange {
                    change_type: "added".to_string(),
                    resource_type: "environment".to_string(),
                    resource_id: id,
                    resource_name: name,
                    source: "local".to_string(),
                });
            }
        }
    }
    
    // Check global variables
    if config.sync_global_variables {
        let global_vars_path = sync_path.join("global-variables.yaml");
        if !global_vars_path.exists() {
            let count: i32 = {
                let db = app.state::<Arc<Database>>();
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                conn.query_row(
                    "SELECT COUNT(*) FROM global_variables WHERE is_secret = 0",
                    [],
                    |row| row.get(0),
                ).unwrap_or(0)
            };
            
            if count > 0 {
                changes.push(SyncChange {
                    change_type: "added".to_string(),
                    resource_type: "global_variable".to_string(),
                    resource_id: "global".to_string(),
                    resource_name: "Global Variables".to_string(),
                    source: "local".to_string(),
                });
            }
        }
    }
    
    Ok(changes)
}

/// Detect changes in files that haven't been imported to the app
fn detect_external_changes(app: &tauri::AppHandle, config: &SyncConfig) -> Result<Vec<SyncChange>, String> {
    let mut changes = Vec::new();
    let sync_path = get_sync_path(config);
    
    // Check for new collection files
    if config.sync_collections {
        let collections_path = sync_path.join("collections");
        if collections_path.exists() {
            // Get existing IDs
            let existing_ids: Vec<String> = {
                let db = app.state::<Arc<Database>>();
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                let mut stmt = conn.prepare("SELECT id FROM collections")
                    .map_err(|e| e.to_string())?;
                let result: Vec<String> = stmt.query_map([], |row| row.get(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(|r| r.ok())
                    .collect();
                result
            };
            
            // Check files
            if let Ok(entries) = fs::read_dir(&collections_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(collection) = import_collection_yaml_sync(&content) {
                                if let Some(id) = collection.get("id").and_then(|v| v.as_str()) {
                                    if !existing_ids.contains(&id.to_string()) {
                                        let name = collection.get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Unknown");
                                        changes.push(SyncChange {
                                            change_type: "added".to_string(),
                                            resource_type: "collection".to_string(),
                                            resource_id: id.to_string(),
                                            resource_name: name.to_string(),
                                            source: "external".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Check for new environment files
    if config.sync_environments {
        let environments_path = sync_path.join("environments");
        if environments_path.exists() {
            // Get existing IDs
            let existing_ids: Vec<String> = {
                let db = app.state::<Arc<Database>>();
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                let mut stmt = conn.prepare("SELECT id FROM environments")
                    .map_err(|e| e.to_string())?;
                let result: Vec<String> = stmt.query_map([], |row| row.get(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(|r| r.ok())
                    .collect();
                result
            };
            
            // Check files
            if let Ok(entries) = fs::read_dir(&environments_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(env) = serde_yaml::from_str::<ExportedEnvironment>(&content) {
                                if !existing_ids.contains(&env.id) {
                                    changes.push(SyncChange {
                                        change_type: "added".to_string(),
                                        resource_type: "environment".to_string(),
                                        resource_id: env.id,
                                        resource_name: env.name,
                                        source: "external".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(changes)
}

/// Export all data to the sync directory
#[tauri::command]
pub async fn sync_export_all(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    
    if !config.enabled {
        return Err("Sync is not enabled".to_string());
    }
    
    let sync_path = get_sync_path(&config);
    ensure_sync_directories(&sync_path)?;
    
    let mut exported_files = Vec::new();
    
    // Export collections
    if config.sync_collections {
        let files = sync_export_collections(app.clone()).await?;
        exported_files.extend(files);
    }
    
    // Export environments
    if config.sync_environments {
        let files = sync_export_environments(app.clone()).await?;
        exported_files.extend(files);
    }
    
    // Export global variables
    if config.sync_global_variables {
        if let Ok(file) = sync_export_global_variables(app.clone()).await {
            exported_files.push(file);
        }
    }
    
    // Update workspace.yaml with last sync time
    let workspace_path = sync_path.join("workspace.yaml");
    let metadata = WorkspaceMetadata {
        version: "1.0".to_string(),
        last_sync: Some(chrono::Utc::now().timestamp()),
        collections: vec![], // TODO: populate with actual collection names
        environments: vec![], // TODO: populate with actual environment names
    };
    let yaml_content = serde_yaml::to_string(&metadata)
        .map_err(|e| format!("Failed to serialize workspace metadata: {}", e))?;
    fs::write(&workspace_path, yaml_content)
        .map_err(|e| format!("Failed to update workspace.yaml: {}", e))?;
    
    Ok(exported_files)
}

/// Export all collections to YAML files
#[tauri::command]
pub async fn sync_export_collections(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    let collections_path = sync_path.join("collections");
    
    fs::create_dir_all(&collections_path)
        .map_err(|e| format!("Failed to create collections directory: {}", e))?;
    
    // Fetch collections - use a helper function to avoid lifetime issues
    let collections = fetch_all_collections(&app)?;
    
    let mut exported_files = Vec::new();
    
    for collection in collections {
        let name = collection.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("untitled");
        
        let safe_name = sanitize_filename(name);
        let yaml_content = export_collection_yaml(collection).await?;
        let file_path = collections_path.join(format!("{}.yaml", safe_name));
        
        fs::write(&file_path, &yaml_content)
            .map_err(|e| format!("Failed to write collection file: {}", e))?;
        
        exported_files.push(file_path.to_string_lossy().to_string());
    }
    
    Ok(exported_files)
}

/// Export shareable environments to YAML files
#[tauri::command]
pub async fn sync_export_environments(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    let environments_path = sync_path.join("environments");
    
    fs::create_dir_all(&environments_path)
        .map_err(|e| format!("Failed to create environments directory: {}", e))?;
    
    // Load shareable environments in a block
    let environments: Vec<Environment> = {
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, name, color, variables, is_default, shareable, created_at FROM environments WHERE shareable = 1")
            .map_err(|e| e.to_string())?;
        let result: Vec<Environment> = stmt
            .query_map([], |row| {
                let variables_str: String = row.get(3)?;
                let variables: Vec<Variable> = serde_json::from_str(&variables_str).unwrap_or_default();
                Ok(Environment {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    variables,
                    is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    
    let mut exported_files = Vec::new();
    
    for env in environments {
        let exported_vars: Vec<ExportedVariable> = env.variables.iter()
            .map(|v| export_variable(v))
            .collect();
        
        let exported_env = ExportedEnvironment {
            version: "1.0".to_string(),
            id: env.id,
            name: env.name.clone(),
            color: env.color,
            shareable: true,
            variables: exported_vars,
            is_default: env.is_default,
            created_at: env.created_at,
        };
        
        let safe_name = sanitize_filename(&env.name);
        let yaml_content = serde_yaml::to_string(&exported_env)
            .map_err(|e| format!("Failed to serialize environment: {}", e))?;
        let file_path = environments_path.join(format!("{}.yaml", safe_name));
        
        fs::write(&file_path, &yaml_content)
            .map_err(|e| format!("Failed to write environment file: {}", e))?;
        
        exported_files.push(file_path.to_string_lossy().to_string());
    }
    
    Ok(exported_files)
}

/// Export non-secret global variables to YAML
#[tauri::command]
pub async fn sync_export_global_variables(app: tauri::AppHandle) -> Result<String, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    
    // Load global variables in a block
    let variables: Vec<Variable> = {
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, key, value, description, is_secret, enabled FROM global_variables")
            .map_err(|e| e.to_string())?;
        let result: Vec<Variable> = stmt
            .query_map([], |row| {
                Ok(Variable {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    description: row.get(3)?,
                    is_secret: row.get::<_, i32>(4)? != 0,
                    secret_provider: None,
                    enabled: row.get::<_, i32>(5)? != 0,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    
    let exported_vars: Vec<ExportedVariable> = variables.iter()
        .map(|v| export_variable(v))
        .collect();
    
    let exported = ExportedGlobalVariables {
        version: "1.0".to_string(),
        variables: exported_vars,
    };
    
    let yaml_content = serde_yaml::to_string(&exported)
        .map_err(|e| format!("Failed to serialize global variables: {}", e))?;
    
    let file_path = sync_path.join("global-variables.yaml");
    fs::write(&file_path, &yaml_content)
        .map_err(|e| format!("Failed to write global variables file: {}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

/// Import all data from the sync directory
#[tauri::command]
pub async fn sync_import_all(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    
    if !config.enabled {
        return Err("Sync is not enabled".to_string());
    }
    
    let mut imported_items = Vec::new();
    
    // Import collections
    if config.sync_collections {
        let items = sync_import_collections(app.clone()).await?;
        imported_items.extend(items);
    }
    
    // Import environments
    if config.sync_environments {
        let items = sync_import_environments(app.clone()).await?;
        imported_items.extend(items);
    }
    
    // Import global variables
    if config.sync_global_variables {
        let count = sync_import_global_variables(app.clone()).await?;
        if count > 0 {
            imported_items.push(format!("{} global variables", count));
        }
    }
    
    Ok(imported_items)
}

/// Import collections from YAML files
#[tauri::command]
pub async fn sync_import_collections(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    let collections_path = sync_path.join("collections");
    
    if !collections_path.exists() {
        return Ok(vec![]);
    }
    
    let mut imported = Vec::new();
    
    let entries = fs::read_dir(&collections_path)
        .map_err(|e| format!("Failed to read collections directory: {}", e))?;
    
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file {:?}: {}", path, e))?;
            
            let collection = import_collection_yaml_sync(&content)?;
            
            // Save to database
            let db = app.state::<Arc<Database>>();
            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            
            let id = collection.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = collection.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let requests = serde_json::to_string(collection.get("requests").unwrap_or(&serde_json::json!([]))).unwrap_or_default();
            let folders = collection.get("folders").map(|f| serde_json::to_string(f).unwrap_or_default());
            let settings = collection.get("settings").map(|s| serde_json::to_string(s).unwrap_or_default());
            let created_at = collection.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0);
            
            conn.execute(
                "INSERT OR REPLACE INTO collections (id, name, requests, folders, settings, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id, name, requests, folders, settings, created_at],
            ).map_err(|e| format!("Failed to save collection: {}", e))?;
            
            imported.push(name.to_string());
        }
    }
    
    Ok(imported)
}

/// Import environments from YAML files
#[tauri::command]
pub async fn sync_import_environments(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    let environments_path = sync_path.join("environments");
    
    if !environments_path.exists() {
        return Ok(vec![]);
    }
    
    // Get existing environment variables to preserve secret values
    let existing_secrets: HashMap<String, HashMap<String, String>> = {
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, variables FROM environments")
            .map_err(|e| e.to_string())?;
        let result: HashMap<String, HashMap<String, String>> = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let vars_str: String = row.get(1)?;
                let vars: Vec<Variable> = serde_json::from_str(&vars_str).unwrap_or_default();
                let secrets: HashMap<String, String> = vars.iter()
                    .filter(|v| v.is_secret)
                    .map(|v| (v.key.clone(), v.value.clone()))
                    .collect();
                Ok((id, secrets))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    
    let mut imported = Vec::new();
    
    let entries = fs::read_dir(&environments_path)
        .map_err(|e| format!("Failed to read environments directory: {}", e))?;
    
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file {:?}: {}", path, e))?;
            
            let exported_env: ExportedEnvironment = serde_yaml::from_str(&content)
                .map_err(|e| format!("Failed to parse environment YAML: {}", e))?;
            
            // Get existing secrets for this environment
            let env_secrets = existing_secrets.get(&exported_env.id);
            
            // Convert variables, preserving secret values
            let variables: Vec<Variable> = exported_env.variables.iter()
                .map(|v| {
                    let existing_value = env_secrets.and_then(|s| s.get(&v.key)).map(|s| s.as_str());
                    import_variable(v, existing_value)
                })
                .collect();
            
            // Save to database
            {
                let db = app.state::<Arc<Database>>();
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                conn.execute(
                    "INSERT OR REPLACE INTO environments (id, name, color, variables, is_default, shareable, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        exported_env.id,
                        exported_env.name,
                        exported_env.color,
                        serde_json::to_string(&variables).unwrap_or_default(),
                        exported_env.is_default.map(|v| v as i32),
                        1, // shareable = true
                        exported_env.created_at
                    ],
                ).map_err(|e| format!("Failed to save environment: {}", e))?;
            }
            
            imported.push(exported_env.name);
        }
    }
    
    Ok(imported)
}

/// Import global variables from YAML file
#[tauri::command]
pub async fn sync_import_global_variables(app: tauri::AppHandle) -> Result<i32, String> {
    let config = sync_get_config(app.clone()).await?;
    let sync_path = get_sync_path(&config);
    let file_path = sync_path.join("global-variables.yaml");
    
    if !file_path.exists() {
        return Ok(0);
    }
    
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read global variables file: {}", e))?;
    
    let exported: ExportedGlobalVariables = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse global variables YAML: {}", e))?;
    
    // Get existing secret values
    let existing_secrets: HashMap<String, String> = {
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT key, value FROM global_variables WHERE is_secret = 1")
            .map_err(|e| e.to_string())?;
        let result: HashMap<String, String> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        result
    };
    
    // Import variables
    for exported_var in &exported.variables {
        let existing_value = existing_secrets.get(&exported_var.key).map(|s| s.as_str());
        let var = import_variable(exported_var, existing_value);
        
        let db = app.state::<Arc<Database>>();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO global_variables (id, key, value, description, is_secret, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                var.id,
                var.key,
                var.value,
                var.description,
                var.is_secret as i32,
                var.enabled as i32
            ],
        ).map_err(|e| format!("Failed to save global variable: {}", e))?;
    }
    
    Ok(exported.variables.len() as i32)
}

// ============ Git Operations ============

/// Initialize a Git repository in the sync directory
#[tauri::command]
pub async fn git_init(app: tauri::AppHandle) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    println!("[git_init] Initializing git repo at {:?}, config enabled: {}", sync_path, config.enabled);
    
    Repository::init(&sync_path)
        .map_err(|e| format!("Failed to initialize Git repository: {}", e))?;
    
    println!("[git_init] Git repo initialized successfully");
    
    Ok(())
}

/// Get Git repository status
#[tauri::command]
pub async fn git_get_status(app: tauri::AppHandle) -> Result<GitStatus, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    println!("[git_get_status] sync_path: {:?}, enabled: {}", sync_path, config.enabled);
    
    let repo = match Repository::open(&sync_path) {
        Ok(repo) => {
            println!("[git_get_status] Successfully opened repo at {:?}", sync_path);
            repo
        },
        Err(e) => {
            println!("[git_get_status] Failed to open repo at {:?}: {}", sync_path, e);
            return Ok(GitStatus {
                is_repo: false,
                branch: None,
                has_remote: false,
                remote_url: None,
                uncommitted_changes: vec![],
                ahead: 0,
                behind: 0,
            });
        }
    };
    
    // Get current branch
    let branch = repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(|s| s.to_string()));
    
    // Check for remote
    let has_remote = repo.find_remote("origin").is_ok();
    let remote_url = repo.find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|s| s.to_string()));
    
    // Get uncommitted changes
    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(true);
    status_opts.recurse_untracked_dirs(true);  // Show individual files in untracked directories
    
    let statuses = repo.statuses(Some(&mut status_opts))
        .map_err(|e| format!("Failed to get Git status: {}", e))?;
    
    let uncommitted_changes: Vec<GitFileChange> = statuses.iter()
        .filter_map(|entry| {
            let path = entry.path().unwrap_or("");
            
            // Only include istek-related files
            let is_istek_file = path.starts_with("collections/")
                || path.starts_with("environments/")
                || path == "global-variables.yaml"
                || path == "workspace.yaml";
            
            if !is_istek_file {
                return None;
            }
            
            let status = entry.status();
            let status_str = if status.is_wt_new() || status.is_index_new() {
                "new"
            } else if status.is_wt_modified() || status.is_index_modified() {
                "modified"
            } else if status.is_wt_deleted() || status.is_index_deleted() {
                "deleted"
            } else if status.is_wt_renamed() || status.is_index_renamed() {
                "renamed"
            } else {
                "unknown"
            };
            
            Some(GitFileChange {
                path: path.to_string(),
                status: status_str.to_string(),
            })
        })
        .collect();
    
    // Calculate ahead/behind (if tracking remote)
    let (ahead, behind) = if has_remote {
        calculate_ahead_behind(&repo).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    
    Ok(GitStatus {
        is_repo: true,
        branch,
        has_remote,
        remote_url,
        uncommitted_changes,
        ahead,
        behind,
    })
}

/// Get files changed in a specific commit
#[tauri::command]
pub async fn git_get_commit_files(app: tauri::AppHandle, commit_id: String) -> Result<Vec<GitFileChange>, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    let oid = git2::Oid::from_str(&commit_id)
        .map_err(|e| format!("Invalid commit ID: {}", e))?;
    
    let commit = repo.find_commit(oid)
        .map_err(|e| format!("Commit not found: {}", e))?;
    
    let tree = commit.tree()
        .map_err(|e| format!("Failed to get commit tree: {}", e))?;
    
    // Get parent tree (if exists)
    let parent_tree = commit.parent(0)
        .ok()
        .and_then(|p| p.tree().ok());
    
    let mut diff_opts = git2::DiffOptions::new();
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
        .map_err(|e| format!("Failed to get diff: {}", e))?;
    
    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            let path = delta.new_file().path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            
            let status = match delta.status() {
                git2::Delta::Added => "new",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Modified => "modified",
                git2::Delta::Renamed => "renamed",
                _ => "modified",
            };
            
            files.push(GitFileChange {
                path,
                status: status.to_string(),
            });
            true
        },
        None,
        None,
        None,
    ).map_err(|e| format!("Failed to iterate diff: {}", e))?;
    
    Ok(files)
}

/// Get diff for a specific file in a commit
#[tauri::command]
pub async fn git_get_file_diff(app: tauri::AppHandle, commit_id: String, file_path: String) -> Result<String, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    let oid = git2::Oid::from_str(&commit_id)
        .map_err(|e| format!("Invalid commit ID: {}", e))?;
    
    let commit = repo.find_commit(oid)
        .map_err(|e| format!("Commit not found: {}", e))?;
    
    let tree = commit.tree()
        .map_err(|e| format!("Failed to get commit tree: {}", e))?;
    
    // Get parent tree (if exists)
    let parent_tree = commit.parent(0)
        .ok()
        .and_then(|p| p.tree().ok());
    
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(&file_path);
    
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
        .map_err(|e| format!("Failed to get diff: {}", e))?;
    
    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            _ => "",
        };
        if !prefix.is_empty() {
            diff_text.push_str(prefix);
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }
        true
    }).map_err(|e| format!("Failed to print diff: {}", e))?;
    
    Ok(diff_text)
}

fn calculate_ahead_behind(repo: &Repository) -> Result<(u32, u32), git2::Error> {
    let head = repo.head()?;
    let local_oid = head.target().ok_or_else(|| git2::Error::from_str("No HEAD target"))?;
    
    // Try to find the upstream branch
    let branch_name = head.shorthand().unwrap_or("main");
    let upstream_name = format!("origin/{}", branch_name);
    
    if let Ok(upstream_ref) = repo.find_reference(&format!("refs/remotes/{}", upstream_name)) {
        if let Some(upstream_oid) = upstream_ref.target() {
            let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
            return Ok((ahead as u32, behind as u32));
        }
    }
    
    Ok((0, 0))
}

/// Commit all changes with a message
#[tauri::command]
pub async fn git_commit(app: tauri::AppHandle, message: String) -> Result<String, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    // Add all changes to index
    let mut index = repo.index()
        .map_err(|e| format!("Failed to get index: {}", e))?;
    
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Failed to add files: {}", e))?;
    
    index.write()
        .map_err(|e| format!("Failed to write index: {}", e))?;
    
    let tree_id = index.write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;
    
    let tree = repo.find_tree(tree_id)
        .map_err(|e| format!("Failed to find tree: {}", e))?;
    
    // Get signature
    let signature = Signature::now("istek", "istek@local")
        .map_err(|e| format!("Failed to create signature: {}", e))?;
    
    // Get parent commit (if exists)
    let parent_commit = repo.head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    
    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    
    // Create commit
    let commit_oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &parents,
    ).map_err(|e| format!("Failed to create commit: {}", e))?;
    
    Ok(commit_oid.to_string())
}

/// Pull changes from remote
#[tauri::command]
pub async fn git_pull(app: tauri::AppHandle) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    // Fetch from origin
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("Failed to find remote 'origin': {}", e))?;
    
    remote.fetch(&["main"], None, None)
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    // Get the fetch head
    let fetch_head = repo.find_reference("FETCH_HEAD")
        .map_err(|e| format!("Failed to find FETCH_HEAD: {}", e))?;
    
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)
        .map_err(|e| format!("Failed to get annotated commit: {}", e))?;
    
    // Merge
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Failed to analyze merge: {}", e))?;
    
    if analysis.is_fast_forward() {
        // Fast-forward merge
        let refname = "refs/heads/main";
        let mut reference = repo.find_reference(refname)
            .map_err(|e| format!("Failed to find reference: {}", e))?;
        
        reference.set_target(fetch_commit.id(), "Fast-forward")
            .map_err(|e| format!("Failed to set target: {}", e))?;
        
        repo.set_head(refname)
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Failed to checkout: {}", e))?;
    } else if analysis.is_normal() {
        return Err("Merge required - please resolve conflicts manually".to_string());
    }
    
    Ok(())
}

/// Push changes to remote
#[tauri::command]
pub async fn git_push(app: tauri::AppHandle) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("Failed to find remote 'origin': {}", e))?;
    
    // Push to remote - user's responsibility to have auth set up
    remote.push(&["refs/heads/main:refs/heads/main"], None)
        .map_err(|e| format!("Failed to push: {}. Make sure you have authentication configured (SSH key or credential helper).", e))?;
    
    Ok(())
}

/// Add a remote to the repository
#[tauri::command]
pub async fn git_add_remote(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    // Remove existing origin if present
    let _ = repo.remote_delete("origin");
    
    repo.remote("origin", &url)
        .map_err(|e| format!("Failed to add remote: {}", e))?;
    
    Ok(())
}

/// Get commit history
#[tauri::command]
pub async fn git_get_log(app: tauri::AppHandle, limit: Option<u32>) -> Result<Vec<serde_json::Value>, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    let mut revwalk = repo.revwalk()
        .map_err(|e| format!("Failed to create revwalk: {}", e))?;
    
    revwalk.push_head()
        .map_err(|e| format!("Failed to push HEAD: {}", e))?;
    
    let limit = limit.unwrap_or(50) as usize;
    let mut commits = Vec::new();
    
    for oid in revwalk.take(limit) {
        let oid = oid.map_err(|e| format!("Failed to get OID: {}", e))?;
        let commit = repo.find_commit(oid)
            .map_err(|e| format!("Failed to find commit: {}", e))?;
        
        commits.push(serde_json::json!({
            "id": oid.to_string(),
            "message": commit.message().unwrap_or(""),
            "author": commit.author().name().unwrap_or("Unknown"),
            "timestamp": commit.time().seconds(),
        }));
    }
    
    Ok(commits)
}

/// List all branches
#[tauri::command]
pub async fn git_list_branches(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    let branches = repo.branches(Some(git2::BranchType::Local))
        .map_err(|e| format!("Failed to list branches: {}", e))?;
    
    let branch_names: Vec<String> = branches
        .filter_map(|b| b.ok())
        .filter_map(|(branch, _)| branch.name().ok().flatten().map(|s| s.to_string()))
        .collect();
    
    Ok(branch_names)
}

/// Create a new branch
#[tauri::command]
pub async fn git_create_branch(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    // Check if repo has any commits by trying to get HEAD's target
    let has_commits = repo.head().ok().and_then(|h| h.target()).is_some();
    
    if has_commits {
        // Repo has commits, create branch from HEAD
        let head = repo.head()
            .map_err(|e| format!("Failed to get HEAD: {}", e))?;
        let commit = head.peel_to_commit()
            .map_err(|e| format!("Failed to get commit: {}", e))?;
        
        // Create new branch
        repo.branch(&name, &commit, false)
            .map_err(|e| format!("Failed to create branch: {}", e))?;
        
        // Checkout the new branch
        let refname = format!("refs/heads/{}", name);
        repo.set_head(&refname)
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Failed to checkout: {}", e))?;
    } else {
        // No commits yet - for unborn branch, we use set_head with symbolic reference
        // This sets HEAD to point to the new branch (which doesn't exist yet)
        let refname = format!("refs/heads/{}", name);
        repo.set_head(&refname)
            .map_err(|e| format!("Failed to set HEAD to new branch: {}", e))?;
    }
    
    Ok(())
}

/// Switch to an existing branch
#[tauri::command]
pub async fn git_switch_branch(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let config = sync_get_config(app).await?;
    let sync_path = get_sync_path(&config);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open Git repository: {}", e))?;
    
    // Set HEAD to the branch
    let refname = format!("refs/heads/{}", name);
    repo.set_head(&refname)
        .map_err(|e| format!("Failed to set HEAD: {}", e))?;
    
    // Checkout
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .map_err(|e| format!("Failed to checkout: {}", e))?;
    
    Ok(())
}
