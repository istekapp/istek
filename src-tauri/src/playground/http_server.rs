use axum::{
    body::Bytes,
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response, sse::{Event, KeepAlive, Sse}},
    routing::{get, post, any},
    Json, Router,
};
use axum::extract::ws::{Message, WebSocket};
use std::collections::HashMap;
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

/// Echo response - returns all request details
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EchoResponse {
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub body_raw: Option<String>,
    pub timestamp: String,
    pub content_length: usize,
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
            },
            "/echo": {
                "get": {
                    "summary": "Echo GET request",
                    "description": "Returns all request details including headers, query params",
                    "operationId": "echoGet",
                    "tags": ["Echo"],
                    "parameters": [
                        {
                            "name": "any",
                            "in": "query",
                            "description": "Any query parameter - all will be echoed back",
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Echo response",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Echo POST request",
                    "description": "Returns all request details including headers, body, query params",
                    "operationId": "echoPost",
                    "tags": ["Echo"],
                    "requestBody": {
                        "description": "Any request body - will be echoed back",
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            },
                            "text/plain": {
                                "schema": { "type": "string" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Echo response",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
                        }
                    }
                },
                "put": {
                    "summary": "Echo PUT request",
                    "operationId": "echoPut",
                    "tags": ["Echo"],
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Echo response",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
                        }
                    }
                },
                "patch": {
                    "summary": "Echo PATCH request",
                    "operationId": "echoPatch",
                    "tags": ["Echo"],
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Echo response",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
                        }
                    }
                },
                "delete": {
                    "summary": "Echo DELETE request",
                    "operationId": "echoDelete",
                    "tags": ["Echo"],
                    "responses": {
                        "200": {
                            "description": "Echo response",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/EchoResponse" }
                                }
                            }
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
                },
                "EchoResponse": {
                    "type": "object",
                    "description": "Echo response containing all request details",
                    "properties": {
                        "method": { 
                            "type": "string",
                            "description": "HTTP method used (GET, POST, PUT, PATCH, DELETE, etc.)"
                        },
                        "path": { 
                            "type": "string",
                            "description": "Request path"
                        },
                        "queryParams": { 
                            "type": "object",
                            "additionalProperties": { "type": "string" },
                            "description": "Query parameters as key-value pairs"
                        },
                        "headers": { 
                            "type": "object",
                            "additionalProperties": { "type": "string" },
                            "description": "Request headers as key-value pairs"
                        },
                        "body": { 
                            "type": "object",
                            "description": "Request body parsed as JSON (if valid JSON)"
                        },
                        "bodyRaw": { 
                            "type": "string",
                            "description": "Raw request body as string"
                        },
                        "timestamp": { 
                            "type": "string",
                            "format": "date-time",
                            "description": "Server timestamp when request was received"
                        },
                        "contentLength": { 
                            "type": "integer",
                            "description": "Content length of request body in bytes"
                        }
                    },
                    "required": ["method", "path", "queryParams", "headers", "timestamp", "contentLength"]
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
        // Echo endpoint - accepts any HTTP method and returns request details
        .route("/echo", any(echo_handler))
        .route("/echo/*path", any(echo_handler))
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

// --- Echo Handler ---

/// Echo endpoint that returns all request details
/// Accepts any HTTP method (GET, POST, PUT, PATCH, DELETE, etc.)
async fn echo_handler(
    method: Method,
    uri: Uri,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<EchoResponse> {
    // Convert headers to HashMap
    let headers_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
        })
        .collect();

    // Try to parse body as JSON, fallback to raw string
    let body_str = String::from_utf8_lossy(&body).to_string();
    let body_json: Option<serde_json::Value> = if !body.is_empty() {
        serde_json::from_slice(&body).ok()
    } else {
        None
    };
    
    let body_raw = if !body.is_empty() && body_json.is_none() {
        Some(body_str)
    } else if !body.is_empty() {
        Some(body_str)
    } else {
        None
    };

    Json(EchoResponse {
        method: method.to_string(),
        path: uri.path().to_string(),
        query_params,
        headers: headers_map,
        body: body_json,
        body_raw,
        timestamp: chrono::Utc::now().to_rfc3339(),
        content_length: body.len(),
    })
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
            "echo": "/echo",
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
