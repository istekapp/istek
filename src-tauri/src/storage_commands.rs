use std::sync::Arc;
use tauri::Manager;

use crate::storage::{
    AppData, Collection, Environment, HistoryItem, McpServer, SecretProvider, 
    Storage, TestRunHistory, Variable, Workspace
};

// ============ Load App Data ============

#[tauri::command]
pub async fn load_app_data(app: tauri::AppHandle) -> Result<AppData, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.load_app_data()
}

#[tauri::command]
pub async fn load_workspace_data(app: tauri::AppHandle, workspace_id: String) -> Result<AppData, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.load_workspace_data(&workspace_id)
}

// ============ Workspace Commands ============

#[tauri::command]
pub async fn create_workspace(app: tauri::AppHandle, name: String, sync_path: Option<String>) -> Result<Workspace, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.create_workspace(name, sync_path)
}

#[tauri::command]
pub async fn update_workspace(app: tauri::AppHandle, workspace: Workspace) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.update_workspace(&workspace)
}

#[tauri::command]
pub async fn delete_workspace(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.delete_workspace(&id)
}

#[tauri::command]
pub async fn set_active_workspace(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.set_active_workspace(&id)
}

#[tauri::command]
pub async fn get_workspace(app: tauri::AppHandle, id: String) -> Result<Option<Workspace>, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.get_workspace(&id)
}

#[tauri::command]
pub async fn get_default_sync_path(name: String) -> Result<String, String> {
    let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
    
    let safe_name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase();
    
    let path = home_dir
        .join("Desktop")
        .join(safe_name);
    
    Ok(path.to_string_lossy().to_string())
}

// ============ Collection Commands ============

#[tauri::command]
pub async fn save_collection(app: tauri::AppHandle, collection: Collection, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_collection(&ws_id, &collection)
}

#[tauri::command]
pub async fn delete_collection(app: tauri::AppHandle, id: String, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.delete_collection(&ws_id, &id)
}

// ============ History Commands ============

#[tauri::command]
pub async fn save_history_item(app: tauri::AppHandle, item: HistoryItem, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_history_item(&ws_id, &item)
}

#[tauri::command]
pub async fn delete_history_item(app: tauri::AppHandle, id: String, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.delete_history_item(&ws_id, &id)
}

#[tauri::command]
pub async fn clear_history(app: tauri::AppHandle, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.clear_history(&ws_id)
}

// ============ Global Variable Commands ============

#[tauri::command]
pub async fn save_global_variable(app: tauri::AppHandle, variable: Variable, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_global_variable(&ws_id, &variable)
}

#[tauri::command]
pub async fn delete_global_variable(app: tauri::AppHandle, id: String, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.delete_global_variable(&ws_id, &id)
}

#[tauri::command]
pub async fn save_all_global_variables(app: tauri::AppHandle, variables: Vec<Variable>, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_all_global_variables(&ws_id, &variables)
}

// ============ Environment Commands ============

#[tauri::command]
pub async fn save_environment(app: tauri::AppHandle, environment: Environment, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_environment(&ws_id, &environment)
}

#[tauri::command]
pub async fn delete_environment(app: tauri::AppHandle, id: String, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.delete_environment(&ws_id, &id)
}

#[tauri::command]
pub async fn save_all_environments(app: tauri::AppHandle, environments: Vec<Environment>, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.save_all_environments(&ws_id, &environments)
}

#[tauri::command]
pub async fn save_active_environment_id(app: tauri::AppHandle, id: Option<String>, workspace_id: Option<String>) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    let ws_id = workspace_id.or_else(|| storage.get_active_workspace_id().ok().flatten());
    let ws_id = ws_id.ok_or("No active workspace")?;
    
    storage.set_active_environment(&ws_id, id.as_deref())
}

// ============ Secret Provider Commands ============

#[tauri::command]
pub async fn save_secret_provider(app: tauri::AppHandle, provider: SecretProvider) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.save_secret_provider(&provider)
}

#[tauri::command]
pub async fn delete_secret_provider(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.delete_secret_provider(&id)
}

// ============ MCP Server Commands ============

#[tauri::command]
pub async fn get_mcp_servers(app: tauri::AppHandle) -> Result<Vec<McpServer>, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.get_mcp_servers()
}

#[tauri::command]
pub async fn add_mcp_server(
    app: tauri::AppHandle,
    name: String,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
) -> Result<McpServer, String> {
    let storage = app.state::<Arc<Storage>>();
    
    let id = format!("mcp_{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..16]);
    let created_at = chrono::Utc::now().timestamp_millis();
    
    let server = McpServer {
        id: id.clone(),
        name,
        command,
        args,
        env,
        enabled: true,
        created_at,
    };
    
    storage.save_mcp_server(&server)?;
    Ok(server)
}

#[tauri::command]
pub async fn update_mcp_server(app: tauri::AppHandle, server: McpServer) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.save_mcp_server(&server)
}

#[tauri::command]
pub async fn delete_mcp_server(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.delete_mcp_server(&id)
}

#[tauri::command]
pub async fn toggle_mcp_server(app: tauri::AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    let servers = storage.get_mcp_servers()?;
    
    if let Some(mut server) = servers.into_iter().find(|s| s.id == id) {
        server.enabled = enabled;
        storage.save_mcp_server(&server)?;
    }
    
    Ok(())
}

// ============ Test Run Commands ============

#[tauri::command]
pub async fn save_test_run(app: tauri::AppHandle, test_run: TestRunHistory) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.save_test_run(&test_run)
}

#[tauri::command]
pub async fn load_test_runs(app: tauri::AppHandle) -> Result<Vec<TestRunHistory>, String> {
    let storage = app.state::<Arc<Storage>>();
    storage.get_test_runs()
}

#[tauri::command]
pub async fn delete_test_run(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.delete_test_run(&id)
}

#[tauri::command]
pub async fn clear_test_runs(app: tauri::AppHandle) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    storage.clear_test_runs()
}

// ============ Utility Commands ============

#[tauri::command]
pub async fn get_config_dir(app: tauri::AppHandle) -> Result<String, String> {
    let storage = app.state::<Arc<Storage>>();
    Ok(storage.config_dir().to_string_lossy().to_string())
}
