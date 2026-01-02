use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tonic::transport::Channel;

// For reflection
use tonic_reflection::pb::v1::{
    server_reflection_client::ServerReflectionClient,
    server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse,
    ServerReflectionRequest, ServerReflectionResponse,
};

use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorProto, ServiceDescriptorProto};

/// gRPC service info discovered via reflection or proto file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcServiceInfo {
    pub name: String,
    pub full_name: String,
    pub methods: Vec<GrpcMethodInfo>,
}

/// gRPC method info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcMethodInfo {
    pub name: String,
    pub full_name: String,
    pub input_type: String,
    pub output_type: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    /// JSON schema for input message
    pub input_schema: Option<serde_json::Value>,
}

/// Result of discovering services
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcDiscoveryResult {
    pub success: bool,
    pub services: Vec<GrpcServiceInfo>,
    pub error: Option<String>,
    pub source: String, // "reflection" or "proto"
}

/// gRPC call response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcCallResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub status_code: i32,
    pub status_message: String,
    pub metadata: HashMap<String, String>,
    pub time_ms: u64,
}

/// Discover services using gRPC server reflection
#[tauri::command]
pub async fn grpc_discover_services(url: String) -> Result<GrpcDiscoveryResult, String> {
    // Normalize URL - gRPC uses http:// for plaintext, https:// for TLS
    let url = url
        .trim()
        .replace("grpc://", "http://")
        .replace("grpcs://", "https://");
    
    let url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("http://{}", url)
    } else {
        url
    };
    
    // Connect to the server with timeout
    let channel = Channel::from_shared(url.clone())
        .map_err(|e| format!("Invalid URL: {}", e))?
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .connect()
        .await
        .map_err(|e| format!("Failed to connect to {}: {}. Make sure the server is running and the port is correct.", url, e))?;
    
    // Create reflection client
    let mut client = ServerReflectionClient::new(channel);
    
    // First, list all services
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    
    let outbound = tokio_stream::once(request);
    let response: tonic::Response<tonic::Streaming<ServerReflectionResponse>> = client
        .server_reflection_info(outbound)
        .await
        .map_err(|e| format!("Reflection not available: {}", e))?;
    
    let mut stream = response.into_inner();
    
    // Get the response
    use tokio_stream::StreamExt;
    let first_response = stream
        .next()
        .await
        .ok_or_else(|| "No response from reflection".to_string())?
        .map_err(|e| format!("Failed to get services: {}", e))?;
    
    let service_names: Vec<String> = match first_response.message_response {
        Some(MessageResponse::ListServicesResponse(list)) => {
            list.service
                .into_iter()
                .map(|s| s.name)
                // Filter out the reflection service itself
                .filter(|name| !name.contains("grpc.reflection"))
                .collect()
        }
        _ => return Err("Unexpected reflection response".to_string()),
    };
    
    // Now get details for each service
    let mut services = Vec::new();
    
    for service_name in service_names {
        // Request file descriptor for this service
        let channel = Channel::from_shared(url.clone())
            .map_err(|e| format!("Invalid URL: {}", e))?
            .connect()
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        let mut client = ServerReflectionClient::new(channel);
        
        let request = ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::FileContainingSymbol(service_name.clone())),
        };
        
        let outbound = tokio_stream::once(request);
        let response: tonic::Response<tonic::Streaming<ServerReflectionResponse>> = client
            .server_reflection_info(outbound)
            .await
            .map_err(|e| format!("Failed to get service info: {}", e))?;
        
        let mut stream = response.into_inner();
        
        if let Some(Ok(resp)) = stream.next().await {
            if let Some(MessageResponse::FileDescriptorResponse(fd_response)) = resp.message_response {
                for fd_bytes in fd_response.file_descriptor_proto {
                    if let Ok(fd) = FileDescriptorProto::decode(&fd_bytes[..]) {
                        // Find the service in this file descriptor
                        for service in &fd.service {
                            let svc_name = service.name.clone().unwrap_or_default();
                            let full_name = if fd.package.is_some() {
                                format!("{}.{}", fd.package.as_ref().unwrap(), svc_name)
                            } else {
                                svc_name.clone()
                            };
                            
                            if full_name == service_name || svc_name == service_name {
                                let methods = extract_methods(service, &fd);
                                services.push(GrpcServiceInfo {
                                    name: svc_name,
                                    full_name,
                                    methods,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(GrpcDiscoveryResult {
        success: true,
        services,
        error: None,
        source: "reflection".to_string(),
    })
}

/// Parse proto file content and extract services
#[tauri::command]
pub async fn grpc_parse_proto(proto_content: String) -> Result<GrpcDiscoveryResult, String> {
    // For now, we'll do basic parsing. In production, you'd use protoc or prost-build
    // This is a simplified parser that handles common proto3 syntax
    
    let services = parse_proto_content(&proto_content)?;
    
    Ok(GrpcDiscoveryResult {
        success: true,
        services,
        error: None,
        source: "proto".to_string(),
    })
}

/// Make a unary gRPC call
#[tauri::command]
pub async fn grpc_call(
    url: String,
    service: String,
    method: String,
    message: String,
    metadata: HashMap<String, String>,
) -> Result<GrpcCallResponse, String> {
    let start = std::time::Instant::now();
    
    // Normalize URL - gRPC uses http:// for plaintext, https:// for TLS
    let url = url
        .trim()
        .replace("grpc://", "http://")
        .replace("grpcs://", "https://");
    
    let url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("http://{}", url)
    } else {
        url
    };
    
    // Parse the JSON message
    let request_json: serde_json::Value = serde_json::from_str(&message)
        .map_err(|e| format!("Invalid JSON message: {}", e))?;
    
    // Connect to the server
    let channel = Channel::from_shared(url.clone())
        .map_err(|e| format!("Invalid URL: {}", e))?
        .connect()
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;
    
    // We need to use reflection to make dynamic calls
    // First, get the file descriptor for this method
    let mut reflection_client = ServerReflectionClient::new(channel.clone());
    
    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::FileContainingSymbol(service.clone())),
    };
    
    use tokio_stream::StreamExt;
    let outbound = tokio_stream::once(request);
    let response: tonic::Response<tonic::Streaming<ServerReflectionResponse>> = reflection_client
        .server_reflection_info(outbound)
        .await
        .map_err(|e| format!("Reflection error: {}", e))?;
    
    let mut stream = response.into_inner();
    
    let first_response = stream
        .next()
        .await
        .ok_or_else(|| "No response from reflection".to_string())?
        .map_err(|e| format!("Failed to get method info: {}", e))?;
    
    // Get file descriptors as raw bytes for later use
    let fd_bytes_list = match &first_response.message_response {
        Some(MessageResponse::FileDescriptorResponse(fd_response)) => {
            fd_response.file_descriptor_proto.clone()
        }
        _ => return Err("Failed to get file descriptors".to_string()),
    };
    
    // Parse file descriptors
    let mut file_descriptors = Vec::new();
    for fd_bytes in &fd_bytes_list {
        if let Ok(fd) = FileDescriptorProto::decode(&fd_bytes[..]) {
            file_descriptors.push(fd);
        }
    }
    
    // Find the method and its input/output types
    let mut input_type_name = None;
    let mut output_type_name = None;
    let mut input_message_descriptor: Option<DescriptorProto> = None;
    let mut output_message_descriptor: Option<DescriptorProto> = None;
    
    for fd in &file_descriptors {
        for svc in &fd.service {
            let svc_full_name = if let Some(pkg) = &fd.package {
                format!("{}.{}", pkg, svc.name.as_ref().unwrap_or(&String::new()))
            } else {
                svc.name.clone().unwrap_or_default()
            };
            
            if svc_full_name == service || svc.name.as_ref() == Some(&service) {
                for m in &svc.method {
                    if m.name.as_ref() == Some(&method) {
                        input_type_name = m.input_type.clone();
                        output_type_name = m.output_type.clone();
                        break;
                    }
                }
            }
        }
    }
    
    let input_type = input_type_name.ok_or_else(|| format!("Method {} not found", method))?;
    let output_type = output_type_name.ok_or_else(|| format!("Output type not found for method {}", method))?;
    
    // Find message descriptors
    for fd in &file_descriptors {
        if let Some(msg) = find_message(&input_type, fd) {
            input_message_descriptor = Some(msg.clone());
        }
        if let Some(msg) = find_message(&output_type, fd) {
            output_message_descriptor = Some(msg.clone());
        }
    }
    
    let input_msg = input_message_descriptor.ok_or_else(|| format!("Input message type {} not found", input_type))?;
    let output_msg = output_message_descriptor.ok_or_else(|| format!("Output message type {} not found", output_type))?;
    
    // Encode the JSON message to protobuf
    let request_bytes = json_to_protobuf(&request_json, &input_msg)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    
    // Make the actual gRPC call using hyper
    let path = format!("/{}/{}", service, method);
    
    // Use hyper for raw gRPC call
    let response = make_raw_grpc_call(&url, &path, request_bytes, metadata).await?;
    
    // Decode protobuf response(s) to JSON
    // For streaming responses, we may have multiple messages
    let response_data = if response.messages.is_empty() {
        serde_json::json!({})
    } else if response.messages.len() == 1 {
        // Single message (unary response)
        protobuf_to_json(&response.messages[0], &output_msg)
            .unwrap_or_else(|e| serde_json::json!({"error": format!("Failed to decode response: {}", e)}))
    } else {
        // Multiple messages (streaming response) - return as array
        let decoded_messages: Vec<serde_json::Value> = response.messages
            .iter()
            .map(|msg| {
                protobuf_to_json(msg, &output_msg)
                    .unwrap_or_else(|e| serde_json::json!({"error": format!("Failed to decode: {}", e)}))
            })
            .collect();
        serde_json::json!(decoded_messages)
    };
    
    let elapsed = start.elapsed().as_millis() as u64;
    
    Ok(GrpcCallResponse {
        success: response.status_code == 0,
        data: Some(response_data),
        error: if response.status_code != 0 { Some(response.status_message.clone()) } else { None },
        status_code: response.status_code,
        status_message: response.status_message,
        metadata: response.metadata,
        time_ms: elapsed,
    })
}

/// Extract methods from a service descriptor
fn extract_methods(service: &ServiceDescriptorProto, fd: &FileDescriptorProto) -> Vec<GrpcMethodInfo> {
    service
        .method
        .iter()
        .map(|m| {
            let name = m.name.clone().unwrap_or_default();
            let full_name = format!(
                "{}.{}.{}",
                fd.package.as_ref().unwrap_or(&String::new()),
                service.name.as_ref().unwrap_or(&String::new()),
                name
            );
            
            let input_type = m.input_type.clone().unwrap_or_default();
            let output_type = m.output_type.clone().unwrap_or_default();
            
            // Try to generate input schema from message definitions
            let input_schema = generate_message_schema(&input_type, fd);
            
            GrpcMethodInfo {
                name,
                full_name,
                input_type,
                output_type,
                client_streaming: m.client_streaming.unwrap_or(false),
                server_streaming: m.server_streaming.unwrap_or(false),
                input_schema,
            }
        })
        .collect()
}

/// Generate JSON schema for a message type
fn generate_message_schema(type_name: &str, fd: &FileDescriptorProto) -> Option<serde_json::Value> {
    // Strip leading dot if present
    let type_name = type_name.strip_prefix('.').unwrap_or(type_name);
    
    // Find the message in the file descriptor
    let msg = find_message(type_name, fd)?;
    
    let mut properties = serde_json::Map::new();
    
    for field in &msg.field {
        let field_name = field.name.clone().unwrap_or_default();
        let field_type = proto_type_to_json_type(field.r#type());
        
        properties.insert(field_name, serde_json::json!({
            "type": field_type
        }));
    }
    
    Some(serde_json::json!({
        "type": "object",
        "properties": properties
    }))
}

/// Find a message definition by name
fn find_message<'a>(name: &str, fd: &'a FileDescriptorProto) -> Option<&'a DescriptorProto> {
    let simple_name = name.rsplit('.').next().unwrap_or(name);
    
    fd.message_type.iter().find(|m| {
        m.name.as_ref() == Some(&simple_name.to_string()) ||
        m.name.as_ref().map(|n| name.ends_with(n)).unwrap_or(false)
    })
}

/// Convert proto field type to JSON schema type
fn proto_type_to_json_type(t: prost_types::field_descriptor_proto::Type) -> &'static str {
    use prost_types::field_descriptor_proto::Type;
    match t {
        Type::Double | Type::Float => "number",
        Type::Int64 | Type::Uint64 | Type::Int32 | Type::Fixed64 |
        Type::Fixed32 | Type::Uint32 | Type::Sfixed32 | Type::Sfixed64 |
        Type::Sint32 | Type::Sint64 => "integer",
        Type::Bool => "boolean",
        Type::String => "string",
        Type::Bytes => "string", // base64 encoded
        Type::Message | Type::Group => "object",
        Type::Enum => "string",
    }
}

/// Simple proto file parser (for basic cases)
fn parse_proto_content(content: &str) -> Result<Vec<GrpcServiceInfo>, String> {
    let mut services = Vec::new();
    let mut current_package = String::new();
    
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i].trim();
        
        // Parse package
        if line.starts_with("package ") {
            current_package = line
                .strip_prefix("package ")
                .unwrap_or("")
                .trim_end_matches(';')
                .trim()
                .to_string();
        }
        
        // Parse service
        if line.starts_with("service ") {
            let service_name = line
                .strip_prefix("service ")
                .unwrap_or("")
                .split('{')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            
            let full_name = if current_package.is_empty() {
                service_name.clone()
            } else {
                format!("{}.{}", current_package, service_name)
            };
            
            let mut methods = Vec::new();
            
            // Parse methods until we hit closing brace
            i += 1;
            let mut brace_count = 1;
            
            while i < lines.len() && brace_count > 0 {
                let method_line = lines[i].trim();
                
                if method_line.contains('{') {
                    brace_count += method_line.matches('{').count();
                }
                if method_line.contains('}') {
                    brace_count -= method_line.matches('}').count();
                }
                
                // Parse rpc method
                if method_line.starts_with("rpc ") {
                    if let Some(method) = parse_rpc_line(method_line, &full_name) {
                        methods.push(method);
                    }
                }
                
                i += 1;
            }
            
            services.push(GrpcServiceInfo {
                name: service_name,
                full_name,
                methods,
            });
            continue;
        }
        
        i += 1;
    }
    
    Ok(services)
}

/// Parse a single rpc line
fn parse_rpc_line(line: &str, service_full_name: &str) -> Option<GrpcMethodInfo> {
    // rpc MethodName (InputType) returns (OutputType);
    // rpc MethodName (stream InputType) returns (stream OutputType);
    
    let line = line.strip_prefix("rpc ")?.trim();
    
    // Get method name
    let parts: Vec<&str> = line.splitn(2, '(').collect();
    if parts.len() < 2 {
        return None;
    }
    
    let method_name = parts[0].trim().to_string();
    let rest = parts[1];
    
    // Parse input
    let input_parts: Vec<&str> = rest.splitn(2, ')').collect();
    if input_parts.len() < 2 {
        return None;
    }
    
    let input_str = input_parts[0].trim();
    let client_streaming = input_str.starts_with("stream ");
    let input_type = input_str
        .strip_prefix("stream ")
        .unwrap_or(input_str)
        .trim()
        .to_string();
    
    // Parse output
    let output_section = input_parts[1];
    let returns_idx = output_section.find("returns")?;
    let output_str = output_section[returns_idx..]
        .strip_prefix("returns")?
        .trim()
        .trim_start_matches('(')
        .split(')')
        .next()?
        .trim();
    
    let server_streaming = output_str.starts_with("stream ");
    let output_type = output_str
        .strip_prefix("stream ")
        .unwrap_or(output_str)
        .trim()
        .to_string();
    
    Some(GrpcMethodInfo {
        name: method_name.clone(),
        full_name: format!("{}.{}", service_full_name, method_name),
        input_type,
        output_type,
        client_streaming,
        server_streaming,
        input_schema: None,
    })
}

/// Raw gRPC call response
struct RawGrpcResponse {
    /// For unary calls, this contains a single message. For streaming, multiple messages.
    messages: Vec<Vec<u8>>,
    status_code: i32,
    status_message: String,
    metadata: HashMap<String, String>,
}

/// Convert JSON to protobuf bytes using the message descriptor
fn json_to_protobuf(json: &serde_json::Value, msg_desc: &DescriptorProto) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    
    if let serde_json::Value::Object(obj) = json {
        for field in &msg_desc.field {
            let field_name = field.name.clone().unwrap_or_default();
            let field_number = field.number.unwrap_or(0) as u32;
            
            if let Some(value) = obj.get(&field_name) {
                encode_field(&mut buf, field_number, value, field)?;
            }
        }
    }
    
    Ok(buf)
}

/// Encode a single protobuf field
fn encode_field(buf: &mut Vec<u8>, field_number: u32, value: &serde_json::Value, field: &prost_types::FieldDescriptorProto) -> Result<(), String> {
    use prost_types::field_descriptor_proto::Type;
    
    let field_type = field.r#type();
    
    match field_type {
        Type::String => {
            if let serde_json::Value::String(s) = value {
                // Wire type 2 (length-delimited)
                let tag = (field_number << 3) | 2;
                encode_varint(buf, tag as u64);
                encode_varint(buf, s.len() as u64);
                buf.extend_from_slice(s.as_bytes());
            }
        }
        Type::Int32 | Type::Int64 | Type::Uint32 | Type::Uint64 | Type::Sint32 | Type::Sint64 => {
            if let Some(n) = value.as_i64() {
                // Wire type 0 (varint)
                let tag = (field_number << 3) | 0;
                encode_varint(buf, tag as u64);
                encode_varint(buf, n as u64);
            }
        }
        Type::Bool => {
            if let serde_json::Value::Bool(b) = value {
                // Wire type 0 (varint)
                let tag = (field_number << 3) | 0;
                encode_varint(buf, tag as u64);
                encode_varint(buf, if *b { 1 } else { 0 });
            }
        }
        Type::Double => {
            if let Some(n) = value.as_f64() {
                // Wire type 1 (64-bit)
                let tag = (field_number << 3) | 1;
                encode_varint(buf, tag as u64);
                buf.extend_from_slice(&n.to_le_bytes());
            }
        }
        Type::Float => {
            if let Some(n) = value.as_f64() {
                // Wire type 5 (32-bit)
                let tag = (field_number << 3) | 5;
                encode_varint(buf, tag as u64);
                buf.extend_from_slice(&(n as f32).to_le_bytes());
            }
        }
        Type::Bytes => {
            if let serde_json::Value::String(s) = value {
                // Assume base64 encoded
                let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
                    .unwrap_or_else(|_| s.as_bytes().to_vec());
                let tag = (field_number << 3) | 2;
                encode_varint(buf, tag as u64);
                encode_varint(buf, bytes.len() as u64);
                buf.extend_from_slice(&bytes);
            }
        }
        _ => {
            // For unsupported types, try to handle as string
            if let serde_json::Value::String(s) = value {
                let tag = (field_number << 3) | 2;
                encode_varint(buf, tag as u64);
                encode_varint(buf, s.len() as u64);
                buf.extend_from_slice(s.as_bytes());
            }
        }
    }
    
    Ok(())
}

/// Encode a varint
fn encode_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Convert protobuf bytes to JSON using the message descriptor
fn protobuf_to_json(data: &[u8], msg_desc: &DescriptorProto) -> Result<serde_json::Value, String> {
    let mut result = serde_json::Map::new();
    let mut pos = 0;
    
    while pos < data.len() {
        // Read tag
        let (tag, new_pos) = decode_varint(data, pos).ok_or("Failed to decode tag")?;
        pos = new_pos;
        
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u8;
        
        // Find field descriptor
        let field = msg_desc.field.iter().find(|f| f.number == Some(field_number as i32));
        
        let (value, new_pos) = decode_field(data, pos, wire_type, field)?;
        pos = new_pos;
        
        if let Some(f) = field {
            let field_name = f.name.clone().unwrap_or_else(|| format!("field_{}", field_number));
            result.insert(field_name, value);
        }
    }
    
    Ok(serde_json::Value::Object(result))
}

/// Decode a single field from protobuf
fn decode_field(data: &[u8], pos: usize, wire_type: u8, field: Option<&prost_types::FieldDescriptorProto>) -> Result<(serde_json::Value, usize), String> {
    match wire_type {
        0 => {
            // Varint
            let (value, new_pos) = decode_varint(data, pos).ok_or("Failed to decode varint")?;
            
            let json_value = if let Some(f) = field {
                use prost_types::field_descriptor_proto::Type;
                match f.r#type() {
                    Type::Bool => serde_json::Value::Bool(value != 0),
                    _ => serde_json::json!(value as i64),
                }
            } else {
                serde_json::json!(value as i64)
            };
            
            Ok((json_value, new_pos))
        }
        1 => {
            // 64-bit (double, fixed64, sfixed64)
            if pos + 8 > data.len() {
                return Err("Not enough data for 64-bit field".to_string());
            }
            let bytes: [u8; 8] = data[pos..pos + 8].try_into().unwrap();
            let value = f64::from_le_bytes(bytes);
            Ok((serde_json::json!(value), pos + 8))
        }
        2 => {
            // Length-delimited (string, bytes, embedded message)
            let (length, new_pos) = decode_varint(data, pos).ok_or("Failed to decode length")?;
            let length = length as usize;
            
            if new_pos + length > data.len() {
                return Err("Not enough data for length-delimited field".to_string());
            }
            
            let bytes = &data[new_pos..new_pos + length];
            
            // Try to decode as UTF-8 string first
            let value = if let Ok(s) = std::str::from_utf8(bytes) {
                serde_json::Value::String(s.to_string())
            } else {
                // Fall back to base64
                serde_json::Value::String(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes))
            };
            
            Ok((value, new_pos + length))
        }
        5 => {
            // 32-bit (float, fixed32, sfixed32)
            if pos + 4 > data.len() {
                return Err("Not enough data for 32-bit field".to_string());
            }
            let bytes: [u8; 4] = data[pos..pos + 4].try_into().unwrap();
            let value = f32::from_le_bytes(bytes);
            Ok((serde_json::json!(value as f64), pos + 4))
        }
        _ => {
            Err(format!("Unsupported wire type: {}", wire_type))
        }
    }
}

/// Decode a varint from bytes
fn decode_varint(data: &[u8], start: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    let mut pos = start;
    
    loop {
        if pos >= data.len() {
            return None;
        }
        
        let byte = data[pos];
        result |= ((byte & 0x7F) as u64) << shift;
        pos += 1;
        
        if byte & 0x80 == 0 {
            break;
        }
        
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    
    Some((result, pos))
}

/// Make a raw gRPC call using hyper
async fn make_raw_grpc_call(
    url: &str,
    path: &str,
    body: Vec<u8>,
    metadata: HashMap<String, String>,
) -> Result<RawGrpcResponse, String> {
    use hyper::Request;
    use hyper::body::Bytes;
    use http_body_util::{Full, BodyExt};
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;
    
    // Parse URL
    let uri: hyper::Uri = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
    let scheme = uri.scheme_str().unwrap_or("http");
    let host = uri.host().ok_or("No host in URL")?;
    let port = uri.port_u16().unwrap_or(if scheme == "https" { 443 } else { 80 });
    
    let full_uri = format!("{}://{}:{}{}", scheme, host, port, path);
    
    // Create gRPC frame: 1 byte compressed flag (0) + 4 bytes message length + message
    let mut grpc_frame = Vec::with_capacity(5 + body.len());
    grpc_frame.push(0u8); // not compressed
    grpc_frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    grpc_frame.extend_from_slice(&body);
    
    // Build request - use application/grpc+proto for binary protobuf
    let mut request_builder = Request::builder()
        .method("POST")
        .uri(&full_uri)
        .header("content-type", "application/grpc+proto")
        .header("te", "trailers")
        .header("grpc-accept-encoding", "identity");
    
    // Add custom metadata
    for (key, value) in &metadata {
        request_builder = request_builder.header(key.as_str(), value.as_str());
    }
    
    let request = request_builder
        .body(Full::new(Bytes::from(grpc_frame)))
        .map_err(|e| format!("Failed to build request: {}", e))?;
    
    // Create client with HTTP/2
    let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new())
        .http2_only(true)
        .build_http();
    
    // Send request
    let response = client
        .request(request)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Get status from headers/trailers
    let grpc_status = response
        .headers()
        .get("grpc-status")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    
    let grpc_message = response
        .headers()
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("OK")
        .to_string();
    
    // Collect response metadata
    let mut response_metadata = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            response_metadata.insert(key.to_string(), v.to_string());
        }
    }
    
    // Read body
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?
        .to_bytes();
    
    // Parse all gRPC frames from the response
    // Each frame: 1 byte compression flag + 4 bytes length (big-endian) + message
    let messages = parse_grpc_frames(&body_bytes);
    
    Ok(RawGrpcResponse {
        messages,
        status_code: grpc_status,
        status_message: grpc_message,
        metadata: response_metadata,
    })
}

/// Parse multiple gRPC frames from response body
fn parse_grpc_frames(data: &[u8]) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut pos = 0;
    
    while pos + 5 <= data.len() {
        // Read compression flag (1 byte) - we ignore it for now
        let _compressed = data[pos];
        pos += 1;
        
        // Read message length (4 bytes, big-endian)
        if pos + 4 > data.len() {
            break;
        }
        let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        
        // Read message
        if pos + length > data.len() {
            break;
        }
        
        messages.push(data[pos..pos + length].to_vec());
        pos += length;
    }
    
    messages
}
