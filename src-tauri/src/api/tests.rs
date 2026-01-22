use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use utoipa::ToSchema;

use crate::storage::Storage;
use crate::api::ApiError;

// ============ Request/Response Types ============

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssertionType {
    Status,
    StatusRange,
    Jsonpath,
    Contains,
    ResponseTime,
    Header,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JsonPathOperator {
    Equals,
    NotEquals,
    Contains,
    Exists,
    NotExists,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Assertion {
    pub id: String,
    #[serde(rename = "type")]
    pub assertion_type: AssertionType,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<JsonPathOperator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VariableExtraction {
    pub id: String,
    pub variable_name: String,
    pub json_path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestRequest {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub params: Vec<KeyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default = "default_body_type")]
    pub body_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<Vec<Assertion>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract_variables: Option<Vec<VariableExtraction>>,
}

fn default_body_type() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunTestsRequest {
    pub name: String,
    pub requests: Vec<TestRequest>,
    #[serde(default)]
    pub stop_on_failure: bool,
    #[serde(default)]
    pub delay_between_requests: u64,
    /// Initial variables to use in tests (e.g., from environment)
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssertionResult {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedVariable {
    pub variable_name: String,
    pub json_path: String,
    pub value: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub request_id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status: TestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub assertions: Vec<AssertionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_variables: Option<Vec<ExtractedVariable>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestRunSummary {
    pub run_id: String,
    pub name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub total_time: u64,
    pub results: Vec<TestResult>,
}

// ============ Helper Functions ============

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Parsed request with test order for sorting
struct ParsedRequest {
    request: TestRequest,
    test_order: Option<i64>,
}

/// Parse a JSON request object into a TestRequest struct
fn parse_request_json(r: &Value) -> Option<ParsedRequest> {
    let test_order = r.get("testOrder").and_then(|v| v.as_i64());
    
    // Test config can be stored either at root level or inside "testConfig" object
    let test_config = r.get("testConfig");
    
    // Helper to get assertions - check testConfig first, then root level
    let assertions_array = test_config
        .and_then(|tc| tc.get("assertions"))
        .or_else(|| r.get("assertions"))
        .and_then(|a| a.as_array());
    
    // Helper to get extractVariables - check testConfig first, then root level
    let extract_vars_array = test_config
        .and_then(|tc| tc.get("extractVariables"))
        .or_else(|| r.get("extractVariables"))
        .and_then(|e| e.as_array());
    
    Some(ParsedRequest {
        test_order,
        request: TestRequest {
            id: r.get("id")?.as_str()?.to_string(),
            name: r.get("name")?.as_str()?.to_string(),
            method: r.get("method")?.as_str()?.to_string(),
            url: r.get("url")?.as_str()?.to_string(),
            headers: r.get("headers")
                .and_then(|h| h.as_array())
                .map(|arr| arr.iter().filter_map(|h| {
                    Some(KeyValue {
                        key: h.get("key")?.as_str()?.to_string(),
                        value: h.get("value")?.as_str()?.to_string(),
                        enabled: h.get("enabled")?.as_bool()?,
                    })
                }).collect())
                .unwrap_or_default(),
            params: r.get("params")
                .and_then(|p| p.as_array())
                .map(|arr| arr.iter().filter_map(|p| {
                    Some(KeyValue {
                        key: p.get("key")?.as_str()?.to_string(),
                        value: p.get("value")?.as_str()?.to_string(),
                        enabled: p.get("enabled")?.as_bool()?,
                    })
                }).collect())
                .unwrap_or_default(),
            body: r.get("body").and_then(|b| b.as_str()).map(|s| s.to_string()),
            body_type: r.get("bodyType").and_then(|b| b.as_str()).unwrap_or("none").to_string(),
            assertions: assertions_array
                .map(|arr| arr.iter().filter_map(|a| {
                    Some(Assertion {
                        id: a.get("id")?.as_str()?.to_string(),
                        assertion_type: serde_json::from_value(a.get("type")?.clone()).ok()?,
                        enabled: a.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        expected_status: a.get("expectedStatus").and_then(|v| v.as_u64()).map(|v| v as u16),
                        min_status: a.get("minStatus").and_then(|v| v.as_u64()).map(|v| v as u16),
                        max_status: a.get("maxStatus").and_then(|v| v.as_u64()).map(|v| v as u16),
                        json_path: a.get("jsonPath").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        operator: a.get("operator").and_then(|v| serde_json::from_value(v.clone()).ok()),
                        expected_value: a.get("expectedValue").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        search_string: a.get("searchString").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        max_time_ms: a.get("maxTimeMs").and_then(|v| v.as_u64()),
                        header_name: a.get("headerName").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        header_value: a.get("headerValue").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    })
                }).collect()),
            extract_variables: extract_vars_array
                .map(|arr| arr.iter().filter_map(|e| {
                    Some(VariableExtraction {
                        id: e.get("id")?.as_str()?.to_string(),
                        variable_name: e.get("variableName")?.as_str()?.to_string(),
                        json_path: e.get("jsonPath")?.as_str()?.to_string(),
                        enabled: e.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    })
                }).collect()),
        },
    })
}

/// Parse requests from JSON including folders
/// If folder_id is provided, only requests from that folder (and its subfolders) are included
/// Requests are sorted by testOrder field (if present)
fn parse_all_requests(requests_json: &str, folders_json: Option<&str>, folder_id: Option<&str>) -> Vec<TestRequest> {
    let mut parsed_requests: Vec<ParsedRequest> = Vec::new();

    // Parse folders and their requests
    if let Some(folders_str) = folders_json {
        if let Ok(folders_value) = serde_json::from_str::<Value>(folders_str) {
            if let Some(folders_array) = folders_value.as_array() {
                // If folder_id is specified, find and process only that folder
                if let Some(target_folder_id) = folder_id {
                    if let Some(folder) = find_folder_by_id(folders_array, target_folder_id) {
                        collect_folder_requests(&folder, &mut parsed_requests);
                    }
                    // Sort by testOrder and return
                    parsed_requests.sort_by(|a, b| {
                        let order_a = a.test_order.unwrap_or(i64::MAX);
                        let order_b = b.test_order.unwrap_or(i64::MAX);
                        order_a.cmp(&order_b)
                    });
                    return parsed_requests.into_iter().map(|p| p.request).collect();
                }
                
                // No folder filter - collect all folder requests
                for folder in folders_array {
                    collect_folder_requests(folder, &mut parsed_requests);
                }
            }
        }
    }

    // If folder_id is specified, we already returned above
    // Only include root level requests when no folder filter is specified
    if folder_id.is_none() {
        if let Ok(requests_value) = serde_json::from_str::<Value>(requests_json) {
            if let Some(requests_array) = requests_value.as_array() {
                for r in requests_array {
                    if let Some(parsed) = parse_request_json(r) {
                        parsed_requests.push(parsed);
                    }
                }
            }
        }
    }

    // Sort by testOrder (requests without testOrder go to end)
    parsed_requests.sort_by(|a, b| {
        let order_a = a.test_order.unwrap_or(i64::MAX);
        let order_b = b.test_order.unwrap_or(i64::MAX);
        order_a.cmp(&order_b)
    });

    parsed_requests.into_iter().map(|p| p.request).collect()
}

/// Find a folder by ID in the folder tree (including nested folders)
fn find_folder_by_id<'a>(folders: &'a [Value], folder_id: &str) -> Option<&'a Value> {
    for folder in folders {
        if let Some(id) = folder.get("id").and_then(|v| v.as_str()) {
            if id == folder_id {
                return Some(folder);
            }
        }
        // Check nested folders
        if let Some(subfolders) = folder.get("folders").and_then(|f| f.as_array()) {
            if let Some(found) = find_folder_by_id(subfolders, folder_id) {
                return Some(found);
            }
        }
    }
    None
}

/// Recursively collect all requests from a folder and its subfolders
fn collect_folder_requests(folder: &Value, parsed_requests: &mut Vec<ParsedRequest>) {
    // Collect requests from this folder
    if let Some(folder_requests) = folder.get("requests").and_then(|r| r.as_array()) {
        for r in folder_requests {
            if let Some(parsed) = parse_request_json(r) {
                parsed_requests.push(parsed);
            }
        }
    }
    
    // Recursively collect from subfolders
    if let Some(subfolders) = folder.get("folders").and_then(|f| f.as_array()) {
        for subfolder in subfolders {
            collect_folder_requests(subfolder, parsed_requests);
        }
    }
}

fn substitute_variables(text: &str, variables: &HashMap<String, String>) -> String {
    let mut result = text.to_string();
    for (key, value) in variables {
        // Support both {{variableName}} and {variableName} formats
        let double_brace_pattern = format!("{{{{{}}}}}", key);
        let single_brace_pattern = format!("{{{}}}", key);
        result = result.replace(&double_brace_pattern, value);
        result = result.replace(&single_brace_pattern, value);
    }
    result
}

fn evaluate_jsonpath(json_str: &str, path: &str) -> Result<serde_json::Value, String> {
    let json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let json_path = jsonpath_rust::JsonPath::try_from(path)
        .map_err(|e| format!("Invalid JSONPath '{}': {}", path, e))?;
    
    let result = json_path.find(&json);
    
    match &result {
        serde_json::Value::Array(arr) if arr.len() == 1 => Ok(arr[0].clone()),
        serde_json::Value::Array(arr) if arr.is_empty() => Ok(serde_json::Value::Null),
        _ => Ok(result),
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

fn evaluate_assertion(
    assertion: &Assertion,
    response_status: u16,
    response_time_ms: u64,
    response_body: &str,
    response_headers: &HashMap<String, String>,
) -> AssertionResult {
    match assertion.assertion_type {
        AssertionType::Status => {
            let expected = assertion.expected_status.unwrap_or(200);
            let passed = response_status == expected;
            AssertionResult {
                name: format!("Status code equals {}", expected),
                passed,
                expected: expected.to_string(),
                actual: response_status.to_string(),
            }
        }
        AssertionType::StatusRange => {
            let min = assertion.min_status.unwrap_or(200);
            let max = assertion.max_status.unwrap_or(299);
            let passed = response_status >= min && response_status <= max;
            AssertionResult {
                name: format!("Status code in range {}-{}", min, max),
                passed,
                expected: format!("{}-{}", min, max),
                actual: response_status.to_string(),
            }
        }
        AssertionType::Jsonpath => {
            let path = assertion.json_path.as_deref().unwrap_or("$");
            let operator = assertion.operator.as_ref().unwrap_or(&JsonPathOperator::Exists);
            let expected_value = assertion.expected_value.as_deref().unwrap_or("");
            
            match evaluate_jsonpath(response_body, path) {
                Ok(actual_value) => {
                    let actual_str = value_to_string(&actual_value);
                    let passed = match operator {
                        JsonPathOperator::Equals => actual_str == expected_value,
                        JsonPathOperator::NotEquals => actual_str != expected_value,
                        JsonPathOperator::Contains => actual_str.contains(expected_value),
                        JsonPathOperator::Exists => actual_value != serde_json::Value::Null,
                        JsonPathOperator::NotExists => actual_value == serde_json::Value::Null,
                    };
                    AssertionResult {
                        name: format!("JSONPath {} {:?}", path, operator),
                        passed,
                        expected: match operator {
                            JsonPathOperator::Exists => "exists".to_string(),
                            JsonPathOperator::NotExists => "not exists".to_string(),
                            _ => expected_value.to_string(),
                        },
                        actual: actual_str,
                    }
                }
                Err(e) => AssertionResult {
                    name: format!("JSONPath {}", path),
                    passed: false,
                    expected: expected_value.to_string(),
                    actual: format!("Error: {}", e),
                },
            }
        }
        AssertionType::Contains => {
            let search = assertion.search_string.as_deref().unwrap_or("");
            let passed = response_body.contains(search);
            AssertionResult {
                name: format!("Response contains '{}'", search),
                passed,
                expected: format!("contains '{}'", search),
                actual: if passed { "found".to_string() } else { "not found".to_string() },
            }
        }
        AssertionType::ResponseTime => {
            let max_time = assertion.max_time_ms.unwrap_or(5000);
            let passed = response_time_ms <= max_time;
            AssertionResult {
                name: format!("Response time < {}ms", max_time),
                passed,
                expected: format!("< {}ms", max_time),
                actual: format!("{}ms", response_time_ms),
            }
        }
        AssertionType::Header => {
            let header_name = assertion.header_name.as_deref().unwrap_or("");
            let expected_value = assertion.header_value.as_deref();
            
            let actual_value = response_headers
                .iter()
                .find(|(k, _)| k.to_lowercase() == header_name.to_lowercase())
                .map(|(_, v)| v.as_str());
            
            let (passed, expected_str, actual_str) = match (expected_value, actual_value) {
                (Some(expected), Some(actual)) => {
                    (expected == actual, expected.to_string(), actual.to_string())
                }
                (Some(expected), None) => {
                    (false, expected.to_string(), "header not found".to_string())
                }
                (None, Some(actual)) => {
                    (true, "exists".to_string(), actual.to_string())
                }
                (None, None) => {
                    (false, "exists".to_string(), "header not found".to_string())
                }
            };
            
            AssertionResult {
                name: format!("Header '{}'", header_name),
                passed,
                expected: expected_str,
                actual: actual_str,
            }
        }
    }
}

fn extract_variables(
    extractions: &[VariableExtraction],
    response_body: &str,
) -> Vec<ExtractedVariable> {
    extractions
        .iter()
        .filter(|e| e.enabled)
        .map(|extraction| {
            match evaluate_jsonpath(response_body, &extraction.json_path) {
                Ok(value) => {
                    let value_str = value_to_string(&value);
                    ExtractedVariable {
                        variable_name: extraction.variable_name.clone(),
                        json_path: extraction.json_path.clone(),
                        value: value_str,
                        success: value != serde_json::Value::Null,
                        error: if value == serde_json::Value::Null {
                            Some("Path returned null".to_string())
                        } else {
                            None
                        },
                    }
                }
                Err(e) => ExtractedVariable {
                    variable_name: extraction.variable_name.clone(),
                    json_path: extraction.json_path.clone(),
                    value: String::new(),
                    success: false,
                    error: Some(e),
                },
            }
        })
        .collect()
}

async fn execute_single_request(
    request: &TestRequest,
    run_context: &HashMap<String, String>,
) -> TestResult {
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return TestResult {
                request_id: request.id.clone(),
                request_name: request.name.clone(),
                method: request.method.clone(),
                url: request.url.clone(),
                status: TestStatus::Error,
                response_status: None,
                response_time: None,
                response_size: None,
                response_body: None,
                response_headers: None,
                error: Some(format!("Failed to create HTTP client: {}", e)),
                assertions: vec![],
                extracted_variables: None,
            };
        }
    };

    // Substitute variables in URL
    let mut request_url = substitute_variables(&request.url, run_context);
    
    // Build URL with query params
    let enabled_params: HashMap<String, String> = request
        .params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| {
            (
                substitute_variables(&p.key, run_context),
                substitute_variables(&p.value, run_context),
            )
        })
        .collect();

    if !enabled_params.is_empty() {
        let query_string: String = enabled_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        if request_url.contains('?') {
            request_url = format!("{}&{}", request_url, query_string);
        } else {
            request_url = format!("{}?{}", request_url, query_string);
        }
    }

    // Build request
    let mut req = match request.method.to_uppercase().as_str() {
        "GET" => client.get(&request_url),
        "POST" => client.post(&request_url),
        "PUT" => client.put(&request_url),
        "PATCH" => client.patch(&request_url),
        "DELETE" => client.delete(&request_url),
        "HEAD" => client.head(&request_url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, &request_url),
        _ => {
            return TestResult {
                request_id: request.id.clone(),
                request_name: request.name.clone(),
                method: request.method.clone(),
                url: request.url.clone(),
                status: TestStatus::Error,
                response_status: None,
                response_time: None,
                response_size: None,
                response_body: None,
                response_headers: None,
                error: Some(format!("Unsupported method: {}", request.method)),
                assertions: vec![],
                extracted_variables: None,
            };
        }
    };

    // Add headers
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let key = substitute_variables(&header.key, run_context);
            let value = substitute_variables(&header.value, run_context);
            req = req.header(&key, &value);
        }
    }

    // Add body
    if let Some(body_content) = &request.body {
        if !body_content.is_empty() {
            let substituted_body = substitute_variables(body_content, run_context);
            match request.body_type.as_str() {
                "json" => {
                    req = req
                        .header("Content-Type", "application/json")
                        .body(substituted_body);
                }
                "xml" => {
                    req = req
                        .header("Content-Type", "application/xml")
                        .body(substituted_body);
                }
                "html" => {
                    req = req
                        .header("Content-Type", "text/html")
                        .body(substituted_body);
                }
                _ => {
                    req = req.body(substituted_body);
                }
            }
        }
    }

    // Execute request
    let start = Instant::now();
    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                request_id: request.id.clone(),
                request_name: request.name.clone(),
                method: request.method.clone(),
                url: request_url,
                status: TestStatus::Error,
                response_status: None,
                response_time: Some(start.elapsed().as_millis() as u64),
                response_size: None,
                response_body: None,
                response_headers: None,
                error: Some(e.to_string()),
                assertions: vec![],
                extracted_variables: None,
            };
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    
    // Collect response headers
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    // Get body
    let body_bytes = response.bytes().await.unwrap_or_default();
    let size = body_bytes.len();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    // Evaluate assertions
    let mut assertions = vec![];
    
    if let Some(configured_assertions) = &request.assertions {
        for assertion in configured_assertions.iter().filter(|a| a.enabled) {
            assertions.push(evaluate_assertion(
                assertion,
                status_code,
                elapsed,
                &body_str,
                &response_headers,
            ));
        }
    }
    
    // If no assertions configured, add default status check
    if assertions.is_empty() {
        let status_passed = status_code < 400;
        assertions.push(AssertionResult {
            name: "Status code is successful (< 400)".to_string(),
            passed: status_passed,
            expected: "< 400".to_string(),
            actual: status_code.to_string(),
        });
    }

    // Extract variables
    let extracted_variables = if let Some(extractions) = &request.extract_variables {
        if !extractions.is_empty() {
            Some(extract_variables(extractions, &body_str))
        } else {
            None
        }
    } else {
        None
    };

    // Determine overall test status
    let all_passed = assertions.iter().all(|a| a.passed);
    let test_status = if all_passed {
        TestStatus::Passed
    } else {
        TestStatus::Failed
    };

    TestResult {
        request_id: request.id.clone(),
        request_name: request.name.clone(),
        method: request.method.clone(),
        url: request_url,
        status: test_status,
        response_status: Some(status_code),
        response_time: Some(elapsed),
        response_size: Some(size),
        response_body: Some(body_str),
        response_headers: Some(response_headers),
        error: None,
        assertions,
        extracted_variables,
    }
}

// ============ API Handlers ============

/// Run tests for a collection
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/tests/run",
    tag = "Tests",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID")
    ),
    request_body = RunTestsRequest,
    responses(
        (status = 200, description = "Test run completed", body = TestRunSummary),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Workspace not found", body = ApiError),
        (status = 500, description = "Internal error", body = ApiError)
    )
)]
pub async fn run_tests(
    Path(workspace_id): Path<String>,
    State(storage): State<Arc<Storage>>,
    Json(request): Json<RunTestsRequest>,
) -> Result<Json<TestRunSummary>, ApiError> {
    // Verify workspace exists
    let workspace = storage.get_workspace(&workspace_id)
        .map_err(|e| ApiError::internal_error(e))?;
    
    if workspace.is_none() {
        return Err(ApiError::not_found("Workspace not found"));
    }

    if request.requests.is_empty() {
        return Err(ApiError::bad_request("No requests to test"));
    }

    let run_id = generate_id();
    let total = request.requests.len();
    let mut results: Vec<TestResult> = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let start_time = Instant::now();
    
    // Initialize run context with provided variables
    let mut run_context: HashMap<String, String> = request.variables.clone();

    for (index, test_request) in request.requests.iter().enumerate() {
        // Execute the request
        let result = execute_single_request(test_request, &run_context).await;

        // Extract variables for subsequent requests
        if let Some(extracted) = &result.extracted_variables {
            for var in extracted {
                if var.success {
                    run_context.insert(var.variable_name.clone(), var.value.clone());
                }
            }
        }

        // Update counters
        match result.status {
            TestStatus::Passed => passed += 1,
            TestStatus::Failed => failed += 1,
            TestStatus::Error => errors += 1,
            _ => {}
        }

        let should_stop = request.stop_on_failure
            && (result.status == TestStatus::Failed || result.status == TestStatus::Error);

        results.push(result);

        if should_stop {
            break;
        }

        // Delay between requests
        if request.delay_between_requests > 0 && index < total - 1 {
            tokio::time::sleep(std::time::Duration::from_millis(
                request.delay_between_requests,
            ))
            .await;
        }
    }

    let total_time = start_time.elapsed().as_millis() as u64;

    let summary = TestRunSummary {
        run_id,
        name: request.name,
        total,
        passed,
        failed,
        errors,
        total_time,
        results,
    };

    Ok(Json(summary))
}

/// Run tests for a specific collection by ID
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}/tests/run",
    tag = "Tests",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    request_body = RunCollectionTestsRequest,
    responses(
        (status = 200, description = "Test run completed", body = TestRunSummary),
        (status = 404, description = "Collection not found", body = ApiError),
        (status = 500, description = "Internal error", body = ApiError)
    )
)]
pub async fn run_collection_tests(
    Path((workspace_id, collection_id)): Path<(String, String)>,
    State(storage): State<Arc<Storage>>,
    Json(request): Json<RunCollectionTestsRequest>,
) -> Result<Json<TestRunSummary>, ApiError> {
    // Get collection from storage
    let collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;

    let requests_json = serde_json::to_string(&collection.requests)
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    let folders_json = collection.folders.as_ref()
        .map(|f| serde_json::to_string(f).unwrap_or_default());

    let test_requests = parse_all_requests(&requests_json, folders_json.as_deref(), request.folder_id.as_deref());

    if test_requests.is_empty() {
        let error_msg = if request.folder_id.is_some() {
            "Folder has no requests to test"
        } else {
            "Collection has no requests to test"
        };
        return Err(ApiError::bad_request(error_msg));
    }

    // Run tests - include folder name in test name if filtering by folder
    let test_name = if let Some(ref folder_id) = request.folder_id {
        // Try to find folder name
        if let Some(folders_str) = &folders_json {
            if let Ok(folders_value) = serde_json::from_str::<Value>(folders_str) {
                if let Some(folders_array) = folders_value.as_array() {
                    if let Some(folder) = find_folder_by_id(folders_array, folder_id) {
                        let folder_name = folder.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown Folder");
                        format!("{} / {}", collection.name, folder_name)
                    } else {
                        collection.name.clone()
                    }
                } else {
                    collection.name.clone()
                }
            } else {
                collection.name.clone()
            }
        } else {
            collection.name.clone()
        }
    } else {
        collection.name.clone()
    };

    let run_request = RunTestsRequest {
        name: test_name,
        requests: test_requests,
        stop_on_failure: request.stop_on_failure,
        delay_between_requests: request.delay_between_requests,
        variables: request.variables,
    };

    run_tests(Path(workspace_id), State(storage), Json(run_request)).await
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunCollectionTestsRequest {
    #[serde(default)]
    pub stop_on_failure: bool,
    #[serde(default)]
    pub delay_between_requests: u64,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    /// Optional folder ID to filter tests to a specific folder
    #[serde(default)]
    pub folder_id: Option<String>,
}

// ============ SSE Streaming Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum TestEvent {
    #[serde(rename = "start")]
    Start { run_id: String, name: String, total: usize },
    #[serde(rename = "progress")]
    Progress { index: usize, total: usize, result: TestResult },
    #[serde(rename = "complete")]
    Complete { summary: TestRunSummary },
}

/// Run tests for a collection with SSE streaming
#[utoipa::path(
    post,
    path = "/api/workspaces/{workspace_id}/collections/{collection_id}/tests/stream",
    tag = "Tests",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("collection_id" = String, Path, description = "Collection ID")
    ),
    request_body = RunCollectionTestsRequest,
    responses(
        (status = 200, description = "SSE stream of test events"),
        (status = 404, description = "Collection not found", body = ApiError),
        (status = 500, description = "Internal error", body = ApiError)
    )
)]
pub async fn run_collection_tests_stream(
    Path((workspace_id, collection_id)): Path<(String, String)>,
    State(storage): State<Arc<Storage>>,
    Json(request): Json<RunCollectionTestsRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Get collection from storage
    let collection = storage.get_collection(&workspace_id, &collection_id)
        .map_err(|e| ApiError::internal_error(e))?
        .ok_or_else(|| ApiError::not_found("Collection not found"))?;

    let requests_json = serde_json::to_string(&collection.requests)
        .map_err(|e| ApiError::internal_error(e.to_string()))?;
    let folders_json = collection.folders.as_ref()
        .map(|f| serde_json::to_string(f).unwrap_or_default());
    let collection_name = collection.name;

    let test_requests = parse_all_requests(&requests_json, folders_json.as_deref(), request.folder_id.as_deref());

    if test_requests.is_empty() {
        let error_msg = if request.folder_id.is_some() {
            "Folder has no requests to test"
        } else {
            "Collection has no requests to test"
        };
        return Err(ApiError::bad_request(error_msg));
    }

    // Include folder name in test name if filtering by folder
    let test_name = if let Some(ref folder_id) = request.folder_id {
        if let Some(ref folders_str) = folders_json {
            if let Ok(folders_value) = serde_json::from_str::<Value>(folders_str) {
                if let Some(folders_array) = folders_value.as_array() {
                    if let Some(folder) = find_folder_by_id(folders_array, folder_id) {
                        let folder_name = folder.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown Folder");
                        format!("{} / {}", collection_name, folder_name)
                    } else {
                        collection_name.clone()
                    }
                } else {
                    collection_name.clone()
                }
            } else {
                collection_name.clone()
            }
        } else {
            collection_name.clone()
        }
    } else {
        collection_name.clone()
    };

    let run_id = generate_id();
    let total = test_requests.len();
    let name = test_name;
    let stop_on_failure = request.stop_on_failure;
    let delay = request.delay_between_requests;
    let variables = request.variables.clone();

    // Create async stream
    let stream = async_stream::stream! {
        let start_event = TestEvent::Start {
            run_id: run_id.clone(),
            name: name.clone(),
            total,
        };
        yield Ok(Event::default().data(serde_json::to_string(&start_event).unwrap()));

        let mut results: Vec<TestResult> = Vec::new();
        let mut passed = 0;
        let mut failed = 0;
        let mut errors = 0;
        let start_time = Instant::now();
        let mut run_context: HashMap<String, String> = variables;

        for (index, test_request) in test_requests.iter().enumerate() {
            let result = execute_single_request(test_request, &run_context).await;

            // Extract variables for subsequent requests
            if let Some(extracted) = &result.extracted_variables {
                for var in extracted {
                    if var.success {
                        run_context.insert(var.variable_name.clone(), var.value.clone());
                    }
                }
            }

            // Update counters
            match result.status {
                TestStatus::Passed => passed += 1,
                TestStatus::Failed => failed += 1,
                TestStatus::Error => errors += 1,
                _ => {}
            }

            let should_stop = stop_on_failure
                && (result.status == TestStatus::Failed || result.status == TestStatus::Error);

            // Send progress event
            let progress_event = TestEvent::Progress {
                index: index + 1,
                total,
                result: result.clone(),
            };
            yield Ok(Event::default().data(serde_json::to_string(&progress_event).unwrap()));

            results.push(result);

            if should_stop {
                break;
            }

            // Delay between requests
            if delay > 0 && index < total - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }

        let total_time = start_time.elapsed().as_millis() as u64;

        let summary = TestRunSummary {
            run_id,
            name,
            total,
            passed,
            failed,
            errors,
            total_time,
            results,
        };

        let complete_event = TestEvent::Complete { summary };
        yield Ok(Event::default().data(serde_json::to_string(&complete_event).unwrap()));
    };

    Ok(Sse::new(stream))
}
