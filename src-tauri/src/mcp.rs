use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use dashmap::DashMap;
use once_cell::sync::Lazy;

// Connection storage for MCP processes
static MCP_CONNECTIONS: Lazy<DashMap<String, Arc<Mutex<McpConnection>>>> = Lazy::new(DashMap::new);

struct McpConnection {
    process: Child,
    request_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<McpPromptArgument>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnectResult {
    pub success: bool,
    pub connection_id: Option<String>,
    pub server_info: Option<McpServerInfo>,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolResult {
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub time: u64,
}

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

async fn send_jsonrpc(
    conn: &mut McpConnection,
    method: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    let stdin = conn.process.stdin.as_mut()
        .ok_or("Failed to get stdin")?;
    let stdout = conn.process.stdout.as_mut()
        .ok_or("Failed to get stdout")?;
    
    conn.request_id += 1;
    let request = json!({
        "jsonrpc": "2.0",
        "id": conn.request_id,
        "method": method,
        "params": params.unwrap_or(json!({}))
    });
    
    let request_str = format!("{}\n", serde_json::to_string(&request).unwrap());
    stdin.write_all(request_str.as_bytes()).await
        .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    stdin.flush().await
        .map_err(|e| format!("Failed to flush stdin: {}", e))?;
    
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    
    // Read response with timeout
    let read_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        reader.read_line(&mut line)
    ).await;
    
    match read_result {
        Ok(Ok(_)) => {
            let response: Value = serde_json::from_str(&line)
                .map_err(|e| format!("Failed to parse response: {} - Line: {}", e, line))?;
            
            if let Some(error) = response.get("error") {
                return Err(format!("MCP error: {}", error));
            }
            
            Ok(response.get("result").cloned().unwrap_or(json!(null)))
        }
        Ok(Err(e)) => Err(format!("Failed to read response: {}", e)),
        Err(_) => Err("Timeout waiting for MCP response".to_string()),
    }
}

#[tauri::command]
pub async fn mcp_connect(
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
) -> Result<McpConnectResult, String> {
    let connection_id = generate_id();
    
    // Build the full command string
    let full_command = if args.is_empty() {
        command.clone()
    } else {
        format!("{} {}", command, args.join(" "))
    };
    
    // Start the MCP server process using shell
    // This ensures PATH is properly resolved for commands like npx, node, etc.
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", &full_command]);
        c
    };
    
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", &full_command]);
        c
    };
    
    cmd.envs(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    let process = cmd.spawn()
        .map_err(|e| format!("Failed to spawn MCP server: {}", e))?;
    
    let mut conn = McpConnection {
        process,
        request_id: 0,
    };
    
    // Initialize the connection
    let init_result = send_jsonrpc(&mut conn, "initialize", Some(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "roots": { "listChanged": true },
            "sampling": {}
        },
        "clientInfo": {
            "name": "istek",
            "version": "0.1.0"
        }
    }))).await;
    
    let server_info = match &init_result {
        Ok(result) => {
            let info = result.get("serverInfo");
            info.map(|i| McpServerInfo {
                name: i.get("name").and_then(|n| n.as_str()).unwrap_or("unknown").to_string(),
                version: i.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            })
        }
        Err(e) => {
            return Ok(McpConnectResult {
                success: false,
                connection_id: None,
                server_info: None,
                tools: vec![],
                resources: vec![],
                prompts: vec![],
                error: Some(e.clone()),
            });
        }
    };
    
    // Send initialized notification
    let _ = send_jsonrpc(&mut conn, "notifications/initialized", None).await;
    
    // List tools
    let tools = match send_jsonrpc(&mut conn, "tools/list", None).await {
        Ok(result) => {
            result.get("tools")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| serde_json::from_value(t.clone()).ok())
                        .collect()
                })
                .unwrap_or_default()
        }
        Err(_) => vec![],
    };
    
    // List resources
    let resources = match send_jsonrpc(&mut conn, "resources/list", None).await {
        Ok(result) => {
            result.get("resources")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| serde_json::from_value(r.clone()).ok())
                        .collect()
                })
                .unwrap_or_default()
        }
        Err(_) => vec![],
    };
    
    // List prompts
    let prompts = match send_jsonrpc(&mut conn, "prompts/list", None).await {
        Ok(result) => {
            result.get("prompts")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| serde_json::from_value(p.clone()).ok())
                        .collect()
                })
                .unwrap_or_default()
        }
        Err(_) => vec![],
    };
    
    // Store connection
    MCP_CONNECTIONS.insert(connection_id.clone(), Arc::new(Mutex::new(conn)));
    
    Ok(McpConnectResult {
        success: true,
        connection_id: Some(connection_id),
        server_info,
        tools,
        resources,
        prompts,
        error: None,
    })
}

#[tauri::command]
pub async fn mcp_call_tool(
    connection_id: String,
    tool_name: String,
    arguments: Value,
) -> Result<McpToolResult, String> {
    let start = std::time::Instant::now();
    
    let conn_arc = MCP_CONNECTIONS.get(&connection_id)
        .ok_or("Connection not found")?
        .clone();
    
    let mut conn = conn_arc.lock().await;
    
    let result = send_jsonrpc(&mut conn, "tools/call", Some(json!({
        "name": tool_name,
        "arguments": arguments
    }))).await;
    
    let time = start.elapsed().as_millis() as u64;
    
    match result {
        Ok(value) => Ok(McpToolResult {
            success: true,
            result: Some(value),
            error: None,
            time,
        }),
        Err(e) => Ok(McpToolResult {
            success: false,
            result: None,
            error: Some(e),
            time,
        }),
    }
}

#[tauri::command]
pub async fn mcp_read_resource(
    connection_id: String,
    uri: String,
) -> Result<McpToolResult, String> {
    let start = std::time::Instant::now();
    
    let conn_arc = MCP_CONNECTIONS.get(&connection_id)
        .ok_or("Connection not found")?
        .clone();
    
    let mut conn = conn_arc.lock().await;
    
    let result = send_jsonrpc(&mut conn, "resources/read", Some(json!({
        "uri": uri
    }))).await;
    
    let time = start.elapsed().as_millis() as u64;
    
    match result {
        Ok(value) => Ok(McpToolResult {
            success: true,
            result: Some(value),
            error: None,
            time,
        }),
        Err(e) => Ok(McpToolResult {
            success: false,
            result: None,
            error: Some(e),
            time,
        }),
    }
}

#[tauri::command]
pub async fn mcp_get_prompt(
    connection_id: String,
    prompt_name: String,
    arguments: HashMap<String, String>,
) -> Result<McpToolResult, String> {
    let start = std::time::Instant::now();
    
    let conn_arc = MCP_CONNECTIONS.get(&connection_id)
        .ok_or("Connection not found")?
        .clone();
    
    let mut conn = conn_arc.lock().await;
    
    let result = send_jsonrpc(&mut conn, "prompts/get", Some(json!({
        "name": prompt_name,
        "arguments": arguments
    }))).await;
    
    let time = start.elapsed().as_millis() as u64;
    
    match result {
        Ok(value) => Ok(McpToolResult {
            success: true,
            result: Some(value),
            error: None,
            time,
        }),
        Err(e) => Ok(McpToolResult {
            success: false,
            result: None,
            error: Some(e),
            time,
        }),
    }
}

#[tauri::command]
pub async fn mcp_disconnect(connection_id: String) -> Result<(), String> {
    if let Some((_, conn_arc)) = MCP_CONNECTIONS.remove(&connection_id) {
        let mut conn = conn_arc.lock().await;
        let _ = conn.process.kill().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn mcp_list_connections() -> Result<Vec<String>, String> {
    Ok(MCP_CONNECTIONS.iter().map(|r| r.key().clone()).collect())
}

// ============ MCP Config Discovery ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMcp {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub source: String, // "claude", "vscode", "cursor", "codex"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveryResult {
    pub source: String,
    pub config_path: Option<String>,
    pub servers: Vec<DiscoveredMcp>,
    pub error: Option<String>,
}

fn get_home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn parse_mcp_config(content: &str, source: &str) -> Vec<DiscoveredMcp> {
    let mut servers = Vec::new();
    
    let config: Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return servers,
    };
    
    // Both Claude and VS Code use "mcpServers" key
    let mcp_servers = config.get("mcpServers")
        .and_then(|s| s.as_object());
    
    if let Some(servers_map) = mcp_servers {
        for (name, server_config) in servers_map {
            let command = server_config.get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            
            if command.is_empty() {
                continue;
            }
            
            let args: Vec<String> = server_config.get("args")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            
            let env: HashMap<String, String> = server_config.get("env")
                .and_then(|e| e.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| {
                            v.as_str().map(|s| (k.clone(), s.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            
            servers.push(DiscoveredMcp {
                name: name.clone(),
                command,
                args,
                env,
                source: source.to_string(),
            });
        }
    }
    
    servers
}

#[tauri::command]
pub async fn mcp_discover_configs() -> Result<Vec<McpDiscoveryResult>, String> {
    let mut results = Vec::new();
    
    let home = match get_home_dir() {
        Some(h) => h,
        None => return Ok(results),
    };
    
    // Claude Desktop config paths
    let claude_paths = vec![
        // macOS
        home.join("Library/Application Support/Claude/claude_desktop_config.json"),
        // Windows
        home.join("AppData/Roaming/Claude/claude_desktop_config.json"),
        // Linux
        home.join(".config/claude/claude_desktop_config.json"),
    ];
    
    for path in claude_paths {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let servers = parse_mcp_config(&content, "claude");
                    results.push(McpDiscoveryResult {
                        source: "Claude Desktop".to_string(),
                        config_path: Some(path.to_string_lossy().to_string()),
                        servers,
                        error: None,
                    });
                    break;
                }
                Err(e) => {
                    results.push(McpDiscoveryResult {
                        source: "Claude Desktop".to_string(),
                        config_path: Some(path.to_string_lossy().to_string()),
                        servers: vec![],
                        error: Some(format!("Failed to read: {}", e)),
                    });
                }
            }
        }
    }
    
    // VS Code config paths
    let vscode_paths = vec![
        // macOS
        home.join("Library/Application Support/Code/User/settings.json"),
        // Windows
        home.join("AppData/Roaming/Code/User/settings.json"),
        // Linux
        home.join(".config/Code/User/settings.json"),
    ];
    
    for path in vscode_paths {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let servers = parse_mcp_config(&content, "vscode");
                    if !servers.is_empty() {
                        results.push(McpDiscoveryResult {
                            source: "VS Code".to_string(),
                            config_path: Some(path.to_string_lossy().to_string()),
                            servers,
                            error: None,
                        });
                    }
                    break;
                }
                Err(_) => {}
            }
        }
    }
    
    // Cursor config paths
    let cursor_paths = vec![
        // macOS
        home.join("Library/Application Support/Cursor/User/settings.json"),
        // Also check for MCP specific config
        home.join("Library/Application Support/Cursor/mcp.json"),
        // Windows
        home.join("AppData/Roaming/Cursor/User/settings.json"),
        // Linux
        home.join(".config/Cursor/User/settings.json"),
    ];
    
    for path in cursor_paths {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let servers = parse_mcp_config(&content, "cursor");
                    if !servers.is_empty() {
                        results.push(McpDiscoveryResult {
                            source: "Cursor".to_string(),
                            config_path: Some(path.to_string_lossy().to_string()),
                            servers,
                            error: None,
                        });
                        break;
                    }
                }
                Err(_) => {}
            }
        }
    }
    
    // Windsurf/Codeium config paths
    let windsurf_paths = vec![
        // macOS
        home.join("Library/Application Support/Windsurf/User/settings.json"),
        home.join(".codeium/windsurf/mcp_config.json"),
        // Windows
        home.join("AppData/Roaming/Windsurf/User/settings.json"),
        // Linux
        home.join(".config/Windsurf/User/settings.json"),
    ];
    
    for path in windsurf_paths {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let servers = parse_mcp_config(&content, "windsurf");
                    if !servers.is_empty() {
                        results.push(McpDiscoveryResult {
                            source: "Windsurf".to_string(),
                            config_path: Some(path.to_string_lossy().to_string()),
                            servers,
                            error: None,
                        });
                        break;
                    }
                }
                Err(_) => {}
            }
        }
    }
    
    // OpenCode/Codex config paths (similar to Claude format)
    let opencode_paths = vec![
        home.join(".opencode/mcp.json"),
        home.join(".config/opencode/mcp.json"),
        home.join(".codex/mcp.json"),
    ];
    
    for path in opencode_paths {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let servers = parse_mcp_config(&content, "opencode");
                    if !servers.is_empty() {
                        results.push(McpDiscoveryResult {
                            source: "OpenCode".to_string(),
                            config_path: Some(path.to_string_lossy().to_string()),
                            servers,
                            error: None,
                        });
                        break;
                    }
                }
                Err(_) => {}
            }
        }
    }
    
    Ok(results)
}
