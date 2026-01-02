<script setup lang="ts">
const workspaceStore = useWorkspaceStore()

const showDropdown = ref(false)
const showSettingsModal = ref(false)

// Create local computed refs that reference the store's useState refs
const activeWorkspace = computed(() => 
  workspaceStore.workspaces.value.find(w => w.id === workspaceStore.activeWorkspaceId.value)
)
const workspaces = computed(() => workspaceStore.workspaces.value)
const activeWorkspaceId = computed(() => workspaceStore.activeWorkspaceId.value)

const handleSelectWorkspace = async (id: string) => {
  if (id === activeWorkspaceId.value) {
    showDropdown.value = false
    return
  }
  
  await workspaceStore.switchWorkspace(id)
  showDropdown.value = false
}

const handleCreateNew = () => {
  showDropdown.value = false
  workspaceStore.showCreateModal.value = true
}

const handleOpenSettings = () => {
  showDropdown.value = false
  showSettingsModal.value = true
}

// Close dropdown on click outside
const dropdownRef = ref<HTMLElement | null>(null)
onMounted(() => {
  const handleClickOutside = (e: MouseEvent) => {
    if (dropdownRef.value && !dropdownRef.value.contains(e.target as Node)) {
      showDropdown.value = false
    }
  }
  document.addEventListener('click', handleClickOutside)
  
  // Listen for open-workspace-settings event from GitStatusBar
  const handleOpenSettings = () => {
    showSettingsModal.value = true
  }
  window.addEventListener('open-workspace-settings', handleOpenSettings)
  
  onUnmounted(() => {
    document.removeEventListener('click', handleClickOutside)
    window.removeEventListener('open-workspace-settings', handleOpenSettings)
  })
})
</script>

<template>
  <div ref="dropdownRef" class="relative">
    <!-- Workspace Button -->
    <button
      class="flex items-center gap-2 px-2 py-1.5 hover:bg-accent rounded-md transition-colors w-full"
      @click="showDropdown = !showDropdown"
    >
      <Icon name="lucide:folder" class="w-4 h-4 text-muted-foreground" />
      <span class="text-sm font-medium truncate flex-1 text-left">
        {{ activeWorkspace?.name || 'Select Workspace' }}
      </span>
      <Icon 
        :name="showDropdown ? 'lucide:chevron-up' : 'lucide:chevron-down'" 
        class="w-4 h-4 text-muted-foreground" 
      />
    </button>

    <!-- Dropdown Menu -->
    <div
      v-if="showDropdown"
      class="absolute top-full left-0 right-0 mt-1 bg-background border border-border rounded-md shadow-lg z-50 py-1"
    >
      <!-- Workspace List -->
      <div class="max-h-48 overflow-y-auto">
        <button
          v-for="workspace in workspaces"
          :key="workspace.id"
          class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
          :class="{ 'bg-accent/50': workspace.id === activeWorkspaceId }"
          @click="handleSelectWorkspace(workspace.id)"
        >
          <Icon 
            :name="workspace.syncPath ? 'lucide:folder-git-2' : 'lucide:folder'" 
            class="w-4 h-4 text-muted-foreground" 
          />
          <span class="truncate flex-1 text-left">{{ workspace.name }}</span>
          <Icon 
            v-if="workspace.id === activeWorkspaceId" 
            name="lucide:check" 
            class="w-4 h-4 text-primary" 
          />
        </button>
      </div>
      
      <!-- Divider -->
      <div class="border-t border-border my-1"></div>
      
      <!-- Settings -->
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
        @click="handleOpenSettings"
      >
        <Icon name="lucide:settings" class="w-4 h-4 text-muted-foreground" />
        <span>Workspace Settings</span>
      </button>
      
      <!-- Create New -->
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors text-primary"
        @click="handleCreateNew"
      >
        <Icon name="lucide:plus" class="w-4 h-4" />
        <span>New Workspace</span>
      </button>
    </div>

    <!-- Workspace Settings Modal -->
    <WorkspaceSettingsModal v-model:show="showSettingsModal" />
  </div>
</template>
