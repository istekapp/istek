use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use aws_sdk_sts as sts;
use aws_sdk_secretsmanager as secretsmanager;
use aws_credential_types::Credentials;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretProviderConfig {
    pub id: String,
    pub name: String,
    pub provider_type: SecretProviderType,
    pub config: ProviderSpecificConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SecretProviderType {
    Aws,
    Gcp,
    Azure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSpecificConfig {
    // AWS
    pub aws_region: Option<String>,
    pub aws_access_key_id: Option<String>,
    pub aws_secret_access_key: Option<String>,
    pub aws_secret_name: Option<String>,
    
    // GCP
    pub gcp_project_id: Option<String>,
    pub gcp_credentials_json: Option<String>,
    pub gcp_secret_name: Option<String>,
    
    // Azure
    pub azure_vault_url: Option<String>,
    pub azure_tenant_id: Option<String>,
    pub azure_client_id: Option<String>,
    pub azure_client_secret: Option<String>,
    pub azure_secret_name: Option<String>,
    
    // HashiCorp Vault
    pub vault_address: Option<String>,
    pub vault_token: Option<String>,
    pub vault_mount_path: Option<String>,
    pub vault_namespace: Option<String>,
    pub vault_secret_path: Option<String>,
    
    // 1Password
    pub onepassword_service_account_token: Option<String>,
    pub onepassword_vault_id: Option<String>,
    pub onepassword_item_name: Option<String>,
    
    // Bitwarden
    pub bitwarden_server_url: Option<String>,
    pub bitwarden_api_key: Option<String>,
    pub bitwarden_organization_id: Option<String>,
    pub bitwarden_item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchSecretsResult {
    pub success: bool,
    pub secrets: Vec<SecretValue>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTestResult {
    pub success: bool,
    pub message: String,
    pub identity: Option<String>,
}

// ============ AUTH TEST FUNCTIONS ============

// AWS STS GetCallerIdentity using AWS SDK
#[tauri::command]
pub async fn test_aws_auth(
    region: String,
    access_key_id: String,
    secret_access_key: String,
) -> Result<AuthTestResult, String> {
    // Create credentials
    let credentials = Credentials::new(
        &access_key_id,
        &secret_access_key,
        None, // session token
        None, // expiry
        "istek-provider",
    );
    
    // Build config with explicit credentials
    let region_provider = aws_config::Region::new(region.clone());
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .credentials_provider(credentials)
        .load()
        .await;
    
    // Create STS client
    let sts_client = sts::Client::new(&config);
    
    // Call GetCallerIdentity
    match sts_client.get_caller_identity().send().await {
        Ok(response) => {
            let arn = response.arn().map(|s| s.to_string());
            let account = response.account().map(|s| s.to_string());
            let user_id = response.user_id().map(|s| s.to_string());
            
            let identity = match (arn.as_ref(), account.as_ref()) {
                (Some(a), Some(acc)) => Some(format!("{} (Account: {})", a, acc)),
                (Some(a), None) => Some(a.clone()),
                _ => user_id,
            };
            
            Ok(AuthTestResult {
                success: true,
                message: "AWS credentials are valid".to_string(),
                identity,
            })
        }
        Err(e) => {
            Ok(AuthTestResult {
                success: false,
                message: format!("AWS authentication failed: {}", e),
                identity: None,
            })
        }
    }
}

// GCP Auth Test - validate service account
#[tauri::command]
pub async fn test_gcp_auth(
    project_id: String,
    credentials_json: String,
) -> Result<AuthTestResult, String> {
    // Parse credentials
    let creds: serde_json::Value = serde_json::from_str(&credentials_json)
        .map_err(|e| format!("Invalid GCP credentials JSON: {}", e))?;
    
    let client_email = creds["client_email"]
        .as_str()
        .ok_or("Missing client_email in credentials")?;
    let private_key = creds["private_key"]
        .as_str()
        .ok_or("Missing private_key in credentials")?;
    
    // Create JWT
    let now = chrono::Utc::now().timestamp();
    let exp = now + 3600;
    
    let header = base64_url_encode(&serde_json::json!({
        "alg": "RS256",
        "typ": "JWT"
    }).to_string());
    
    let claims = base64_url_encode(&serde_json::json!({
        "iss": client_email,
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "aud": "https://oauth2.googleapis.com/token",
        "iat": now,
        "exp": exp
    }).to_string());
    
    let signing_input = format!("{}.{}", header, claims);
    let signature = sign_rs256(&signing_input, private_key)?;
    let jwt = format!("{}.{}", signing_input, signature);
    
    // Exchange JWT for access token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = token_response.status();
    let token_body: serde_json::Value = token_response.json().await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;
    
    if status.is_success() && token_body["access_token"].is_string() {
        Ok(AuthTestResult {
            success: true,
            message: "GCP credentials are valid".to_string(),
            identity: Some(format!("{} ({})", client_email, project_id)),
        })
    } else {
        let error = token_body["error_description"]
            .as_str()
            .unwrap_or("Authentication failed");
        Ok(AuthTestResult {
            success: false,
            message: format!("GCP authentication failed: {}", error),
            identity: None,
        })
    }
}

// Azure Auth Test - get access token
#[tauri::command]
pub async fn test_azure_auth(
    vault_url: String,
    tenant_id: String,
    client_id: String,
    client_secret: String,
) -> Result<AuthTestResult, String> {
    let client = reqwest::Client::new();
    
    let token_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        tenant_id
    );
    
    let token_response = client
        .post(&token_url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("scope", "https://vault.azure.net/.default"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = token_response.status();
    let token_body: serde_json::Value = token_response.json().await
        .map_err(|e| format!("Failed to parse Azure token response: {}", e))?;
    
    if status.is_success() && token_body["access_token"].is_string() {
        Ok(AuthTestResult {
            success: true,
            message: "Azure credentials are valid".to_string(),
            identity: Some(format!("Client: {} (Tenant: {})", client_id, tenant_id)),
        })
    } else {
        let error = token_body["error_description"]
            .as_str()
            .unwrap_or("Authentication failed");
        Ok(AuthTestResult {
            success: false,
            message: format!("Azure authentication failed: {}", error),
            identity: None,
        })
    }
}

// Vault Auth Test - check token validity
#[tauri::command]
pub async fn test_vault_auth(
    address: String,
    token: String,
    namespace: Option<String>,
) -> Result<AuthTestResult, String> {
    let client = reqwest::Client::new();
    
    let url = format!("{}/v1/auth/token/lookup-self", address.trim_end_matches('/'));
    
    let mut request = client
        .get(&url)
        .header("X-Vault-Token", &token);
    
    if let Some(ns) = namespace {
        if !ns.is_empty() {
            request = request.header("X-Vault-Namespace", ns);
        }
    }
    
    let response = request.send().await.map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse Vault response: {}", e))?;
    
    if status.is_success() {
        let display_name = body["data"]["display_name"]
            .as_str()
            .unwrap_or("unknown");
        Ok(AuthTestResult {
            success: true,
            message: "Vault token is valid".to_string(),
            identity: Some(display_name.to_string()),
        })
    } else {
        let errors = body["errors"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| "Authentication failed".to_string());
        Ok(AuthTestResult {
            success: false,
            message: format!("Vault authentication failed: {}", errors),
            identity: None,
        })
    }
}

// 1Password Auth Test
#[tauri::command]
pub async fn test_1password_auth(
    service_account_token: String,
    vault_id: String,
) -> Result<AuthTestResult, String> {
    let client = reqwest::Client::new();
    
    // Try to list vaults to verify token
    let url = format!("https://api.1password.com/v1/vaults/{}", vault_id);
    
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", service_account_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    
    if status.is_success() {
        let vault_name = body["name"].as_str().unwrap_or("unknown");
        Ok(AuthTestResult {
            success: true,
            message: "1Password credentials are valid".to_string(),
            identity: Some(format!("Vault: {}", vault_name)),
        })
    } else {
        Ok(AuthTestResult {
            success: false,
            message: format!("1Password authentication failed: {}", status),
            identity: None,
        })
    }
}

// Bitwarden Auth Test
#[tauri::command]
pub async fn test_bitwarden_auth(
    server_url: String,
    api_key: String,
) -> Result<AuthTestResult, String> {
    let client = reqwest::Client::new();
    
    let base_url = if server_url.is_empty() {
        "https://api.bitwarden.com".to_string()
    } else {
        server_url.trim_end_matches('/').to_string()
    };
    
    // Try to access the API
    let url = format!("{}/organizations", base_url);
    
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    
    if status.is_success() || status.as_u16() == 404 {
        // 404 means auth worked but no orgs found - still valid
        Ok(AuthTestResult {
            success: true,
            message: "Bitwarden credentials are valid".to_string(),
            identity: None,
        })
    } else {
        Ok(AuthTestResult {
            success: false,
            message: format!("Bitwarden authentication failed: {}", status),
            identity: None,
        })
    }
}

// Generic test provider auth - routes to specific provider
#[tauri::command]
pub async fn test_provider_auth(
    provider: String,
    config: ProviderSpecificConfig,
) -> Result<AuthTestResult, String> {
    match provider.to_lowercase().as_str() {
        "aws" => {
            test_aws_auth(
                config.aws_region.unwrap_or_default(),
                config.aws_access_key_id.unwrap_or_default(),
                config.aws_secret_access_key.unwrap_or_default(),
            ).await
        }
        "gcp" => {
            test_gcp_auth(
                config.gcp_project_id.unwrap_or_default(),
                config.gcp_credentials_json.unwrap_or_default(),
            ).await
        }
        "azure" => {
            test_azure_auth(
                config.azure_vault_url.unwrap_or_default(),
                config.azure_tenant_id.unwrap_or_default(),
                config.azure_client_id.unwrap_or_default(),
                config.azure_client_secret.unwrap_or_default(),
            ).await
        }
        "vault" => {
            test_vault_auth(
                config.vault_address.unwrap_or_default(),
                config.vault_token.unwrap_or_default(),
                config.vault_namespace,
            ).await
        }
        "1password" => {
            test_1password_auth(
                config.onepassword_service_account_token.unwrap_or_default(),
                config.onepassword_vault_id.unwrap_or_default(),
            ).await
        }
        "bitwarden" => {
            test_bitwarden_auth(
                config.bitwarden_server_url.unwrap_or_default(),
                config.bitwarden_api_key.unwrap_or_default(),
            ).await
        }
        _ => Err(format!("Unknown provider type: {}", provider)),
    }
}

// ============ SECRET FETCH FUNCTIONS ============

// AWS Secrets Manager using AWS SDK
#[tauri::command]
pub async fn fetch_aws_secrets(
    region: String,
    access_key_id: String,
    secret_access_key: String,
    secret_name: String,
) -> Result<FetchSecretsResult, String> {
    // Create credentials
    let credentials = Credentials::new(
        &access_key_id,
        &secret_access_key,
        None,
        None,
        "istek-provider",
    );
    
    // Build config
    let region_provider = aws_config::Region::new(region.clone());
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .credentials_provider(credentials)
        .load()
        .await;
    
    // Create Secrets Manager client
    let sm_client = secretsmanager::Client::new(&config);
    
    // Get secret value
    match sm_client.get_secret_value().secret_id(&secret_name).send().await {
        Ok(response) => {
            let secret_string = response.secret_string().unwrap_or("");
            
            // Try to parse as JSON key-value pairs
            let secrets = if let Ok(secret_map) = serde_json::from_str::<HashMap<String, String>>(secret_string) {
                secret_map
                    .into_iter()
                    .map(|(key, value)| SecretValue { key, value })
                    .collect()
            } else {
                // Return as single secret
                vec![SecretValue {
                    key: secret_name.clone(),
                    value: secret_string.to_string(),
                }]
            };
            
            Ok(FetchSecretsResult {
                success: true,
                secrets,
                error: None,
            })
        }
        Err(e) => {
            Ok(FetchSecretsResult {
                success: false,
                secrets: vec![],
                error: Some(format!("AWS Secrets Manager error: {}", e)),
            })
        }
    }
}

// GCP Secret Manager
#[tauri::command]
pub async fn fetch_gcp_secrets(
    project_id: String,
    credentials_json: String,
    secret_name: String,
) -> Result<FetchSecretsResult, String> {
    // Parse credentials to get private key and client email
    let creds: serde_json::Value = serde_json::from_str(&credentials_json)
        .map_err(|e| format!("Invalid GCP credentials JSON: {}", e))?;
    
    let client_email = creds["client_email"]
        .as_str()
        .ok_or("Missing client_email in credentials")?;
    let private_key = creds["private_key"]
        .as_str()
        .ok_or("Missing private_key in credentials")?;
    
    // Create JWT for authentication
    let now = chrono::Utc::now().timestamp();
    let exp = now + 3600;
    
    let header = base64_url_encode(&serde_json::json!({
        "alg": "RS256",
        "typ": "JWT"
    }).to_string());
    
    let claims = base64_url_encode(&serde_json::json!({
        "iss": client_email,
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "aud": "https://oauth2.googleapis.com/token",
        "iat": now,
        "exp": exp
    }).to_string());
    
    let signing_input = format!("{}.{}", header, claims);
    
    // Sign with RSA (simplified - in production use proper RSA library)
    let signature = sign_rs256(&signing_input, private_key)?;
    let jwt = format!("{}.{}", signing_input, signature);
    
    // Exchange JWT for access token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let token_body: serde_json::Value = token_response.json().await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;
    
    let access_token = token_body["access_token"]
        .as_str()
        .ok_or("Failed to get access token from GCP")?;
    
    // Fetch secret
    let secret_url = format!(
        "https://secretmanager.googleapis.com/v1/projects/{}/secrets/{}/versions/latest:access",
        project_id, secret_name
    );
    
    let response = client
        .get(&secret_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if !status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("GCP API error: {} - {}", status, body_text)),
        });
    }
    
    let response_json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse GCP response: {}", e))?;
    
    // Decode base64 payload
    let payload_base64 = response_json["payload"]["data"]
        .as_str()
        .unwrap_or("");
    
    let secret_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        payload_base64
    ).map_err(|e| format!("Failed to decode secret: {}", e))?;
    
    let secret_string = String::from_utf8_lossy(&secret_bytes).to_string();
    
    // Try to parse as JSON key-value pairs
    let secrets = if let Ok(secret_map) = serde_json::from_str::<HashMap<String, String>>(&secret_string) {
        secret_map
            .into_iter()
            .map(|(key, value)| SecretValue { key, value })
            .collect()
    } else {
        vec![SecretValue {
            key: secret_name,
            value: secret_string,
        }]
    };
    
    Ok(FetchSecretsResult {
        success: true,
        secrets,
        error: None,
    })
}

// Azure Key Vault
#[tauri::command]
pub async fn fetch_azure_secrets(
    vault_url: String,
    tenant_id: String,
    client_id: String,
    client_secret: String,
    secret_name: String,
) -> Result<FetchSecretsResult, String> {
    let client = reqwest::Client::new();
    
    // Get access token from Azure AD
    let token_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        tenant_id
    );
    
    let token_response = client
        .post(&token_url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("scope", "https://vault.azure.net/.default"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let token_status = token_response.status();
    let token_body: serde_json::Value = token_response.json().await
        .map_err(|e| format!("Failed to parse Azure token response: {}", e))?;
    
    if !token_status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Azure authentication failed: {}", token_body)),
        });
    }
    
    let access_token = token_body["access_token"]
        .as_str()
        .ok_or("Failed to get access token from Azure")?;
    
    // Fetch secret from Key Vault
    let vault_url = vault_url.trim_end_matches('/');
    let secret_url = format!(
        "{}/secrets/{}?api-version=7.4",
        vault_url, secret_name
    );
    
    let response = client
        .get(&secret_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if !status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Azure Key Vault error: {} - {}", status, body_text)),
        });
    }
    
    let response_json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse Azure response: {}", e))?;
    
    let secret_value = response_json["value"]
        .as_str()
        .unwrap_or("");
    
    // Try to parse as JSON key-value pairs
    let secrets = if let Ok(secret_map) = serde_json::from_str::<HashMap<String, String>>(secret_value) {
        secret_map
            .into_iter()
            .map(|(key, value)| SecretValue { key, value })
            .collect()
    } else {
        vec![SecretValue {
            key: secret_name,
            value: secret_value.to_string(),
        }]
    };
    
    Ok(FetchSecretsResult {
        success: true,
        secrets,
        error: None,
    })
}

// HashiCorp Vault
#[tauri::command]
pub async fn fetch_vault_secrets(
    address: String,
    token: String,
    mount_path: String,
    namespace: Option<String>,
    secret_path: String,
) -> Result<FetchSecretsResult, String> {
    let client = reqwest::Client::new();
    
    // Build the URL for KV v2 secrets engine
    let mount = if mount_path.is_empty() { "secret".to_string() } else { mount_path };
    let url = format!("{}/v1/{}/data/{}", address.trim_end_matches('/'), mount, secret_path);
    
    let mut request = client
        .get(&url)
        .header("X-Vault-Token", &token);
    
    // Add namespace header if provided (for Vault Enterprise)
    if let Some(ns) = namespace {
        if !ns.is_empty() {
            request = request.header("X-Vault-Namespace", ns);
        }
    }
    
    let response = request.send().await.map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if !status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Vault API error: {} - {}", status, body_text)),
        });
    }
    
    let response_json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse Vault response: {}", e))?;
    
    // KV v2 returns data under data.data
    let data = &response_json["data"]["data"];
    
    let secrets: Vec<SecretValue> = if let Some(obj) = data.as_object() {
        obj.iter()
            .map(|(key, value)| SecretValue {
                key: key.clone(),
                value: value.as_str().unwrap_or(&value.to_string()).to_string(),
            })
            .collect()
    } else {
        vec![]
    };
    
    Ok(FetchSecretsResult {
        success: true,
        secrets,
        error: None,
    })
}

// 1Password (using Connect API)
#[tauri::command]
pub async fn fetch_1password_secrets(
    service_account_token: String,
    vault_id: String,
    item_name: String,
) -> Result<FetchSecretsResult, String> {
    let client = reqwest::Client::new();
    
    // 1Password Connect API endpoint
    let url = format!(
        "https://api.1password.com/v1/vaults/{}/items?filter=title eq \"{}\"",
        vault_id, item_name
    );
    
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", service_account_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if !status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("1Password API error: {} - {}", status, body_text)),
        });
    }
    
    let items: Vec<serde_json::Value> = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse 1Password response: {}", e))?;
    
    if items.is_empty() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Item '{}' not found in vault", item_name)),
        });
    }
    
    // Get the first matching item's ID and fetch full details
    let item_id = items[0]["id"].as_str().unwrap_or("");
    let item_url = format!(
        "https://api.1password.com/v1/vaults/{}/items/{}",
        vault_id, item_id
    );
    
    let item_response = client
        .get(&item_url)
        .header("Authorization", format!("Bearer {}", service_account_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let item_body: serde_json::Value = item_response.json().await
        .map_err(|e| format!("Failed to parse item response: {}", e))?;
    
    // Extract fields from the item
    let secrets: Vec<SecretValue> = if let Some(fields) = item_body["fields"].as_array() {
        fields.iter()
            .filter_map(|field| {
                let label = field["label"].as_str()?;
                let value = field["value"].as_str()?;
                Some(SecretValue {
                    key: label.to_string(),
                    value: value.to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };
    
    Ok(FetchSecretsResult {
        success: true,
        secrets,
        error: None,
    })
}

// Bitwarden (using CLI or API)
#[tauri::command]
pub async fn fetch_bitwarden_secrets(
    server_url: String,
    api_key: String,
    organization_id: Option<String>,
    item_name: String,
) -> Result<FetchSecretsResult, String> {
    let client = reqwest::Client::new();
    
    // Use Bitwarden Secrets Manager API
    let base_url = if server_url.is_empty() {
        "https://api.bitwarden.com".to_string()
    } else {
        server_url.trim_end_matches('/').to_string()
    };
    
    // Search for the item
    let search_url = format!("{}/secrets", base_url);
    
    let response = client
        .get(&search_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if !status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Bitwarden API error: {} - {}", status, body_text)),
        });
    }
    
    let response_json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Failed to parse Bitwarden response: {}", e))?;
    
    // Find the secret by name
    let secrets_list = response_json["data"].as_array();
    
    if let Some(secrets) = secrets_list {
        for secret in secrets {
            let key = secret["key"].as_str().unwrap_or("");
            if key == item_name {
                let value = secret["value"].as_str().unwrap_or("");
                return Ok(FetchSecretsResult {
                    success: true,
                    secrets: vec![SecretValue {
                        key: key.to_string(),
                        value: value.to_string(),
                    }],
                    error: None,
                });
            }
        }
    }
    
    Ok(FetchSecretsResult {
        success: false,
        secrets: vec![],
        error: Some(format!("Secret '{}' not found", item_name)),
    })
}

// Test connection to a secret provider
#[tauri::command]
pub async fn test_secret_provider_connection(
    provider: String,
    config: ProviderSpecificConfig,
) -> Result<FetchSecretsResult, String> {
    match provider.to_lowercase().as_str() {
        "aws" => {
            fetch_aws_secrets(
                config.aws_region.unwrap_or_default(),
                config.aws_access_key_id.unwrap_or_default(),
                config.aws_secret_access_key.unwrap_or_default(),
                config.aws_secret_name.unwrap_or_default(),
            ).await
        }
        "gcp" => {
            fetch_gcp_secrets(
                config.gcp_project_id.unwrap_or_default(),
                config.gcp_credentials_json.unwrap_or_default(),
                config.gcp_secret_name.unwrap_or_default(),
            ).await
        }
        "azure" => {
            fetch_azure_secrets(
                config.azure_vault_url.unwrap_or_default(),
                config.azure_tenant_id.unwrap_or_default(),
                config.azure_client_id.unwrap_or_default(),
                config.azure_client_secret.unwrap_or_default(),
                config.azure_secret_name.unwrap_or_default(),
            ).await
        }
        "vault" => {
            fetch_vault_secrets(
                config.vault_address.unwrap_or_default(),
                config.vault_token.unwrap_or_default(),
                config.vault_mount_path.unwrap_or_default(),
                config.vault_namespace,
                config.vault_secret_path.unwrap_or_default(),
            ).await
        }
        "1password" => {
            fetch_1password_secrets(
                config.onepassword_service_account_token.unwrap_or_default(),
                config.onepassword_vault_id.unwrap_or_default(),
                config.onepassword_item_name.unwrap_or_default(),
            ).await
        }
        "bitwarden" => {
            fetch_bitwarden_secrets(
                config.bitwarden_server_url.unwrap_or_default(),
                config.bitwarden_api_key.unwrap_or_default(),
                config.bitwarden_organization_id,
                config.bitwarden_item_name.unwrap_or_default(),
            ).await
        }
        _ => Err(format!("Unknown provider type: {}", provider)),
    }
}

// Helper functions for AWS signature
fn sha256_hex(data: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn base64_url_encode(data: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data.as_bytes())
}

fn sign_rs256(data: &str, private_key_pem: &str) -> Result<String, String> {
    use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey};
    use rsa::pkcs1v15::SigningKey;
    use sha2::Sha256;
    use signature::{Signer, SignatureEncoding};
    
    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .map_err(|e| format!("Failed to parse private key: {}", e))?;
    
    let signing_key = SigningKey::<Sha256>::new(private_key);
    let signature = signing_key.sign(data.as_bytes());
    
    use base64::Engine;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}
