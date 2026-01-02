<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'

const workspaceStore = useWorkspaceStore()
const variableStore = useVariableStore()
const { appZoom } = variableStore

const showModal = computed({
  get: () => workspaceStore.showCreateModal.value,
  set: (val) => workspaceStore.showCreateModal.value = val
})

// Form state
const workspaceName = ref('')
const enableSync = ref(false)
const syncPath = ref('')
const isCreating = ref(false)
const error = ref<string | null>(null)

// Update default sync path when name changes
watch(workspaceName, async (name) => {
  if (name && enableSync.value && !syncPath.value) {
    syncPath.value = await workspaceStore.getDefaultSyncPath(name)
  }
})

// Update sync path when enabling sync
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
      // Append workspace name to the selected directory
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

const handleCreate = async () => {
  if (!workspaceName.value.trim()) return
  
  try {
    isCreating.value = true
    error.value = null
    
    await workspaceStore.createWorkspace(
      workspaceName.value.trim(),
      enableSync.value ? syncPath.value : undefined
    )
    
    // Reset form and close modal
    workspaceName.value = ''
    enableSync.value = false
    syncPath.value = ''
    showModal.value = false
  } catch (e) {
    error.value = String(e)
  } finally {
    isCreating.value = false
  }
}

const handleClose = () => {
  workspaceName.value = ''
  enableSync.value = false
  syncPath.value = ''
  error.value = null
  showModal.value = false
}
</script>

<template>
  <div
    v-if="showModal"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    :style="{ zoom: 1 / appZoom }"
    @click.self="handleClose"
  >
    <div class="bg-background border border-border rounded-lg w-[500px] shadow-xl">
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-lg font-semibold">Create Workspace</h2>
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
            autofocus
            @keydown.enter="handleCreate"
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
            <span class="text-sm font-medium">Sync to filesystem</span>
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
          @click="handleCreate"
          :disabled="!workspaceName.trim() || isCreating"
        >
          <Icon v-if="isCreating" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
          Create Workspace
        </UiButton>
      </div>
    </div>
  </div>
</template>
