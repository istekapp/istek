use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use git2::{Repository, Signature, StatusOptions, IndexAddOption, Cred, RemoteCallbacks, PushOptions, FetchOptions};

use crate::storage::Storage;

/// Create credential callbacks for git operations
fn create_credentials_callback<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed_types| {
        println!("[Git Auth] URL: {}", url);
        println!("[Git Auth] Username from URL: {:?}", username_from_url);
        println!("[Git Auth] Allowed types: {:?}", allowed_types);
        
        // Try SSH agent first
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            println!("[Git Auth] Trying SSH...");
            if let Some(username) = username_from_url {
                // Try SSH agent
                println!("[Git Auth] Trying SSH agent for user: {}", username);
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    println!("[Git Auth] SSH agent succeeded!");
                    return Ok(cred);
                }
                println!("[Git Auth] SSH agent failed, trying key files...");
                
                // Try default SSH key paths
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let ssh_key = PathBuf::from(&home).join(".ssh").join("id_rsa");
                let ssh_key_ed25519 = PathBuf::from(&home).join(".ssh").join("id_ed25519");
                
                if ssh_key_ed25519.exists() {
                    println!("[Git Auth] Trying ed25519 key: {:?}", ssh_key_ed25519);
                    if let Ok(cred) = Cred::ssh_key(username, None, &ssh_key_ed25519, None) {
                        println!("[Git Auth] ed25519 key succeeded!");
                        return Ok(cred);
                    }
                }
                if ssh_key.exists() {
                    println!("[Git Auth] Trying RSA key: {:?}", ssh_key);
                    if let Ok(cred) = Cred::ssh_key(username, None, &ssh_key, None) {
                        println!("[Git Auth] RSA key succeeded!");
                        return Ok(cred);
                    }
                }
            }
        }
        
        // Try credential helper (for HTTPS)
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            println!("[Git Auth] Trying credential helper...");
            match git2::Config::open_default() {
                Ok(config) => {
                    match Cred::credential_helper(&config, url, username_from_url) {
                        Ok(cred) => {
                            println!("[Git Auth] Credential helper succeeded!");
                            return Ok(cred);
                        }
                        Err(e) => {
                            println!("[Git Auth] Credential helper failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("[Git Auth] Failed to open git config: {}", e);
                }
            }
        }
        
        // Default credentials (for anonymous access)
        if allowed_types.contains(git2::CredentialType::DEFAULT) {
            println!("[Git Auth] Trying default credentials...");
            return Cred::default();
        }
        
        println!("[Git Auth] No valid credentials found!");
        Err(git2::Error::from_str("no valid credentials found"))
    });
    callbacks
}

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    pub enabled: bool,
    pub sync_path: String,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            enabled: false,
            sync_path: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub uncommitted_changes: Vec<GitFileChange>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitFileChange {
    pub path: String,
    pub status: String,  // "new", "modified", "deleted", "renamed"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

// ============ Sync Config Commands ============

/// Get the sync path for the active workspace (which is its storage directory)
#[tauri::command]
pub async fn sync_get_config(app: tauri::AppHandle) -> Result<SyncConfig, String> {
    let storage = app.state::<Arc<Storage>>();
    
    // Get active workspace
    let workspace_id = storage.get_active_workspace_id()?
        .ok_or_else(|| "No active workspace".to_string())?;
    
    let workspace = storage.get_workspace(&workspace_id)?
        .ok_or_else(|| "Workspace not found".to_string())?;
    
    // The sync path is the workspace's storage directory
    let sync_path = storage.config_dir()
        .join("workspaces")
        .join(&workspace_id);
    
    Ok(SyncConfig {
        enabled: workspace.sync_path.is_some(),
        // Always return the actual storage directory, not the potentially stale sync_path
        sync_path: sync_path.to_string_lossy().to_string(),
    })
}

/// Save sync config (enable/disable sync for workspace)
#[tauri::command]
pub async fn sync_save_config(app: tauri::AppHandle, config: SyncConfig) -> Result<(), String> {
    let storage = app.state::<Arc<Storage>>();
    
    // Get active workspace
    let workspace_id = storage.get_active_workspace_id()?
        .ok_or_else(|| "No active workspace".to_string())?;
    
    let mut workspace = storage.get_workspace(&workspace_id)?
        .ok_or_else(|| "Workspace not found".to_string())?;
    
    // The sync path is always the workspace's storage directory
    let sync_path = storage.config_dir()
        .join("workspaces")
        .join(&workspace_id);
    
    // Update workspace sync path - use the correct storage path, ignore config.sync_path
    workspace.sync_path = if config.enabled {
        Some(sync_path.to_string_lossy().to_string())
    } else {
        None
    };
    
    storage.update_workspace(&workspace)?;
    
    Ok(())
}

/// Initialize sync for the active workspace
#[tauri::command]
pub async fn sync_init(app: tauri::AppHandle) -> Result<SyncConfig, String> {
    let storage = app.state::<Arc<Storage>>();
    
    // Get active workspace
    let workspace_id = storage.get_active_workspace_id()?
        .ok_or_else(|| "No active workspace".to_string())?;
    
    let mut workspace = storage.get_workspace(&workspace_id)?
        .ok_or_else(|| "Workspace not found".to_string())?;
    
    // The sync path is the workspace's storage directory
    let sync_path = storage.config_dir()
        .join("workspaces")
        .join(&workspace_id);
    
    // Enable sync with workspace directory as sync path
    workspace.sync_path = Some(sync_path.to_string_lossy().to_string());
    storage.update_workspace(&workspace)?;
    
    Ok(SyncConfig {
        enabled: true,
        sync_path: sync_path.to_string_lossy().to_string(),
    })
}

// ============ Git Operations ============

fn get_sync_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let storage = app.state::<Arc<Storage>>();
    
    let workspace_id = storage.get_active_workspace_id()?
        .ok_or_else(|| "No active workspace".to_string())?;
    
    // Always use the storage workspace directory - this is where YAML files are stored
    let sync_path = storage.config_dir()
        .join("workspaces")
        .join(&workspace_id);
    
    Ok(sync_path)
}

/// Initialize a Git repository in the workspace directory
#[tauri::command]
pub async fn git_init(app: tauri::AppHandle) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    
    // Create .gitignore file
    let gitignore_content = r#"# Istek - ignore files that shouldn't be synced
history.yaml
*.local.yaml
*.log
*.tmp
.DS_Store
"#;
    
    let gitignore_path = sync_path.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, gitignore_content)
            .map_err(|e| format!("Failed to create .gitignore: {}", e))?;
    }
    
    Repository::init(&sync_path)
        .map_err(|e| format!("Failed to initialize Git repository: {}", e))?;
    
    Ok(())
}

/// Get Git repository status
#[tauri::command]
pub async fn git_get_status(app: tauri::AppHandle) -> Result<GitStatus, String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = match Repository::open(&sync_path) {
        Ok(repo) => repo,
        Err(_) => {
            return Ok(GitStatus {
                is_repo: false,
                branch: None,
                has_remote: false,
                remote_url: None,
                uncommitted_changes: vec![],
                ahead: 0,
                behind: 0,
            });
        }
    };
    
    // Get current branch
    let branch = repo.head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));
    
    // Check for remote
    let has_remote = repo.find_remote("origin").is_ok();
    let remote_url = repo.find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|s| s.to_string()));
    
    // Get uncommitted changes
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    
    let statuses = repo.statuses(Some(&mut opts))
        .map_err(|e| format!("Failed to get git status: {}", e))?;
    
    let uncommitted_changes: Vec<GitFileChange> = statuses.iter()
        .map(|entry| {
            let status = entry.status();
            let status_str = if status.is_index_new() || status.is_wt_new() {
                "new"
            } else if status.is_index_modified() || status.is_wt_modified() {
                "modified"
            } else if status.is_index_deleted() || status.is_wt_deleted() {
                "deleted"
            } else if status.is_index_renamed() || status.is_wt_renamed() {
                "renamed"
            } else {
                "unknown"
            };
            
            GitFileChange {
                path: entry.path().unwrap_or("").to_string(),
                status: status_str.to_string(),
            }
        })
        .collect();
    
    // Get ahead/behind counts
    let (ahead, behind) = if has_remote {
        get_ahead_behind(&repo).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    
    Ok(GitStatus {
        is_repo: true,
        branch,
        has_remote,
        remote_url,
        uncommitted_changes,
        ahead,
        behind,
    })
}

fn get_ahead_behind(repo: &Repository) -> Result<(u32, u32), git2::Error> {
    let head = repo.head()?;
    let local_oid = head.target().ok_or(git2::Error::from_str("No HEAD target"))?;
    
    let branch_name = head.shorthand().unwrap_or("main");
    let remote_branch = format!("origin/{}", branch_name);
    
    let remote_ref = match repo.find_reference(&format!("refs/remotes/{}", remote_branch)) {
        Ok(r) => r,
        Err(_) => return Ok((0, 0)),
    };
    
    let remote_oid = remote_ref.target().ok_or(git2::Error::from_str("No remote target"))?;
    
    let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;
    Ok((ahead as u32, behind as u32))
}

/// Get Git commit log
#[tauri::command]
pub async fn git_get_log(app: tauri::AppHandle, limit: Option<usize>) -> Result<Vec<GitCommit>, String> {
    let sync_path = get_sync_path(&app)?;
    let limit = limit.unwrap_or(20);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let mut revwalk = repo.revwalk()
        .map_err(|e| format!("Failed to create revwalk: {}", e))?;
    
    revwalk.push_head()
        .map_err(|e| format!("Failed to push HEAD: {}", e))?;
    
    let commits: Vec<GitCommit> = revwalk
        .take(limit)
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let author_name = commit.author().name().unwrap_or("Unknown").to_string();
            let message = commit.message().unwrap_or("").trim().to_string();
            let timestamp = commit.time().seconds();
            Some(GitCommit {
                id: oid.to_string()[..7].to_string(),
                message,
                author: author_name,
                timestamp,
            })
        })
        .collect();
    
    Ok(commits)
}

/// Commit all changes
#[tauri::command]
pub async fn git_commit(app: tauri::AppHandle, message: String) -> Result<String, String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    // Add all files to index
    let mut index = repo.index()
        .map_err(|e| format!("Failed to get index: {}", e))?;
    
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Failed to add files: {}", e))?;
    
    // Also add deleted files
    index.update_all(["."].iter(), None)
        .map_err(|e| format!("Failed to update index: {}", e))?;
    
    index.write()
        .map_err(|e| format!("Failed to write index: {}", e))?;
    
    let tree_oid = index.write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e))?;
    
    let tree = repo.find_tree(tree_oid)
        .map_err(|e| format!("Failed to find tree: {}", e))?;
    
    // Get signature from git config or use defaults
    let sig = repo.signature()
        .or_else(|_| {
            // Fallback: try to get from global git config
            git2::Config::open_default()
                .and_then(|config| {
                    let name = config.get_string("user.name").unwrap_or_else(|_| "Istek User".to_string());
                    let email = config.get_string("user.email").unwrap_or_else(|_| "user@istek.app".to_string());
                    Signature::now(&name, &email)
                })
        })
        .or_else(|_| Signature::now("Istek User", "user@istek.app"))
        .map_err(|e| format!("Failed to create signature: {}", e))?;
    
    // Get parent commit if exists
    let parent = repo.head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    
    let commit_oid = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &tree,
        &parents,
    ).map_err(|e| format!("Failed to create commit: {}", e))?;
    
    Ok(commit_oid.to_string()[..7].to_string())
}

/// Add remote origin
#[tauri::command]
pub async fn git_add_remote(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    // Remove existing origin if present
    let _ = repo.remote_delete("origin");
    
    repo.remote("origin", &url)
        .map_err(|e| format!("Failed to add remote: {}", e))?;
    
    Ok(())
}

/// Push to remote
#[tauri::command]
pub async fn git_push(app: tauri::AppHandle) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    println!("[Git Push] Sync path: {:?}", sync_path);
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;
    
    let remote_url = remote.url().unwrap_or("unknown");
    println!("[Git Push] Remote URL: {}", remote_url);
    
    let branch = repo.head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());
    
    println!("[Git Push] Branch: {}", branch);
    
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
    println!("[Git Push] Refspec: {}", refspec);
    
    // Create push options with credentials callback
    let callbacks = create_credentials_callback();
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    
    println!("[Git Push] Starting push...");
    remote.push(&[&refspec], Some(&mut push_options))
        .map_err(|e| {
            println!("[Git Push] Push failed: {}", e);
            format!("Failed to push: {}", e)
        })?;
    
    println!("[Git Push] Push successful!");
    Ok(())
}

/// Pull from remote
#[tauri::command]
pub async fn git_pull(app: tauri::AppHandle) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("Failed to find remote: {}", e))?;
    
    let branch = repo.head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());
    
    // Fetch with credentials callback
    let callbacks = create_credentials_callback();
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    remote.fetch(&[&branch], Some(&mut fetch_options), None)
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    // Get fetch head
    let fetch_head = repo.find_reference("FETCH_HEAD")
        .map_err(|e| format!("Failed to find FETCH_HEAD: {}", e))?;
    
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)
        .map_err(|e| format!("Failed to get fetch commit: {}", e))?;
    
    // Merge
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Failed to analyze merge: {}", e))?;
    
    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{}", branch);
        let mut reference = repo.find_reference(&refname)
            .map_err(|e| format!("Failed to find reference: {}", e))?;
        
        reference.set_target(fetch_commit.id(), "Fast-forward")
            .map_err(|e| format!("Failed to set target: {}", e))?;
        
        repo.set_head(&refname)
            .map_err(|e| format!("Failed to set HEAD: {}", e))?;
        
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Failed to checkout: {}", e))?;
    } else if analysis.is_normal() {
        return Err("Merge required - please resolve conflicts manually".to_string());
    }
    
    Ok(())
}

/// List branches
#[tauri::command]
pub async fn git_list_branches(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let branches = repo.branches(Some(git2::BranchType::Local))
        .map_err(|e| format!("Failed to list branches: {}", e))?;
    
    let branch_names: Vec<String> = branches
        .filter_map(|b| {
            let (branch, _) = b.ok()?;
            branch.name().ok()?.map(|s| s.to_string())
        })
        .collect();
    
    Ok(branch_names)
}

/// Create a new branch
#[tauri::command]
pub async fn git_create_branch(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let head = repo.head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    
    let commit = head.peel_to_commit()
        .map_err(|e| format!("Failed to get commit: {}", e))?;
    
    repo.branch(&name, &commit, false)
        .map_err(|e| format!("Failed to create branch: {}", e))?;
    
    Ok(())
}

/// Switch to a branch
#[tauri::command]
pub async fn git_switch_branch(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    let refname = format!("refs/heads/{}", name);
    
    repo.set_head(&refname)
        .map_err(|e| format!("Failed to set HEAD: {}", e))?;
    
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .map_err(|e| format!("Failed to checkout: {}", e))?;
    
    Ok(())
}

/// Get files changed in a specific commit
#[tauri::command]
pub async fn git_get_commit_files(app: tauri::AppHandle, commit_id: String) -> Result<Vec<GitFileChange>, String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    // Find the commit using revparse (supports short hashes)
    let obj = repo.revparse_single(&commit_id)
        .map_err(|e| format!("Failed to find commit '{}': {}", commit_id, e))?;
    
    let commit = obj.peel_to_commit()
        .map_err(|e| format!("Failed to get commit: {}", e))?;
    
    let commit_tree = commit.tree()
        .map_err(|e| format!("Failed to get commit tree: {}", e))?;
    
    // Get parent tree (or empty tree for initial commit)
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)
            .map_err(|e| format!("Failed to get parent commit: {}", e))?
            .tree()
            .map_err(|e| format!("Failed to get parent tree: {}", e))?)
    } else {
        None
    };
    
    // Diff between parent and commit
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
        .map_err(|e| format!("Failed to create diff: {}", e))?;
    
    let mut files = Vec::new();
    
    diff.foreach(
        &mut |delta, _| {
            let status = match delta.status() {
                git2::Delta::Added => "new",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Modified => "modified",
                git2::Delta::Renamed => "renamed",
                _ => "modified",
            };
            
            let path = delta.new_file().path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            
            files.push(GitFileChange {
                path,
                status: status.to_string(),
            });
            
            true
        },
        None,
        None,
        None,
    ).map_err(|e| format!("Failed to iterate diff: {}", e))?;
    
    Ok(files)
}

/// Get diff for a specific file in a commit
#[tauri::command]
pub async fn git_get_file_diff(app: tauri::AppHandle, commit_id: String, file_path: String) -> Result<String, String> {
    let sync_path = get_sync_path(&app)?;
    
    let repo = Repository::open(&sync_path)
        .map_err(|e| format!("Failed to open repository: {}", e))?;
    
    // Find the commit using revparse (supports short hashes)
    let obj = repo.revparse_single(&commit_id)
        .map_err(|e| format!("Failed to find commit '{}': {}", commit_id, e))?;
    
    let commit = obj.peel_to_commit()
        .map_err(|e| format!("Failed to get commit: {}", e))?;
    
    let commit_tree = commit.tree()
        .map_err(|e| format!("Failed to get commit tree: {}", e))?;
    
    // Get parent tree (or empty tree for initial commit)
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)
            .map_err(|e| format!("Failed to get parent commit: {}", e))?
            .tree()
            .map_err(|e| format!("Failed to get parent tree: {}", e))?)
    } else {
        None
    };
    
    // Create diff options to filter by path
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(&file_path);
    
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), Some(&mut diff_opts))
        .map_err(|e| format!("Failed to create diff: {}", e))?;
    
    // Generate patch
    let mut diff_text = String::new();
    
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            diff_text.push(origin);
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }
        true
    }).map_err(|e| format!("Failed to generate diff: {}", e))?;
    
    Ok(diff_text)
}
