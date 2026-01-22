use jsonpath_rust::JsonPath;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

// ============ Assertion Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionType {
    Status,
    StatusRange,
    Jsonpath,
    Contains,
    ResponseTime,
    Header,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonPathOperator {
    Equals,
    NotEquals,
    Contains,
    Exists,
    NotExists,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assertion {
    pub id: String,
    #[serde(rename = "type")]
    pub assertion_type: AssertionType,
    pub enabled: bool,
    // For status
    pub expected_status: Option<u16>,
    // For status_range
    pub min_status: Option<u16>,
    pub max_status: Option<u16>,
    // For jsonpath
    pub json_path: Option<String>,
    pub operator: Option<JsonPathOperator>,
    pub expected_value: Option<String>,
    // For contains
    pub search_string: Option<String>,
    // For response_time
    pub max_time_ms: Option<u64>,
    // For header
    pub header_name: Option<String>,
    pub header_value: Option<String>,
}

// ============ Variable Extraction ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableExtraction {
    pub id: String,
    pub variable_name: String,
    pub json_path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedVariable {
    pub variable_name: String,
    pub json_path: String,
    pub value: String,
    pub success: bool,
    pub error: Option<String>,
}

// ============ Request & Result Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRequest {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub params: Vec<KeyValue>,
    pub body: Option<String>,
    pub body_type: String,
    // Configurable assertions for this request
    pub assertions: Option<Vec<Assertion>>,
    // Variables to extract from response
    pub extract_variables: Option<Vec<VariableExtraction>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub request_id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status: TestStatus,
    pub response_status: Option<u16>,
    pub response_time: Option<u64>,
    pub response_size: Option<usize>,
    pub response_body: Option<String>,
    pub response_headers: Option<HashMap<String, String>>,
    pub error: Option<String>,
    pub assertions: Vec<AssertionResult>,
    pub extracted_variables: Option<Vec<ExtractedVariable>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssertionResult {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunConfig {
    pub id: String,
    pub name: String,
    pub requests: Vec<TestRequest>,
    pub stop_on_failure: bool,
    pub delay_between_requests: u64, // milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProgressEvent {
    pub run_id: String,
    pub current: usize,
    pub total: usize,
    pub result: TestResult,
}

// ============ Test Run History ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunHistory {
    pub id: String,
    pub run_id: String,
    pub collection_id: Option<String>,
    pub collection_name: String,
    pub timestamp: u64,
    pub summary: TestRunSummary,
}

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ============ JSONPath Evaluation ============

fn evaluate_jsonpath(json_str: &str, path: &str) -> Result<Value, String> {
    let json: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let json_path = JsonPath::try_from(path)
        .map_err(|e| format!("Invalid JSONPath '{}': {}", path, e))?;
    
    // The find method returns a Value directly (wrapped as an Array if multiple matches)
    let result = json_path.find(&json);
    
    // If result is an array with one element, unwrap it
    match &result {
        Value::Array(arr) if arr.len() == 1 => Ok(arr[0].clone()),
        Value::Array(arr) if arr.is_empty() => Ok(Value::Null),
        _ => Ok(result),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

// ============ Assertion Evaluation ============

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
                        JsonPathOperator::Exists => actual_value != Value::Null,
                        JsonPathOperator::NotExists => actual_value == Value::Null,
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

// ============ Variable Extraction ============

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
                        success: value != Value::Null,
                        error: if value == Value::Null {
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

// ============ Variable Substitution ============

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

// ============ Request Execution ============

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
    
    // Build URL with query params (also substitute variables)
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

    // Add headers (with variable substitution)
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let key = substitute_variables(&header.key, run_context);
            let value = substitute_variables(&header.value, run_context);
            req = req.header(&key, &value);
        }
    }

    // Add body (with variable substitution)
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

    // Determine overall test status based on all assertions
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

#[tauri::command]
pub async fn run_collection_tests(
    app: AppHandle,
    config: TestRunConfig,
) -> Result<TestRunSummary, String> {
    let run_id = config.id.clone();
    let total = config.requests.len();
    let mut results: Vec<TestResult> = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let start_time = Instant::now();
    
    // Run context for variable extraction/substitution
    let mut run_context: HashMap<String, String> = HashMap::new();

    for (index, request) in config.requests.iter().enumerate() {
        // Execute the request with current context
        let result = execute_single_request(request, &run_context).await;

        // Extract variables and add to context for subsequent requests
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

        // Emit progress event
        let progress = TestProgressEvent {
            run_id: run_id.clone(),
            current: index + 1,
            total,
            result: result.clone(),
        };
        let _ = app.emit("test-progress", &progress);

        results.push(result.clone());

        // Stop on failure if configured
        if config.stop_on_failure
            && (result.status == TestStatus::Failed || result.status == TestStatus::Error)
        {
            break;
        }

        // Delay between requests
        if config.delay_between_requests > 0 && index < total - 1 {
            tokio::time::sleep(std::time::Duration::from_millis(
                config.delay_between_requests,
            ))
            .await;
        }
    }

    let total_time = start_time.elapsed().as_millis() as u64;

    let summary = TestRunSummary {
        run_id,
        name: config.name,
        total,
        passed,
        failed,
        errors,
        total_time,
        results,
    };

    // Emit completion event
    let _ = app.emit("test-complete", &summary);

    Ok(summary)
}

#[tauri::command]
pub fn create_test_config(
    name: String,
    requests: Vec<TestRequest>,
    stop_on_failure: bool,
    delay_between_requests: u64,
) -> TestRunConfig {
    TestRunConfig {
        id: generate_id(),
        name,
        requests,
        stop_on_failure,
        delay_between_requests,
    }
}

// ============ Helper Commands ============

#[tauri::command]
pub fn evaluate_jsonpath_test(json_str: String, path: String) -> Result<String, String> {
    evaluate_jsonpath(&json_str, &path).map(|v| value_to_string(&v))
}
