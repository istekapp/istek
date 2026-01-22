use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};

// Check for updates every 4 hours
const UPDATE_CHECK_INTERVAL_SECS: u64 = 4 * 60 * 60;

// GitHub repository info
const GITHUB_OWNER: &str = "istekapp";
const GITHUB_REPO: &str = "istek";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub release_notes: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
}

pub struct UpdateChecker {
    last_check: Mutex<Option<Instant>>,
    cached_update: Mutex<Option<UpdateInfo>>,
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self {
            last_check: Mutex::new(None),
            cached_update: Mutex::new(None),
        }
    }

    /// Get current app version from Cargo.toml
    pub fn get_current_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Check if enough time has passed since last check
    fn should_check(&self) -> bool {
        let last_check = self.last_check.lock().unwrap();
        match *last_check {
            None => true,
            Some(instant) => instant.elapsed() > Duration::from_secs(UPDATE_CHECK_INTERVAL_SECS),
        }
    }

    /// Update the last check timestamp
    fn update_last_check(&self) {
        let mut last_check = self.last_check.lock().unwrap();
        *last_check = Some(Instant::now());
    }

    /// Get cached update info
    pub fn get_cached_update(&self) -> Option<UpdateInfo> {
        self.cached_update.lock().unwrap().clone()
    }

    /// Cache update info
    fn cache_update(&self, update: Option<UpdateInfo>) {
        let mut cached = self.cached_update.lock().unwrap();
        *cached = update;
    }

    /// Fetch latest release from GitHub
    async fn fetch_latest_release(&self) -> Result<GitHubRelease, String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            GITHUB_OWNER, GITHUB_REPO
        );

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", format!("istek/{}", Self::get_current_version()))
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch release: {}", e))?;

        if response.status() == 404 {
            return Err("No releases found".to_string());
        }

        if !response.status().is_success() {
            return Err(format!("GitHub API error: {}", response.status()));
        }

        response
            .json::<GitHubRelease>()
            .await
            .map_err(|e| format!("Failed to parse release: {}", e))
    }

    /// Compare versions (semver-like comparison)
    fn is_newer_version(current: &str, latest: &str) -> bool {
        // Remove 'v' prefix if present
        let current = current.trim_start_matches('v');
        let latest = latest.trim_start_matches('v');

        let parse_version = |v: &str| -> Vec<u32> {
            v.split('.')
                .filter_map(|s| s.parse::<u32>().ok())
                .collect()
        };

        let current_parts = parse_version(current);
        let latest_parts = parse_version(latest);

        for (c, l) in current_parts.iter().zip(latest_parts.iter()) {
            if l > c {
                return true;
            }
            if l < c {
                return false;
            }
        }

        // If latest has more parts (e.g., 1.0.0 vs 1.0.0.1)
        latest_parts.len() > current_parts.len()
    }

    /// Check for updates (respects rate limiting)
    pub async fn check_for_update(&self, force: bool) -> Result<Option<UpdateInfo>, String> {
        // Skip if checked recently (unless forced)
        if !force && !self.should_check() {
            return Ok(self.get_cached_update());
        }

        self.update_last_check();

        let release = match self.fetch_latest_release().await {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to check for updates: {}", e);
                return Ok(None);
            }
        };

        // Skip prereleases and drafts
        if release.prerelease || release.draft {
            return Ok(None);
        }

        let current_version = Self::get_current_version();
        let latest_version = release.tag_name.clone();
        let has_update = Self::is_newer_version(&current_version, &latest_version);

        let update_info = UpdateInfo {
            current_version,
            latest_version,
            has_update,
            release_url: release.html_url,
            release_notes: release.body,
            published_at: release.published_at,
        };

        self.cache_update(Some(update_info.clone()));

        Ok(Some(update_info))
    }

    /// Check and emit update event if available
    pub async fn check_and_notify<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        if let Some(update_info) = self.check_for_update(false).await? {
            if update_info.has_update {
                let _ = app.emit("update-available", &update_info);
            }
        }
        Ok(())
    }
}

// Tauri commands

/// Check for updates (can be forced)
#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    force: Option<bool>,
) -> Result<Option<UpdateInfo>, String> {
    let checker = app.state::<UpdateChecker>();
    checker.check_for_update(force.unwrap_or(false)).await
}

/// Get current app version
#[tauri::command]
pub fn get_app_version() -> String {
    UpdateChecker::get_current_version()
}

/// Dismiss update notification (just clears the cache)
#[tauri::command]
pub fn dismiss_update(app: tauri::AppHandle) {
    let checker = app.state::<UpdateChecker>();
    let mut cached = checker.cached_update.lock().unwrap();
    if let Some(ref mut update) = *cached {
        update.has_update = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(UpdateChecker::is_newer_version("1.0.0", "1.0.1"));
        assert!(UpdateChecker::is_newer_version("1.0.0", "1.1.0"));
        assert!(UpdateChecker::is_newer_version("1.0.0", "2.0.0"));
        assert!(UpdateChecker::is_newer_version("1.0.0", "v1.0.1"));
        assert!(UpdateChecker::is_newer_version("v1.0.0", "1.0.1"));
        
        assert!(!UpdateChecker::is_newer_version("1.0.1", "1.0.0"));
        assert!(!UpdateChecker::is_newer_version("1.0.0", "1.0.0"));
        assert!(!UpdateChecker::is_newer_version("2.0.0", "1.9.9"));
    }
}
