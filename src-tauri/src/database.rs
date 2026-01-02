use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub sync_path: Option<String>,  // If set, enables filesystem sync
    pub is_default: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub requests: serde_json::Value, // Store as JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<serde_json::Value>, // Store as JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>, // Store FolderSettings as JSON
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub request: serde_json::Value, // Store as JSON
    pub response: Option<serde_json::Value>, // Store as JSON
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub id: String,
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub is_secret: bool,
    pub secret_provider: Option<serde_json::Value>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub color: String,
    pub variables: Vec<Variable>,
    pub is_default: Option<bool>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecretProvider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub enabled: bool,
    pub config: Option<serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestRunHistory {
    pub id: String,
    pub run_id: String,
    pub collection_id: Option<String>,
    pub collection_name: String,
    pub timestamp: i64,
    pub summary: serde_json::Value, // Store TestRunSummary as JSON
}

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

// Database state
pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_data_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create app data directory: {}", e))?;

        let db_path = app_data_dir.join("istek.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Create base tables first (without workspace_id for compatibility with existing DBs)
        conn.execute_batch(
            "
            -- Workspaces table
            CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                sync_path TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );

            -- Collections table (base schema - workspace_id added via migration)
            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                requests TEXT NOT NULL DEFAULT '[]',
                created_at INTEGER NOT NULL
            );

            -- History table (base schema - workspace_id added via migration)
            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                request TEXT NOT NULL,
                response TEXT,
                timestamp INTEGER NOT NULL
            );

            -- Global variables table (base schema - workspace_id added via migration)
            CREATE TABLE IF NOT EXISTS global_variables (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL DEFAULT '',
                description TEXT,
                is_secret INTEGER NOT NULL DEFAULT 0,
                secret_provider TEXT,
                enabled INTEGER NOT NULL DEFAULT 1
            );

            -- Environments table (base schema - workspace_id added via migration)
            CREATE TABLE IF NOT EXISTS environments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL,
                variables TEXT NOT NULL DEFAULT '[]',
                is_default INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL
            );

            -- Secret providers table (global - not workspace-scoped)
            CREATE TABLE IF NOT EXISTS secret_providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config TEXT,
                created_at INTEGER NOT NULL
            );

            -- App settings table
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Test runs history table (base schema - workspace_id added via migration)
            CREATE TABLE IF NOT EXISTS test_runs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                collection_id TEXT,
                collection_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                summary TEXT NOT NULL
            );

            -- Create base indexes (non-workspace ones)
            CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_collections_created ON collections(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_test_runs_timestamp ON test_runs(timestamp DESC);
            "
        ).map_err(|e| format!("Failed to create schema: {}", e))?;

        // Run migrations for existing databases (adds workspace_id columns and indexes)
        Self::run_migrations(&conn)?;

        Ok(())
    }

    fn run_migrations(conn: &Connection) -> Result<(), String> {
        // Helper to check if column exists
        let column_exists = |table: &str, column: &str| -> bool {
            conn.prepare(&format!("SELECT 1 FROM pragma_table_info('{}') WHERE name = '{}'", table, column))
                .and_then(|mut stmt| stmt.exists([]))
                .unwrap_or(false)
        };

        // Collections migrations
        if !column_exists("collections", "folders") {
            conn.execute("ALTER TABLE collections ADD COLUMN folders TEXT", [])
                .map_err(|e| format!("Failed to add folders column: {}", e))?;
        }
        if !column_exists("collections", "settings") {
            conn.execute("ALTER TABLE collections ADD COLUMN settings TEXT", [])
                .map_err(|e| format!("Failed to add settings column: {}", e))?;
        }
        if !column_exists("collections", "workspace_id") {
            conn.execute("ALTER TABLE collections ADD COLUMN workspace_id TEXT", [])
                .map_err(|e| format!("Failed to add workspace_id to collections: {}", e))?;
        }

        // Environments migrations
        if !column_exists("environments", "shareable") {
            conn.execute("ALTER TABLE environments ADD COLUMN shareable INTEGER DEFAULT 0", [])
                .map_err(|e| format!("Failed to add shareable column: {}", e))?;
        }
        if !column_exists("environments", "workspace_id") {
            conn.execute("ALTER TABLE environments ADD COLUMN workspace_id TEXT", [])
                .map_err(|e| format!("Failed to add workspace_id to environments: {}", e))?;
        }

        // History migrations
        if !column_exists("history", "workspace_id") {
            conn.execute("ALTER TABLE history ADD COLUMN workspace_id TEXT", [])
                .map_err(|e| format!("Failed to add workspace_id to history: {}", e))?;
        }

        // Global variables migrations
        if !column_exists("global_variables", "workspace_id") {
            conn.execute("ALTER TABLE global_variables ADD COLUMN workspace_id TEXT", [])
                .map_err(|e| format!("Failed to add workspace_id to global_variables: {}", e))?;
        }

        // Test runs migrations
        if !column_exists("test_runs", "workspace_id") {
            conn.execute("ALTER TABLE test_runs ADD COLUMN workspace_id TEXT", [])
                .map_err(|e| format!("Failed to add workspace_id to test_runs: {}", e))?;
        }

        // Create indexes
        conn.execute("CREATE INDEX IF NOT EXISTS idx_collections_workspace ON collections(workspace_id)", []).ok();
        conn.execute("CREATE INDEX IF NOT EXISTS idx_history_workspace ON history(workspace_id)", []).ok();
        conn.execute("CREATE INDEX IF NOT EXISTS idx_environments_workspace ON environments(workspace_id)", []).ok();
        conn.execute("CREATE INDEX IF NOT EXISTS idx_global_variables_workspace ON global_variables(workspace_id)", []).ok();
        conn.execute("CREATE INDEX IF NOT EXISTS idx_test_runs_workspace ON test_runs(workspace_id)", []).ok();

        // Ensure workspaces table exists (for existing databases)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                sync_path TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            )",
            [],
        ).map_err(|e| format!("Failed to create workspaces table: {}", e))?;

        // Create a default workspace if none exists
        let workspace_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .unwrap_or(0);

        let default_workspace_id: String;
        if workspace_count == 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            
            default_workspace_id = format!("ws_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..16].to_string());
            conn.execute(
                "INSERT INTO workspaces (id, name, sync_path, is_default, created_at) VALUES (?1, ?2, NULL, 1, ?3)",
                params![&default_workspace_id, "Default Workspace", now],
            ).map_err(|e| format!("Failed to create default workspace: {}", e))?;
        } else {
            // Get the default workspace ID
            default_workspace_id = conn
                .query_row("SELECT id FROM workspaces WHERE is_default = 1", [], |row| row.get(0))
                .or_else(|_| conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |row| row.get(0)))
                .unwrap_or_default();
        }

        // Migrate existing data to default workspace (if workspace_id is NULL)
        if !default_workspace_id.is_empty() {
            conn.execute("UPDATE collections SET workspace_id = ?1 WHERE workspace_id IS NULL", params![&default_workspace_id]).ok();
            conn.execute("UPDATE history SET workspace_id = ?1 WHERE workspace_id IS NULL", params![&default_workspace_id]).ok();
            conn.execute("UPDATE environments SET workspace_id = ?1 WHERE workspace_id IS NULL", params![&default_workspace_id]).ok();
            conn.execute("UPDATE global_variables SET workspace_id = ?1 WHERE workspace_id IS NULL", params![&default_workspace_id]).ok();
            conn.execute("UPDATE test_runs SET workspace_id = ?1 WHERE workspace_id IS NULL", params![&default_workspace_id]).ok();
        }

        Ok(())
    }
}

// ============ Tauri Commands ============

#[tauri::command]
pub async fn load_app_data(app: tauri::AppHandle) -> Result<AppData, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Load workspaces
    let mut stmt = conn.prepare("SELECT id, name, sync_path, is_default, created_at FROM workspaces ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let workspaces: Vec<Workspace> = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                sync_path: row.get(2)?,
                is_default: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load active workspace ID (or use default)
    let active_workspace_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
            [],
            |row| row.get(0),
        )
        .ok()
        .or_else(|| workspaces.iter().find(|w| w.is_default).map(|w| w.id.clone()))
        .or_else(|| workspaces.first().map(|w| w.id.clone()));

    // Get workspace ID for filtering (use active or default)
    let workspace_id = active_workspace_id.clone().unwrap_or_default();

    // Load collections for active workspace
    let mut stmt = conn.prepare("SELECT id, name, requests, folders, settings, created_at FROM collections WHERE workspace_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let collections: Vec<Collection> = stmt
        .query_map(params![&workspace_id], |row| {
            let folders_str: Option<String> = row.get(3)?;
            let settings_str: Option<String> = row.get(4)?;
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                requests: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or(serde_json::json!([])),
                folders: folders_str.and_then(|s| serde_json::from_str(&s).ok()),
                settings: settings_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load history for active workspace
    let mut stmt = conn.prepare("SELECT id, request, response, timestamp FROM history WHERE workspace_id = ?1 ORDER BY timestamp DESC LIMIT 100")
        .map_err(|e| e.to_string())?;
    let history: Vec<HistoryItem> = stmt
        .query_map(params![&workspace_id], |row| {
            let response_str: Option<String> = row.get(2)?;
            Ok(HistoryItem {
                id: row.get(0)?,
                request: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or(serde_json::json!({})),
                response: response_str.and_then(|s| serde_json::from_str(&s).ok()),
                timestamp: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load global variables for active workspace
    let mut stmt = conn.prepare("SELECT id, key, value, description, is_secret, secret_provider, enabled FROM global_variables WHERE workspace_id = ?1")
        .map_err(|e| e.to_string())?;
    let global_variables: Vec<Variable> = stmt
        .query_map(params![&workspace_id], |row| {
            let secret_provider_str: Option<String> = row.get(5)?;
            Ok(Variable {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                description: row.get(3)?,
                is_secret: row.get::<_, i32>(4)? != 0,
                secret_provider: secret_provider_str.and_then(|s| serde_json::from_str(&s).ok()),
                enabled: row.get::<_, i32>(6)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load environments for active workspace
    let mut stmt = conn.prepare("SELECT id, name, color, variables, is_default, created_at FROM environments WHERE workspace_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let environments: Vec<Environment> = stmt
        .query_map(params![&workspace_id], |row| {
            let variables_str: String = row.get(3)?;
            let variables: Vec<Variable> = serde_json::from_str(&variables_str).unwrap_or_default();
            Ok(Environment {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                variables,
                is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load secret providers (global - not workspace scoped)
    let mut stmt = conn.prepare("SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let secret_providers: Vec<SecretProvider> = stmt
        .query_map([], |row| {
            let config_str: Option<String> = row.get(4)?;
            Ok(SecretProvider {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
                config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load active environment ID for this workspace
    let active_env_key = format!("active_environment_id_{}", workspace_id);
    let active_environment_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![&active_env_key],
            |row| row.get(0),
        )
        .ok();

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

// ============ Workspace Commands ============

#[tauri::command]
pub async fn create_workspace(app: tauri::AppHandle, name: String, sync_path: Option<String>) -> Result<Workspace, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let id = format!("ws_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..16].to_string());

    // If sync_path is provided, create the directory structure
    if let Some(ref path) = sync_path {
        let sync_dir = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&sync_dir)
            .map_err(|e| format!("Failed to create sync directory: {}", e))?;
        
        // Create subdirectories
        std::fs::create_dir_all(sync_dir.join("collections"))
            .map_err(|e| format!("Failed to create collections directory: {}", e))?;
        std::fs::create_dir_all(sync_dir.join("environments"))
            .map_err(|e| format!("Failed to create environments directory: {}", e))?;
        
        // Create .gitignore
        let gitignore_content = "# Local files (not synced)\n*.local.yaml\n.secrets/\n\n# OS files\n.DS_Store\nThumbs.db\n";
        std::fs::write(sync_dir.join(".gitignore"), gitignore_content)
            .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
        
        // Initialize git repository
        if let Err(e) = git2::Repository::init(&sync_dir) {
            // Log but don't fail - git init is optional
            eprintln!("Note: Could not initialize git repository: {}", e);
        }
    }

    conn.execute(
        "INSERT INTO workspaces (id, name, sync_path, is_default, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
        params![id, name, sync_path, now],
    ).map_err(|e| format!("Failed to create workspace: {}", e))?;

    // Set as active workspace
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_workspace_id', ?1)",
        params![id],
    ).map_err(|e| format!("Failed to set active workspace: {}", e))?;

    Ok(Workspace {
        id,
        name,
        sync_path,
        is_default: false,
        created_at: now,
    })
}

#[tauri::command]
pub async fn update_workspace(app: tauri::AppHandle, workspace: Workspace) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // If sync_path is being set, create the directory structure
    if let Some(ref path) = workspace.sync_path {
        let sync_dir = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&sync_dir)
            .map_err(|e| format!("Failed to create sync directory: {}", e))?;
        
        // Create subdirectories
        std::fs::create_dir_all(sync_dir.join("collections"))
            .map_err(|e| format!("Failed to create collections directory: {}", e))?;
        std::fs::create_dir_all(sync_dir.join("environments"))
            .map_err(|e| format!("Failed to create environments directory: {}", e))?;
        
        // Create .gitignore if it doesn't exist
        let gitignore_path = sync_dir.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content = "# Local files (not synced)\n*.local.yaml\n.secrets/\n\n# OS files\n.DS_Store\nThumbs.db\n";
            std::fs::write(&gitignore_path, gitignore_content)
                .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
        }
    }

    conn.execute(
        "UPDATE workspaces SET name = ?1, sync_path = ?2 WHERE id = ?3",
        params![workspace.name, workspace.sync_path, workspace.id],
    ).map_err(|e| format!("Failed to update workspace: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_workspace(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Check if it's the default workspace
    let is_default: bool = conn
        .query_row("SELECT is_default FROM workspaces WHERE id = ?1", params![id], |row| row.get::<_, i32>(0))
        .map(|v| v != 0)
        .unwrap_or(false);

    if is_default {
        return Err("Cannot delete the default workspace".to_string());
    }

    // Delete all collections in this workspace
    conn.execute("DELETE FROM collections WHERE workspace_id = ?1", params![id])
        .map_err(|e| format!("Failed to delete workspace collections: {}", e))?;

    // Delete the workspace
    conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete workspace: {}", e))?;

    // If this was the active workspace, switch to default
    let active_id: Option<String> = conn
        .query_row("SELECT value FROM app_settings WHERE key = 'active_workspace_id'", [], |row| row.get(0))
        .ok();

    if active_id == Some(id) {
        let default_id: Option<String> = conn
            .query_row("SELECT id FROM workspaces WHERE is_default = 1", [], |row| row.get(0))
            .ok();

        if let Some(default_id) = default_id {
            conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_workspace_id', ?1)",
                params![default_id],
            ).map_err(|e| format!("Failed to set active workspace: {}", e))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn set_active_workspace(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_workspace_id', ?1)",
        params![id],
    ).map_err(|e| format!("Failed to set active workspace: {}", e))?;

    Ok(())
}

/// Load data for a specific workspace (used when switching workspaces)
#[tauri::command]
pub async fn load_workspace_data(app: tauri::AppHandle, workspace_id: String) -> Result<AppData, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Load workspaces
    let mut stmt = conn.prepare("SELECT id, name, sync_path, is_default, created_at FROM workspaces ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let workspaces: Vec<Workspace> = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                sync_path: row.get(2)?,
                is_default: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load collections for workspace
    let mut stmt = conn.prepare("SELECT id, name, requests, folders, settings, created_at FROM collections WHERE workspace_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let collections: Vec<Collection> = stmt
        .query_map(params![&workspace_id], |row| {
            let folders_str: Option<String> = row.get(3)?;
            let settings_str: Option<String> = row.get(4)?;
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                requests: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or(serde_json::json!([])),
                folders: folders_str.and_then(|s| serde_json::from_str(&s).ok()),
                settings: settings_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load history for workspace
    let mut stmt = conn.prepare("SELECT id, request, response, timestamp FROM history WHERE workspace_id = ?1 ORDER BY timestamp DESC LIMIT 100")
        .map_err(|e| e.to_string())?;
    let history: Vec<HistoryItem> = stmt
        .query_map(params![&workspace_id], |row| {
            let response_str: Option<String> = row.get(2)?;
            Ok(HistoryItem {
                id: row.get(0)?,
                request: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or(serde_json::json!({})),
                response: response_str.and_then(|s| serde_json::from_str(&s).ok()),
                timestamp: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load global variables for workspace
    let mut stmt = conn.prepare("SELECT id, key, value, description, is_secret, secret_provider, enabled FROM global_variables WHERE workspace_id = ?1")
        .map_err(|e| e.to_string())?;
    let global_variables: Vec<Variable> = stmt
        .query_map(params![&workspace_id], |row| {
            let secret_provider_str: Option<String> = row.get(5)?;
            Ok(Variable {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                description: row.get(3)?,
                is_secret: row.get::<_, i32>(4)? != 0,
                secret_provider: secret_provider_str.and_then(|s| serde_json::from_str(&s).ok()),
                enabled: row.get::<_, i32>(6)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load environments for workspace
    let mut stmt = conn.prepare("SELECT id, name, color, variables, is_default, created_at FROM environments WHERE workspace_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let environments: Vec<Environment> = stmt
        .query_map(params![&workspace_id], |row| {
            let variables_str: String = row.get(3)?;
            let variables: Vec<Variable> = serde_json::from_str(&variables_str).unwrap_or_default();
            Ok(Environment {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                variables,
                is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load secret providers (global)
    let mut stmt = conn.prepare("SELECT id, name, provider_type, enabled, config, created_at FROM secret_providers ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let secret_providers: Vec<SecretProvider> = stmt
        .query_map([], |row| {
            let config_str: Option<String> = row.get(4)?;
            Ok(SecretProvider {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
                config: config_str.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Load active environment for this workspace
    let active_env_key = format!("active_environment_id_{}", workspace_id);
    let active_environment_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![&active_env_key],
            |row| row.get(0),
        )
        .ok();

    Ok(AppData {
        workspaces,
        active_workspace_id: Some(workspace_id),
        collections,
        history,
        global_variables,
        environments,
        secret_providers,
        active_environment_id,
    })
}

#[tauri::command]
pub async fn get_workspace(app: tauri::AppHandle, id: String) -> Result<Option<Workspace>, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let workspace = conn
        .query_row(
            "SELECT id, name, sync_path, is_default, created_at FROM workspaces WHERE id = ?1",
            params![id],
            |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sync_path: row.get(2)?,
                    is_default: row.get::<_, i32>(3)? != 0,
                    created_at: row.get(4)?,
                })
            },
        )
        .ok();

    Ok(workspace)
}

#[tauri::command]
pub async fn get_default_sync_path(name: String) -> Result<String, String> {
    let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
    
    // Sanitize name for filesystem
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
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get workspace_id - use provided or get active
    let ws_id = workspace_id.or_else(|| {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
            [],
            |row| row.get(0),
        ).ok()
    });

    let folders_json: Option<String> = collection.folders.as_ref()
        .map(|f| serde_json::to_string(f).unwrap_or_default());
    
    let settings_json: Option<String> = collection.settings.as_ref()
        .map(|s| serde_json::to_string(s).unwrap_or_default());

    conn.execute(
        "INSERT OR REPLACE INTO collections (id, name, requests, folders, settings, workspace_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            collection.id,
            collection.name,
            serde_json::to_string(&collection.requests).unwrap_or_default(),
            folders_json,
            settings_json,
            ws_id,
            collection.created_at
        ],
    ).map_err(|e| format!("Failed to save collection: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_collection(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM collections WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete collection: {}", e))?;

    Ok(())
}

// ============ History Commands ============

#[tauri::command]
pub async fn save_history_item(app: tauri::AppHandle, item: HistoryItem, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get workspace_id - use provided or get active
    let ws_id = workspace_id.or_else(|| {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
            [],
            |row| row.get(0),
        ).ok()
    });

    conn.execute(
        "INSERT OR REPLACE INTO history (id, request, response, workspace_id, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            item.id,
            serde_json::to_string(&item.request).unwrap_or_default(),
            item.response.map(|r| serde_json::to_string(&r).unwrap_or_default()),
            ws_id,
            item.timestamp
        ],
    ).map_err(|e| format!("Failed to save history item: {}", e))?;

    // Keep only the latest 100 history items per workspace
    if let Some(ref ws) = ws_id {
        conn.execute(
            "DELETE FROM history WHERE workspace_id = ?1 AND id NOT IN (SELECT id FROM history WHERE workspace_id = ?1 ORDER BY timestamp DESC LIMIT 100)",
            params![ws],
        ).map_err(|e| format!("Failed to cleanup history: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn clear_history(app: tauri::AppHandle, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get workspace_id - use provided or get active
    let ws_id = workspace_id.or_else(|| {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
            [],
            |row| row.get(0),
        ).ok()
    });

    if let Some(ws) = ws_id {
        conn.execute("DELETE FROM history WHERE workspace_id = ?1", params![ws])
            .map_err(|e| format!("Failed to clear history: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_history_item(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM history WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete history item: {}", e))?;

    Ok(())
}

// ============ Global Variable Commands ============

#[tauri::command]
pub async fn save_global_variable(app: tauri::AppHandle, variable: Variable, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let ws_id = workspace_id.or_else(|| {
        conn.query_row("SELECT value FROM app_settings WHERE key = 'active_workspace_id'", [], |row| row.get(0)).ok()
    });

    conn.execute(
        "INSERT OR REPLACE INTO global_variables (id, key, value, description, is_secret, secret_provider, enabled, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            variable.id,
            variable.key,
            variable.value,
            variable.description,
            variable.is_secret as i32,
            variable.secret_provider.map(|sp| serde_json::to_string(&sp).unwrap_or_default()),
            variable.enabled as i32,
            ws_id
        ],
    ).map_err(|e| format!("Failed to save global variable: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_global_variable(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM global_variables WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete global variable: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn save_all_global_variables(app: tauri::AppHandle, variables: Vec<Variable>, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let ws_id = workspace_id.or_else(|| {
        conn.query_row("SELECT value FROM app_settings WHERE key = 'active_workspace_id'", [], |row| row.get(0)).ok()
    });

    // Clear existing for this workspace and insert all
    if let Some(ref ws) = ws_id {
        conn.execute("DELETE FROM global_variables WHERE workspace_id = ?1", params![ws])
            .map_err(|e| format!("Failed to clear global variables: {}", e))?;
    }

    for variable in variables {
        conn.execute(
            "INSERT INTO global_variables (id, key, value, description, is_secret, secret_provider, enabled, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                variable.id,
                variable.key,
                variable.value,
                variable.description,
                variable.is_secret as i32,
                variable.secret_provider.map(|sp| serde_json::to_string(&sp).unwrap_or_default()),
                variable.enabled as i32,
                ws_id
            ],
        ).map_err(|e| format!("Failed to save global variable: {}", e))?;
    }

    Ok(())
}

// ============ Environment Commands ============

#[tauri::command]
pub async fn save_environment(app: tauri::AppHandle, environment: Environment, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let ws_id = workspace_id.or_else(|| {
        conn.query_row("SELECT value FROM app_settings WHERE key = 'active_workspace_id'", [], |row| row.get(0)).ok()
    });

    conn.execute(
        "INSERT OR REPLACE INTO environments (id, name, color, variables, is_default, workspace_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            environment.id,
            environment.name,
            environment.color,
            serde_json::to_string(&environment.variables).unwrap_or_default(),
            environment.is_default.map(|v| v as i32),
            ws_id,
            environment.created_at
        ],
    ).map_err(|e| format!("Failed to save environment: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_environment(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM environments WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete environment: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn save_all_environments(app: tauri::AppHandle, environments: Vec<Environment>, workspace_id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let ws_id = workspace_id.or_else(|| {
        conn.query_row("SELECT value FROM app_settings WHERE key = 'active_workspace_id'", [], |row| row.get(0)).ok()
    });

    // Clear existing for this workspace and insert all
    if let Some(ref ws) = ws_id {
        conn.execute("DELETE FROM environments WHERE workspace_id = ?1", params![ws])
            .map_err(|e| format!("Failed to clear environments: {}", e))?;
    }

    for env in environments {
        conn.execute(
            "INSERT INTO environments (id, name, color, variables, is_default, workspace_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                env.id,
                env.name,
                env.color,
                serde_json::to_string(&env.variables).unwrap_or_default(),
                env.is_default.map(|v| v as i32),
                ws_id,
                env.created_at
            ],
        ).map_err(|e| format!("Failed to save environment: {}", e))?;
    }

    Ok(())
}

// ============ Secret Provider Commands ============

#[tauri::command]
pub async fn save_secret_provider(app: tauri::AppHandle, provider: SecretProvider) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO secret_providers (id, name, provider_type, enabled, config, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            provider.id,
            provider.name,
            provider.provider_type,
            provider.enabled as i32,
            provider.config.map(|c| serde_json::to_string(&c).unwrap_or_default()),
            provider.created_at
        ],
    ).map_err(|e| format!("Failed to save secret provider: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_secret_provider(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM secret_providers WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete secret provider: {}", e))?;

    Ok(())
}

// ============ App Settings Commands ============

#[tauri::command]
pub async fn save_active_environment_id(app: tauri::AppHandle, id: Option<String>) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    match id {
        Some(env_id) => {
            conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('active_environment_id', ?1)",
                params![env_id],
            ).map_err(|e| format!("Failed to save active environment: {}", e))?;
        }
        None => {
            conn.execute(
                "DELETE FROM app_settings WHERE key = 'active_environment_id'",
                [],
            ).map_err(|e| format!("Failed to clear active environment: {}", e))?;
        }
    }

    Ok(())
}

// ============ Test Run History Commands ============

#[tauri::command]
pub async fn save_test_run(app: tauri::AppHandle, test_run: TestRunHistory) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO test_runs (id, run_id, collection_id, collection_name, timestamp, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            test_run.id,
            test_run.run_id,
            test_run.collection_id,
            test_run.collection_name,
            test_run.timestamp,
            serde_json::to_string(&test_run.summary).unwrap_or_default()
        ],
    ).map_err(|e| format!("Failed to save test run: {}", e))?;

    // Keep only the latest 50 test runs
    conn.execute(
        "DELETE FROM test_runs WHERE id NOT IN (SELECT id FROM test_runs ORDER BY timestamp DESC LIMIT 50)",
        [],
    ).map_err(|e| format!("Failed to cleanup old test runs: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn load_test_runs(app: tauri::AppHandle) -> Result<Vec<TestRunHistory>, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT id, run_id, collection_id, collection_name, timestamp, summary FROM test_runs ORDER BY timestamp DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    
    let test_runs: Vec<TestRunHistory> = stmt
        .query_map([], |row| {
            let summary_str: String = row.get(5)?;
            Ok(TestRunHistory {
                id: row.get(0)?,
                run_id: row.get(1)?,
                collection_id: row.get(2)?,
                collection_name: row.get(3)?,
                timestamp: row.get(4)?,
                summary: serde_json::from_str(&summary_str).unwrap_or(serde_json::json!({})),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(test_runs)
}

#[tauri::command]
pub async fn delete_test_run(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM test_runs WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete test run: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn clear_test_runs(app: tauri::AppHandle) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM test_runs", [])
        .map_err(|e| format!("Failed to clear test runs: {}", e))?;

    Ok(())
}

// ============ Environment Shareable Commands ============

#[tauri::command]
pub async fn set_environment_shareable(app: tauri::AppHandle, id: String, shareable: bool) -> Result<(), String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE environments SET shareable = ?1 WHERE id = ?2",
        params![shareable as i32, id],
    ).map_err(|e| format!("Failed to update environment shareable: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_shareable_environments(app: tauri::AppHandle) -> Result<Vec<Environment>, String> {
    let db = app.state::<Arc<Database>>();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT id, name, color, variables, is_default, created_at FROM environments WHERE shareable = 1")
        .map_err(|e| e.to_string())?;
    
    let environments: Vec<Environment> = stmt
        .query_map([], |row| {
            let variables_str: String = row.get(3)?;
            let variables: Vec<Variable> = serde_json::from_str(&variables_str).unwrap_or_default();
            Ok(Environment {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                variables,
                is_default: row.get::<_, Option<i32>>(4)?.map(|v| v != 0),
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(environments)
}
