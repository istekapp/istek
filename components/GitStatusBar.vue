<script setup lang="ts">
const workspaceStore = useWorkspaceStore()

const showMenu = ref(false)
const menuRef = ref<HTMLElement | null>(null)

// New branch dialog
const showNewBranchDialog = ref(false)
const newBranchName = ref('')
const isCreatingBranch = ref(false)
const branchError = ref<string | null>(null)

// Remote dialog
const showRemoteDialog = ref(false)
const remoteUrl = ref('')
const isAddingRemote = ref(false)
const remoteError = ref<string | null>(null)

// Push/Pull state
const isPushing = ref(false)
const isPulling = ref(false)
const pushPullError = ref<string | null>(null)

// Access store's useState refs directly for proper reactivity
const gitStatus = computed(() => workspaceStore.gitStatus.value)
// Git sync is enabled if .git directory exists (isRepo = true)
const hasSyncEnabled = computed(() => gitStatus.value?.isRepo === true)
const hasGitRepo = computed(() => {
  const status = gitStatus.value
  console.log('[GitStatusBar] hasGitRepo computed, gitStatus:', status)
  return status?.isRepo === true
})
const currentBranch = computed(() => gitStatus.value?.branch || 'main')
const uncommittedCount = computed(() => gitStatus.value?.uncommittedChanges?.length ?? 0)
const branches = computed(() => workspaceStore.branches.value)
const isGitLoading = computed(() => workspaceStore.isGitLoading.value)
const hasRemote = computed(() => gitStatus.value?.remoteUrl != null)
const remoteUrlDisplay = computed(() => gitStatus.value?.remoteUrl || '')
const aheadBehind = computed(() => ({
  ahead: gitStatus.value?.ahead ?? 0,
  behind: gitStatus.value?.behind ?? 0
}))

const handleInitGit = async () => {
  try {
    console.log('[GitStatusBar] Before gitInit, hasGitRepo:', hasGitRepo.value)
    await workspaceStore.gitInit()
    console.log('[GitStatusBar] After gitInit, hasGitRepo:', hasGitRepo.value)
    showMenu.value = false
  } catch (e) {
    console.error('Failed to init git:', e)
  }
}

const handleShowCommitModal = () => {
  showMenu.value = false
  workspaceStore.showCommitModal.value = true
}

const handleShowHistoryModal = () => {
  showMenu.value = false
  workspaceStore.showHistoryModal.value = true
}

const handleSwitchBranch = async (branch: string) => {
  if (branch === currentBranch.value) return
  
  try {
    await workspaceStore.gitSwitchBranch(branch)
    showMenu.value = false
  } catch (e) {
    console.error('Failed to switch branch:', e)
  }
}

const handleCreateBranch = () => {
  showMenu.value = false
  newBranchName.value = ''
  branchError.value = null
  showNewBranchDialog.value = true
}

const confirmCreateBranch = async () => {
  const name = newBranchName.value.trim()
  if (!name) return
  
  // Validate branch name (no spaces, special chars)
  if (!/^[a-zA-Z0-9_\-\/]+$/.test(name)) {
    branchError.value = 'Branch name can only contain letters, numbers, hyphens, underscores, and slashes'
    return
  }
  
  try {
    isCreatingBranch.value = true
    branchError.value = null
    await workspaceStore.gitCreateBranch(name)
    showNewBranchDialog.value = false
    newBranchName.value = ''
  } catch (e) {
    branchError.value = String(e)
    console.error('Failed to create branch:', e)
  } finally {
    isCreatingBranch.value = false
  }
}

const cancelCreateBranch = () => {
  showNewBranchDialog.value = false
  newBranchName.value = ''
  branchError.value = null
}

// Remote handlers
const handleAddRemote = () => {
  showMenu.value = false
  remoteUrl.value = ''
  remoteError.value = null
  showRemoteDialog.value = true
}

const confirmAddRemote = async () => {
  const url = remoteUrl.value.trim()
  if (!url) return
  
  try {
    isAddingRemote.value = true
    remoteError.value = null
    await workspaceStore.gitAddRemote(url)
    showRemoteDialog.value = false
    remoteUrl.value = ''
  } catch (e) {
    remoteError.value = String(e)
    console.error('Failed to add remote:', e)
  } finally {
    isAddingRemote.value = false
  }
}

const cancelAddRemote = () => {
  showRemoteDialog.value = false
  remoteUrl.value = ''
  remoteError.value = null
}

// Push/Pull handlers
const handlePush = async () => {
  try {
    isPushing.value = true
    pushPullError.value = null
    showMenu.value = false
    await workspaceStore.gitPush()
  } catch (e) {
    pushPullError.value = String(e)
    console.error('Failed to push:', e)
  } finally {
    isPushing.value = false
  }
}

const handlePull = async () => {
  try {
    isPulling.value = true
    pushPullError.value = null
    showMenu.value = false
    await workspaceStore.gitPull()
  } catch (e) {
    pushPullError.value = String(e)
    console.error('Failed to pull:', e)
  } finally {
    isPulling.value = false
  }
}

// Emit event to open workspace settings
const emit = defineEmits<{
  'open-settings': []
}>()

const openWorkspaceSettings = () => {
  // This will be handled by the parent - we can use a global event or store
  // For now, let's just set showCreateModal to open workspace settings
  // Actually we need to emit an event or use a different approach
  // Let's use a simple approach - trigger the workspace selector dropdown
  const event = new CustomEvent('open-workspace-settings')
  window.dispatchEvent(event)
}

// Close menu on click outside
onMounted(() => {
  const handleClickOutside = (e: MouseEvent) => {
    if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
      showMenu.value = false
    }
  }
  document.addEventListener('click', handleClickOutside)
  onUnmounted(() => document.removeEventListener('click', handleClickOutside))
})
</script>

<template>
  <div ref="menuRef" class="relative p-3">
    <!-- No Sync Path - Show Enable Sync button -->
    <div v-if="!hasSyncEnabled">
      <button
        class="flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent rounded-md transition-colors w-full text-muted-foreground"
        @click="openWorkspaceSettings"
      >
        <Icon name="lucide:cloud-off" class="w-4 h-4" />
        <span>Enable Sync</span>
      </button>
    </div>

    <!-- Has Sync - Show Git Settings button with dropdown -->
    <template v-else>
      <button
        class="w-full flex items-center gap-2 px-3 py-2.5 hover:bg-accent rounded-md transition-colors text-sm border border-transparent hover:border-border"
        @click="showMenu = !showMenu"
      >
        <Icon name="lucide:git-branch" class="w-4 h-4 text-muted-foreground" />
        
        <!-- Show branch name if git initialized, otherwise "Git Settings" -->
        <span v-if="hasGitRepo" class="font-mono">{{ currentBranch }}</span>
        <span v-else class="text-muted-foreground">Git Settings</span>
        
        <!-- Uncommitted changes badge (only if git initialized) -->
        <span 
          v-if="hasGitRepo && uncommittedCount > 0" 
          class="ml-auto bg-yellow-500/20 text-yellow-500 text-xs px-1.5 py-0.5 rounded"
        >
          {{ uncommittedCount }}
        </span>
        
        <Icon name="lucide:chevron-up" class="w-4 h-4 text-muted-foreground ml-auto" />
      </button>

      <!-- Dropdown Menu -->
      <div
        v-if="showMenu"
        class="absolute bottom-full left-0 right-0 mb-1 bg-background border border-border rounded-md shadow-lg z-50 py-1"
      >
        <!-- Git NOT initialized - show init option -->
        <template v-if="!hasGitRepo">
          <button
            class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
            @click="handleInitGit"
            :disabled="isGitLoading"
          >
            <Icon v-if="isGitLoading" name="lucide:loader-2" class="w-4 h-4 animate-spin" />
            <Icon v-else name="lucide:git-branch-plus" class="w-4 h-4 text-muted-foreground" />
            <span>Initialize Git Repo</span>
          </button>
        </template>

        <!-- Git initialized - show full menu -->
        <template v-else>
          <!-- Quick Actions -->
          <button
            class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
            @click="handleShowHistoryModal"
          >
            <Icon name="lucide:history" class="w-4 h-4 text-muted-foreground" />
            <span>View History</span>
          </button>
          
          <button
            class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
            @click="handleCreateBranch"
          >
            <Icon name="lucide:git-branch-plus" class="w-4 h-4 text-muted-foreground" />
            <span>New Branch</span>
          </button>

          <!-- Divider -->
          <div class="border-t border-border my-1"></div>

          <!-- Remote Section -->
          <template v-if="!hasRemote">
            <button
              class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
              @click="handleAddRemote"
            >
              <Icon name="lucide:cloud" class="w-4 h-4 text-muted-foreground" />
              <span>Add Remote</span>
            </button>
          </template>
          <template v-else>
            <!-- Push -->
            <button
              class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
              :disabled="isPushing"
              @click="handlePush"
            >
              <Icon v-if="isPushing" name="lucide:loader-2" class="w-4 h-4 animate-spin" />
              <Icon v-else name="lucide:arrow-up" class="w-4 h-4 text-muted-foreground" />
              <span>Push</span>
              <span v-if="aheadBehind.ahead > 0" class="ml-auto text-xs bg-blue-500/20 text-blue-500 px-1.5 py-0.5 rounded">
                {{ aheadBehind.ahead }}
              </span>
            </button>
            
            <!-- Pull -->
            <button
              class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
              :disabled="isPulling"
              @click="handlePull"
            >
              <Icon v-if="isPulling" name="lucide:loader-2" class="w-4 h-4 animate-spin" />
              <Icon v-else name="lucide:arrow-down" class="w-4 h-4 text-muted-foreground" />
              <span>Pull</span>
              <span v-if="aheadBehind.behind > 0" class="ml-auto text-xs bg-orange-500/20 text-orange-500 px-1.5 py-0.5 rounded">
                {{ aheadBehind.behind }}
              </span>
            </button>
            
            <!-- Change Remote -->
            <button
              class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors text-muted-foreground"
              @click="handleAddRemote"
            >
              <Icon name="lucide:settings" class="w-4 h-4" />
              <span class="truncate text-xs">{{ remoteUrlDisplay }}</span>
            </button>
          </template>

          <!-- Divider -->
          <div class="border-t border-border my-1"></div>

          <!-- Commit Action -->
          <button
            class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
            :class="{ 'opacity-50 cursor-not-allowed': uncommittedCount === 0 }"
            :disabled="uncommittedCount === 0"
            @click="handleShowCommitModal"
          >
            <Icon name="lucide:git-commit" class="w-4 h-4" />
            <span>Commit Changes</span>
            <span v-if="uncommittedCount > 0" class="ml-auto text-xs text-muted-foreground">
              {{ uncommittedCount }}
            </span>
          </button>

          <!-- Divider -->
          <div class="border-t border-border my-1"></div>

          <!-- Branch List -->
          <div class="px-3 py-1 text-xs text-muted-foreground uppercase tracking-wide">
            Branches
          </div>
          
          <button
            v-for="branch in branches"
            :key="branch"
            class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
            @click="handleSwitchBranch(branch)"
          >
            <Icon 
              v-if="branch === currentBranch" 
              name="lucide:check" 
              class="w-4 h-4 text-primary" 
            />
            <span v-else class="w-4"></span>
            <span class="font-mono">{{ branch }}</span>
          </button>
        </template>
      </div>
    </template>
    
    <!-- New Branch Dialog -->
    <Teleport to="body">
      <div
        v-if="showNewBranchDialog"
        class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
        @click.self="cancelCreateBranch"
      >
        <div class="bg-background border border-border rounded-lg w-[400px] shadow-xl">
          <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">Create New Branch</h3>
            <button class="p-1 hover:bg-accent rounded" @click="cancelCreateBranch">
              <Icon name="lucide:x" class="w-4 h-4" />
            </button>
          </div>
          
          <div class="p-4 space-y-4">
            <div v-if="branchError" class="text-sm text-destructive bg-destructive/10 border border-destructive/20 rounded-md p-2">
              {{ branchError }}
            </div>
            
            <div>
              <label class="text-sm text-muted-foreground mb-1 block">Branch name</label>
              <input
                v-model="newBranchName"
                type="text"
                placeholder="feature/my-branch"
                class="w-full bg-secondary/50 border border-border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary font-mono"
                @keydown.enter="confirmCreateBranch"
                @keydown.escape="cancelCreateBranch"
              />
            </div>
          </div>
          
          <div class="flex items-center justify-end gap-2 p-4 border-t border-border">
            <UiButton variant="outline" size="sm" @click="cancelCreateBranch">
              Cancel
            </UiButton>
            <UiButton 
              size="sm" 
              :disabled="!newBranchName.trim() || isCreatingBranch"
              @click="confirmCreateBranch"
            >
              <Icon v-if="isCreatingBranch" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
              Create Branch
            </UiButton>
          </div>
        </div>
      </div>
    </Teleport>
    
    <!-- Add Remote Dialog -->
    <Teleport to="body">
      <div
        v-if="showRemoteDialog"
        class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
        @click.self="cancelAddRemote"
      >
        <div class="bg-background border border-border rounded-lg w-[500px] shadow-xl">
          <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">{{ hasRemote ? 'Change Remote' : 'Add Remote' }}</h3>
            <button class="p-1 hover:bg-accent rounded" @click="cancelAddRemote">
              <Icon name="lucide:x" class="w-4 h-4" />
            </button>
          </div>
          
          <div class="p-4 space-y-4">
            <div v-if="remoteError" class="text-sm text-destructive bg-destructive/10 border border-destructive/20 rounded-md p-2">
              {{ remoteError }}
            </div>
            
            <div>
              <label class="text-sm text-muted-foreground mb-1 block">Remote URL</label>
              <input
                v-model="remoteUrl"
                type="text"
                placeholder="https://github.com/username/repo.git"
                class="w-full bg-secondary/50 border border-border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary font-mono"
                @keydown.enter="confirmAddRemote"
                @keydown.escape="cancelAddRemote"
              />
              <p class="text-xs text-muted-foreground mt-2">
                Istek uses your system's Git credentials (Keychain, Credential Manager, etc.).<br/>
                If you can push/pull from terminal, it will work here too.
              </p>
            </div>
          </div>
          
          <div class="flex items-center justify-end gap-2 p-4 border-t border-border">
            <UiButton variant="outline" size="sm" @click="cancelAddRemote">
              Cancel
            </UiButton>
            <UiButton 
              size="sm" 
              :disabled="!remoteUrl.trim() || isAddingRemote"
              @click="confirmAddRemote"
            >
              <Icon v-if="isAddingRemote" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
              {{ hasRemote ? 'Update Remote' : 'Add Remote' }}
            </UiButton>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
/* Ensure the git menu appears above other elements */
</style>
