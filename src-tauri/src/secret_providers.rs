use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use aws_sdk_sts as sts;
use aws_sdk_secretsmanager as secretsmanager;
use aws_credential_types::Credentials;
use base64::prelude::*;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use hkdf::Hkdf;
use sha2::Sha256;
use hmac::{Hmac, Mac};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

// Bitwarden encryption key structure
#[derive(Clone)]
struct BitwardenKeys {
    enc_key: [u8; 32],  // AES-256 encryption key
    mac_key: [u8; 32],  // HMAC-SHA256 key
}

// Bitwarden Secrets Manager key derivation for access tokens
// Based on the official Bitwarden SDK implementation:
// 1. HMAC-SHA256 with key="bitwarden-accesstoken" and data=encryption_key → PRK (32 bytes)
// 2. HKDF-Expand with info="sm-access-token" → 64 bytes
// 3. Split: first 32 bytes = enc_key, last 32 bytes = mac_key
fn derive_bitwarden_keys(encryption_key: &[u8]) -> Result<BitwardenKeys, String> {
    
    // The encryption key should be exactly 16 bytes
    if encryption_key.len() != 16 {
        return Err(format!("Invalid encryption key length: expected 16, got {}", encryption_key.len()));
    }
    
    // Step 1: HMAC-SHA256 with key="bitwarden-accesstoken" and data=encryption_key
    let mut hmac = Hmac::<Sha256>::new_from_slice(b"bitwarden-accesstoken")
        .map_err(|e| format!("Failed to create HMAC: {:?}", e))?;
    hmac.update(encryption_key);
    let prk = hmac.finalize().into_bytes();
    
    // Step 2: HKDF-Expand with info="sm-access-token" to get 64 bytes
    let hk = Hkdf::<Sha256>::from_prk(&prk)
        .map_err(|e| format!("Failed to create HKDF from PRK: {:?}", e))?;
    
    let mut okm = [0u8; 64];
    hk.expand(b"sm-access-token", &mut okm)
        .map_err(|e| format!("Failed to expand HKDF: {:?}", e))?;
    
    // Step 3: Split into enc_key (first 32 bytes) and mac_key (last 32 bytes)
    let mut enc_key = [0u8; 32];
    let mut mac_key = [0u8; 32];
    enc_key.copy_from_slice(&okm[..32]);
    mac_key.copy_from_slice(&okm[32..]);
    
    Ok(BitwardenKeys { enc_key, mac_key })
}

// Decrypt a Bitwarden encrypted string
// Format: {encType}.{iv}|{data}|{mac}
fn decrypt_bitwarden_string(encrypted: &str, keys: &BitwardenKeys) -> Result<String, String> {
    // Parse the encrypted string
    let parts: Vec<&str> = encrypted.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid encrypted string format: {}", encrypted));
    }
    
    let enc_type: u8 = parts[0].parse()
        .map_err(|_| format!("Invalid encryption type: {}", parts[0]))?;
    
    // Only support type 2 (AES-256-CBC with HMAC)
    if enc_type != 2 {
        return Err(format!("Unsupported encryption type: {}", enc_type));
    }
    
    let payload_parts: Vec<&str> = parts[1].split('|').collect();
    if payload_parts.len() != 3 {
        return Err(format!("Invalid payload format, expected 3 parts, got {}", payload_parts.len()));
    }
    
    let iv = BASE64_STANDARD.decode(payload_parts[0])
        .map_err(|e| format!("Failed to decode IV: {}", e))?;
    let data = BASE64_STANDARD.decode(payload_parts[1])
        .map_err(|e| format!("Failed to decode data: {}", e))?;
    let mac = BASE64_STANDARD.decode(payload_parts[2])
        .map_err(|e| format!("Failed to decode MAC: {}", e))?;
    
    // Verify HMAC
    let mut hmac = Hmac::<Sha256>::new_from_slice(&keys.mac_key)
        .map_err(|e| format!("Failed to create HMAC: {:?}", e))?;
    hmac.update(&iv);
    hmac.update(&data);
    hmac.verify_slice(&mac)
        .map_err(|_| "HMAC verification failed")?;
    
    // Decrypt
    let iv_array: [u8; 16] = iv.try_into()
        .map_err(|_| "Invalid IV length")?;
    
    let mut buf = data.clone();
    let decrypted = Aes256CbcDec::new(&keys.enc_key.into(), &iv_array.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("Decryption failed: {:?}", e))?;
    
    String::from_utf8(decrypted.to_vec())
        .map_err(|e| format!("Invalid UTF-8 in decrypted data: {}", e))
}

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
    _vault_url: String,
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

// Bitwarden Auth Test
#[tauri::command]
pub async fn test_bitwarden_auth(
    server_url: String,
    access_token: String,
) -> Result<AuthTestResult, String> {
    let client = reqwest::Client::new();
    
    // Parse access token: format is "0.{client_id}.{client_secret}:{encryption_key}"
    // Example: 0.2fd91392-21d1-4d62-9d97-b3d90169fc82.J0tD8a6AvHXozgq8MUuyuSfa74G3pD:89CTHkZ63/u6xFqaV8D16w==
    let parts: Vec<&str> = access_token.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Ok(AuthTestResult {
            success: false,
            message: "Invalid access token format. Expected: version.client_id.client_secret:encryption_key".to_string(),
            identity: None,
        });
    }
    
    let client_id = parts[1]; // UUID
    let secret_and_key = parts[2]; // client_secret:encryption_key
    
    // Split the third part to get client_secret (before the colon)
    let secret_parts: Vec<&str> = secret_and_key.splitn(2, ':').collect();
    let client_secret = secret_parts[0];
    
    // Use provided server URL or default to Bitwarden cloud
    // Server URL can be base URL or identity URL
    let identity_url = if !server_url.is_empty() {
        let base = server_url.trim_end_matches('/');
        if base.ends_with("/identity") {
            base.to_string()
        } else if base.ends_with("/api") {
            base.replace("/api", "/identity")
        } else {
            format!("{}/identity", base)
        }
    } else {
        "https://identity.bitwarden.com".to_string()
    };
    
    // Exchange credentials for bearer token
    let token_url = format!("{}/connect/token", identity_url);
    
    let response = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=client_credentials&scope=api.secrets&client_id={}&client_secret={}",
            client_id, client_secret
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let status = response.status();
    let body_text = response.text().await.map_err(|e| e.to_string())?;
    
    if status.is_success() {
        let token_response: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("Failed to parse token response: {}", e))?;
        
        if token_response.get("access_token").is_some() {
            Ok(AuthTestResult {
                success: true,
                message: "Bitwarden Secrets Manager authentication successful".to_string(),
                identity: None,
            })
        } else {
            Ok(AuthTestResult {
                success: false,
                message: format!("Unexpected response: {}", body_text),
                identity: None,
            })
        }
    } else {
        Ok(AuthTestResult {
            success: false,
            message: format!("Bitwarden authentication failed: {} - {}", status, body_text),
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

// Bitwarden Secrets Manager
#[tauri::command]
pub async fn fetch_bitwarden_secrets(
    server_url: String,
    access_token: String,
    _organization_id: Option<String>,
    secret_name: String,
) -> Result<FetchSecretsResult, String> {
    let client = reqwest::Client::new();
    
    // Parse access token: format is "0.{client_id}.{client_secret}:{encryption_key}"
    // Example: 0.2fd91392-21d1-4d62-9d97-b3d90169fc82.J0tD8a6AvHXozgq8MUuyuSfa74G3pD:89CTHkZ63/u6xFqaV8D16w==
    let parts: Vec<&str> = access_token.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some("Invalid access token format".to_string()),
        });
    }
    
    let client_id = parts[1]; // UUID
    let secret_and_key = parts[2]; // client_secret:encryption_key
    
    // Split the third part to get client_secret and encryption_key
    let secret_parts: Vec<&str> = secret_and_key.splitn(2, ':').collect();
    let client_secret = secret_parts[0];
    let encryption_key_b64 = if secret_parts.len() > 1 { secret_parts[1] } else { "" };
    
    // Decode and derive encryption keys
    let encryption_key = if !encryption_key_b64.is_empty() {
        match BASE64_STANDARD.decode(encryption_key_b64) {
            Ok(key) => Some(key),
            Err(_) => None,
        }
    } else {
        None
    };
    
    // Derive keys using the correct Bitwarden Secrets Manager algorithm
    let bitwarden_keys = encryption_key.as_ref().and_then(|key| {
        derive_bitwarden_keys(key).ok()
    });
    
    // Use provided server URL or default to Bitwarden cloud
    // Server URL should be base URL like https://vault.bitwarden.com
    let (identity_url, api_url) = if !server_url.is_empty() {
        let base = server_url.trim_end_matches('/');
        if base.ends_with("/identity") {
            // User provided identity URL directly
            let api = base.replace("/identity", "/api");
            (base.to_string(), api)
        } else if base.ends_with("/api") {
            // User provided API URL directly
            let identity = base.replace("/api", "/identity");
            (identity, base.to_string())
        } else {
            // User provided base URL
            (format!("{}/identity", base), format!("{}/api", base))
        }
    } else {
        ("https://identity.bitwarden.com".to_string(), "https://api.bitwarden.com".to_string())
    };
    
    // Step 1: Exchange credentials for bearer token
    let token_url = format!("{}/connect/token", identity_url);
    
    let token_response = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=client_credentials&scope=api.secrets&client_id={}&client_secret={}",
            client_id, client_secret
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let token_status = token_response.status();
    let token_body = token_response.text().await.map_err(|e| e.to_string())?;
    
    if !token_status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Authentication failed: {} - {}", token_status, token_body)),
        });
    }
    
    let token_json: serde_json::Value = match serde_json::from_str(&token_body) {
        Ok(v) => v,
        Err(e) => {
            return Ok(FetchSecretsResult {
                success: false,
                secrets: vec![],
                error: Some(format!("Failed to parse auth response: {}", e)),
            });
        }
    };
    
    let bearer_token = match token_json.get("access_token").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return Ok(FetchSecretsResult {
                success: false,
                secrets: vec![],
                error: Some("No access token in auth response".to_string()),
            });
        }
    };
    
    // Step 2: Decrypt the encrypted_payload to get the organization's encryption key
    // The access token's derived key is used to decrypt the payload, which contains the org key
    let org_encryption_key = if let (Some(ref keys), Some(encrypted_payload)) = 
        (&bitwarden_keys, token_json.get("encrypted_payload").or_else(|| token_json.get("EncryptedPayload")).and_then(|v| v.as_str())) 
    {
        // Decrypt the payload using the access token's derived key
        match decrypt_bitwarden_string(encrypted_payload, keys) {
            Ok(decrypted_json) => {
                // Parse the decrypted JSON to extract the encryptionKey
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    if let Some(enc_key_b64) = payload.get("encryptionKey").and_then(|v| v.as_str()) {
                        // Decode the base64 org encryption key (should be 64 bytes for AES-256-CBC-HMAC)
                        match BASE64_STANDARD.decode(enc_key_b64) {
                            Ok(key_bytes) if key_bytes.len() == 64 => {
                                let mut enc_key = [0u8; 32];
                                let mut mac_key = [0u8; 32];
                                enc_key.copy_from_slice(&key_bytes[..32]);
                                mac_key.copy_from_slice(&key_bytes[32..]);
                                Some(BitwardenKeys { enc_key, mac_key })
                            }
                            Ok(key_bytes) => {
                                // Log unexpected key length but continue
                                eprintln!("Unexpected org key length: {}", key_bytes.len());
                                None
                            }
                            Err(e) => {
                                eprintln!("Failed to decode org encryption key: {}", e);
                                None
                            }
                        }
                    } else {
                        eprintln!("No encryptionKey in decrypted payload");
                        None
                    }
                } else {
                    eprintln!("Failed to parse decrypted payload as JSON");
                    None
                }
            }
            Err(e) => {
                eprintln!("Failed to decrypt encrypted_payload: {}", e);
                None
            }
        }
    } else {
        // No encrypted_payload in response, fall back to using access token key
        // This might be the case for older API versions
        bitwarden_keys.clone()
    };
    
    // Step 3: Get organization ID from the token response or use provided one
    let org_id = _organization_id.as_deref()
        .or_else(|| token_json.get("organization_id").and_then(|v| v.as_str()))
        .or_else(|| token_json.get("organizationId").and_then(|v| v.as_str()));
    
    // Step 3: List secrets - need organization ID in path
    let secrets_url = if let Some(org) = org_id {
        format!("{}/organizations/{}/secrets", api_url, org)
    } else {
        // Try to get org ID from sync endpoint first
        let sync_url = format!("{}/sync", api_url);
        let sync_response = client
            .get(&sync_url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .send()
            .await;
        
        if let Ok(resp) = sync_response {
            if let Ok(body) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(org) = json.get("organizationId").and_then(|v| v.as_str()) {
                        format!("{}/organizations/{}/secrets", api_url, org)
                    } else {
                        format!("{}/secrets", api_url)
                    }
                } else {
                    format!("{}/secrets", api_url)
                }
            } else {
                format!("{}/secrets", api_url)
            }
        } else {
            format!("{}/secrets", api_url)
        }
    };
    
    let secrets_response = client
        .get(&secrets_url)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let secrets_status = secrets_response.status();
    let secrets_body = secrets_response.text().await.map_err(|e| e.to_string())?;
    
    if !secrets_status.is_success() {
        return Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Failed to list secrets: {} - {} (URL: {})", secrets_status, secrets_body, secrets_url)),
        });
    }
    
    let secrets_json: serde_json::Value = match serde_json::from_str(&secrets_body) {
        Ok(v) => v,
        Err(e) => {
            return Ok(FetchSecretsResult {
                success: false,
                secrets: vec![],
                error: Some(format!("Failed to parse secrets response: {}", e)),
            });
        }
    };
    
    // Find the secret by name in the list - check multiple possible fields
    let secrets_list = secrets_json.get("data").and_then(|d| d.as_array())
        .or_else(|| secrets_json.get("secrets").and_then(|d| d.as_array()))
        .or_else(|| secrets_json.as_array());
    
    if let Some(secrets) = secrets_list {
        // Decrypt and collect available secret names for error message
        let mut available_names: Vec<String> = Vec::new();
        
        for secret in secrets {
            // Get the encrypted key
            let encrypted_key = secret.get("key").and_then(|k| k.as_str())
                .or_else(|| secret.get("name").and_then(|n| n.as_str()))
                .unwrap_or("");
            
            // Try to decrypt the key name using the organization's encryption key
            let decrypted_key = {
                if let Some(ref keys) = org_encryption_key {
                    match decrypt_bitwarden_string(encrypted_key, keys) {
                        Ok(decrypted) => decrypted,
                        Err(e) => format!("[decrypt failed: {}] {}", e, encrypted_key)
                    }
                } else {
                    format!("[no org encryption key] {}", encrypted_key)
                }
            };
            
            available_names.push(decrypted_key.clone());
            
            if decrypted_key == secret_name || decrypted_key.eq_ignore_ascii_case(&secret_name) {
                // Found the secret, now fetch its value by ID
                let secret_id = match secret.get("id").and_then(|i| i.as_str()) {
                    Some(id) => id,
                    None => continue,
                };
                
                // Fetch the full secret with value
                let secret_url = format!("{}/secrets/{}", api_url, secret_id);
                
                let secret_response = client
                    .get(&secret_url)
                    .header("Authorization", format!("Bearer {}", bearer_token))
                    .header("Content-Type", "application/json")
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                
                let secret_status = secret_response.status();
                let secret_body = secret_response.text().await.map_err(|e| e.to_string())?;
                
                if !secret_status.is_success() {
                    return Ok(FetchSecretsResult {
                        success: false,
                        secrets: vec![],
                        error: Some(format!("Failed to fetch secret: {} - {}", secret_status, secret_body)),
                    });
                }
                
                let secret_detail: serde_json::Value = match serde_json::from_str(&secret_body) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(FetchSecretsResult {
                            success: false,
                            secrets: vec![],
                            error: Some(format!("Failed to parse secret: {}", e)),
                        });
                    }
                };
                
                let encrypted_value = secret_detail.get("value").and_then(|v| v.as_str()).unwrap_or("");
                
                // Decrypt the value using the organization's encryption key
                let decrypted_value = {
                    if let Some(ref keys) = org_encryption_key {
                        match decrypt_bitwarden_string(encrypted_value, keys) {
                            Ok(decrypted) => decrypted,
                            Err(e) => format!("[decrypt failed: {}]", e)
                        }
                    } else {
                        "[no org encryption key]".to_string()
                    }
                };
                
                return Ok(FetchSecretsResult {
                    success: true,
                    secrets: vec![SecretValue {
                        key: decrypted_key.clone(),
                        value: decrypted_value,
                    }],
                    error: None,
                });
            }
        }
        
        // Secret not found - show available (decrypted) names
        Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some(format!("Secret '{}' not found. Available secrets: {}", secret_name, available_names.join(", "))),
        })
    } else {
        Ok(FetchSecretsResult {
            success: false,
            secrets: vec![],
            error: Some("No secrets found in response".to_string()),
        })
    }
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

#[cfg(test)]
mod bitwarden_tests {
    use super::*;
    
    // Test against Bitwarden SDK test vectors
    #[test]
    fn test_derive_shareable_key_compatible() {
        // From Bitwarden SDK test:
        // derive_shareable_key(Zeroizing::new(*b"&/$%F1a895g67HlX"), "test_key", None)
        // Expected: "4PV6+PcmF2w7YHRatvyMcVQtI7zvCyssv/wFWmzjiH6Iv9altjmDkuBD1aagLVaLezbthbSe+ktR+U6qswxNnQ=="
        
        let secret = b"&/$%F1a895g67HlX";
        
        // Step 1: HMAC-SHA256 with key="bitwarden-test_key" and data=secret
        let mut hmac = Hmac::<Sha256>::new_from_slice(b"bitwarden-test_key").unwrap();
        hmac.update(secret);
        let prk = hmac.finalize().into_bytes();
        
        // Step 2: HKDF-Expand with info=None (empty) to get 64 bytes
        let hk = Hkdf::<Sha256>::from_prk(&prk).unwrap();
        let mut okm = [0u8; 64];
        hk.expand(b"", &mut okm).unwrap();
        
        // Encode to base64 and compare
        let result = BASE64_STANDARD.encode(&okm);
        let expected = "4PV6+PcmF2w7YHRatvyMcVQtI7zvCyssv/wFWmzjiH6Iv9altjmDkuBD1aagLVaLezbthbSe+ktR+U6qswxNnQ==";
        
        println!("Result:   {}", result);
        println!("Expected: {}", expected);
        assert_eq!(result, expected);
    }
    
    #[test]
    fn test_derive_shareable_key_with_info() {
        // From Bitwarden SDK test:
        // derive_shareable_key(Zeroizing::new(*b"67t9b5g67$%Dh89n"), "test_key", Some("test"))
        // Expected: "F9jVQmrACGx9VUPjuzfMYDjr726JtL300Y3Yg+VYUnVQtQ1s8oImJ5xtp1KALC9h2nav04++1LDW4iFD+infng=="
        
        let secret = b"67t9b5g67$%Dh89n";
        
        // Step 1: HMAC-SHA256 with key="bitwarden-test_key" and data=secret
        let mut hmac = Hmac::<Sha256>::new_from_slice(b"bitwarden-test_key").unwrap();
        hmac.update(secret);
        let prk = hmac.finalize().into_bytes();
        
        // Step 2: HKDF-Expand with info="test" to get 64 bytes
        let hk = Hkdf::<Sha256>::from_prk(&prk).unwrap();
        let mut okm = [0u8; 64];
        hk.expand(b"test", &mut okm).unwrap();
        
        // Encode to base64 and compare
        let result = BASE64_STANDARD.encode(&okm);
        let expected = "F9jVQmrACGx9VUPjuzfMYDjr726JtL300Y3Yg+VYUnVQtQ1s8oImJ5xtp1KALC9h2nav04++1LDW4iFD+infng==";
        
        println!("Result:   {}", result);
        println!("Expected: {}", expected);
        assert_eq!(result, expected);
    }
    
    #[test]
    fn test_access_token_derivation() {
        // From Bitwarden SDK test in access_token.rs:
        // Input token: "0.ec2c1d46-6a4b-4751-a310-af9601317f2d.C2IgxjjLF7qSshsbwe8JGcbM075YXw:X8vbvA0bduihIDe/qrzIQQ=="
        // Expected key: "H9/oIRLtL9nGCQOVDjSMoEbJsjWXSOCb3qeyDt6ckzS3FhyboEDWyTP/CQfbIszNmAVg2ExFganG1FVFGXO/Jg=="
        
        let encryption_key_b64 = "X8vbvA0bduihIDe/qrzIQQ==";
        let encryption_key = BASE64_STANDARD.decode(encryption_key_b64).unwrap();
        
        println!("Encryption key length: {}", encryption_key.len());
        println!("Encryption key: {:?}", encryption_key);
        
        // Derive using our function
        let keys = derive_bitwarden_keys(&encryption_key).unwrap();
        
        // Combine enc_key and mac_key
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&keys.enc_key);
        combined[32..].copy_from_slice(&keys.mac_key);
        
        let result = BASE64_STANDARD.encode(&combined);
        let expected = "H9/oIRLtL9nGCQOVDjSMoEbJsjWXSOCb3qeyDt6ckzS3FhyboEDWyTP/CQfbIszNmAVg2ExFganG1FVFGXO/Jg==";
        
        println!("Result:   {}", result);
        println!("Expected: {}", expected);
        assert_eq!(result, expected);
    }
}
