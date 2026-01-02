import type { Workspace, GitStatus, GitCommit } from '~/types'
import { invoke } from '@tauri-apps/api/core'

export const useWorkspaceStore = () => {
  // Workspaces
  const workspaces = useState<Workspace[]>('workspaces', () => [])
  const activeWorkspaceId = useState<string | null>('activeWorkspaceId', () => null)
  
  // Git state for active workspace
  const gitStatus = useState<GitStatus | null>('workspaceGitStatus', () => null)
  const gitCommits = useState<GitCommit[]>('workspaceGitCommits', () => [])
  const branches = useState<string[]>('workspaceBranches', () => [])
  
  // Loading states
  const isLoading = useState<boolean>('workspaceIsLoading', () => false)
  const isCreating = useState<boolean>('workspaceIsCreating', () => false)
  const isGitLoading = useState<boolean>('workspaceGitLoading', () => false)
  
  // UI state
  const showCreateModal = useState<boolean>('showCreateWorkspaceModal', () => false)
  const showGitMenu = useState<boolean>('showGitMenu', () => false)
  const showCommitModal = useState<boolean>('showCommitModal', () => false)
  const showHistoryModal = useState<boolean>('showHistoryModal', () => false)
  
  // Error state
  const error = useState<string | null>('workspaceError', () => null)

  // Computed
  const activeWorkspace = computed(() => 
    workspaces.value.find(w => w.id === activeWorkspaceId.value)
  )

  const hasSyncEnabled = computed(() => 
    activeWorkspace.value?.syncPath != null
  )

  const hasGitRepo = computed(() => 
    gitStatus.value?.isRepo === true
  )

  const currentBranch = computed(() => 
    gitStatus.value?.branch || 'main'
  )

  const hasUncommittedChanges = computed(() => 
    (gitStatus.value?.uncommittedChanges?.length ?? 0) > 0
  )

  const uncommittedCount = computed(() => 
    gitStatus.value?.uncommittedChanges?.length ?? 0
  )

  // ============ Workspace Management ============

  async function loadWorkspaces(): Promise<void> {
    // Workspaces are loaded via useAppStore.loadDataFromDatabase
    // This is just a helper to refresh
  }

  async function setWorkspaces(data: { workspaces: Workspace[], activeWorkspaceId: string | null }): Promise<void> {
    console.log('[setWorkspaces] Setting workspaces, activeId:', data.activeWorkspaceId)
    workspaces.value = data.workspaces
    activeWorkspaceId.value = data.activeWorkspaceId
    
    // If there's an active workspace with sync enabled, update sync config and load git status
    const active = data.workspaces.find(w => w.id === data.activeWorkspaceId)
    console.log('[setWorkspaces] Active workspace:', active?.name, 'syncPath:', active?.syncPath)
    
    if (active?.syncPath) {
      // Update sync config to use workspace's sync path
      try {
        console.log('[setWorkspaces] Saving sync config for path:', active.syncPath)
        await invoke('sync_save_config', {
          config: {
            enabled: true,
            syncPath: active.syncPath,
            syncCollections: true,
            syncEnvironments: true,
            syncGlobalVariables: true
          }
        })
        await refreshGitStatus()
      } catch (e) {
        console.error('Failed to update sync config:', e)
      }
    } else {
      // Clear git status if no sync path
      gitStatus.value = null
    }
  }

  async function createWorkspace(name: string, syncPath?: string): Promise<Workspace> {
    try {
      isCreating.value = true
      error.value = null
      
      const workspace = await invoke<Workspace>('create_workspace', { 
        name, 
        syncPath: syncPath || null 
      })
      
      workspaces.value = [...workspaces.value, workspace]
      activeWorkspaceId.value = workspace.id
      
      // If sync is enabled, load git status
      if (workspace.syncPath) {
        await refreshGitStatus()
      }
      
      return workspace
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isCreating.value = false
    }
  }

  async function switchWorkspace(id: string): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      
      // Set active workspace in backend
      await invoke('set_active_workspace', { id })
      activeWorkspaceId.value = id
      
      // Clear git state
      gitStatus.value = null
      gitCommits.value = []
      
      // Load workspace-specific data from backend
      const data = await invoke<{
        workspaces: { id: string; name: string; syncPath?: string; isDefault: boolean; createdAt: number }[]
        activeWorkspaceId: string | null
        collections: Array<{ id: string; name: string; requests: any; folders?: any; settings?: any; createdAt: number }>
        history: Array<{ id: string; request: any; response?: any; timestamp: number }>
        globalVariables: Array<{ id: string; key: string; value: string; description?: string; isSecret: boolean; secretProvider?: any; enabled: boolean }>
        environments: Array<{ id: string; name: string; color: string; variables: any[]; isDefault?: boolean; createdAt: number }>
        activeEnvironmentId: string | null
      }>('load_workspace_data', { workspaceId: id })
      
      // Update app store with new collections and history
      const appStore = useAppStore()
      appStore.setWorkspaceData({
        collections: data.collections,
        history: data.history
      })
      
      // Update variable store with new variables and environments
      const variableStore = useVariableStore()
      variableStore.setWorkspaceData({
        globalVariables: data.globalVariables,
        environments: data.environments,
        activeEnvironmentId: data.activeEnvironmentId
      })
      
      // If the new workspace has sync enabled, update the sync config and load git status
      const workspace = workspaces.value.find(w => w.id === id)
      if (workspace?.syncPath) {
        // Update sync config to use workspace's sync path
        await invoke('sync_save_config', {
          config: {
            enabled: true,
            syncPath: workspace.syncPath,
            syncCollections: true,
            syncEnvironments: true,
            syncGlobalVariables: true
          }
        })
        await refreshGitStatus()
      } else {
        // Disable sync for workspaces without sync path
        await invoke('sync_save_config', {
          config: {
            enabled: false,
            syncPath: '',
            syncCollections: true,
            syncEnvironments: true,
            syncGlobalVariables: true
          }
        })
      }
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  async function deleteWorkspace(id: string): Promise<void> {
    try {
      isLoading.value = true
      error.value = null
      
      await invoke('delete_workspace', { id })
      workspaces.value = workspaces.value.filter(w => w.id !== id)
      
      // If deleted workspace was active, activeWorkspaceId will be updated by backend
      if (activeWorkspaceId.value === id) {
        const defaultWorkspace = workspaces.value.find(w => w.isDefault)
        activeWorkspaceId.value = defaultWorkspace?.id || workspaces.value[0]?.id || null
      }
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  async function getDefaultSyncPath(name: string): Promise<string> {
    try {
      return await invoke<string>('get_default_sync_path', { name })
    } catch (e) {
      console.error('Failed to get default sync path:', e)
      return ''
    }
  }

  // ============ Git Operations ============

  async function refreshGitStatus(): Promise<void> {
    const workspace = activeWorkspace.value
    console.log('[refreshGitStatus] activeWorkspaceId:', activeWorkspaceId.value)
    console.log('[refreshGitStatus] workspaces:', workspaces.value.map(w => ({ id: w.id, name: w.name })))
    console.log('[refreshGitStatus] workspace:', workspace?.name, 'syncPath:', workspace?.syncPath)
    
    if (!workspace?.syncPath) {
      console.log('[refreshGitStatus] No sync path, clearing gitStatus')
      gitStatus.value = null
      return
    }

    try {
      isGitLoading.value = true
      error.value = null
      
      console.log('[refreshGitStatus] Calling git_get_status...')
      const status = await invoke<GitStatus>('git_get_status')
      console.log('[refreshGitStatus] Got status:', status)
      gitStatus.value = status
      
      if (status.isRepo) {
        console.log('[refreshGitStatus] Is repo, loading commits and branches...')
        
        // Load commit history - may fail if no commits yet
        try {
          const commits = await invoke<GitCommit[]>('git_get_log', { limit: 20 })
          gitCommits.value = commits
        } catch (logError) {
          console.log('[refreshGitStatus] No commits yet:', logError)
          gitCommits.value = []
        }
        
        // Load branches - may fail if no commits yet
        try {
          const branchList = await invoke<string[]>('git_list_branches')
          branches.value = branchList.length > 0 ? branchList : ['main']
        } catch (branchError) {
          console.log('[refreshGitStatus] No branches yet:', branchError)
          branches.value = ['main']
        }
        
        console.log('[refreshGitStatus] Branches:', branches.value)
      }
    } catch (e) {
      console.error('[refreshGitStatus] Failed to refresh git status:', e)
      // Don't set error for git operations - just clear status
      gitStatus.value = null
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitInit(): Promise<void> {
    console.log('[gitInit] Starting git init...')
    try {
      isGitLoading.value = true
      error.value = null
      
      console.log('[gitInit] Calling invoke git_init...')
      await invoke('git_init')
      console.log('[gitInit] git_init completed, now refreshing status...')
      await refreshGitStatus()
      console.log('[gitInit] refreshGitStatus completed. hasGitRepo:', hasGitRepo.value)
    } catch (e) {
      console.error('[gitInit] Error:', e)
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitCommit(message: string): Promise<string> {
    try {
      isGitLoading.value = true
      error.value = null
      
      const commitId = await invoke<string>('git_commit', { message })
      await refreshGitStatus()
      
      return commitId
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitPush(): Promise<void> {
    try {
      isGitLoading.value = true
      error.value = null
      
      await invoke('git_push')
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitPull(): Promise<void> {
    try {
      isGitLoading.value = true
      error.value = null
      
      await invoke('git_pull')
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitAddRemote(url: string): Promise<void> {
    try {
      isGitLoading.value = true
      error.value = null
      
      await invoke('git_add_remote', { url })
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitCreateBranch(name: string): Promise<void> {
    try {
      isGitLoading.value = true
      error.value = null
      
      await invoke('git_create_branch', { name })
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  async function gitSwitchBranch(name: string): Promise<void> {
    try {
      isGitLoading.value = true
      error.value = null
      
      await invoke('git_switch_branch', { name })
      await refreshGitStatus()
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isGitLoading.value = false
    }
  }

  // ============ Sync Operations ============

  async function exportAll(): Promise<string[]> {
    try {
      isLoading.value = true
      error.value = null
      
      const files = await invoke<string[]>('sync_export_all')
      await refreshGitStatus()
      
      return files
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  async function importAll(): Promise<string[]> {
    try {
      isLoading.value = true
      error.value = null
      
      const items = await invoke<string[]>('sync_import_all')
      await refreshGitStatus()
      
      return items
    } catch (e) {
      error.value = String(e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  return {
    // State
    workspaces,
    activeWorkspaceId,
    gitStatus,
    gitCommits,
    branches,
    isLoading,
    isCreating,
    isGitLoading,
    error,
    
    // UI State
    showCreateModal,
    showGitMenu,
    showCommitModal,
    showHistoryModal,
    
    // Computed
    activeWorkspace,
    hasSyncEnabled,
    hasGitRepo,
    currentBranch,
    hasUncommittedChanges,
    uncommittedCount,
    
    // Workspace Management
    loadWorkspaces,
    setWorkspaces,
    createWorkspace,
    switchWorkspace,
    deleteWorkspace,
    getDefaultSyncPath,
    
    // Git Operations
    refreshGitStatus,
    gitInit,
    gitCommit,
    gitPush,
    gitPull,
    gitAddRemote,
    gitCreateBranch,
    gitSwitchBranch,
    
    // Sync Operations
    exportAll,
    importAll,
  }
}
