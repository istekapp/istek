<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps<{
  show: boolean
}>()

const emit = defineEmits<{
  'update:show': [value: boolean]
}>()

const workspaceStore = useWorkspaceStore()

// Create local computed refs that reference the store's useState refs
const activeWorkspace = computed(() => 
  workspaceStore.workspaces.value.find(w => w.id === workspaceStore.activeWorkspaceId.value)
)

// Form state
const workspaceName = ref('')
const enableSync = ref(false)
const syncPath = ref('')
const isSaving = ref(false)
const error = ref<string | null>(null)

// Initialize form when modal opens
watch(() => props.show, (show) => {
  const workspace = activeWorkspace.value
  if (show && workspace) {
    workspaceName.value = workspace.name
    syncPath.value = workspace.syncPath || ''
    enableSync.value = !!workspace.syncPath
    error.value = null
  }
})

// Update default sync path when enabling sync
watch(enableSync, async (enabled) => {
  if (enabled && workspaceName.value && !syncPath.value) {
    syncPath.value = await workspaceStore.getDefaultSyncPath(workspaceName.value)
  }
})

const handleChooseDirectory = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Choose sync directory'
    })
    
    if (selected) {
      const safeName = workspaceName.value
        .replace(/[^a-zA-Z0-9-_]/g, '-')
        .toLowerCase()
      syncPath.value = `${selected}/${safeName || 'workspace'}`
    }
  } catch (e) {
    console.error('Failed to open directory picker:', e)
  }
}

const clearSyncPath = () => {
  syncPath.value = ''
}

const handleSave = async () => {
  const workspace = activeWorkspace.value
  if (!workspaceName.value.trim() || !workspace) return
  
  try {
    isSaving.value = true
    error.value = null
    
    const newSyncPath = enableSync.value ? syncPath.value : null
    
    // Update workspace (backend will create directories if needed)
    await invoke('update_workspace', {
      workspace: {
        id: workspace.id,
        name: workspaceName.value.trim(),
        syncPath: newSyncPath,
        isDefault: workspace.isDefault,
        createdAt: workspace.createdAt
      }
    })
    
    // Update local state - create new array to trigger reactivity
    const updatedWorkspaces = workspaceStore.workspaces.value.map(w => {
      if (w.id === workspace.id) {
        return {
          ...w,
          name: workspaceName.value.trim(),
          syncPath: newSyncPath || undefined
        }
      }
      return w
    })
    workspaceStore.workspaces.value = updatedWorkspaces
    
    // Update sync config if sync path is set
    if (newSyncPath) {
      await invoke('sync_save_config', {
        config: {
          enabled: true,
          syncPath: newSyncPath
        }
      })
    } else {
      // Disable sync config if sync path removed
      await invoke('sync_save_config', {
        config: {
          enabled: false,
          syncPath: ''
        }
      })
    }
    
    // Refresh git status
    await workspaceStore.refreshGitStatus()
    
    emit('update:show', false)
  } catch (e) {
    error.value = String(e)
  } finally {
    isSaving.value = false
  }
}

const handleClose = () => {
  emit('update:show', false)
}
</script>

<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="handleClose"
  >
    <div class="bg-background border border-border rounded-lg w-[500px] shadow-xl">
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-lg font-semibold">Workspace Settings</h2>
        <button
          class="p-1 hover:bg-accent rounded"
          @click="handleClose"
        >
          <Icon name="lucide:x" class="w-5 h-5" />
        </button>
      </div>

      <!-- Content -->
      <div class="p-4 space-y-4">
        <!-- Error Alert -->
        <div v-if="error" class="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-sm text-destructive flex items-center gap-2">
          <Icon name="lucide:alert-circle" class="w-4 h-4 flex-shrink-0" />
          {{ error }}
        </div>

        <!-- Name Input -->
        <div class="space-y-2">
          <label class="text-sm font-medium">Name</label>
          <input
            v-model="workspaceName"
            type="text"
            placeholder="My Workspace"
            class="w-full bg-secondary/30 border border-border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
          />
        </div>

        <!-- Sync to filesystem (collapsible) -->
        <div class="border border-border rounded-md">
          <button
            class="w-full flex items-center gap-2 p-3 text-left hover:bg-accent/50 transition-colors"
            @click="enableSync = !enableSync"
          >
            <Icon 
              :name="enableSync ? 'lucide:chevron-down' : 'lucide:chevron-right'" 
              class="w-4 h-4 text-muted-foreground" 
            />
            <Icon 
              :name="enableSync ? 'lucide:folder-git-2' : 'lucide:folder'" 
              class="w-4 h-4" 
              :class="enableSync ? 'text-primary' : 'text-muted-foreground'"
            />
            <span class="text-sm font-medium">Sync to filesystem</span>
            <span v-if="activeWorkspace?.syncPath" class="ml-auto text-xs text-green-500">Enabled</span>
          </button>
          
          <div v-if="enableSync" class="px-3 pb-3 space-y-3">
            <div class="bg-secondary/20 rounded-md p-3 text-sm text-muted-foreground">
              When enabled, workspace data syncs to the chosen folder as text files, ideal for backup and Git collaboration.
            </div>
            
            <!-- Directory Selection -->
            <div class="flex items-center gap-2">
              <button
                class="px-3 py-1.5 text-sm border border-primary text-primary rounded hover:bg-primary/10 transition-colors"
                @click="handleChooseDirectory"
              >
                Change Directory
              </button>
              
              <button
                v-if="syncPath"
                class="p-1.5 hover:bg-accent rounded"
                @click="clearSyncPath"
                title="Clear path"
              >
                <Icon name="lucide:x" class="w-4 h-4" />
              </button>
              
              <span v-if="syncPath" class="text-sm text-muted-foreground truncate flex-1 font-mono">
                {{ syncPath.length > 35 ? '...' + syncPath.slice(-32) : syncPath }}
              </span>
              <span v-else class="text-sm text-muted-foreground italic">No directory selected</span>
            </div>
            
            <!-- Warning if changing sync path -->
            <div v-if="activeWorkspace?.syncPath && syncPath !== activeWorkspace.syncPath" class="bg-yellow-500/10 border border-yellow-500/20 rounded-md p-3 text-sm text-yellow-600 flex items-start gap-2">
              <Icon name="lucide:alert-triangle" class="w-4 h-4 flex-shrink-0 mt-0.5" />
              <span>Changing sync path will not migrate existing files. You may need to manually copy files from the old location.</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-end gap-2 p-4 border-t border-border">
        <UiButton variant="ghost" @click="handleClose">
          Cancel
        </UiButton>
        <UiButton 
          @click="handleSave"
          :disabled="!workspaceName.trim() || isSaving || (enableSync && !syncPath)"
        >
          <Icon v-if="isSaving" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
          Save Changes
        </UiButton>
      </div>
    </div>
  </div>
</template>
