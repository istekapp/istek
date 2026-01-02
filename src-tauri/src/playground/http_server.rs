use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response, sse::{Event, KeepAlive, Sse}},
    routing::{get, post},
    Json, Router,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::convert::Infallible;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

use super::data::{PlaygroundData, Product};
use super::graphql::create_graphql_router;

/// Request body for creating/updating products
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProductRequest {
    pub name: String,
    pub price: f64,
    pub category: String,
    pub in_stock: Option<bool>,
    pub description: Option<String>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: &str) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

/// Generate OpenAPI spec dynamically to avoid raw string issues
fn generate_openapi_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Istek Playground API",
            "description": "A demo REST API for testing Istek features",
            "version": "1.0.0"
        },
        "servers": [
            {
                "url": "http://localhost:19510",
                "description": "Local playground server"
            }
        ],
        "paths": {
            "/api/products": {
                "get": {
                    "summary": "List all products",
                    "operationId": "listProducts",
                    "tags": ["Products"],
                    "responses": {
                        "200": {
                            "description": "Successful response",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "success": { "type": "boolean" },
                                            "data": {
                                                "type": "array",
                                                "items": { "$ref": "#/components/schemas/Product" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Create a new product",
                    "operationId": "createProduct",
                    "tags": ["Products"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateProduct" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Product created",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "success": { "type": "boolean" },
                                            "data": { "$ref": "#/components/schemas/Product" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/products/{id}": {
                "get": {
                    "summary": "Get a product by ID",
                    "operationId": "getProduct",
                    "tags": ["Products"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Successful response",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "success": { "type": "boolean" },
                                            "data": { "$ref": "#/components/schemas/Product" }
                                        }
                                    }
                                }
                            }
                        },
                        "404": {
                            "description": "Product not found"
                        }
                    }
                },
                "put": {
                    "summary": "Update a product",
                    "operationId": "updateProduct",
                    "tags": ["Products"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateProduct" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Product updated"
                        },
                        "404": {
                            "description": "Product not found"
                        }
                    }
                },
                "delete": {
                    "summary": "Delete a product",
                    "operationId": "deleteProduct",
                    "tags": ["Products"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Product deleted"
                        },
                        "404": {
                            "description": "Product not found"
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "Product": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "price": { "type": "number" },
                        "category": { "type": "string" },
                        "inStock": { "type": "boolean" },
                        "description": { "type": "string" }
                    },
                    "required": ["id", "name", "price", "category", "inStock"]
                },
                "CreateProduct": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "price": { "type": "number" },
                        "category": { "type": "string" },
                        "inStock": { "type": "boolean" },
                        "description": { "type": "string" }
                    },
                    "required": ["name", "price", "category"]
                }
            }
        }
    })
}

/// Create the HTTP server router
pub fn create_http_router(data: Arc<PlaygroundData>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create GraphQL router
    let graphql_router = create_graphql_router(data.clone());

    Router::new()
        // REST API endpoints
        .route("/api/products", get(list_products).post(create_product))
        .route("/api/products/{id}", get(get_product).put(update_product).delete(delete_product))
        .route("/api/reset", post(reset_data))
        // OpenAPI spec
        .route("/openapi.json", get(openapi_spec))
        // WebSocket echo
        .route("/ws/echo", get(ws_echo_handler))
        // SSE endpoints
        .route("/sse/events", get(sse_events_handler))
        .route("/sse/counter", get(sse_counter_handler))
        .route("/sse/time", get(sse_time_handler))
        // Health check
        .route("/health", get(health_check))
        // Root info
        .route("/", get(root_info))
        // Merge GraphQL router
        .merge(graphql_router)
        .layer(cors)
        .with_state(data)
}

// --- REST API Handlers ---

async fn list_products(
    State(data): State<Arc<PlaygroundData>>,
) -> Json<ApiResponse<Vec<Product>>> {
    let products = data.products.read().await.clone();
    Json(ApiResponse::success(products))
}

async fn get_product(
    State(data): State<Arc<PlaygroundData>>,
    Path(id): Path<u32>,
) -> Response {
    let products = data.products.read().await;
    match products.iter().find(|p| p.id == id) {
        Some(product) => Json(ApiResponse::success(product.clone())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Product not found")),
        ).into_response(),
    }
}

async fn create_product(
    State(data): State<Arc<PlaygroundData>>,
    Json(req): Json<CreateProductRequest>,
) -> Response {
    let product = Product {
        id: data.next_product_id(),
        name: req.name,
        price: req.price,
        category: req.category,
        in_stock: req.in_stock.unwrap_or(true),
        description: req.description.unwrap_or_default(),
    };

    data.products.write().await.push(product.clone());

    (StatusCode::CREATED, Json(ApiResponse::success(product))).into_response()
}

async fn update_product(
    State(data): State<Arc<PlaygroundData>>,
    Path(id): Path<u32>,
    Json(req): Json<CreateProductRequest>,
) -> Response {
    let mut products = data.products.write().await;
    match products.iter_mut().find(|p| p.id == id) {
        Some(product) => {
            product.name = req.name;
            product.price = req.price;
            product.category = req.category;
            product.in_stock = req.in_stock.unwrap_or(product.in_stock);
            product.description = req.description.unwrap_or_else(|| product.description.clone());
            Json(ApiResponse::success(product.clone())).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Product not found")),
        ).into_response(),
    }
}

async fn delete_product(
    State(data): State<Arc<PlaygroundData>>,
    Path(id): Path<u32>,
) -> Response {
    let mut products = data.products.write().await;
    let initial_len = products.len();
    products.retain(|p| p.id != id);

    if products.len() < initial_len {
        Json(ApiResponse::success(serde_json::json!({"deleted": id}))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Product not found")),
        ).into_response()
    }
}

async fn reset_data(State(data): State<Arc<PlaygroundData>>) -> Json<ApiResponse<&'static str>> {
    data.reset().await;
    Json(ApiResponse::success("Data reset to initial state"))
}

async fn openapi_spec() -> Json<serde_json::Value> {
    Json(generate_openapi_spec())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "istek-playground",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn root_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Istek Playground",
        "version": "1.0.0",
        "endpoints": {
            "rest": "/api/products",
            "graphql": "/graphql",
            "websocket": "/ws/echo",
            "sse": {
                "events": "/sse/events",
                "counter": "/sse/counter",
                "time": "/sse/time"
            },
            "openapi": "/openapi.json",
            "health": "/health"
        }
    }))
}

// --- WebSocket Handler ---

async fn ws_echo_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_ws_echo)
}

async fn handle_ws_echo(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = serde_json::json!({
                    "type": "echo",
                    "original": text.to_string(),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                let response_text = serde_json::to_string(&response).unwrap_or_default();
                if socket.send(Message::Text(response_text.into())).await.is_err() {
                    break;
                }
            }
            Ok(Message::Binary(data)) => {
                if socket.send(Message::Binary(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Ping(data)) => {
                if socket.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

// --- SSE Handlers ---

/// SSE endpoint that sends random events
async fn sse_events_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::unfold(0u64, |state| async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let event_types = ["message", "notification", "update", "alert"];
        let event_type = event_types[state as usize % event_types.len()];
        
        let data = serde_json::json!({
            "id": state + 1,
            "message": format!("Event #{}", state + 1),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": event_type
        });
        
        let event = Event::default()
            .event(event_type)
            .id((state + 1).to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        
        Some((Ok(event), state + 1))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE endpoint that sends a counter every second
async fn sse_counter_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::unfold(0u64, |count| async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        let data = serde_json::json!({
            "count": count + 1,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        let event = Event::default()
            .event("counter")
            .id((count + 1).to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        
        Some((Ok(event), count + 1))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE endpoint that sends current time every 5 seconds
async fn sse_time_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::unfold(0u64, |state| async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let now = chrono::Utc::now();
        let data = serde_json::json!({
            "time": now.to_rfc3339(),
            "unix": now.timestamp(),
            "formatted": now.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        });
        
        let event = Event::default()
            .event("time")
            .id((state + 1).to_string())
            .data(serde_json::to_string(&data).unwrap_or_default());
        
        Some((Ok(event), state + 1))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
