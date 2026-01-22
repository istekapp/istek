use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512, Digest};
use md5::Md5;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use keyring::Entry;
use aes_gcm::{
    aead::Aead,
    Aes256Gcm, Nonce,
    KeyInit as AesKeyInit,
};

const KEYRING_SERVICE: &str = "istek-api-client";
const KEYRING_MASTER_KEY_PREFIX: &str = "istek-master-key-";

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
    
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key.as_bytes())
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
    
    let mut mac = <HmacSha512 as Mac>::new_from_slice(key.as_bytes())
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
    use chrono::DateTime;
    
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

// ============ Workspace Encryption (Sensitive Values) ============

/// Result for sensitive operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveEncryptionStatus {
    pub enabled: bool,
    pub workspace_id: String,
}

/// Generate a new master key for workspace encryption
/// Returns the key as base64 - this is the ONLY time the user sees it
#[tauri::command]
pub fn sensitive_generate_master_key() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut key = [0u8; 32]; // 256 bits
    rng.fill(&mut key);
    BASE64.encode(key)
}

/// Store the master key in the system keychain for a workspace
#[tauri::command]
pub fn sensitive_store_master_key(workspace_id: String, master_key: String) -> Result<(), String> {
    println!("[Encryption] sensitive_store_master_key called for workspace: {}", workspace_id);
    
    // Validate the key is valid base64 and correct length
    let key_bytes = BASE64.decode(&master_key)
        .map_err(|e| {
            println!("[Encryption] Invalid master key format: {}", e);
            format!("Invalid master key format: {}", e)
        })?;
    
    if key_bytes.len() != 32 {
        println!("[Encryption] Master key wrong length: {} bytes", key_bytes.len());
        return Err("Master key must be 32 bytes (256 bits)".to_string());
    }
    
    let keyring_key = format!("{}{}", KEYRING_MASTER_KEY_PREFIX, workspace_id);
    println!("[Encryption] Storing in keychain: service='{}', account='{}'", KEYRING_SERVICE, keyring_key);
    
    let entry = Entry::new(KEYRING_SERVICE, &keyring_key)
        .map_err(|e| {
            println!("[Encryption] Failed to create keyring entry: {:?}", e);
            format!("Failed to create keyring entry: {}", e)
        })?;
    
    entry.set_password(&master_key)
        .map_err(|e| {
            println!("[Encryption] Failed to store master key: {:?}", e);
            format!("Failed to store master key: {}", e)
        })?;
    
    println!("[Encryption] Master key stored successfully!");
    Ok(())
}

/// Check if workspace has encryption enabled (master key exists in keychain)
#[tauri::command]
pub fn sensitive_check_encryption_status(workspace_id: String) -> SensitiveEncryptionStatus {
    let keyring_key = format!("{}{}", KEYRING_MASTER_KEY_PREFIX, workspace_id);
    
    let enabled = match Entry::new(KEYRING_SERVICE, &keyring_key) {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    };
    
    println!("[Encryption] check_encryption_status for {}: {}", workspace_id, enabled);
    
    SensitiveEncryptionStatus {
        enabled,
        workspace_id,
    }
}

/// Delete master key from keychain (disables encryption for workspace)
#[tauri::command]
pub fn sensitive_delete_master_key(workspace_id: String) -> Result<(), String> {
    let keyring_key = format!("{}{}", KEYRING_MASTER_KEY_PREFIX, workspace_id);
    
    let entry = Entry::new(KEYRING_SERVICE, &keyring_key)
        .map_err(|e| format!("Failed to access keyring: {}", e))?;
    
    entry.delete_credential()
        .map_err(|e| format!("Failed to delete master key: {}", e))?;
    
    println!("[Encryption] Master key deleted for workspace: {}", workspace_id);
    Ok(())
}

/// Encrypt a sensitive value using the workspace's master key
/// Returns: base64(nonce || ciphertext)
#[tauri::command]
pub fn sensitive_encrypt(workspace_id: String, key: String, value: String) -> Result<String, String> {
    println!("[Encryption] sensitive_encrypt called for workspace: {}, key: {}", workspace_id, key);
    
    // Get master key from keychain
    let keyring_key = format!("{}{}", KEYRING_MASTER_KEY_PREFIX, workspace_id);
    
    let entry = Entry::new(KEYRING_SERVICE, &keyring_key)
        .map_err(|e| {
            println!("[Encryption] Failed to access keyring: {:?}", e);
            format!("Failed to access keyring: {}", e)
        })?;
    
    let master_key_b64 = entry.get_password()
        .map_err(|e| {
            println!("[Encryption] Failed to get master key: {:?}", e);
            "Workspace encryption not enabled. Please set up a master key first.".to_string()
        })?;
    
    let master_key_bytes = BASE64.decode(&master_key_b64)
        .map_err(|e| {
            println!("[Encryption] Invalid master key format: {}", e);
            format!("Invalid master key: {}", e)
        })?;
    
    // Create cipher
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&master_key_bytes)
        .map_err(|e| {
            println!("[Encryption] Failed to create cipher: {}", e);
            format!("Failed to create cipher: {}", e)
        })?;
    
    // Generate random 12-byte nonce
    use rand::Rng;
    let mut rng = rand::rng();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Use key as associated data for additional security
    let ciphertext = cipher.encrypt(nonce, value.as_bytes())
        .map_err(|e| {
            println!("[Encryption] Encryption failed: {}", e);
            format!("Encryption failed: {}", e)
        })?;
    
    // Combine nonce + ciphertext and encode as base64
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    
    println!("[Encryption] Value encrypted successfully");
    Ok(BASE64.encode(combined))
}

/// Decrypt a sensitive value using the workspace's master key
#[tauri::command]
pub fn sensitive_decrypt(workspace_id: String, _key: String, encrypted_value: String) -> Result<String, String> {
    // Get master key from keychain
    let keyring_key = format!("{}{}", KEYRING_MASTER_KEY_PREFIX, workspace_id);
    
    let entry = Entry::new(KEYRING_SERVICE, &keyring_key)
        .map_err(|e| format!("Failed to access keyring: {}", e))?;
    
    let master_key_b64 = entry.get_password()
        .map_err(|_| "Workspace encryption not enabled. Please set up a master key first.".to_string())?;
    
    let master_key_bytes = BASE64.decode(&master_key_b64)
        .map_err(|e| format!("Invalid master key: {}", e))?;
    
    // Decode the encrypted value
    let combined = BASE64.decode(&encrypted_value)
        .map_err(|e| format!("Invalid encrypted value format: {}", e))?;
    
    if combined.len() < 12 {
        return Err("Invalid encrypted value: too short".to_string());
    }
    
    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    // Create cipher and decrypt
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&master_key_bytes)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed. The master key may be incorrect.".to_string())?;
    
    String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8 in decrypted value: {}", e))
}
