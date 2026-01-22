use rquickjs::{Context, Runtime, Object, Function, Value, prelude::Rest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============ Script Context Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptContext {
    pub request: ScriptRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ScriptResponse>,
    pub variables: HashMap<String, String>,
    pub environment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub success: bool,
    pub error: Option<String>,
    pub console_output: Vec<String>,
    pub modified_variables: HashMap<String, String>,
    pub modified_headers: HashMap<String, String>,
    pub abort_request: bool,
}

impl Default for ScriptResult {
    fn default() -> Self {
        Self {
            success: true,
            error: None,
            console_output: vec![],
            modified_variables: HashMap::new(),
            modified_headers: HashMap::new(),
            abort_request: false,
        }
    }
}

// ============ JavaScript Runtime ============

/// Execute a pre-request script
#[tauri::command]
pub async fn run_pre_request_script(
    script: String,
    context: ScriptContext,
) -> Result<ScriptResult, String> {
    if script.trim().is_empty() {
        return Ok(ScriptResult::default());
    }
    
    run_script_internal(script, context, false).await
}

/// Execute a post-request script
#[tauri::command]
pub async fn run_post_request_script(
    script: String,
    context: ScriptContext,
) -> Result<ScriptResult, String> {
    if script.trim().is_empty() {
        return Ok(ScriptResult::default());
    }
    
    run_script_internal(script, context, true).await
}

async fn run_script_internal(
    script: String,
    context: ScriptContext,
    is_post_request: bool,
) -> Result<ScriptResult, String> {
    let rt = Runtime::new().map_err(|e| format!("Failed to create JS runtime: {}", e))?;
    let ctx = Context::full(&rt).map_err(|e| format!("Failed to create JS context: {}", e))?;
    
    let mut result = ScriptResult::default();
    let console_output: std::sync::Arc<std::sync::Mutex<Vec<String>>> = 
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let modified_vars: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>> = 
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let modified_headers: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>> = 
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let abort_flag: std::sync::Arc<std::sync::Mutex<bool>> = 
        std::sync::Arc::new(std::sync::Mutex::new(false));
    
    ctx.with(|ctx| {
        let globals = ctx.globals();
        
        // Create console object
        let console_out = console_output.clone();
        let console = Object::new(ctx.clone()).unwrap();
        
        // console.log
        let log_out = console_out.clone();
        let log_fn = Function::new(ctx.clone(), move |args: Rest<Value>| {
            let msg: String = args.iter()
                .map(|v| format!("{:?}", v))
                .collect::<Vec<_>>()
                .join(" ");
            if let Ok(mut output) = log_out.lock() {
                output.push(format!("[LOG] {}", msg));
            }
        }).unwrap();
        console.set("log", log_fn).unwrap();
        
        // console.warn
        let warn_out = console_out.clone();
        let warn_fn = Function::new(ctx.clone(), move |args: Rest<Value>| {
            let msg: String = args.iter()
                .map(|v| format!("{:?}", v))
                .collect::<Vec<_>>()
                .join(" ");
            if let Ok(mut output) = warn_out.lock() {
                output.push(format!("[WARN] {}", msg));
            }
        }).unwrap();
        console.set("warn", warn_fn).unwrap();
        
        // console.error
        let error_out = console_out.clone();
        let error_fn = Function::new(ctx.clone(), move |args: Rest<Value>| {
            let msg: String = args.iter()
                .map(|v| format!("{:?}", v))
                .collect::<Vec<_>>()
                .join(" ");
            if let Ok(mut output) = error_out.lock() {
                output.push(format!("[ERROR] {}", msg));
            }
        }).unwrap();
        console.set("error", error_fn).unwrap();
        
        globals.set("console", console).unwrap();
        
        // Create istek object (our API)
        let istek = Object::new(ctx.clone()).unwrap();
        
        // istek.variables
        let vars_obj = Object::new(ctx.clone()).unwrap();
        let vars_for_get = context.variables.clone();
        let vars_store = modified_vars.clone();
        
        // istek.variables.get(name)
        let get_fn = Function::new(ctx.clone(), move |name: String| -> Option<String> {
            vars_for_get.get(&name).cloned()
        }).unwrap();
        vars_obj.set("get", get_fn).unwrap();
        
        // istek.variables.set(name, value)
        let set_fn = Function::new(ctx.clone(), move |name: String, value: String| {
            if let Ok(mut vars) = vars_store.lock() {
                vars.insert(name, value);
            }
        }).unwrap();
        vars_obj.set("set", set_fn).unwrap();
        
        istek.set("variables", vars_obj).unwrap();
        
        // istek.request
        let request_obj = Object::new(ctx.clone()).unwrap();
        request_obj.set("method", context.request.method.as_str()).unwrap();
        request_obj.set("url", context.request.url.as_str()).unwrap();
        
        let headers_obj = Object::new(ctx.clone()).unwrap();
        for (k, v) in &context.request.headers {
            headers_obj.set(k.as_str(), v.as_str()).unwrap();
        }
        request_obj.set("headers", headers_obj).unwrap();
        
        let params_obj = Object::new(ctx.clone()).unwrap();
        for (k, v) in &context.request.params {
            params_obj.set(k.as_str(), v.as_str()).unwrap();
        }
        request_obj.set("params", params_obj).unwrap();
        
        if let Some(body) = &context.request.body {
            request_obj.set("body", body.as_str()).unwrap();
        }
        
        // istek.request.setHeader(name, value)
        let headers_store = modified_headers.clone();
        let set_header_fn = Function::new(ctx.clone(), move |name: String, value: String| {
            if let Ok(mut headers) = headers_store.lock() {
                headers.insert(name, value);
            }
        }).unwrap();
        request_obj.set("setHeader", set_header_fn).unwrap();
        
        istek.set("request", request_obj).unwrap();
        
        // istek.response (only for post-request scripts)
        if is_post_request {
            if let Some(response) = &context.response {
                let response_obj = Object::new(ctx.clone()).unwrap();
                response_obj.set("status", response.status as u32).unwrap();
                response_obj.set("statusText", response.status_text.as_str()).unwrap();
                response_obj.set("body", response.body.as_str()).unwrap();
                response_obj.set("time", response.time as u32).unwrap();
                
                let resp_headers_obj = Object::new(ctx.clone()).unwrap();
                for (k, v) in &response.headers {
                    resp_headers_obj.set(k.as_str(), v.as_str()).unwrap();
                }
                response_obj.set("headers", resp_headers_obj).unwrap();
                
                // istek.response.json() - parse body as JSON
                let _body_for_json = response.body.clone();
                let json_fn = Function::new(ctx.clone(), move || -> rquickjs::Result<Value> {
                    // Return the raw body, let JS parse it
                    Err(rquickjs::Error::Exception)
                }).unwrap();
                response_obj.set("json", json_fn).unwrap();
                
                istek.set("response", response_obj).unwrap();
            }
        }
        
        // istek.abort() - abort the request (only for pre-request)
        let abort_store = abort_flag.clone();
        let abort_fn = Function::new(ctx.clone(), move || {
            if let Ok(mut abort) = abort_store.lock() {
                *abort = true;
            }
        }).unwrap();
        istek.set("abort", abort_fn).unwrap();
        
        // istek.environment
        istek.set("environment", context.environment.as_str()).unwrap();
        
        globals.set("istek", istek).unwrap();
        
        // Also create JSON.parse helper that works
        let json_parse_script = r#"
            if (typeof istek.response !== 'undefined' && istek.response.body) {
                istek.response.json = function() {
                    return JSON.parse(this.body);
                };
            }
        "#;
        
        if is_post_request {
            let _ = ctx.eval::<(), _>(json_parse_script);
        }
    });
    
    // Execute the user script
    let script_result = ctx.with(|ctx| {
        ctx.eval::<(), _>(script.as_str())
    });
    
    match script_result {
        Ok(_) => {
            result.success = true;
        }
        Err(e) => {
            result.success = false;
            result.error = Some(format!("Script error: {}", e));
        }
    }
    
    // Collect results
    if let Ok(output) = console_output.lock() {
        result.console_output = output.clone();
    }
    if let Ok(vars) = modified_vars.lock() {
        result.modified_variables = vars.clone();
    }
    if let Ok(headers) = modified_headers.lock() {
        result.modified_headers = headers.clone();
    }
    if let Ok(abort) = abort_flag.lock() {
        result.abort_request = *abort;
    }
    
    Ok(result)
}

// ============ Test Script Evaluation ============

/// Test a script without actually running a request
#[tauri::command]
pub async fn test_script(script: String) -> Result<ScriptResult, String> {
    let mock_context = ScriptContext {
        request: ScriptRequest {
            method: "GET".to_string(),
            url: "https://api.example.com/test".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: None,
        },
        response: Some(ScriptResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: r#"{"message": "Hello, World!"}"#.to_string(),
            time: 150,
        }),
        variables: HashMap::new(),
        environment: "Development".to_string(),
    };
    
    run_script_internal(script, mock_context, true).await
}
