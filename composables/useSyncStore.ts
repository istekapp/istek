import type { SyncConfig, SyncStatus, SyncChange, GitStatus, GitCommit } from '~/types'
import { invoke } from '@tauri-apps/api/core'

export const useSyncStore = () => {
  // Sync config
  const syncConfig = useState<SyncConfig | null>('syncConfig', () => null)
  
  // Sync status
  const syncStatus = useState<SyncStatus | null>('syncStatus', () => null)
  
  // Git status
  const gitStatus = useState<GitStatus | null>('gitStatus', () => null)
  
  // Git commit history
  const gitCommits = useState<GitCommit[]>('gitCommits', () => [])
  
  // Loading states
  const isLoading = useState<boolean>('syncIsLoading', () => false)
  const isExporting = useState<boolean>('syncIsExporting', () => false)
  const isImporting = useState<boolean>('syncIsImporting', () => false)
  const isCommitting = useState<boolean>('syncIsCommitting', () => false)
  const isPushing = useState<boolean>('syncIsPushing', () => false)
  const isPulling = useState<boolean>('syncIsPulling', () => false)
  
  // Error state
  const error = useState<string | null>('syncError', () => null)
  
  // UI state
  const showSyncScreen = useState<boolean>('showSyncScreen', () => false)
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
      await refreshStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to initialize sync:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  // ============ Status ============
  
  async function refreshStatus(): Promise<void> {
    try {
      error.value = null
      
      // Load sync status
      const status = await invoke<SyncStatus>('sync_get_status')
      syncStatus.value = status
      
      // Load git status if sync is initialized
      if (status.isInitialized) {
        const git = await invoke<GitStatus>('git_get_status')
        gitStatus.value = git
        
        // Load commit history if it's a git repo
        if (git.isRepo) {
          const commits = await invoke<GitCommit[]>('git_get_log', { limit: 20 })
          gitCommits.value = commits
        }
      }
    } catch (e) {
      error.value = String(e)
      console.error('Failed to refresh sync status:', e)
    }
  }

  // ============ Export (App -> Directory) ============
  
  async function exportAll(): Promise<string[]> {
    try {
      isExporting.value = true
      error.value = null
      const files = await invoke<string[]>('sync_export_all')
      await refreshStatus()
      return files
    } catch (e) {
      error.value = String(e)
      console.error('Failed to export:', e)
      throw e
    } finally {
      isExporting.value = false
    }
  }
  
  async function exportCollections(): Promise<string[]> {
    try {
      isExporting.value = true
      error.value = null
      const files = await invoke<string[]>('sync_export_collections')
      await refreshStatus()
      return files
    } catch (e) {
      error.value = String(e)
      console.error('Failed to export collections:', e)
      throw e
    } finally {
      isExporting.value = false
    }
  }
  
  async function exportEnvironments(): Promise<string[]> {
    try {
      isExporting.value = true
      error.value = null
      const files = await invoke<string[]>('sync_export_environments')
      await refreshStatus()
      return files
    } catch (e) {
      error.value = String(e)
      console.error('Failed to export environments:', e)
      throw e
    } finally {
      isExporting.value = false
    }
  }

  // ============ Import (Directory -> App) ============
  
  async function importAll(): Promise<string[]> {
    try {
      isImporting.value = true
      error.value = null
      const items = await invoke<string[]>('sync_import_all')
      await refreshStatus()
      return items
    } catch (e) {
      error.value = String(e)
      console.error('Failed to import:', e)
      throw e
    } finally {
      isImporting.value = false
    }
  }
  
  async function importCollections(): Promise<string[]> {
    try {
      isImporting.value = true
      error.value = null
      const items = await invoke<string[]>('sync_import_collections')
      await refreshStatus()
      return items
    } catch (e) {
      error.value = String(e)
      console.error('Failed to import collections:', e)
      throw e
    } finally {
      isImporting.value = false
    }
  }
  
  async function importEnvironments(): Promise<string[]> {
    try {
      isImporting.value = true
      error.value = null
      const items = await invoke<string[]>('sync_import_environments')
      await refreshStatus()
      return items
    } catch (e) {
      error.value = String(e)
      console.error('Failed to import environments:', e)
      throw e
    } finally {
      isImporting.value = false
    }
  }

  // ============ Git Operations ============
  
  async function gitInit(): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      await invoke('git_init')
      await refreshStatus()
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
      await refreshStatus()
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
      await refreshStatus()
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
      await refreshStatus()
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
      await refreshStatus()
    } catch (e) {
      error.value = String(e)
      console.error('Failed to add remote:', e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  // ============ Environment Shareable ============
  
  async function setEnvironmentShareable(id: string, shareable: boolean): Promise<void> {
    try {
      await invoke('set_environment_shareable', { id, shareable })
    } catch (e) {
      error.value = String(e)
      console.error('Failed to set environment shareable:', e)
      throw e
    }
  }

  // ============ Computed ============
  
  const isInitialized = computed(() => syncStatus.value?.isInitialized ?? false)
  const hasLocalChanges = computed(() => (syncStatus.value?.localChanges?.length ?? 0) > 0)
  const hasExternalChanges = computed(() => (syncStatus.value?.externalChanges?.length ?? 0) > 0)
  const hasUncommittedChanges = computed(() => (gitStatus.value?.uncommittedChanges?.length ?? 0) > 0)
  const canPush = computed(() => gitStatus.value?.hasRemote && (gitStatus.value?.ahead ?? 0) > 0)
  const canPull = computed(() => gitStatus.value?.hasRemote && (gitStatus.value?.behind ?? 0) > 0)

  return {
    // State
    syncConfig,
    syncStatus,
    gitStatus,
    gitCommits,
    isLoading,
    isExporting,
    isImporting,
    isCommitting,
    isPushing,
    isPulling,
    error,
    showSyncScreen,
    commitMessage,
    
    // Computed
    isInitialized,
    hasLocalChanges,
    hasExternalChanges,
    hasUncommittedChanges,
    canPush,
    canPull,
    
    // Config
    loadConfig,
    saveConfig,
    initSync,
    
    // Status
    refreshStatus,
    
    // Export
    exportAll,
    exportCollections,
    exportEnvironments,
    
    // Import
    importAll,
    importCollections,
    importEnvironments,
    
    // Git
    gitInit,
    gitCommit,
    gitPull,
    gitPush,
    gitAddRemote,
    
    // Environment
    setEnvironmentShareable,
  }
}
