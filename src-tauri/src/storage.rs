use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_path: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_provider: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub variables: Vec<Variable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub requests: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    #[serde(default = "default_protocol_type")]
    pub protocol_type: String,
    pub created_at: i64,
}

fn default_protocol_type() -> String {
    "http".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub request: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<serde_json::Value>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecretProvider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestRunHistory {
    pub id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    pub collection_name: String,
    pub timestamp: i64,
    pub summary: serde_json::Value,
}

// ============ Config Files ============

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_workspace_id: Option<String>,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRef {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(skip_serializing, default)]  // Never serialize sync_path - it's machine-specific, but allow deserialize for backwards compat
    pub sync_path: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_environment_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFile {
    #[serde(default)]
    pub items: Vec<HistoryItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalVariablesFile {
    #[serde(default)]
    pub variables: Vec<Variable>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecretProvidersFile {
    #[serde(default)]
    pub providers: Vec<SecretProvider>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServersFile {
    #[serde(default)]
    pub servers: Vec<McpServer>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TestRunsFile {
    #[serde(default)]
    pub runs: Vec<TestRunHistory>,
}

// ============ Sensitive Values (Encrypted) ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveValue {
    pub key: String,
    pub encrypted_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveValuesFile {
    #[serde(default)]
    pub values: Vec<SensitiveValue>,
}

// ============ AppData (for frontend compatibility) ============

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: Option<String>,
    pub collections: Vec<Collection>,
    pub history: Vec<HistoryItem>,
    pub global_variables: Vec<Variable>,
    pub environments: Vec<Environment>,
    pub secret_providers: Vec<SecretProvider>,
    pub active_environment_id: Option<String>,
}

// ============ Storage Implementation ============

pub struct Storage {
    config_dir: PathBuf,
    cache: RwLock<StorageCache>,
}

#[derive(Default)]
struct StorageCache {
    global_config: Option<GlobalConfig>,
}

impl Storage {
    pub fn new() -> Result<Self, String> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not determine config directory")?
            .join("istek");
        
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
        
        let storage = Storage {
            config_dir,
            cache: RwLock::new(StorageCache::default()),
        };
        
        storage.ensure_default_workspace()?;
        
        Ok(storage)
    }

    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    // ============ Path Helpers ============

    fn global_config_path(&self) -> PathBuf {
        self.config_dir.join("config.yaml")
    }

    fn workspaces_dir(&self) -> PathBuf {
        self.config_dir.join("workspaces")
    }

    fn workspace_dir(&self, workspace_id: &str) -> PathBuf {
        self.workspaces_dir().join(workspace_id)
    }

    fn workspace_config_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("workspace.yaml")
    }

    fn collections_dir(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("collections")
    }

    fn environments_dir(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("environments")
    }

    fn global_variables_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("global-variables.yaml")
    }

    fn history_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("history.yaml")
    }

    fn secret_providers_path(&self) -> PathBuf {
        self.config_dir.join("secret-providers.yaml")
    }

    fn mcp_servers_path(&self) -> PathBuf {
        self.config_dir.join("mcp-servers.yaml")
    }

    fn test_runs_path(&self) -> PathBuf {
        self.config_dir.join("test-runs.yaml")
    }

    fn sensitive_values_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("sensitive-values.yaml")
    }

    // ============ YAML Helpers ============

    fn read_yaml<T: for<'de> Deserialize<'de> + Default>(&self, path: &PathBuf) -> Result<T, String> {
        if !path.exists() {
            return Ok(T::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        if content.trim().is_empty() {
            return Ok(T::default());
        }
        serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }

    /// Read a YAML file that may or may not exist. Returns None if not found or empty.
    fn read_yaml_optional<T: for<'de> Deserialize<'de>>(&self, path: &PathBuf) -> Result<Option<T>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        if content.trim().is_empty() {
            return Ok(None);
        }
        serde_yaml::from_str(&content)
            .map(Some)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }

    fn write_yaml<T: Serialize>(&self, path: &PathBuf, data: &T) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        let content = serde_yaml::to_string(data)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
    }

    // ============ Global Config ============

    pub fn load_global_config(&self) -> Result<GlobalConfig, String> {
        self.read_yaml(&self.global_config_path())
    }

    pub fn save_global_config(&self, config: &GlobalConfig) -> Result<(), String> {
        self.write_yaml(&self.global_config_path(), config)?;
        let mut cache = self.cache.write().map_err(|e| e.to_string())?;
        cache.global_config = Some(config.clone());
        Ok(())
    }

    fn ensure_default_workspace(&self) -> Result<(), String> {
        let config = self.load_global_config()?;
        
        if config.workspaces.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            
            let workspace_id = format!("ws_{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..16]);
            
            // Create workspace directory structure
            let ws_dir = self.workspace_dir(&workspace_id);
            fs::create_dir_all(self.collections_dir(&workspace_id))
                .map_err(|e| format!("Failed to create collections dir: {}", e))?;
            fs::create_dir_all(self.environments_dir(&workspace_id))
                .map_err(|e| format!("Failed to create environments dir: {}", e))?;
            
            // Create workspace config
            let ws_config = WorkspaceConfig {
                id: workspace_id.clone(),
                name: "Default Workspace".to_string(),
                sync_path: None,
                is_default: true,
                created_at: now,
                active_environment_id: None,
            };
            self.write_yaml(&self.workspace_config_path(&workspace_id), &ws_config)?;
            
            // Create .gitignore in workspace
            let gitignore_path = ws_dir.join(".gitignore");
            let gitignore_content = "# Local files (secrets, history)\nhistory.yaml\n*.local.yaml\n";
            fs::write(&gitignore_path, gitignore_content)
                .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
            
            // Update global config
            let new_config = GlobalConfig {
                active_workspace_id: Some(workspace_id.clone()),
                workspaces: vec![WorkspaceRef {
                    id: workspace_id.clone(),
                    name: "Default Workspace".to_string(),
                    path: ws_dir.to_string_lossy().to_string(),
                }],
            };
            self.save_global_config(&new_config)?;
        }
        
        Ok(())
    }

    // ============ Workspace Operations ============

    pub fn get_workspaces(&self) -> Result<Vec<Workspace>, String> {
        let config = self.load_global_config()?;
        let mut workspaces = Vec::new();
        
        for ws_ref in &config.workspaces {
            if let Ok(ws_config) = self.load_workspace_config(&ws_ref.id) {
                workspaces.push(Workspace {
                    id: ws_config.id,
                    name: ws_config.name,
                    sync_path: ws_config.sync_path,
                    is_default: ws_config.is_default,
                    created_at: ws_config.created_at,
                });
            }
        }
        
        Ok(workspaces)
    }

    pub fn get_workspace(&self, id: &str) -> Result<Option<Workspace>, String> {
        match self.load_workspace_config(id) {
            Ok(ws_config) => Ok(Some(Workspace {
                id: ws_config.id,
                name: ws_config.name,
                sync_path: ws_config.sync_path,
                is_default: ws_config.is_default,
                created_at: ws_config.created_at,
            })),
            Err(_) => Ok(None),
        }
    }

    pub fn load_workspace_config(&self, workspace_id: &str) -> Result<WorkspaceConfig, String> {
        self.read_yaml(&self.workspace_config_path(workspace_id))
    }

    pub fn save_workspace_config(&self, config: &WorkspaceConfig) -> Result<(), String> {
        self.write_yaml(&self.workspace_config_path(&config.id), config)
    }

    pub fn create_workspace(&self, name: String, sync_path: Option<String>) -> Result<Workspace, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        
        let workspace_id = format!("ws_{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..16]);
        
        // Create workspace directory structure
        let ws_dir = self.workspace_dir(&workspace_id);
        fs::create_dir_all(self.collections_dir(&workspace_id))
            .map_err(|e| format!("Failed to create collections dir: {}", e))?;
        fs::create_dir_all(self.environments_dir(&workspace_id))
            .map_err(|e| format!("Failed to create environments dir: {}", e))?;
        
        // If sync_path is provided, also create that directory structure
        if let Some(ref path) = sync_path {
            let sync_dir = PathBuf::from(path);
            fs::create_dir_all(&sync_dir)
                .map_err(|e| format!("Failed to create sync directory: {}", e))?;
            fs::create_dir_all(sync_dir.join("collections"))
                .map_err(|e| format!("Failed to create sync collections dir: {}", e))?;
            fs::create_dir_all(sync_dir.join("environments"))
                .map_err(|e| format!("Failed to create sync environments dir: {}", e))?;
        }
        
        // Create workspace config
        let ws_config = WorkspaceConfig {
            id: workspace_id.clone(),
            name: name.clone(),
            sync_path: sync_path.clone(),
            is_default: false,
            created_at: now,
            active_environment_id: None,
        };
        self.write_yaml(&self.workspace_config_path(&workspace_id), &ws_config)?;
        
        // Create .gitignore in workspace
        let gitignore_path = ws_dir.join(".gitignore");
        let gitignore_content = "# Local files (secrets, history)\nhistory.yaml\n*.local.yaml\n";
        fs::write(&gitignore_path, gitignore_content)
            .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
        
        // Update global config
        let mut config = self.load_global_config()?;
        config.workspaces.push(WorkspaceRef {
            id: workspace_id.clone(),
            name: name.clone(),
            path: ws_dir.to_string_lossy().to_string(),
        });
        config.active_workspace_id = Some(workspace_id.clone());
        self.save_global_config(&config)?;
        
        Ok(Workspace {
            id: workspace_id,
            name,
            sync_path,
            is_default: false,
            created_at: now,
        })
    }

    pub fn update_workspace(&self, workspace: &Workspace) -> Result<(), String> {
        let mut ws_config = self.load_workspace_config(&workspace.id)?;
        ws_config.name = workspace.name.clone();
        ws_config.sync_path = workspace.sync_path.clone();
        self.save_workspace_config(&ws_config)?;
        
        // Update global config reference
        let mut config = self.load_global_config()?;
        if let Some(ws_ref) = config.workspaces.iter_mut().find(|w| w.id == workspace.id) {
            ws_ref.name = workspace.name.clone();
        }
        self.save_global_config(&config)?;
        
        Ok(())
    }

    pub fn delete_workspace(&self, id: &str) -> Result<(), String> {
        let ws_config = self.load_workspace_config(id)?;
        if ws_config.is_default {
            return Err("Cannot delete the default workspace".to_string());
        }
        
        // Remove workspace directory
        let ws_dir = self.workspace_dir(id);
        if ws_dir.exists() {
            fs::remove_dir_all(&ws_dir)
                .map_err(|e| format!("Failed to delete workspace directory: {}", e))?;
        }
        
        // Update global config
        let mut config = self.load_global_config()?;
        config.workspaces.retain(|w| w.id != id);
        
        // If deleted workspace was active, switch to default
        if config.active_workspace_id.as_deref() == Some(id) {
            config.active_workspace_id = config.workspaces.first().map(|w| w.id.clone());
        }
        self.save_global_config(&config)?;
        
        Ok(())
    }

    pub fn set_active_workspace(&self, id: &str) -> Result<(), String> {
        let mut config = self.load_global_config()?;
        config.active_workspace_id = Some(id.to_string());
        self.save_global_config(&config)
    }

    pub fn get_active_workspace_id(&self) -> Result<Option<String>, String> {
        let config = self.load_global_config()?;
        Ok(config.active_workspace_id)
    }

    // ============ Collection Operations ============

    fn collection_filename(name: &str) -> String {
        let safe_name: String = name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        format!("{}.yaml", safe_name)
    }

    pub fn get_collections(&self, workspace_id: &str) -> Result<Vec<Collection>, String> {
        let collections_dir = self.collections_dir(workspace_id);
        if !collections_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut collections = Vec::new();
        let entries = fs::read_dir(&collections_dir)
            .map_err(|e| format!("Failed to read collections directory: {}", e))?;
        
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                if let Ok(Some(collection)) = self.read_yaml_optional::<Collection>(&path) {
                    collections.push(collection);
                }
            }
        }
        
        collections.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(collections)
    }

    pub fn get_collection(&self, workspace_id: &str, collection_id: &str) -> Result<Option<Collection>, String> {
        let collections = self.get_collections(workspace_id)?;
        Ok(collections.into_iter().find(|c| c.id == collection_id))
    }

    pub fn save_collection(&self, workspace_id: &str, collection: &Collection) -> Result<(), String> {
        // First, try to find existing file by ID (to handle renames)
        let collections_dir = self.collections_dir(workspace_id);
        fs::create_dir_all(&collections_dir)
            .map_err(|e| format!("Failed to create collections directory: {}", e))?;
        
        // Remove old file if exists with different name
        if let Ok(entries) = fs::read_dir(&collections_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                    if let Ok(Some(existing)) = self.read_yaml_optional::<Collection>(&path) {
                        if existing.id == collection.id && existing.name != collection.name {
                            fs::remove_file(&path).ok();
                        }
                    }
                }
            }
        }
        
        let filename = Self::collection_filename(&collection.name);
        let path = collections_dir.join(filename);
        self.write_yaml(&path, collection)
    }

    pub fn delete_collection(&self, workspace_id: &str, collection_id: &str) -> Result<(), String> {
        let collections_dir = self.collections_dir(workspace_id);
        if !collections_dir.exists() {
            return Ok(());
        }
        
        let entries = fs::read_dir(&collections_dir)
            .map_err(|e| format!("Failed to read collections directory: {}", e))?;
        
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                if let Ok(Some(collection)) = self.read_yaml_optional::<Collection>(&path) {
                    if collection.id == collection_id {
                        fs::remove_file(&path)
                            .map_err(|e| format!("Failed to delete collection file: {}", e))?;
                        return Ok(());
                    }
                }
            }
        }
        
        Ok(())
    }

    // ============ Environment Operations ============

    fn environment_filename(name: &str) -> String {
        let safe_name: String = name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        format!("{}.yaml", safe_name)
    }

    pub fn get_environments(&self, workspace_id: &str) -> Result<Vec<Environment>, String> {
        let environments_dir = self.environments_dir(workspace_id);
        if !environments_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut environments = Vec::new();
        let entries = fs::read_dir(&environments_dir)
            .map_err(|e| format!("Failed to read environments directory: {}", e))?;
        
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                if let Ok(Some(environment)) = self.read_yaml_optional::<Environment>(&path) {
                    environments.push(environment);
                }
            }
        }
        
        environments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(environments)
    }

    pub fn save_environment(&self, workspace_id: &str, environment: &Environment) -> Result<(), String> {
        let environments_dir = self.environments_dir(workspace_id);
        fs::create_dir_all(&environments_dir)
            .map_err(|e| format!("Failed to create environments directory: {}", e))?;
        
        // Remove old file if exists with different name (handle renames)
        if let Ok(entries) = fs::read_dir(&environments_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                    if let Ok(Some(existing)) = self.read_yaml_optional::<Environment>(&path) {
                        if existing.id == environment.id && existing.name != environment.name {
                            fs::remove_file(&path).ok();
                        }
                    }
                }
            }
        }
        
        // Create a copy with secret values cleared for git sync
        // Keep the variable metadata (id, key, isSecret flag) but clear the value
        let mut env_to_save = environment.clone();
        env_to_save.variables = env_to_save.variables.into_iter()
            .map(|mut v| {
                if v.is_secret {
                    // Clear the value for secret variables - actual value is in sensitive-values.yaml
                    v.value = String::new();
                }
                v
            })
            .collect();
        
        let filename = Self::environment_filename(&environment.name);
        let path = environments_dir.join(filename);
        self.write_yaml(&path, &env_to_save)
    }

    pub fn delete_environment(&self, workspace_id: &str, environment_id: &str) -> Result<(), String> {
        let environments_dir = self.environments_dir(workspace_id);
        if !environments_dir.exists() {
            return Ok(());
        }
        
        let entries = fs::read_dir(&environments_dir)
            .map_err(|e| format!("Failed to read environments directory: {}", e))?;
        
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                if let Ok(Some(environment)) = self.read_yaml_optional::<Environment>(&path) {
                    if environment.id == environment_id {
                        fs::remove_file(&path)
                            .map_err(|e| format!("Failed to delete environment file: {}", e))?;
                        return Ok(());
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn save_all_environments(&self, workspace_id: &str, environments: &[Environment]) -> Result<(), String> {
        let environments_dir = self.environments_dir(workspace_id);
        
        // Remove all existing environment files
        if environments_dir.exists() {
            if let Ok(entries) = fs::read_dir(&environments_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                        fs::remove_file(&path).ok();
                    }
                }
            }
        }
        
        // Save all environments
        for env in environments {
            self.save_environment(workspace_id, env)?;
        }
        
        Ok(())
    }

    pub fn set_active_environment(&self, workspace_id: &str, environment_id: Option<&str>) -> Result<(), String> {
        let mut ws_config = self.load_workspace_config(workspace_id)?;
        ws_config.active_environment_id = environment_id.map(|s| s.to_string());
        self.save_workspace_config(&ws_config)
    }

    pub fn get_active_environment_id(&self, workspace_id: &str) -> Result<Option<String>, String> {
        let ws_config = self.load_workspace_config(workspace_id)?;
        Ok(ws_config.active_environment_id)
    }

    // ============ Global Variables Operations ============

    pub fn get_global_variables(&self, workspace_id: &str) -> Result<Vec<Variable>, String> {
        let file: GlobalVariablesFile = self.read_yaml(&self.global_variables_path(workspace_id))?;
        Ok(file.variables)
    }

    pub fn save_global_variable(&self, workspace_id: &str, variable: &Variable) -> Result<(), String> {
        let mut file: GlobalVariablesFile = self.read_yaml(&self.global_variables_path(workspace_id))?;
        
        // Create a copy with value cleared if it's a secret variable
        // The actual encrypted value is stored in sensitive-values.yaml
        let mut var_to_save = variable.clone();
        if var_to_save.is_secret {
            var_to_save.value = String::new();
        }
        
        // Update or insert
        if let Some(existing) = file.variables.iter_mut().find(|v| v.id == variable.id) {
            *existing = var_to_save;
        } else {
            file.variables.push(var_to_save);
        }
        
        self.write_yaml(&self.global_variables_path(workspace_id), &file)
    }

    pub fn delete_global_variable(&self, workspace_id: &str, variable_id: &str) -> Result<(), String> {
        let mut file: GlobalVariablesFile = self.read_yaml(&self.global_variables_path(workspace_id))?;
        file.variables.retain(|v| v.id != variable_id);
        self.write_yaml(&self.global_variables_path(workspace_id), &file)
    }

    pub fn save_all_global_variables(&self, workspace_id: &str, variables: &[Variable]) -> Result<(), String> {
        // Clear values for secret variables before saving
        let vars_to_save: Vec<Variable> = variables.iter()
            .map(|v| {
                let mut var = v.clone();
                if var.is_secret {
                    var.value = String::new();
                }
                var
            })
            .collect();
        
        let file = GlobalVariablesFile {
            variables: vars_to_save,
        };
        self.write_yaml(&self.global_variables_path(workspace_id), &file)
    }

    // ============ History Operations ============

    pub fn get_history(&self, workspace_id: &str) -> Result<Vec<HistoryItem>, String> {
        let file: HistoryFile = self.read_yaml(&self.history_path(workspace_id))?;
        Ok(file.items)
    }

    pub fn save_history_item(&self, workspace_id: &str, item: &HistoryItem) -> Result<(), String> {
        let mut file: HistoryFile = self.read_yaml(&self.history_path(workspace_id))?;
        
        // Remove existing if same ID
        file.items.retain(|i| i.id != item.id);
        
        // Insert at beginning
        file.items.insert(0, item.clone());
        
        // Keep only last 100
        file.items.truncate(100);
        
        self.write_yaml(&self.history_path(workspace_id), &file)
    }

    pub fn delete_history_item(&self, workspace_id: &str, item_id: &str) -> Result<(), String> {
        let mut file: HistoryFile = self.read_yaml(&self.history_path(workspace_id))?;
        file.items.retain(|i| i.id != item_id);
        self.write_yaml(&self.history_path(workspace_id), &file)
    }

    pub fn clear_history(&self, workspace_id: &str) -> Result<(), String> {
        let file = HistoryFile { items: Vec::new() };
        self.write_yaml(&self.history_path(workspace_id), &file)
    }

    // ============ Secret Provider Operations (Global) ============

    pub fn get_secret_providers(&self) -> Result<Vec<SecretProvider>, String> {
        let file: SecretProvidersFile = self.read_yaml(&self.secret_providers_path())?;
        Ok(file.providers)
    }

    pub fn save_secret_provider(&self, provider: &SecretProvider) -> Result<(), String> {
        let mut file: SecretProvidersFile = self.read_yaml(&self.secret_providers_path())?;
        
        if let Some(existing) = file.providers.iter_mut().find(|p| p.id == provider.id) {
            *existing = provider.clone();
        } else {
            file.providers.push(provider.clone());
        }
        
        self.write_yaml(&self.secret_providers_path(), &file)
    }

    pub fn delete_secret_provider(&self, provider_id: &str) -> Result<(), String> {
        let mut file: SecretProvidersFile = self.read_yaml(&self.secret_providers_path())?;
        file.providers.retain(|p| p.id != provider_id);
        self.write_yaml(&self.secret_providers_path(), &file)
    }

    // ============ MCP Server Operations (Global) ============

    pub fn get_mcp_servers(&self) -> Result<Vec<McpServer>, String> {
        let file: McpServersFile = self.read_yaml(&self.mcp_servers_path())?;
        Ok(file.servers)
    }

    pub fn save_mcp_server(&self, server: &McpServer) -> Result<(), String> {
        let mut file: McpServersFile = self.read_yaml(&self.mcp_servers_path())?;
        
        if let Some(existing) = file.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server.clone();
        } else {
            file.servers.push(server.clone());
        }
        
        self.write_yaml(&self.mcp_servers_path(), &file)
    }

    pub fn delete_mcp_server(&self, server_id: &str) -> Result<(), String> {
        let mut file: McpServersFile = self.read_yaml(&self.mcp_servers_path())?;
        file.servers.retain(|s| s.id != server_id);
        self.write_yaml(&self.mcp_servers_path(), &file)
    }

    // ============ Test Run Operations (Global) ============

    pub fn get_test_runs(&self) -> Result<Vec<TestRunHistory>, String> {
        let file: TestRunsFile = self.read_yaml(&self.test_runs_path())?;
        Ok(file.runs)
    }

    pub fn save_test_run(&self, run: &TestRunHistory) -> Result<(), String> {
        let mut file: TestRunsFile = self.read_yaml(&self.test_runs_path())?;
        
        // Remove existing if same ID
        file.runs.retain(|r| r.id != run.id);
        
        // Insert at beginning
        file.runs.insert(0, run.clone());
        
        // Keep only last 50
        file.runs.truncate(50);
        
        self.write_yaml(&self.test_runs_path(), &file)
    }

    pub fn delete_test_run(&self, run_id: &str) -> Result<(), String> {
        let mut file: TestRunsFile = self.read_yaml(&self.test_runs_path())?;
        file.runs.retain(|r| r.id != run_id);
        self.write_yaml(&self.test_runs_path(), &file)
    }

    pub fn clear_test_runs(&self) -> Result<(), String> {
        let file = TestRunsFile { runs: Vec::new() };
        self.write_yaml(&self.test_runs_path(), &file)
    }

    // ============ Sensitive Values Operations (Workspace-scoped) ============

    pub fn get_sensitive_values(&self, workspace_id: &str) -> Result<Vec<SensitiveValue>, String> {
        let file: SensitiveValuesFile = self.read_yaml(&self.sensitive_values_path(workspace_id))?;
        Ok(file.values)
    }

    pub fn get_sensitive_value(&self, workspace_id: &str, key: &str) -> Result<Option<SensitiveValue>, String> {
        let values = self.get_sensitive_values(workspace_id)?;
        Ok(values.into_iter().find(|v| v.key == key))
    }

    pub fn save_sensitive_value(&self, workspace_id: &str, value: &SensitiveValue) -> Result<(), String> {
        let mut file: SensitiveValuesFile = self.read_yaml(&self.sensitive_values_path(workspace_id))?;
        
        // Update or insert
        if let Some(existing) = file.values.iter_mut().find(|v| v.key == value.key) {
            *existing = value.clone();
        } else {
            file.values.push(value.clone());
        }
        
        self.write_yaml(&self.sensitive_values_path(workspace_id), &file)
    }

    pub fn delete_sensitive_value(&self, workspace_id: &str, key: &str) -> Result<(), String> {
        let mut file: SensitiveValuesFile = self.read_yaml(&self.sensitive_values_path(workspace_id))?;
        file.values.retain(|v| v.key != key);
        self.write_yaml(&self.sensitive_values_path(workspace_id), &file)
    }

    pub fn clear_sensitive_values(&self, workspace_id: &str) -> Result<(), String> {
        let file = SensitiveValuesFile { values: Vec::new() };
        self.write_yaml(&self.sensitive_values_path(workspace_id), &file)
    }

    // ============ Load All Data ============

    pub fn load_app_data(&self) -> Result<AppData, String> {
        let config = self.load_global_config()?;
        let workspaces = self.get_workspaces()?;
        
        let active_workspace_id = config.active_workspace_id.clone()
            .or_else(|| workspaces.iter().find(|w| w.is_default).map(|w| w.id.clone()))
            .or_else(|| workspaces.first().map(|w| w.id.clone()));
        
        let workspace_id = active_workspace_id.clone().unwrap_or_default();
        
        let collections = if !workspace_id.is_empty() {
            self.get_collections(&workspace_id)?
        } else {
            Vec::new()
        };
        
        let history = if !workspace_id.is_empty() {
            self.get_history(&workspace_id)?
        } else {
            Vec::new()
        };
        
        let global_variables = if !workspace_id.is_empty() {
            self.get_global_variables(&workspace_id)?
        } else {
            Vec::new()
        };
        
        let environments = if !workspace_id.is_empty() {
            self.get_environments(&workspace_id)?
        } else {
            Vec::new()
        };
        
        let secret_providers = self.get_secret_providers()?;
        
        let active_environment_id = if !workspace_id.is_empty() {
            self.get_active_environment_id(&workspace_id)?
        } else {
            None
        };
        
        Ok(AppData {
            workspaces,
            active_workspace_id,
            collections,
            history,
            global_variables,
            environments,
            secret_providers,
            active_environment_id,
        })
    }

    pub fn load_workspace_data(&self, workspace_id: &str) -> Result<AppData, String> {
        let workspaces = self.get_workspaces()?;
        let collections = self.get_collections(workspace_id)?;
        let history = self.get_history(workspace_id)?;
        let global_variables = self.get_global_variables(workspace_id)?;
        let environments = self.get_environments(workspace_id)?;
        let secret_providers = self.get_secret_providers()?;
        let active_environment_id = self.get_active_environment_id(workspace_id)?;
        
        Ok(AppData {
            workspaces,
            active_workspace_id: Some(workspace_id.to_string()),
            collections,
            history,
            global_variables,
            environments,
            secret_providers,
            active_environment_id,
        })
    }

    // ============ App Settings Operations (Global) ============

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.yaml")
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let file: HashMap<String, serde_json::Value> = self.read_yaml(&self.settings_path())?;
        Ok(file.get(key).and_then(|v| {
            if let serde_json::Value::String(s) = v {
                Some(s.clone())
            } else {
                serde_json::to_string(v).ok()
            }
        }))
    }

    pub fn save_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let mut file: HashMap<String, serde_json::Value> = self.read_yaml(&self.settings_path())?;
        
        // Try to parse as JSON, otherwise store as string
        let json_value = serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()));
        file.insert(key.to_string(), json_value);
        
        self.write_yaml(&self.settings_path(), &file)
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), String> {
        let mut file: HashMap<String, serde_json::Value> = self.read_yaml(&self.settings_path())?;
        file.remove(key);
        self.write_yaml(&self.settings_path(), &file)
    }
}
