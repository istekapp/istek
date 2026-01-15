mod http;
mod websocket;
mod mqtt;
mod graphql;
mod unix_socket;
mod sse;
mod storage;
mod storage_commands;
mod import;
mod mcp;
mod mock_server;
mod test_runner;
mod secret_providers;
mod fake_data;
mod playground;
mod grpc_client;
mod template_functions;
mod scripting;
mod git_export;
mod sync;
mod api;
mod api_server;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tauri::Manager;

// Global connection managers
pub static WS_CONNECTIONS: Lazy<DashMap<String, websocket::WsConnection>> = Lazy::new(DashMap::new);
pub static MQTT_CONNECTIONS: Lazy<DashMap<String, mqtt::MqttConnection>> = Lazy::new(DashMap::new);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Initialize YAML-based storage
            let storage = storage::Storage::new()
                .expect("Failed to initialize storage");
            
            // Wrap in Arc for sharing between Tauri and API server
            let storage_arc = Arc::new(storage);
            
            // Start the internal REST API server (port 47835)
            let storage_for_api = storage_arc.clone();
            tauri::async_runtime::spawn(async move {
                api_server::start_server(storage_for_api).await;
            });
            
            // Manage the Arc<Storage>
            app.manage(storage_arc);
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // HTTP
            http::send_http_request,
            http::send_multipart_request,
            http::generate_curl_command,
            http::generate_code_snippet,
            http::parse_curl_command,
            // WebSocket
            websocket::ws_connect,
            websocket::ws_send,
            websocket::ws_disconnect,
            // MQTT
            mqtt::mqtt_connect,
            mqtt::mqtt_subscribe,
            mqtt::mqtt_publish,
            mqtt::mqtt_disconnect,
            // GraphQL
            graphql::send_graphql_request,
            // Unix Socket
            unix_socket::send_unix_socket_request,
            // SSE
            sse::sse_connect,
            sse::sse_disconnect,
            // Storage - Load
            storage_commands::load_app_data,
            storage_commands::load_workspace_data,
            // Storage - Workspaces
            storage_commands::create_workspace,
            storage_commands::update_workspace,
            storage_commands::delete_workspace,
            storage_commands::set_active_workspace,
            storage_commands::get_workspace,
            storage_commands::get_default_sync_path,
            // Storage - Collections
            storage_commands::save_collection,
            storage_commands::delete_collection,
            // Storage - History
            storage_commands::save_history_item,
            storage_commands::clear_history,
            storage_commands::delete_history_item,
            // Storage - Global Variables
            storage_commands::save_global_variable,
            storage_commands::delete_global_variable,
            storage_commands::save_all_global_variables,
            // Storage - Environments
            storage_commands::save_environment,
            storage_commands::delete_environment,
            storage_commands::save_all_environments,
            // Storage - Secret Providers
            storage_commands::save_secret_provider,
            storage_commands::delete_secret_provider,
            // Storage - MCP Servers
            storage_commands::get_mcp_servers,
            storage_commands::add_mcp_server,
            storage_commands::update_mcp_server,
            storage_commands::delete_mcp_server,
            storage_commands::toggle_mcp_server,
            // Storage - Settings
            storage_commands::save_active_environment_id,
            // Storage - Test Runs
            storage_commands::save_test_run,
            storage_commands::load_test_runs,
            storage_commands::delete_test_run,
            storage_commands::clear_test_runs,
            // Storage - Utility
            storage_commands::get_config_dir,
            // Import
            import::import_openapi,
            import::import_postman,
            import::import_from_url,
            import::generate_mock_response,
            import::generate_mock_response_smart,
            // MCP
            mcp::mcp_connect,
            mcp::mcp_call_tool,
            mcp::mcp_read_resource,
            mcp::mcp_get_prompt,
            mcp::mcp_disconnect,
            mcp::mcp_list_connections,
            mcp::mcp_discover_configs,
            // Mock Server
            mock_server::mock_server_start,
            mock_server::mock_server_stop,
            mock_server::mock_server_list,
            mock_server::mock_server_stop_all,
            mock_server::create_mock_endpoint,
            // Test Runner
            test_runner::run_collection_tests,
            test_runner::create_test_config,
            test_runner::evaluate_jsonpath_test,
            // Secret Providers
            secret_providers::fetch_aws_secrets,
            secret_providers::fetch_gcp_secrets,
            secret_providers::fetch_azure_secrets,
            secret_providers::test_secret_provider_connection,
            secret_providers::test_provider_auth,
            // Playground
            playground::playground_start,
            playground::playground_stop,
            playground::playground_status,
            // gRPC Client
            grpc_client::grpc_discover_services,
            grpc_client::grpc_parse_proto,
            grpc_client::grpc_call,
            // Template Functions - Hash
            template_functions::hash_md5,
            template_functions::hash_sha1,
            template_functions::hash_sha256,
            template_functions::hash_sha512,
            template_functions::hmac_sha256,
            template_functions::hmac_sha512,
            // Template Functions - Encoding
            template_functions::encode_base64,
            template_functions::decode_base64,
            template_functions::encode_url,
            template_functions::decode_url,
            // Template Functions - Encryption (Keychain)
            template_functions::encrypt_store,
            template_functions::encrypt_retrieve,
            template_functions::encrypt_delete,
            template_functions::encrypt_list_keys,
            // Template Functions - Utility
            template_functions::generate_uuid,
            template_functions::timestamp_now,
            template_functions::timestamp_now_ms,
            template_functions::format_timestamp,
            // Template Functions - Random
            template_functions::random_int,
            template_functions::random_float,
            template_functions::random_string,
            template_functions::random_hex,
            // Scripting
            scripting::run_pre_request_script,
            scripting::run_post_request_script,
            scripting::test_script,
            // Git Export
            git_export::export_collection_yaml,
            git_export::import_collection_yaml,
            git_export::export_all_collections_yaml,
            // Sync
            sync::sync_init,
            sync::sync_get_config,
            sync::sync_save_config,
            sync::sync_get_status,
            sync::sync_export_all,
            sync::sync_export_collections,
            sync::sync_export_environments,
            sync::sync_export_global_variables,
            sync::sync_import_all,
            sync::sync_import_collections,
            sync::sync_import_environments,
            sync::sync_import_global_variables,
            // Git Operations
            sync::git_init,
            sync::git_get_status,
            sync::git_commit,
            sync::git_pull,
            sync::git_push,
            sync::git_add_remote,
            sync::git_get_log,
            sync::git_list_branches,
            sync::git_create_branch,
            sync::git_switch_branch,
            sync::git_get_commit_files,
            sync::git_get_file_diff,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
