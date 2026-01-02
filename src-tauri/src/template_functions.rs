use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512, Digest};
use md5::Md5;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use keyring::Entry;

const KEYRING_SERVICE: &str = "istek-api-client";

// ============ Hash Functions ============

#[derive(Debug, Serialize, Deserialize)]
pub struct HashResult {
    pub hex: String,
    pub base64: String,
}

#[tauri::command]
pub fn hash_md5(input: String) -> HashResult {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

#[tauri::command]
pub fn hash_sha1(input: String) -> HashResult {
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

#[tauri::command]
pub fn hash_sha256(input: String) -> HashResult {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

#[tauri::command]
pub fn hash_sha512(input: String) -> HashResult {
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

// ============ HMAC Functions ============

#[tauri::command]
pub fn hmac_sha256(input: String, key: String) -> HashResult {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(input.as_bytes());
    let result = mac.finalize().into_bytes();
    
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

#[tauri::command]
pub fn hmac_sha512(input: String, key: String) -> HashResult {
    use hmac::{Hmac, Mac};
    type HmacSha512 = Hmac<Sha512>;
    
    let mut mac = HmacSha512::new_from_slice(key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(input.as_bytes());
    let result = mac.finalize().into_bytes();
    
    HashResult {
        hex: hex::encode(&result),
        base64: BASE64.encode(&result),
    }
}

// ============ Encoding Functions ============

#[tauri::command]
pub fn encode_base64(input: String) -> String {
    BASE64.encode(input.as_bytes())
}

#[tauri::command]
pub fn decode_base64(input: String) -> Result<String, String> {
    BASE64.decode(&input)
        .map_err(|e| format!("Invalid base64: {}", e))
        .and_then(|bytes| {
            String::from_utf8(bytes)
                .map_err(|e| format!("Invalid UTF-8: {}", e))
        })
}

#[tauri::command]
pub fn encode_url(input: String) -> String {
    urlencoding::encode(&input).to_string()
}

#[tauri::command]
pub fn decode_url(input: String) -> Result<String, String> {
    urlencoding::decode(&input)
        .map(|s| s.to_string())
        .map_err(|e| format!("Invalid URL encoding: {}", e))
}

// ============ Encryption (System Keychain) ============

/// Store a secret value in the system keychain
/// The key is used as the "account" name in the keychain
#[tauri::command]
pub fn encrypt_store(key: String, value: String) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    
    entry.set_password(&value)
        .map_err(|e| format!("Failed to store secret: {}", e))?;
    
    Ok(())
}

/// Retrieve a secret value from the system keychain
#[tauri::command]
pub fn encrypt_retrieve(key: String) -> Result<String, String> {
    let entry = Entry::new(KEYRING_SERVICE, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    
    entry.get_password()
        .map_err(|e| format!("Failed to retrieve secret: {}", e))
}

/// Delete a secret value from the system keychain
#[tauri::command]
pub fn encrypt_delete(key: String) -> Result<(), String> {
    let entry = Entry::new(KEYRING_SERVICE, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    
    entry.delete_credential()
        .map_err(|e| format!("Failed to delete secret: {}", e))?;
    
    Ok(())
}

/// List all stored secret keys (not values) - used for autocomplete
#[tauri::command]
pub fn encrypt_list_keys() -> Vec<String> {
    // Note: keyring crate doesn't support listing all keys
    // This would require platform-specific implementations
    // For now, we return an empty list and manage keys in the app
    vec![]
}

// ============ Utility Functions ============

#[tauri::command]
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[tauri::command]
pub fn timestamp_now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[tauri::command]
pub fn timestamp_now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[tauri::command]
pub fn format_timestamp(timestamp: i64, format: String) -> Result<String, String> {
    use chrono::{DateTime, Utc};
    
    let datetime = DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| "Invalid timestamp".to_string())?;
    
    Ok(datetime.format(&format).to_string())
}

// ============ Random Functions ============

#[tauri::command]
pub fn random_int(min: i64, max: i64) -> i64 {
    use rand::Rng;
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}

#[tauri::command]
pub fn random_float(min: f64, max: f64) -> f64 {
    use rand::Rng;
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}

#[tauri::command]
pub fn random_string(length: usize, charset: Option<String>) -> String {
    use rand::Rng;
    let chars: Vec<char> = charset
        .unwrap_or_else(|| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string())
        .chars()
        .collect();
    
    let mut rng = rand::rng();
    (0..length)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect()
}

#[tauri::command]
pub fn random_hex(length: usize) -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..length).map(|_| rng.random()).collect();
    hex::encode(bytes)
}
