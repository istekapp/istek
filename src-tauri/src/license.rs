use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tauri::Manager;

use crate::storage::Storage;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationInfo {
    /// Timestamp (epoch seconds) when the evaluation period started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    /// Whether the user has dismissed the evaluation expired dialog
    #[serde(default)]
    pub expired_dismissed: bool,
}

fn evaluation_path(storage: &Storage) -> std::path::PathBuf {
    storage.config_dir().join("evaluation.yaml")
}

fn read_evaluation(storage: &Storage) -> EvaluationInfo {
    let path = evaluation_path(storage);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(info) = serde_yaml::from_str::<EvaluationInfo>(&content) {
                return info;
            }
        }
    }
    EvaluationInfo::default()
}

fn write_evaluation(storage: &Storage, info: &EvaluationInfo) -> Result<(), String> {
    let path = evaluation_path(storage);
    let content = serde_yaml::to_string(info)
        .map_err(|e| format!("Failed to serialize evaluation info: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write evaluation file: {}", e))?;
    Ok(())
}

/// Get or initialize the evaluation info.
/// If no evaluation has started yet, this starts it now.
#[tauri::command]
pub fn get_evaluation_info(app: tauri::AppHandle) -> Result<EvaluationInfo, String> {
    let storage = app.state::<Arc<Storage>>();
    let mut info = read_evaluation(&storage);

    if info.started_at.is_none() {
        // First launch - start evaluation now
        info.started_at = Some(chrono::Utc::now().timestamp());
        write_evaluation(&storage, &info)?;
    }

    Ok(info)
}

/// Dismiss the evaluation expired dialog
#[tauri::command]
pub fn dismiss_evaluation_expired(app: tauri::AppHandle) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    let mut info = read_evaluation(&storage);
    info.expired_dismissed = true;
    write_evaluation(&storage, &info)
}

#[tauri::command]
pub fn get_device_name() -> Result<String, String> {
    // Get the computer/hostname
    if let Ok(hostname) = hostname::get() {
        if let Some(name) = hostname.to_str() {
            return Ok(name.to_string());
        }
    }

    // Fallback: try system command
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("scutil")
            .args(&["--get", "ComputerName"])
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("hostname")
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("hostname")
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
}

#[tauri::command]
pub fn get_device_fingerprint() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ioreg")
            .args(&["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .map_err(|e| e.to_string())?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("IOPlatformUUID") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() > 1 {
                    return Ok(parts[1].trim().trim_matches('"').to_string());
                }
            }
        }
        Err("Could not find IOPlatformUUID".to_string())
    }

    #[cfg(target_os = "linux")]
    {
        // Try /etc/machine-id
        if let Ok(content) = std::fs::read_to_string("/etc/machine-id") {
            return Ok(content.trim().to_string());
        }
        // Try /var/lib/dbus/machine-id
        if let Ok(content) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            return Ok(content.trim().to_string());
        }
        Err("Could not find machine-id".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("wmic")
            .args(&["csproduct", "get", "uuid"])
            .output()
            .map_err(|e| e.to_string())?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut lines = output_str.lines();
        // Skip header "UUID"
        lines.next();
        if let Some(uuid) = lines.next() {
            return Ok(uuid.trim().to_string());
        }
        Err("Could not find UUID".to_string())
    }
}
