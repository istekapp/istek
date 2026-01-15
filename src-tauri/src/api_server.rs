use axum::{
    Router,
    routing::{get, post, put, delete},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::{
    health::{self, HealthResponse},
    workspaces::{self, WorkspaceResponse, CreateWorkspaceRequest, UpdateWorkspaceRequest},
    collections::{self, CollectionResponse, CreateCollectionRequest, UpdateCollectionRequest, AddRequestToCollectionRequest, UpdateRequestRequest},
    environments::{self, EnvironmentResponse, CreateEnvironmentRequest, UpdateEnvironmentRequest, VariableResponse as EnvVariableResponse},
    variables::{self, VariableResponse, CreateVariableRequest, UpdateVariableRequest},
    integrations::{self, IntegrationResponse, CreateIntegrationRequest, UpdateIntegrationRequest, TestIntegrationRequest, TestIntegrationResponse, FetchSecretRequest, FetchSecretResponse},
    history::{self, HistoryItemResponse},
    tests::{self, RunTestsRequest, RunCollectionTestsRequest, TestRunSummary, TestResult, TestRequest, Assertion, AssertionResult, KeyValue as TestKeyValue, TestStatus, AssertionType, JsonPathOperator, VariableExtraction, ExtractedVariable},
    ApiError, ErrorDetail, SuccessResponse,
};
use crate::storage::Storage;

const API_PORT: u16 = 47835; // ISTEK in phone keypad: I=4, S=7, T=8, E=3, K=5

#[derive(OpenApi)]
#[openapi(
    paths(
        // Health
        health::health,
        // Workspaces
        workspaces::list_workspaces,
        workspaces::create_workspace,
        workspaces::get_workspace,
        workspaces::update_workspace,
        workspaces::delete_workspace,
        workspaces::activate_workspace,
        // Collections
        collections::list_collections,
        collections::create_collection,
        collections::get_collection,
        collections::update_collection,
        collections::delete_collection,
        collections::add_request,
        collections::update_request,
        collections::delete_request,
        // Environments
        environments::list_environments,
        environments::create_environment,
        environments::get_environment,
        environments::update_environment,
        environments::delete_environment,
        environments::activate_environment,
        environments::deactivate_environment,
        // Variables
        variables::list_variables,
        variables::create_variable,
        variables::update_variable,
        variables::delete_variable,
        // Integrations
        integrations::list_integrations,
        integrations::create_integration,
        integrations::get_integration,
        integrations::update_integration,
        integrations::delete_integration,
        integrations::toggle_integration,
        integrations::test_integration,
        integrations::fetch_secret,
        // History
        history::list_history,
        history::get_history_item,
        history::delete_history_item,
        history::clear_history,
        // Tests
        tests::run_tests,
        tests::run_collection_tests,
    ),
    components(
        schemas(
            // Common
            ApiError,
            ErrorDetail,
            SuccessResponse,
            // Health
            HealthResponse,
            // Workspaces
            WorkspaceResponse,
            CreateWorkspaceRequest,
            UpdateWorkspaceRequest,
            // Collections
            CollectionResponse,
            CreateCollectionRequest,
            UpdateCollectionRequest,
            AddRequestToCollectionRequest,
            UpdateRequestRequest,
            // Environments
            EnvironmentResponse,
            CreateEnvironmentRequest,
            UpdateEnvironmentRequest,
            EnvVariableResponse,
            // Variables
            VariableResponse,
            CreateVariableRequest,
            UpdateVariableRequest,
            // Integrations
            IntegrationResponse,
            CreateIntegrationRequest,
            UpdateIntegrationRequest,
            TestIntegrationRequest,
            TestIntegrationResponse,
            FetchSecretRequest,
            FetchSecretResponse,
            // History
            HistoryItemResponse,
            // Tests
            RunTestsRequest,
            RunCollectionTestsRequest,
            TestRunSummary,
            TestResult,
            TestRequest,
            Assertion,
            AssertionResult,
            TestKeyValue,
            TestStatus,
            AssertionType,
            JsonPathOperator,
            VariableExtraction,
            ExtractedVariable,
        )
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Workspaces", description = "Workspace management"),
        (name = "Collections", description = "Collection and request management"),
        (name = "Environments", description = "Environment management"),
        (name = "Variables", description = "Global variable management"),
        (name = "Integrations", description = "Secret provider integrations"),
        (name = "History", description = "Request history"),
        (name = "Tests", description = "Test runner endpoints")
    ),
    info(
        title = "Istek API",
        version = "1.0.0",
        description = "Internal REST API for Istek API Client"
    )
)]
struct ApiDoc;

pub fn create_router(storage: Arc<Storage>) -> Router {
    // CORS configuration - allow all origins for local development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    Router::new()
        // Health
        .route("/api/health", get(health::health))
        // Workspaces
        .route("/api/workspaces", get(workspaces::list_workspaces).post(workspaces::create_workspace))
        .route("/api/workspaces/:workspace_id", get(workspaces::get_workspace).put(workspaces::update_workspace).delete(workspaces::delete_workspace))
        .route("/api/workspaces/:workspace_id/activate", put(workspaces::activate_workspace))
        // Collections
        .route("/api/workspaces/:workspace_id/collections", get(collections::list_collections).post(collections::create_collection))
        .route("/api/workspaces/:workspace_id/collections/:collection_id", get(collections::get_collection).put(collections::update_collection).delete(collections::delete_collection))
        .route("/api/workspaces/:workspace_id/collections/:collection_id/requests", post(collections::add_request))
        .route("/api/workspaces/:workspace_id/collections/:collection_id/requests/:request_id", put(collections::update_request).delete(collections::delete_request))
        // Environments
        .route("/api/workspaces/:workspace_id/environments", get(environments::list_environments).post(environments::create_environment))
        .route("/api/workspaces/:workspace_id/environments/active", delete(environments::deactivate_environment))
        .route("/api/workspaces/:workspace_id/environments/:environment_id", get(environments::get_environment).put(environments::update_environment).delete(environments::delete_environment))
        .route("/api/workspaces/:workspace_id/environments/:environment_id/activate", put(environments::activate_environment))
        // Variables
        .route("/api/workspaces/:workspace_id/variables", get(variables::list_variables).post(variables::create_variable))
        .route("/api/workspaces/:workspace_id/variables/:variable_id", put(variables::update_variable).delete(variables::delete_variable))
        // Integrations
        .route("/api/workspaces/:workspace_id/integrations", get(integrations::list_integrations).post(integrations::create_integration))
        .route("/api/workspaces/:workspace_id/integrations/:integration_id", get(integrations::get_integration).put(integrations::update_integration).delete(integrations::delete_integration))
        .route("/api/workspaces/:workspace_id/integrations/:integration_id/toggle", put(integrations::toggle_integration))
        .route("/api/workspaces/:workspace_id/integrations/:integration_id/test", post(integrations::test_integration))
        .route("/api/workspaces/:workspace_id/integrations/:integration_id/fetch", post(integrations::fetch_secret))
        // History
        .route("/api/workspaces/:workspace_id/history", get(history::list_history).delete(history::clear_history))
        .route("/api/workspaces/:workspace_id/history/:history_id", get(history::get_history_item).delete(history::delete_history_item))
        // Tests
        .route("/api/workspaces/:workspace_id/tests/run", post(tests::run_tests))
        .route("/api/workspaces/:workspace_id/collections/:collection_id/tests/run", post(tests::run_collection_tests))
        .route("/api/workspaces/:workspace_id/collections/:collection_id/tests/stream", post(tests::run_collection_tests_stream))
        // Swagger UI (also serves /api/openapi.json)
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", ApiDoc::openapi()))
        // State and middleware
        .with_state(storage)
        .layer(cors)
}

pub async fn start_server(storage: Arc<Storage>) {
    let router = create_router(storage);
    
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], API_PORT));
    
    // Start server
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            eprintln!("Istek API server started on http://localhost:{}", API_PORT);
            l
        },
        Err(e) => {
            eprintln!("Failed to start API server on port {}: {}", API_PORT, e);
            return;
        }
    };
    
    // Run the server
    if let Err(e) = axum::serve(listener, router).await {
        eprintln!("API server error: {}", e);
    }
}
