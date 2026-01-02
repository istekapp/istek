use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest;
use crate::fake_data;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedRequest {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueItem>,
    pub params: Vec<KeyValueItem>,
    pub body: String,
    pub body_type: String,
    /// Response schema from OpenAPI/Swagger for mock generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyValueItem {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFolder {
    pub id: String,
    pub name: String,
    pub requests: Vec<ImportedRequest>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedCollection {
    pub id: String,
    pub name: String,
    pub requests: Vec<ImportedRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<ImportedFolder>>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success: bool,
    pub collection: Option<ImportedCollection>,
    pub error: Option<String>,
    pub request_count: usize,
}

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn create_key_value(key: &str, value: &str) -> KeyValueItem {
    KeyValueItem {
        id: generate_id(),
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
        required: None,
        description: None,
    }
}

fn create_key_value_with_meta(key: &str, value: &str, required: bool, description: Option<&str>) -> KeyValueItem {
    KeyValueItem {
        id: generate_id(),
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
        required: if required { Some(true) } else { None },
        description: description.map(|s| s.to_string()),
    }
}

// Parse OpenAPI 3.0 format
fn parse_openapi3(spec: &Value, base_name: &str) -> Result<ImportedCollection, String> {
    use std::collections::HashMap;
    
    // Map to group requests by tag
    let mut tag_requests: HashMap<String, Vec<ImportedRequest>> = HashMap::new();
    let mut untagged_requests: Vec<ImportedRequest> = Vec::new();
    
    // Get base URL from servers
    let base_url = spec.get("servers")
        .and_then(|s| s.as_array())
        .and_then(|arr| arr.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("{{BASE_URL}}");
    
    // Parse paths
    let paths = spec.get("paths")
        .and_then(|p| p.as_object())
        .ok_or("No paths found in OpenAPI spec")?;
    
    for (path, methods) in paths {
        let methods_obj = methods.as_object()
            .ok_or(format!("Invalid path object for {}", path))?;
        
        for (method, operation) in methods_obj {
            // Skip non-HTTP methods (like parameters, summary, etc.)
            let http_methods = ["get", "post", "put", "patch", "delete", "head", "options"];
            if !http_methods.contains(&method.as_str()) {
                continue;
            }
            
            let op = operation.as_object();
            
            // Get tags for this operation
            let tags: Vec<String> = op
                .and_then(|o| o.get("tags"))
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            
            // Get operation name
            let name = op
                .and_then(|o| o.get("summary"))
                .and_then(|s| s.as_str())
                .or_else(|| op.and_then(|o| o.get("operationId")).and_then(|s| s.as_str()))
                .unwrap_or(&format!("{} {}", method.to_uppercase(), path))
                .to_string();
            
            // Parse parameters
            let mut headers = Vec::new();
            let mut params = Vec::new();
            
            if let Some(parameters) = op.and_then(|o| o.get("parameters")).and_then(|p| p.as_array()) {
                for param in parameters {
                    let param_name = param.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let param_in = param.get("in").and_then(|i| i.as_str()).unwrap_or("");
                    let required = param.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                    let description = param.get("description").and_then(|d| d.as_str());
                    
                    // Get example or default value - check multiple sources
                    let value = param.get("example")
                        .or_else(|| param.get("default"))
                        .or_else(|| param.get("schema").and_then(|s| s.get("example")))
                        .or_else(|| param.get("schema").and_then(|s| s.get("default")))
                        // Try to get first enum value from schema
                        .or_else(|| {
                            param.get("schema")
                                .and_then(|s| s.get("enum"))
                                .and_then(|e| e.as_array())
                                .and_then(|arr| arr.first())
                        })
                        // Try to get first enum value from items (for array type)
                        .or_else(|| {
                            param.get("schema")
                                .and_then(|s| s.get("items"))
                                .and_then(|i| i.get("enum"))
                                .and_then(|e| e.as_array())
                                .and_then(|arr| arr.first())
                        })
                        // Try items.default for array type
                        .or_else(|| {
                            param.get("schema")
                                .and_then(|s| s.get("items"))
                                .and_then(|i| i.get("default"))
                        })
                        // Try enum directly on param (Swagger 2.0 style)
                        .or_else(|| {
                            param.get("enum")
                                .and_then(|e| e.as_array())
                                .and_then(|arr| arr.first())
                        })
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => v.to_string().trim_matches('"').to_string(),
                        })
                        .unwrap_or_else(|| if required { format!("{{{{{}}}}}",  param_name) } else { String::new() });
                    
                    match param_in {
                        "header" => headers.push(create_key_value_with_meta(param_name, &value, required, description)),
                        "query" => params.push(create_key_value_with_meta(param_name, &value, required, description)),
                        _ => {}
                    }
                }
            }
            
            // Parse request body
            let mut body = String::new();
            let mut body_type = "none".to_string();
            
            if let Some(request_body) = op.and_then(|o| o.get("requestBody")) {
                if let Some(content) = request_body.get("content").and_then(|c| c.as_object()) {
                    // Prefer JSON
                    if let Some(json_content) = content.get("application/json") {
                        body_type = "json".to_string();
                        
                        // Try to get example
                        if let Some(example) = json_content.get("example") {
                            body = serde_json::to_string_pretty(example).unwrap_or_default();
                        } else if let Some(schema) = json_content.get("schema") {
                            // Generate example from schema
                            body = generate_example_from_schema(schema);
                        }
                        
                        // Add Content-Type header if not present
                        if !headers.iter().any(|h| h.key.to_lowercase() == "content-type") {
                            headers.insert(0, create_key_value("Content-Type", "application/json"));
                        }
                    } else if content.contains_key("application/xml") {
                        body_type = "xml".to_string();
                        if !headers.iter().any(|h| h.key.to_lowercase() == "content-type") {
                            headers.insert(0, create_key_value("Content-Type", "application/xml"));
                        }
                    } else if content.contains_key("text/html") {
                        body_type = "html".to_string();
                    }
                }
            }
            
            // Add empty key-value for user to add more
            headers.push(create_key_value("", ""));
            params.push(create_key_value("", ""));
            
            // Build full URL
            let url = if base_url.ends_with('/') && path.starts_with('/') {
                format!("{}{}", &base_url[..base_url.len()-1], path)
            } else if !base_url.ends_with('/') && !path.starts_with('/') {
                format!("{}/{}", base_url, path)
            } else {
                format!("{}{}", base_url, path)
            };
            
            // Extract response schema for mock generation
            let response_schema = operation.as_object()
                .and_then(|_| extract_response_schema_openapi3(operation, spec));
            
            let request = ImportedRequest {
                id: generate_id(),
                name,
                protocol: "http".to_string(),
                method: method.to_uppercase(),
                url,
                headers,
                params,
                body,
                body_type,
                response_schema,
            };
            
            // Group by first tag, or put in untagged
            if let Some(first_tag) = tags.first() {
                tag_requests.entry(first_tag.clone()).or_insert_with(Vec::new).push(request);
            } else {
                untagged_requests.push(request);
            }
        }
    }
    
    // Get collection name from spec
    let collection_name = spec.get("info")
        .and_then(|i| i.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or(base_name);
    
    // Create folders from tags
    let mut folders: Vec<ImportedFolder> = tag_requests
        .into_iter()
        .map(|(tag, requests)| ImportedFolder {
            id: generate_id(),
            name: capitalize_first(&tag),
            requests,
        })
        .collect();
    
    // Sort folders by name
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(ImportedCollection {
        id: generate_id(),
        name: collection_name.to_string(),
        requests: untagged_requests,
        folders: if folders.is_empty() { None } else { Some(folders) },
        created_at: chrono_timestamp(),
    })
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Resolve a $ref reference in OpenAPI spec
/// e.g., "#/components/schemas/User" -> actual schema object
fn resolve_ref<'a>(spec: &'a Value, ref_path: &str) -> Option<&'a Value> {
    if !ref_path.starts_with("#/") {
        return None;
    }
    
    let path_parts: Vec<&str> = ref_path[2..].split('/').collect();
    let mut current = spec;
    
    for part in path_parts {
        current = current.get(part)?;
    }
    
    Some(current)
}

/// Recursively resolve all $ref in a schema, returning a fully resolved schema
fn resolve_schema_refs(schema: &Value, spec: &Value, depth: usize) -> Value {
    // Prevent infinite recursion
    if depth > 10 {
        return schema.clone();
    }
    
    // If it's a $ref, resolve it
    if let Some(ref_path) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(resolved) = resolve_ref(spec, ref_path) {
            return resolve_schema_refs(resolved, spec, depth + 1);
        }
        return schema.clone();
    }
    
    // Handle allOf, oneOf, anyOf by merging/taking first
    if let Some(all_of) = schema.get("allOf").and_then(|a| a.as_array()) {
        let mut merged = serde_json::Map::new();
        merged.insert("type".to_string(), Value::String("object".to_string()));
        let mut properties = serde_json::Map::new();
        
        for sub_schema in all_of {
            let resolved = resolve_schema_refs(sub_schema, spec, depth + 1);
            if let Some(props) = resolved.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in props {
                    properties.insert(k.clone(), v.clone());
                }
            }
        }
        
        if !properties.is_empty() {
            merged.insert("properties".to_string(), Value::Object(properties));
        }
        return Value::Object(merged);
    }
    
    if let Some(one_of) = schema.get("oneOf").and_then(|a| a.as_array()) {
        if let Some(first) = one_of.first() {
            return resolve_schema_refs(first, spec, depth + 1);
        }
    }
    
    if let Some(any_of) = schema.get("anyOf").and_then(|a| a.as_array()) {
        if let Some(first) = any_of.first() {
            return resolve_schema_refs(first, spec, depth + 1);
        }
    }
    
    // Recursively resolve properties for objects
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        let mut new_schema = schema.clone();
        let mut new_props = serde_json::Map::new();
        
        for (key, prop_schema) in properties {
            new_props.insert(key.clone(), resolve_schema_refs(prop_schema, spec, depth + 1));
        }
        
        if let Some(obj) = new_schema.as_object_mut() {
            obj.insert("properties".to_string(), Value::Object(new_props));
        }
        return new_schema;
    }
    
    // Resolve items for arrays
    if let Some(items) = schema.get("items") {
        let mut new_schema = schema.clone();
        let resolved_items = resolve_schema_refs(items, spec, depth + 1);
        
        if let Some(obj) = new_schema.as_object_mut() {
            obj.insert("items".to_string(), resolved_items);
        }
        return new_schema;
    }
    
    schema.clone()
}

/// Extract response schema from OpenAPI 3.0 operation
fn extract_response_schema_openapi3(operation: &Value, spec: &Value) -> Option<Value> {
    let responses = operation.get("responses")?;
    
    // Try to get 200, 201, or 2xx response in order of preference
    let response = responses.get("200")
        .or_else(|| responses.get("201"))
        .or_else(|| responses.get("2XX"))
        .or_else(|| responses.get("default"))?;
    
    // Get content -> application/json -> schema
    let schema = response.get("content")
        .and_then(|c| c.get("application/json"))
        .and_then(|j| j.get("schema"))?;
    
    // Resolve all $ref in the schema
    Some(resolve_schema_refs(schema, spec, 0))
}

/// Extract response schema from Swagger 2.0 operation
fn extract_response_schema_swagger2(operation: &Value, spec: &Value) -> Option<Value> {
    let responses = operation.get("responses")?;
    
    // Try to get 200, 201, or default response
    let response = responses.get("200")
        .or_else(|| responses.get("201"))
        .or_else(|| responses.get("default"))?;
    
    // In Swagger 2.0, schema is directly under the response
    let schema = response.get("schema")?;
    
    // Resolve all $ref (Swagger 2.0 uses #/definitions/...)
    Some(resolve_schema_refs(schema, spec, 0))
}

// Parse Swagger 2.0 format
fn parse_swagger2(spec: &Value, base_name: &str) -> Result<ImportedCollection, String> {
    use std::collections::HashMap;
    
    // Map to group requests by tag
    let mut tag_requests: HashMap<String, Vec<ImportedRequest>> = HashMap::new();
    let mut untagged_requests: Vec<ImportedRequest> = Vec::new();
    
    // Get base URL
    let host = spec.get("host").and_then(|h| h.as_str()).unwrap_or("{{HOST}}");
    let base_path = spec.get("basePath").and_then(|b| b.as_str()).unwrap_or("");
    let schemes = spec.get("schemes")
        .and_then(|s| s.as_array())
        .and_then(|arr| arr.first())
        .and_then(|s| s.as_str())
        .unwrap_or("https");
    
    let base_url = format!("{}://{}{}", schemes, host, base_path);
    
    // Parse paths
    let paths = spec.get("paths")
        .and_then(|p| p.as_object())
        .ok_or("No paths found in Swagger spec")?;
    
    for (path, methods) in paths {
        let methods_obj = methods.as_object()
            .ok_or(format!("Invalid path object for {}", path))?;
        
        for (method, operation) in methods_obj {
            let http_methods = ["get", "post", "put", "patch", "delete", "head", "options"];
            if !http_methods.contains(&method.as_str()) {
                continue;
            }
            
            let op = operation.as_object();
            
            // Get tags for this operation
            let tags: Vec<String> = op
                .and_then(|o| o.get("tags"))
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            
            let name = op
                .and_then(|o| o.get("summary"))
                .and_then(|s| s.as_str())
                .or_else(|| op.and_then(|o| o.get("operationId")).and_then(|s| s.as_str()))
                .unwrap_or(&format!("{} {}", method.to_uppercase(), path))
                .to_string();
            
            let mut headers = Vec::new();
            let mut params = Vec::new();
            let mut body = String::new();
            let mut body_type = "none".to_string();
            
            if let Some(parameters) = op.and_then(|o| o.get("parameters")).and_then(|p| p.as_array()) {
                for param in parameters {
                    let param_name = param.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let param_in = param.get("in").and_then(|i| i.as_str()).unwrap_or("");
                    let required = param.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                    let description = param.get("description").and_then(|d| d.as_str());
                    
                    // Get value from default, example, or enum (Swagger 2.0 has enum directly on param)
                    let value = param.get("default")
                        .or_else(|| param.get("example"))
                        // Try enum directly on param (Swagger 2.0 style)
                        .or_else(|| {
                            param.get("enum")
                                .and_then(|e| e.as_array())
                                .and_then(|arr| arr.first())
                        })
                        // Try items.enum for array types
                        .or_else(|| {
                            param.get("items")
                                .and_then(|i| i.get("enum"))
                                .and_then(|e| e.as_array())
                                .and_then(|arr| arr.first())
                        })
                        // Try items.default for array types
                        .or_else(|| {
                            param.get("items")
                                .and_then(|i| i.get("default"))
                        })
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => v.to_string().trim_matches('"').to_string(),
                        })
                        .unwrap_or_else(|| if required { format!("{{{{{}}}}}", param_name) } else { String::new() });
                    
                    match param_in {
                        "header" => headers.push(create_key_value_with_meta(param_name, &value, required, description)),
                        "query" => params.push(create_key_value_with_meta(param_name, &value, required, description)),
                        "body" => {
                            body_type = "json".to_string();
                            if let Some(schema) = param.get("schema") {
                                body = generate_example_from_schema(schema);
                            }
                            if !headers.iter().any(|h| h.key.to_lowercase() == "content-type") {
                                headers.insert(0, create_key_value("Content-Type", "application/json"));
                            }
                        }
                        _ => {}
                    }
                }
            }
            
            // Check consumes for content type
            if let Some(consumes) = op.and_then(|o| o.get("consumes")).and_then(|c| c.as_array()) {
                if consumes.iter().any(|c| c.as_str() == Some("application/json")) {
                    if !headers.iter().any(|h| h.key.to_lowercase() == "content-type") && body_type == "json" {
                        headers.insert(0, create_key_value("Content-Type", "application/json"));
                    }
                }
            }
            
            headers.push(create_key_value("", ""));
            params.push(create_key_value("", ""));
            
            let url = format!("{}{}", base_url, path);
            
            // Extract response schema for mock generation
            let response_schema = operation.as_object()
                .and_then(|_| extract_response_schema_swagger2(operation, spec));
            
            let request = ImportedRequest {
                id: generate_id(),
                name,
                protocol: "http".to_string(),
                method: method.to_uppercase(),
                url,
                headers,
                params,
                body,
                body_type,
                response_schema,
            };
            
            // Group by first tag, or put in untagged
            if let Some(first_tag) = tags.first() {
                tag_requests.entry(first_tag.clone()).or_insert_with(Vec::new).push(request);
            } else {
                untagged_requests.push(request);
            }
        }
    }
    
    let collection_name = spec.get("info")
        .and_then(|i| i.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or(base_name);
    
    // Create folders from tags
    let mut folders: Vec<ImportedFolder> = tag_requests
        .into_iter()
        .map(|(tag, requests)| ImportedFolder {
            id: generate_id(),
            name: capitalize_first(&tag),
            requests,
        })
        .collect();
    
    // Sort folders by name
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(ImportedCollection {
        id: generate_id(),
        name: collection_name.to_string(),
        requests: untagged_requests,
        folders: if folders.is_empty() { None } else { Some(folders) },
        created_at: chrono_timestamp(),
    })
}

// Generate example JSON from OpenAPI schema
fn generate_example_from_schema(schema: &Value) -> String {
    let example = schema_to_example(schema);
    serde_json::to_string_pretty(&example).unwrap_or_default()
}

fn schema_to_example(schema: &Value) -> Value {
    // If there's an explicit example, use it
    if let Some(example) = schema.get("example") {
        return example.clone();
    }
    
    // Handle $ref (simplified - just return placeholder)
    if schema.get("$ref").is_some() {
        return Value::Object(serde_json::Map::new());
    }
    
    let schema_type = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");
    
    match schema_type {
        "object" => {
            let mut obj = serde_json::Map::new();
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for (key, prop_schema) in properties {
                    obj.insert(key.clone(), schema_to_example(prop_schema));
                }
            }
            Value::Object(obj)
        }
        "array" => {
            if let Some(items) = schema.get("items") {
                Value::Array(vec![schema_to_example(items)])
            } else {
                Value::Array(vec![])
            }
        }
        "string" => {
            if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                enum_values.first().cloned().unwrap_or(Value::String("string".to_string()))
            } else if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
                match format {
                    "date" => Value::String("2024-01-01".to_string()),
                    "date-time" => Value::String("2024-01-01T00:00:00Z".to_string()),
                    "email" => Value::String("user@example.com".to_string()),
                    "uri" | "url" => Value::String("https://example.com".to_string()),
                    "uuid" => Value::String("550e8400-e29b-41d4-a716-446655440000".to_string()),
                    _ => Value::String("string".to_string()),
                }
            } else {
                Value::String("string".to_string())
            }
        }
        "integer" | "number" => {
            if let Some(default) = schema.get("default") {
                default.clone()
            } else if schema_type == "integer" {
                Value::Number(serde_json::Number::from(0))
            } else {
                Value::Number(serde_json::Number::from_f64(0.0).unwrap())
            }
        }
        "boolean" => {
            schema.get("default").cloned().unwrap_or(Value::Bool(false))
        }
        _ => Value::Null
    }
}

fn chrono_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tauri::command]
pub async fn import_openapi(content: String, file_name: String) -> Result<ImportResult, String> {
    // Parse JSON
    let spec: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    // Detect version
    let is_openapi3 = spec.get("openapi").is_some();
    let is_swagger2 = spec.get("swagger").is_some();
    
    let base_name = file_name.trim_end_matches(".json").trim_end_matches(".yaml").to_string();
    
    let result = if is_openapi3 {
        parse_openapi3(&spec, &base_name)
    } else if is_swagger2 {
        parse_swagger2(&spec, &base_name)
    } else {
        Err("Unknown API specification format. Expected OpenAPI 3.0 or Swagger 2.0".to_string())
    };
    
    match result {
        Ok(collection) => {
            let count = collection.requests.len();
            Ok(ImportResult {
                success: true,
                collection: Some(collection),
                error: None,
                request_count: count,
            })
        }
        Err(e) => {
            Ok(ImportResult {
                success: false,
                collection: None,
                error: Some(e),
                request_count: 0,
            })
        }
    }
}

#[tauri::command]
pub async fn import_postman(content: String, _file_name: String) -> Result<ImportResult, String> {
    let spec: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    // Check if it's a Postman collection
    let info = spec.get("info").ok_or("Not a valid Postman collection")?;
    let schema = info.get("schema").and_then(|s| s.as_str()).unwrap_or("");
    
    if !schema.contains("postman") {
        return Err("Not a valid Postman collection".to_string());
    }
    
    let collection_name = info.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("Imported Collection");
    
    let mut requests = Vec::new();
    
    // Parse items recursively
    fn parse_items(items: &Value, requests: &mut Vec<ImportedRequest>, folder_prefix: &str) {
        if let Some(arr) = items.as_array() {
            for item in arr {
                // Check if it's a folder (has items) or a request
                if let Some(sub_items) = item.get("item") {
                    let folder_name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let new_prefix = if folder_prefix.is_empty() {
                        folder_name.to_string()
                    } else {
                        format!("{}/{}", folder_prefix, folder_name)
                    };
                    parse_items(sub_items, requests, &new_prefix);
                } else if let Some(request) = item.get("request") {
                    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("Request");
                    let full_name = if folder_prefix.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}/{}", folder_prefix, name)
                    };
                    
                    // Parse request
                    let method = request.get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("GET")
                        .to_uppercase();
                    
                    // Parse URL
                    let url = if let Some(url_obj) = request.get("url") {
                        if let Some(raw) = url_obj.get("raw").and_then(|r| r.as_str()) {
                            raw.to_string()
                        } else if let Some(url_str) = url_obj.as_str() {
                            url_str.to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    
                    // Parse headers
                    let mut headers: Vec<KeyValueItem> = request.get("header")
                        .and_then(|h| h.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|h| {
                                let key = h.get("key").and_then(|k| k.as_str())?;
                                let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                let disabled = h.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                                Some(KeyValueItem {
                                    id: generate_id(),
                                    key: key.to_string(),
                                    value: value.to_string(),
                                    enabled: !disabled,
                                    required: None,
                                    description: None,
                                })
                            }).collect()
                        })
                        .unwrap_or_default();
                    headers.push(create_key_value("", ""));
                    
                    // Parse query params
                    let mut params: Vec<KeyValueItem> = request.get("url")
                        .and_then(|u| u.get("query"))
                        .and_then(|q| q.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|p| {
                                let key = p.get("key").and_then(|k| k.as_str())?;
                                let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                let disabled = p.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                                let desc = p.get("description").and_then(|d| d.as_str());
                                Some(KeyValueItem {
                                    id: generate_id(),
                                    key: key.to_string(),
                                    value: value.to_string(),
                                    enabled: !disabled,
                                    required: None,
                                    description: desc.map(|s| s.to_string()),
                                })
                            }).collect()
                        })
                        .unwrap_or_default();
                    params.push(create_key_value("", ""));
                    
                    // Parse body
                    let (body, body_type) = if let Some(body_obj) = request.get("body") {
                        let mode = body_obj.get("mode").and_then(|m| m.as_str()).unwrap_or("");
                        match mode {
                            "raw" => {
                                let raw = body_obj.get("raw").and_then(|r| r.as_str()).unwrap_or("");
                                let lang = body_obj.get("options")
                                    .and_then(|o| o.get("raw"))
                                    .and_then(|r| r.get("language"))
                                    .and_then(|l| l.as_str())
                                    .unwrap_or("json");
                                let bt = match lang {
                                    "json" => "json",
                                    "xml" => "xml",
                                    "html" => "html",
                                    _ => "raw",
                                };
                                (raw.to_string(), bt.to_string())
                            }
                            _ => (String::new(), "none".to_string()),
                        }
                    } else {
                        (String::new(), "none".to_string())
                    };
                    
                    requests.push(ImportedRequest {
                        id: generate_id(),
                        name: full_name,
                        protocol: "http".to_string(),
                        method,
                        url,
                        headers,
                        params,
                        body,
                        body_type,
                        response_schema: None,
                    });
                }
            }
        }
    }
    
    if let Some(items) = spec.get("item") {
        parse_items(items, &mut requests, "");
    }
    
    let count = requests.len();
    
    Ok(ImportResult {
        success: true,
        collection: Some(ImportedCollection {
            id: generate_id(),
            name: collection_name.to_string(),
            requests,
            folders: None,
            created_at: chrono_timestamp(),
        }),
        error: None,
        request_count: count,
    })
}

#[tauri::command]
pub async fn import_from_url(url: String) -> Result<ImportResult, String> {
    // Fetch the spec from URL
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }
    
    let content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON
    let spec: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    // Detect format
    let is_openapi3 = spec.get("openapi").is_some();
    let is_swagger2 = spec.get("swagger").is_some();
    let is_postman = spec.get("info")
        .and_then(|i| i.get("schema"))
        .and_then(|s| s.as_str())
        .map(|s| s.contains("postman"))
        .unwrap_or(false);
    
    // Extract name from URL for fallback
    let url_name = url
        .split('/')
        .last()
        .unwrap_or("imported")
        .trim_end_matches(".json")
        .trim_end_matches(".yaml")
        .trim_end_matches(".yml")
        .to_string();
    
    if is_postman {
        // Use import_postman logic
        let info = spec.get("info").ok_or("Not a valid Postman collection")?;
        let collection_name = info.get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(&url_name);
        
        let mut requests = Vec::new();
        
        fn parse_postman_items(items: &Value, requests: &mut Vec<ImportedRequest>, folder_prefix: &str) {
            if let Some(arr) = items.as_array() {
                for item in arr {
                    if let Some(sub_items) = item.get("item") {
                        let folder_name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let new_prefix = if folder_prefix.is_empty() {
                            folder_name.to_string()
                        } else {
                            format!("{}/{}", folder_prefix, folder_name)
                        };
                        parse_postman_items(sub_items, requests, &new_prefix);
                    } else if let Some(request) = item.get("request") {
                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("Request");
                        let full_name = if folder_prefix.is_empty() {
                            name.to_string()
                        } else {
                            format!("{}/{}", folder_prefix, name)
                        };
                        
                        let method = request.get("method")
                            .and_then(|m| m.as_str())
                            .unwrap_or("GET")
                            .to_uppercase();
                        
                        let url = if let Some(url_obj) = request.get("url") {
                            if let Some(raw) = url_obj.get("raw").and_then(|r| r.as_str()) {
                                raw.to_string()
                            } else if let Some(url_str) = url_obj.as_str() {
                                url_str.to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        
                        let mut headers: Vec<KeyValueItem> = request.get("header")
                            .and_then(|h| h.as_array())
                            .map(|arr| {
                                arr.iter().filter_map(|h| {
                                    let key = h.get("key").and_then(|k| k.as_str())?;
                                    let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                    let disabled = h.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                                    Some(KeyValueItem {
                                        id: generate_id(),
                                        key: key.to_string(),
                                        value: value.to_string(),
                                        enabled: !disabled,
                                        required: None,
                                        description: None,
                                    })
                                }).collect()
                            })
                            .unwrap_or_default();
                        headers.push(create_key_value("", ""));
                        
                        let mut params: Vec<KeyValueItem> = request.get("url")
                            .and_then(|u| u.get("query"))
                            .and_then(|q| q.as_array())
                            .map(|arr| {
                                arr.iter().filter_map(|p| {
                                    let key = p.get("key").and_then(|k| k.as_str())?;
                                    let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                    let disabled = p.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false);
                                    let desc = p.get("description").and_then(|d| d.as_str());
                                    Some(KeyValueItem {
                                        id: generate_id(),
                                        key: key.to_string(),
                                        value: value.to_string(),
                                        enabled: !disabled,
                                        required: None,
                                        description: desc.map(|s| s.to_string()),
                                    })
                                }).collect()
                            })
                            .unwrap_or_default();
                        params.push(create_key_value("", ""));
                        
                        let (body, body_type) = if let Some(body_obj) = request.get("body") {
                            let mode = body_obj.get("mode").and_then(|m| m.as_str()).unwrap_or("");
                            match mode {
                                "raw" => {
                                    let raw = body_obj.get("raw").and_then(|r| r.as_str()).unwrap_or("");
                                    let lang = body_obj.get("options")
                                        .and_then(|o| o.get("raw"))
                                        .and_then(|r| r.get("language"))
                                        .and_then(|l| l.as_str())
                                        .unwrap_or("json");
                                    let bt = match lang {
                                        "json" => "json",
                                        "xml" => "xml",
                                        "html" => "html",
                                        _ => "raw",
                                    };
                                    (raw.to_string(), bt.to_string())
                                }
                                _ => (String::new(), "none".to_string()),
                            }
                        } else {
                            (String::new(), "none".to_string())
                        };
                        
                        requests.push(ImportedRequest {
                            id: generate_id(),
                            name: full_name,
                            protocol: "http".to_string(),
                            method,
                            url,
                            headers,
                            params,
                            body,
                            body_type,
                            response_schema: None,
                        });
                    }
                }
            }
        }
        
        if let Some(items) = spec.get("item") {
            parse_postman_items(items, &mut requests, "");
        }
        
        let count = requests.len();
        return Ok(ImportResult {
            success: true,
            collection: Some(ImportedCollection {
                id: generate_id(),
                name: collection_name.to_string(),
                requests,
                folders: None,
                created_at: chrono_timestamp(),
            }),
            error: None,
            request_count: count,
        });
    }
    
    let result = if is_openapi3 {
        parse_openapi3(&spec, &url_name)
    } else if is_swagger2 {
        parse_swagger2(&spec, &url_name)
    } else {
        Err("Unknown API specification format. Expected OpenAPI 3.0, Swagger 2.0, or Postman Collection".to_string())
    };
    
    match result {
        Ok(collection) => {
            let count = collection.requests.len();
            Ok(ImportResult {
                success: true,
                collection: Some(collection),
                error: None,
                request_count: count,
            })
        }
        Err(e) => {
            Ok(ImportResult {
                success: false,
                collection: None,
                error: Some(e),
                request_count: 0,
            })
        }
    }
}

/// Generate fake mock response data from a JSON schema
/// This uses realistic fake data based on field types and names (similar to Podam in Java)
#[tauri::command]
pub fn generate_mock_response(schema: Value) -> Result<String, String> {
    Ok(fake_data::generate_fake_json(&schema))
}

/// Generate mock response with property name hints for smarter generation
#[tauri::command]
pub fn generate_mock_response_smart(schema: Value) -> Result<Value, String> {
    Ok(fake_data::generate_fake_from_schema(&schema))
}
