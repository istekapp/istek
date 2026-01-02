use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedCurlRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueParam>,
    pub body: Option<String>,
    #[serde(rename = "bodyType")]
    pub body_type: String,
    #[serde(rename = "formData")]
    pub form_data: Option<Vec<KeyValueParam>>,
}

/// Parse a curl command and extract request details
#[tauri::command]
pub fn parse_curl_command(curl: String) -> Result<ParsedCurlRequest, String> {
    let input = curl.trim();
    
    // Check if it starts with curl
    if !input.to_lowercase().starts_with("curl") {
        return Err("Input does not start with 'curl'".to_string());
    }
    
    // Remove line continuations (backslash + newline)
    let normalized = input
        .replace("\\\n", " ")
        .replace("\\\r\n", " ")
        .replace("  ", " ");
    
    let mut method = "GET".to_string();
    let mut url = String::new();
    let mut headers: Vec<KeyValueParam> = Vec::new();
    let mut body: Option<String> = None;
    let mut body_type = "none".to_string();
    
    // Tokenize the command respecting quotes
    let tokens = tokenize_curl(&normalized)?;
    
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        
        match token.as_str() {
            "curl" => {
                // Skip the curl command itself
            }
            "-X" | "--request" => {
                if i + 1 < tokens.len() {
                    method = tokens[i + 1].to_uppercase();
                    i += 1;
                }
            }
            "-H" | "--header" => {
                if i + 1 < tokens.len() {
                    let header_str = &tokens[i + 1];
                    if let Some(colon_pos) = header_str.find(':') {
                        let key = header_str[..colon_pos].trim().to_string();
                        let value = header_str[colon_pos + 1..].trim().to_string();
                        headers.push(KeyValueParam {
                            key,
                            value,
                            enabled: true,
                        });
                    }
                    i += 1;
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                if i + 1 < tokens.len() {
                    let data = &tokens[i + 1];
                    // Append to existing body with & separator (for form data)
                    if let Some(ref mut existing_body) = body {
                        existing_body.push('&');
                        existing_body.push_str(data);
                    } else {
                        body = Some(data.clone());
                    }
                    // If method is still GET and we have a body, change to POST
                    if method == "GET" {
                        method = "POST".to_string();
                    }
                    i += 1;
                }
            }
            "--data-urlencode" => {
                if i + 1 < tokens.len() {
                    // URL encoded data - append to body
                    let data = &tokens[i + 1];
                    if let Some(ref mut existing_body) = body {
                        existing_body.push('&');
                        existing_body.push_str(data);
                    } else {
                        body = Some(data.clone());
                    }
                    body_type = "raw".to_string();
                    if method == "GET" {
                        method = "POST".to_string();
                    }
                    i += 1;
                }
            }
            "-u" | "--user" => {
                if i + 1 < tokens.len() {
                    let credentials = &tokens[i + 1];
                    // Create Basic auth header
                    let encoded = base64_encode(credentials);
                    headers.push(KeyValueParam {
                        key: "Authorization".to_string(),
                        value: format!("Basic {}", encoded),
                        enabled: true,
                    });
                    i += 1;
                }
            }
            "-A" | "--user-agent" => {
                if i + 1 < tokens.len() {
                    headers.push(KeyValueParam {
                        key: "User-Agent".to_string(),
                        value: tokens[i + 1].clone(),
                        enabled: true,
                    });
                    i += 1;
                }
            }
            "-e" | "--referer" => {
                if i + 1 < tokens.len() {
                    headers.push(KeyValueParam {
                        key: "Referer".to_string(),
                        value: tokens[i + 1].clone(),
                        enabled: true,
                    });
                    i += 1;
                }
            }
            "--compressed" | "-k" | "--insecure" | "-L" | "--location" | 
            "-s" | "--silent" | "-v" | "--verbose" | "-i" | "--include" => {
                // Skip these flags - they don't affect the request we build
            }
            _ => {
                // Check if it looks like a URL
                if token.starts_with("http://") || token.starts_with("https://") || 
                   token.starts_with("{{") || // Variable
                   (token.contains('.') && !token.starts_with('-')) {
                    url = token.clone();
                }
                // Skip other unknown flags
            }
        }
        i += 1;
    }
    
    // Detect body_type from Content-Type header first
    let mut is_form_urlencoded = false;
    for header in &headers {
        if header.key.to_lowercase() == "content-type" {
            let ct = header.value.to_lowercase();
            if ct.contains("application/json") {
                body_type = "json".to_string();
            } else if ct.contains("application/xml") || ct.contains("text/xml") {
                body_type = "xml".to_string();
            } else if ct.contains("text/html") {
                body_type = "html".to_string();
            } else if ct.contains("application/x-www-form-urlencoded") {
                body_type = "x-www-form-urlencoded".to_string();
                is_form_urlencoded = true;
            } else if ct.contains("multipart/form-data") {
                body_type = "form-data".to_string();
                is_form_urlencoded = true;
            }
            break;
        }
    }
    
    // If no Content-Type header, try to detect from body content
    if body_type == "none" {
        if let Some(ref b) = body {
            let trimmed = b.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}')) ||
               (trimmed.starts_with('[') && trimmed.ends_with(']')) {
                body_type = "json".to_string();
            } else if trimmed.starts_with('<') {
                if trimmed.to_lowercase().contains("<!doctype html") || 
                   trimmed.to_lowercase().contains("<html") {
                    body_type = "html".to_string();
                } else {
                    body_type = "xml".to_string();
                }
            } else if !trimmed.is_empty() {
                // Check if it looks like URL encoded form data (key=value&key2=value2)
                if trimmed.contains('=') && !trimmed.contains(' ') {
                    body_type = "x-www-form-urlencoded".to_string();
                    is_form_urlencoded = true;
                } else {
                    body_type = "raw".to_string();
                }
            }
        }
    }
    
    // Parse form data if it's URL encoded
    let form_data = if is_form_urlencoded {
        if let Some(ref b) = body {
            Some(parse_form_urlencoded(b))
        } else {
            Some(Vec::new())
        }
    } else {
        None
    };
    
    if url.is_empty() {
        return Err("Could not find URL in curl command".to_string());
    }
    
    Ok(ParsedCurlRequest {
        method,
        url,
        headers,
        body,
        body_type,
        form_data,
    })
}

/// Parse URL encoded form data (key=value&key2=value2) into key-value pairs
fn parse_form_urlencoded(data: &str) -> Vec<KeyValueParam> {
    data.split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key_raw = parts.next().unwrap_or("");
            let value_raw = parts.next().unwrap_or("");
            // URL decode the values
            let key = urlencoding::decode(key_raw)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| key_raw.to_string());
            let value = urlencoding::decode(value_raw)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| value_raw.to_string());
            KeyValueParam {
                key,
                value,
                enabled: true,
            }
        })
        .collect()
}

/// Tokenize curl command respecting quoted strings
fn tokenize_curl(input: &str) -> Result<Vec<String>, String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = input.chars().peekable();
    
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                if in_single_quote {
                    in_single_quote = false;
                } else {
                    in_single_quote = true;
                }
            }
            '"' if !in_single_quote => {
                if in_double_quote {
                    in_double_quote = false;
                } else {
                    in_double_quote = true;
                }
            }
            '\\' if in_double_quote => {
                // Handle escape sequences in double quotes
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' || next == 'n' || next == 't' {
                        chars.next();
                        match next {
                            'n' => current.push('\n'),
                            't' => current.push('\t'),
                            _ => current.push(next),
                        }
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    
    if !current.is_empty() {
        tokens.push(current);
    }
    
    if in_single_quote || in_double_quote {
        return Err("Unclosed quote in curl command".to_string());
    }
    
    Ok(tokens)
}

/// Simple base64 encoding for Basic auth
fn base64_encode(input: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub time: u64,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValueParam {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

/// Generate a curl command from HTTP request parameters
#[tauri::command]
pub fn generate_curl_command(
    method: String,
    url: String,
    headers: Vec<KeyValueParam>,
    params: Vec<KeyValueParam>,
    body: Option<String>,
    body_type: String,
) -> String {
    let mut parts: Vec<String> = vec!["curl".to_string()];
    
    // Add method (skip for GET as it's default)
    if method.to_uppercase() != "GET" {
        parts.push(format!("-X {}", method.to_uppercase()));
    }
    
    // Build URL with query params
    let mut request_url = url.clone();
    let enabled_params: Vec<&KeyValueParam> = params.iter().filter(|p| p.enabled && !p.key.is_empty()).collect();
    
    if !enabled_params.is_empty() {
        let query_string: String = enabled_params
            .iter()
            .map(|p| format!("{}={}", urlencoding::encode(&p.key), urlencoding::encode(&p.value)))
            .collect::<Vec<_>>()
            .join("&");
        
        if request_url.contains('?') {
            request_url = format!("{}&{}", request_url, query_string);
        } else {
            request_url = format!("{}?{}", request_url, query_string);
        }
    }
    
    // Add URL (with quotes for safety)
    parts.push(format!("'{}'", request_url));
    
    // Add headers
    for header in headers.iter().filter(|h| h.enabled && !h.key.is_empty()) {
        // Escape single quotes in header values
        let escaped_value = header.value.replace("'", "'\\''");
        parts.push(format!("-H '{}: {}'", header.key, escaped_value));
    }
    
    // Add Content-Type header based on body type (if not already present)
    let has_content_type = headers.iter().any(|h| h.enabled && h.key.to_lowercase() == "content-type");
    
    if !has_content_type {
        if let Some(ref body_content) = body {
            if !body_content.is_empty() {
                let content_type = match body_type.as_str() {
                    "json" => Some("application/json"),
                    "xml" => Some("application/xml"),
                    "html" => Some("text/html"),
                    _ => None,
                };
                
                if let Some(ct) = content_type {
                    parts.push(format!("-H 'Content-Type: {}'", ct));
                }
            }
        }
    }
    
    // Add body
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            // Escape single quotes in body
            let escaped_body = body_content.replace("'", "'\\''");
            parts.push(format!("-d '{}'", escaped_body));
        }
    }
    
    parts.join(" \\\n  ")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleKeyValue {
    pub key: String,
    pub value: String,
}

/// Generate code snippet in various programming languages
#[tauri::command]
pub fn generate_code_snippet(
    language: String,
    method: String,
    url: String,
    headers: Vec<SimpleKeyValue>,
    params: Vec<SimpleKeyValue>,
    body: Option<String>,
    body_type: String,
) -> String {
    // Build URL with query params
    let mut request_url = url.clone();
    if !params.is_empty() {
        let query_string: String = params
            .iter()
            .map(|p| format!("{}={}", urlencoding::encode(&p.key), urlencoding::encode(&p.value)))
            .collect::<Vec<_>>()
            .join("&");
        
        if request_url.contains('?') {
            request_url = format!("{}&{}", request_url, query_string);
        } else {
            request_url = format!("{}?{}", request_url, query_string);
        }
    }

    match language.as_str() {
        "curl" => generate_curl(&method, &request_url, &headers, &body, &body_type),
        "python" => generate_python(&method, &request_url, &headers, &body, &body_type),
        "javascript" => generate_javascript(&method, &request_url, &headers, &body, &body_type),
        "go" => generate_go(&method, &request_url, &headers, &body, &body_type),
        "rust" => generate_rust(&method, &request_url, &headers, &body, &body_type),
        "java" => generate_java(&method, &request_url, &headers, &body, &body_type),
        "csharp" => generate_csharp(&method, &request_url, &headers, &body, &body_type),
        "php" => generate_php(&method, &request_url, &headers, &body, &body_type),
        "ruby" => generate_ruby(&method, &request_url, &headers, &body, &body_type),
        _ => format!("// Unsupported language: {}", language),
    }
}

fn generate_curl(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, body_type: &str) -> String {
    let mut parts: Vec<String> = vec!["curl".to_string()];
    
    if method.to_uppercase() != "GET" {
        parts.push(format!("-X {}", method.to_uppercase()));
    }
    
    parts.push(format!("'{}'", url));
    
    for header in headers {
        let escaped_value = header.value.replace("'", "'\\''");
        parts.push(format!("-H '{}: {}'", header.key, escaped_value));
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            let escaped_body = body_content.replace("'", "'\\''");
            parts.push(format!("-d '{}'", escaped_body));
        }
    }
    
    parts.join(" \\\n  ")
}

fn generate_python(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, body_type: &str) -> String {
    let mut code = String::from("import requests\n\n");
    
    code.push_str(&format!("url = \"{}\"\n\n", url));
    
    if !headers.is_empty() {
        code.push_str("headers = {\n");
        for h in headers {
            code.push_str(&format!("    \"{}\": \"{}\",\n", h.key, h.value.replace("\"", "\\\"")));
        }
        code.push_str("}\n\n");
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            if body_type == "json" {
                code.push_str("import json\n\n");
                code.push_str(&format!("data = {}\n\n", body_content));
            } else {
                code.push_str(&format!("data = \"\"\"{}\"\"\"\n\n", body_content));
            }
        }
    }
    
    let method_lower = method.to_lowercase();
    code.push_str(&format!("response = requests.{}(\n", method_lower));
    code.push_str("    url,\n");
    if !headers.is_empty() {
        code.push_str("    headers=headers,\n");
    }
    if body.as_ref().map(|b| !b.is_empty()).unwrap_or(false) {
        if body_type == "json" {
            code.push_str("    json=data,\n");
        } else {
            code.push_str("    data=data,\n");
        }
    }
    code.push_str(")\n\n");
    code.push_str("print(response.status_code)\n");
    code.push_str("print(response.text)");
    
    code
}

fn generate_javascript(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, body_type: &str) -> String {
    let mut code = String::new();
    
    code.push_str(&format!("const url = '{}';\n\n", url));
    
    code.push_str("const options = {\n");
    code.push_str(&format!("  method: '{}',\n", method.to_uppercase()));
    
    if !headers.is_empty() {
        code.push_str("  headers: {\n");
        for h in headers {
            code.push_str(&format!("    '{}': '{}',\n", h.key, h.value.replace("'", "\\'")));
        }
        code.push_str("  },\n");
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            if body_type == "json" {
                code.push_str(&format!("  body: JSON.stringify({}),\n", body_content));
            } else {
                code.push_str(&format!("  body: `{}`,\n", body_content.replace("`", "\\`")));
            }
        }
    }
    
    code.push_str("};\n\n");
    
    code.push_str("fetch(url, options)\n");
    code.push_str("  .then(response => response.json())\n");
    code.push_str("  .then(data => console.log(data))\n");
    code.push_str("  .catch(error => console.error('Error:', error));");
    
    code
}

fn generate_go(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("package main\n\n");
    code.push_str("import (\n");
    code.push_str("    \"fmt\"\n");
    code.push_str("    \"io\"\n");
    code.push_str("    \"net/http\"\n");
    if body.as_ref().map(|b| !b.is_empty()).unwrap_or(false) {
        code.push_str("    \"strings\"\n");
    }
    code.push_str(")\n\n");
    
    code.push_str("func main() {\n");
    code.push_str(&format!("    url := \"{}\"\n\n", url));
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            code.push_str(&format!("    body := strings.NewReader(`{}`)\n", body_content));
            code.push_str(&format!("    req, err := http.NewRequest(\"{}\", url, body)\n", method.to_uppercase()));
        } else {
            code.push_str(&format!("    req, err := http.NewRequest(\"{}\", url, nil)\n", method.to_uppercase()));
        }
    } else {
        code.push_str(&format!("    req, err := http.NewRequest(\"{}\", url, nil)\n", method.to_uppercase()));
    }
    
    code.push_str("    if err != nil {\n");
    code.push_str("        panic(err)\n");
    code.push_str("    }\n\n");
    
    for h in headers {
        code.push_str(&format!("    req.Header.Set(\"{}\", \"{}\")\n", h.key, h.value));
    }
    
    code.push_str("\n    client := &http.Client{}\n");
    code.push_str("    resp, err := client.Do(req)\n");
    code.push_str("    if err != nil {\n");
    code.push_str("        panic(err)\n");
    code.push_str("    }\n");
    code.push_str("    defer resp.Body.Close()\n\n");
    code.push_str("    respBody, _ := io.ReadAll(resp.Body)\n");
    code.push_str("    fmt.Println(string(respBody))\n");
    code.push_str("}");
    
    code
}

fn generate_rust(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("use reqwest;\n\n");
    code.push_str("#[tokio::main]\n");
    code.push_str("async fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
    code.push_str("    let client = reqwest::Client::new();\n\n");
    
    let method_lower = method.to_lowercase();
    code.push_str(&format!("    let response = client.{}(\"{}\")\n", method_lower, url));
    
    for h in headers {
        code.push_str(&format!("        .header(\"{}\", \"{}\")\n", h.key, h.value));
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            code.push_str(&format!("        .body(r#\"{}\"#)\n", body_content));
        }
    }
    
    code.push_str("        .send()\n");
    code.push_str("        .await?;\n\n");
    code.push_str("    println!(\"Status: {}\", response.status());\n");
    code.push_str("    println!(\"Body: {}\", response.text().await?);\n\n");
    code.push_str("    Ok(())\n");
    code.push_str("}");
    
    code
}

fn generate_java(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("import java.net.URI;\n");
    code.push_str("import java.net.http.HttpClient;\n");
    code.push_str("import java.net.http.HttpRequest;\n");
    code.push_str("import java.net.http.HttpResponse;\n\n");
    
    code.push_str("public class Main {\n");
    code.push_str("    public static void main(String[] args) throws Exception {\n");
    code.push_str("        HttpClient client = HttpClient.newHttpClient();\n\n");
    
    code.push_str("        HttpRequest request = HttpRequest.newBuilder()\n");
    code.push_str(&format!("            .uri(URI.create(\"{}\"))\n", url));
    
    for h in headers {
        code.push_str(&format!("            .header(\"{}\", \"{}\")\n", h.key, h.value.replace("\"", "\\\"")));
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            let escaped = body_content.replace("\"", "\\\"").replace("\n", "\\n");
            code.push_str(&format!("            .method(\"{}\", HttpRequest.BodyPublishers.ofString(\"{}\"))\n", method.to_uppercase(), escaped));
        } else {
            code.push_str(&format!("            .method(\"{}\", HttpRequest.BodyPublishers.noBody())\n", method.to_uppercase()));
        }
    } else {
        code.push_str(&format!("            .method(\"{}\", HttpRequest.BodyPublishers.noBody())\n", method.to_uppercase()));
    }
    
    code.push_str("            .build();\n\n");
    code.push_str("        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());\n");
    code.push_str("        System.out.println(response.statusCode());\n");
    code.push_str("        System.out.println(response.body());\n");
    code.push_str("    }\n");
    code.push_str("}");
    
    code
}

fn generate_csharp(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("using System;\n");
    code.push_str("using System.Net.Http;\n");
    code.push_str("using System.Threading.Tasks;\n\n");
    
    code.push_str("class Program\n{\n");
    code.push_str("    static async Task Main()\n    {\n");
    code.push_str("        using var client = new HttpClient();\n\n");
    
    code.push_str(&format!("        var request = new HttpRequestMessage(HttpMethod.{}, \"{}\");\n", 
        method.chars().next().unwrap().to_uppercase().to_string() + &method[1..].to_lowercase(), 
        url));
    
    for h in headers {
        code.push_str(&format!("        request.Headers.Add(\"{}\", \"{}\");\n", h.key, h.value.replace("\"", "\\\"")));
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            let escaped = body_content.replace("\"", "\\\"").replace("\n", "\\n");
            code.push_str(&format!("        request.Content = new StringContent(\"{}\");\n", escaped));
        }
    }
    
    code.push_str("\n        var response = await client.SendAsync(request);\n");
    code.push_str("        var content = await response.Content.ReadAsStringAsync();\n");
    code.push_str("        Console.WriteLine(response.StatusCode);\n");
    code.push_str("        Console.WriteLine(content);\n");
    code.push_str("    }\n");
    code.push_str("}");
    
    code
}

fn generate_php(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("<?php\n\n");
    
    code.push_str("$ch = curl_init();\n\n");
    code.push_str(&format!("curl_setopt($ch, CURLOPT_URL, '{}');\n", url));
    code.push_str("curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);\n");
    
    if method.to_uppercase() != "GET" {
        code.push_str(&format!("curl_setopt($ch, CURLOPT_CUSTOMREQUEST, '{}');\n", method.to_uppercase()));
    }
    
    if !headers.is_empty() {
        code.push_str("\n$headers = [\n");
        for h in headers {
            code.push_str(&format!("    '{}: {}',\n", h.key, h.value.replace("'", "\\'")));
        }
        code.push_str("];\n");
        code.push_str("curl_setopt($ch, CURLOPT_HTTPHEADER, $headers);\n");
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            code.push_str(&format!("\ncurl_setopt($ch, CURLOPT_POSTFIELDS, '{}');\n", body_content.replace("'", "\\'")));
        }
    }
    
    code.push_str("\n$response = curl_exec($ch);\n");
    code.push_str("$httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);\n");
    code.push_str("curl_close($ch);\n\n");
    code.push_str("echo \"Status: $httpCode\\n\";\n");
    code.push_str("echo $response;\n");
    code.push_str("?>");
    
    code
}

fn generate_ruby(method: &str, url: &str, headers: &[SimpleKeyValue], body: &Option<String>, _body_type: &str) -> String {
    let mut code = String::from("require 'net/http'\n");
    code.push_str("require 'uri'\n");
    code.push_str("require 'json'\n\n");
    
    code.push_str(&format!("uri = URI.parse('{}')\n", url));
    code.push_str("http = Net::HTTP.new(uri.host, uri.port)\n");
    code.push_str("http.use_ssl = uri.scheme == 'https'\n\n");
    
    let class_name = match method.to_uppercase().as_str() {
        "GET" => "Get",
        "POST" => "Post",
        "PUT" => "Put",
        "PATCH" => "Patch",
        "DELETE" => "Delete",
        "HEAD" => "Head",
        "OPTIONS" => "Options",
        _ => "Get",
    };
    
    code.push_str(&format!("request = Net::HTTP::{}::new(uri.request_uri)\n", class_name));
    
    for h in headers {
        code.push_str(&format!("request['{}'] = '{}'\n", h.key, h.value.replace("'", "\\'")));
    }
    
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            code.push_str(&format!("request.body = '{}'\n", body_content.replace("'", "\\'")));
        }
    }
    
    code.push_str("\nresponse = http.request(request)\n");
    code.push_str("puts response.code\n");
    code.push_str("puts response.body");
    
    code
}

#[tauri::command]
pub async fn send_http_request(
    method: String,
    url: String,
    headers: HashMap<String, String>,
    params: HashMap<String, String>,
    body: Option<String>,
    body_type: String,
) -> Result<HttpResponse, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // For testing with self-signed certs
        .build()
        .map_err(|e| e.to_string())?;

    // Build URL with query params
    let mut request_url = url.clone();
    if !params.is_empty() {
        let query_string: String = params
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
    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&request_url),
        "POST" => client.post(&request_url),
        "PUT" => client.put(&request_url),
        "PATCH" => client.patch(&request_url),
        "DELETE" => client.delete(&request_url),
        "HEAD" => client.head(&request_url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, &request_url),
        _ => return Err(format!("Unsupported method: {}", method)),
    };

    // Add headers
    for (key, value) in headers {
        request = request.header(&key, &value);
    }

    // Add body
    if let Some(body_content) = body {
        if !body_content.is_empty() {
            match body_type.as_str() {
                "json" => {
                    request = request
                        .header("Content-Type", "application/json")
                        .body(body_content);
                }
                "xml" => {
                    request = request
                        .header("Content-Type", "application/xml")
                        .body(body_content);
                }
                "html" => {
                    request = request
                        .header("Content-Type", "text/html")
                        .body(body_content);
                }
                _ => {
                    request = request.body(body_content);
                }
            }
        }
    }

    // Send request and measure time
    let start = Instant::now();
    let response = request.send().await.map_err(|e| e.to_string())?;
    let elapsed = start.elapsed().as_millis() as u64;

    // Get response details
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    // Get headers
    let mut response_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            response_headers.insert(key.to_string(), v.to_string());
        }
    }

    // Get body
    let body_bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let size = body_bytes.len();
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    Ok(HttpResponse {
        status,
        status_text,
        headers: response_headers,
        body,
        time: elapsed,
        size,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormDataField {
    pub key: String,
    pub value: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(rename = "filePath")]
    pub file_path: Option<String>,
}

#[tauri::command]
pub async fn send_multipart_request(
    method: String,
    url: String,
    headers: HashMap<String, String>,
    params: HashMap<String, String>,
    form_fields: Vec<FormDataField>,
) -> Result<HttpResponse, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;

    // Build URL with query params
    let mut request_url = url.clone();
    if !params.is_empty() {
        let query_string: String = params
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

    // Build multipart form
    let mut form = reqwest::multipart::Form::new();
    
    for field in form_fields {
        if field.field_type == "file" {
            if let Some(file_path) = field.file_path {
                // Read file
                let file_content = tokio::fs::read(&file_path)
                    .await
                    .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
                
                // Get filename from path
                let filename = std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                
                // Detect mime type
                let mime_type = mime_guess::from_path(&file_path)
                    .first_or_octet_stream()
                    .to_string();
                
                let part = reqwest::multipart::Part::bytes(file_content)
                    .file_name(filename)
                    .mime_str(&mime_type)
                    .map_err(|e| e.to_string())?;
                
                form = form.part(field.key, part);
            }
        } else {
            // Text field
            form = form.text(field.key, field.value);
        }
    }

    // Build request
    let mut request = match method.to_uppercase().as_str() {
        "POST" => client.post(&request_url),
        "PUT" => client.put(&request_url),
        "PATCH" => client.patch(&request_url),
        _ => return Err(format!("Multipart form not supported for method: {}", method)),
    };

    // Add headers (except Content-Type which is set by multipart)
    for (key, value) in headers {
        if key.to_lowercase() != "content-type" {
            request = request.header(&key, &value);
        }
    }

    // Add multipart form
    request = request.multipart(form);

    // Send request and measure time
    let start = Instant::now();
    let response = request.send().await.map_err(|e| e.to_string())?;
    let elapsed = start.elapsed().as_millis() as u64;

    // Get response details
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    // Get headers
    let mut response_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            response_headers.insert(key.to_string(), v.to_string());
        }
    }

    // Get body
    let body_bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let size = body_bytes.len();
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    Ok(HttpResponse {
        status,
        status_text,
        headers: response_headers,
        body,
        time: elapsed,
        size,
    })
}
