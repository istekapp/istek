use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLResponse {
    pub data: Option<Value>,
    pub errors: Option<Vec<GraphQLError>>,
    pub time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    pub locations: Option<Vec<GraphQLLocation>>,
    pub path: Option<Vec<Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLLocation {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphQLRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "operationName")]
    operation_name: Option<String>,
}

#[tauri::command]
pub async fn send_graphql_request(
    url: String,
    headers: HashMap<String, String>,
    query: String,
    variables: Option<String>,
    operation_name: Option<String>,
) -> Result<GraphQLResponse, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;

    // Parse variables if provided
    let variables_json: Option<Value> = match variables {
        Some(v) if !v.is_empty() => {
            Some(serde_json::from_str(&v).map_err(|e| format!("Invalid variables JSON: {}", e))?)
        }
        _ => None,
    };

    let operation = if operation_name.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        None
    } else {
        operation_name
    };

    let gql_request = GraphQLRequest {
        query,
        variables: variables_json,
        operation_name: operation,
    };

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json");

    // Add custom headers
    for (key, value) in headers {
        request = request.header(&key, &value);
    }

    let start = Instant::now();
    let response = request
        .json(&gql_request)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let elapsed = start.elapsed().as_millis() as u64;

    let body: Value = response.json().await.map_err(|e| e.to_string())?;

    let data = body.get("data").cloned();
    let errors: Option<Vec<GraphQLError>> = body
        .get("errors")
        .and_then(|e| serde_json::from_value(e.clone()).ok());

    Ok(GraphQLResponse {
        data,
        errors,
        time: elapsed,
    })
}

// Introspection query for schema discovery
pub const INTROSPECTION_QUERY: &str = r#"
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      ...FullType
    }
  }
}

fragment FullType on __Type {
  kind
  name
  description
  fields(includeDeprecated: true) {
    name
    description
    args {
      ...InputValue
    }
    type {
      ...TypeRef
    }
    isDeprecated
    deprecationReason
  }
  inputFields {
    ...InputValue
  }
  interfaces {
    ...TypeRef
  }
  enumValues(includeDeprecated: true) {
    name
    description
    isDeprecated
    deprecationReason
  }
  possibleTypes {
    ...TypeRef
  }
}

fragment InputValue on __InputValue {
  name
  description
  type { ...TypeRef }
  defaultValue
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
        }
      }
    }
  }
}
"#;
