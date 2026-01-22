import type { SyncConfig, GitStatus, GitCommit } from '~/types'
import { invoke } from '@tauri-apps/api/core'

export const useSyncStore = () => {
  // Sync config
  const syncConfig = useState<SyncConfig | null>('syncConfig', () => null)
  
  // Git status
  const gitStatus = useState<GitStatus | null>('gitStatus', () => null)
  
  // Git commit history
  const gitCommits = useState<GitCommit[]>('gitCommits', () => [])
  
  // Loading states
  const isLoading = useState<boolean>('syncIsLoading', () => false)
  const isCommitting = useState<boolean>('syncIsCommitting', () => false)
  const isPushing = useState<boolean>('syncIsPushing', () => false)
  const isPulling = useState<boolean>('syncIsPulling', () => false)
  
  // Error state
  const error = useState<string | null>('syncError', () => null)
  
  // UI state
  const commitMessage = useState<string>('syncCommitMessage', () => '')

  // ============ Config Management ============
  
  async function loadConfig(): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      const config = await invoke<SyncConfig>('sync_get_config')
      syncConfig.value = config
    } catch (e) {
      error.value = String(e)
      console.error('Failed to load sync config:', e)
    } finally {
      isLoading.value = false
    }
  }
  
  async function saveConfig(config: SyncConfig): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      await invoke('sync_save_config', { config })
      syncConfig.value = config
    } catch (e) {
      error.value = String(e)
      console.error('Failed to save sync config:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }
  
  async function initSync(): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      const config = await invoke<SyncConfig>('sync_init')
      syncConfig.value = config
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to initialize sync:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  // ============ Git Status ============
  
  async function refreshGitStatus(): Promise<void> {
    try {
      error.value = null
      const git = await invoke<GitStatus>('git_get_status')
      gitStatus.value = git
      
      // Load commit history if it's a git repo
      if (git.isRepo) {
        try {
          const commits = await invoke<GitCommit[]>('git_get_log', { limit: 20 })
          gitCommits.value = commits
        } catch (e) {
          // No commits yet is fine
          gitCommits.value = []
        }
      }
    } catch (e) {
      error.value = String(e)
      console.error('Failed to refresh git status:', e)
    }
  }

  // ============ Git Operations ============
  
  async function gitInit(): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      await invoke('git_init')
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to initialize git:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }
  
  async function gitCommit(message: string): Promise<string> {
    try {
      isCommitting.value = true
      error.value = null
      const commitId = await invoke<string>('git_commit', { message })
      commitMessage.value = ''
      await refreshGitStatus()
      return commitId
    } catch (e) {
      error.value = String(e)
      console.error('Failed to commit:', e)
      throw e
    } finally {
      isCommitting.value = false
    }
  }
  
  async function gitPull(): Promise<void> {
    try {
      isPulling.value = true
      error.value = null
      await invoke('git_pull')
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to pull:', e)
      throw e
    } finally {
      isPulling.value = false
    }
  }
  
  async function gitPush(): Promise<void> {
    try {
      isPushing.value = true
      error.value = null
      await invoke('git_push')
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to push:', e)
      throw e
    } finally {
      isPushing.value = false
    }
  }
  
  async function gitAddRemote(url: string): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      await invoke('git_add_remote', { url })
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to add remote:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  // ============ Computed ============
  
  const isGitRepo = computed(() => gitStatus.value?.isRepo ?? false)
  const isInitialized = computed(() => syncConfig.value?.enabled ?? false)
  const hasUncommittedChanges = computed(() => (gitStatus.value?.uncommittedChanges?.length ?? 0) > 0)
  const uncommittedCount = computed(() => gitStatus.value?.uncommittedChanges?.length ?? 0)
  const canPush = computed(() => gitStatus.value?.hasRemote && (gitStatus.value?.ahead ?? 0) > 0)
  const canPull = computed(() => gitStatus.value?.hasRemote && (gitStatus.value?.behind ?? 0) > 0)
  
  // UI State
  const showSyncScreen = useState<boolean>('showSyncScreen', () => false)
  const isExporting = useState<boolean>('syncIsExporting', () => false)
  const isImporting = useState<boolean>('syncIsImporting', () => false)
  
  // ============ Export/Import Operations ============
  
  async function exportAll(): Promise<string[]> {
    try {
      isExporting.value = true
      error.value = null
      const files = await invoke<string[]>('sync_export_collections')
      await refreshGitStatus()
      return files
    } catch (e) {
      error.value = String(e)
      console.error('Failed to export:', e)
      throw e
    } finally {
      isExporting.value = false
    }
  }
  
  async function importAll(): Promise<number> {
    try {
      isImporting.value = true
      error.value = null
      const count = await invoke<number>('sync_import_collections')
      return count
    } catch (e) {
      error.value = String(e)
      console.error('Failed to import:', e)
      throw e
    } finally {
      isImporting.value = false
    }
  }
  
  async function refreshStatus(): Promise<void> {
    await loadConfig()
    await refreshGitStatus()
  }

  return {
    // State
    syncConfig,
    gitStatus,
    gitCommits,
    isLoading,
    isCommitting,
    isPushing,
    isPulling,
    isExporting,
    isImporting,
    error,
    commitMessage,
    showSyncScreen,
    
    // Computed
    isGitRepo,
    isInitialized,
    hasUncommittedChanges,
    uncommittedCount,
    canPush,
    canPull,
    
    // Config
    loadConfig,
    saveConfig,
    initSync,
    
    // Git Status
    refreshGitStatus,
    refreshStatus,
    
    // Git Operations
    gitInit,
    gitCommit,
    gitPull,
    gitPush,
    gitAddRemote,
    
    // Export/Import
    exportAll,
    importAll,
  }
}
