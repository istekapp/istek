<script setup lang="ts">
import type { SyncChange, GitFileChange } from '~/types'

const syncStore = useSyncStore()

// Use toRefs to maintain reactivity when destructuring
const syncConfig = computed(() => syncStore.syncConfig.value)
const syncStatus = computed(() => syncStore.syncStatus.value)
const gitStatus = computed(() => syncStore.gitStatus.value)
const gitCommits = computed(() => syncStore.gitCommits.value)
const isLoading = computed(() => syncStore.isLoading.value)
const isExporting = computed(() => syncStore.isExporting.value)
const isImporting = computed(() => syncStore.isImporting.value)
const isCommitting = computed(() => syncStore.isCommitting.value)
const isPushing = computed(() => syncStore.isPushing.value)
const isPulling = computed(() => syncStore.isPulling.value)
const error = computed(() => syncStore.error.value)
const commitMessage = syncStore.commitMessage
const isInitialized = computed(() => syncStore.isInitialized.value)
const hasUncommittedChanges = computed(() => syncStore.hasUncommittedChanges.value)
const canPush = computed(() => syncStore.canPush.value)
const canPull = computed(() => syncStore.canPull.value)

// Keep showSyncScreen as a direct ref for two-way binding
const showSyncScreen = syncStore.showSyncScreen

// Local state
const remoteUrl = ref('')
const showRemoteInput = ref(false)
const activeTab = ref<'changes' | 'history'>('changes')

// Load data when screen opens
watch(showSyncScreen, async (show) => {
  if (show) {
    await syncStore.loadConfig()
    await syncStore.refreshStatus()
  }
})

const closeModal = () => {
  showSyncScreen.value = false
}

const handleInitSync = async () => {
  try {
    await syncStore.initSync()
  } catch (e) {
    console.error('Init sync failed:', e)
  }
}

const handleGitInit = async () => {
  try {
    await syncStore.gitInit()
  } catch (e) {
    console.error('Git init failed:', e)
  }
}

const handleExportAll = async () => {
  try {
    const files = await syncStore.exportAll()
    console.log('Exported files:', files)
  } catch (e) {
    console.error('Export failed:', e)
  }
}

const handleImportAll = async () => {
  try {
    const items = await syncStore.importAll()
    console.log('Imported items:', items)
  } catch (e) {
    console.error('Import failed:', e)
  }
}

const handleCommit = async () => {
  if (!commitMessage.value.trim()) return
  try {
    await syncStore.gitCommit(commitMessage.value.trim())
  } catch (e) {
    console.error('Commit failed:', e)
  }
}

const handlePush = async () => {
  try {
    await syncStore.gitPush()
  } catch (e) {
    console.error('Push failed:', e)
  }
}

const handlePull = async () => {
  try {
    await syncStore.gitPull()
  } catch (e) {
    console.error('Pull failed:', e)
  }
}

const handleAddRemote = async () => {
  if (!remoteUrl.value.trim()) return
  try {
    await syncStore.gitAddRemote(remoteUrl.value.trim())
    remoteUrl.value = ''
    showRemoteInput.value = false
  } catch (e) {
    console.error('Add remote failed:', e)
  }
}

const getChangeIcon = (change: SyncChange) => {
  switch (change.changeType) {
    case 'added': return 'lucide:plus'
    case 'modified': return 'lucide:pencil'
    case 'deleted': return 'lucide:trash-2'
    default: return 'lucide:file'
  }
}

const getChangeColor = (change: SyncChange) => {
  switch (change.changeType) {
    case 'added': return 'text-green-400'
    case 'modified': return 'text-yellow-400'
    case 'deleted': return 'text-red-400'
    default: return 'text-muted-foreground'
  }
}

const getGitStatusIcon = (status: string) => {
  switch (status) {
    case 'new': return 'lucide:plus'
    case 'modified': return 'lucide:pencil'
    case 'deleted': return 'lucide:trash-2'
    case 'renamed': return 'lucide:arrow-right'
    default: return 'lucide:file'
  }
}

const getGitStatusColor = (status: string) => {
  switch (status) {
    case 'new': return 'text-green-400'
    case 'modified': return 'text-yellow-400'
    case 'deleted': return 'text-red-400'
    case 'renamed': return 'text-blue-400'
    default: return 'text-muted-foreground'
  }
}

const formatTimestamp = (ts: number) => {
  return new Date(ts * 1000).toLocaleString()
}
</script>

<template>
  <div
    v-if="showSyncScreen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="closeModal"
  >
    <div class="bg-background border border-border rounded-lg w-[900px] max-h-[85vh] flex flex-col shadow-xl">
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-border">
        <div class="flex items-center gap-3">
          <Icon name="lucide:git-branch" class="w-5 h-5 text-primary" />
          <h2 class="text-lg font-semibold">Git Sync</h2>
          <span v-if="syncConfig?.syncPath" class="text-xs text-muted-foreground font-mono">
            {{ syncConfig.syncPath }}
          </span>
        </div>
        <button
          class="p-1 hover:bg-accent rounded"
          @click="closeModal"
        >
          <Icon name="lucide:x" class="w-5 h-5" />
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-hidden flex flex-col p-4 gap-4">
        <!-- Not Initialized State -->
        <div v-if="!isInitialized" class="flex flex-col items-center justify-center h-full gap-4 text-center">
          <Icon name="lucide:folder-sync" class="w-16 h-16 text-muted-foreground" />
          <div>
            <h3 class="text-lg font-medium mb-1">Initialize Git Sync</h3>
            <p class="text-sm text-muted-foreground max-w-md">
              Sync your collections, environments, and variables to a local directory for version control with Git.
              Secrets will be excluded from sync.
            </p>
          </div>
          <UiButton @click="handleInitSync" :disabled="isLoading">
            <Icon v-if="isLoading" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
            Initialize Sync Directory
          </UiButton>
        </div>

        <!-- Initialized State -->
        <template v-else>
          <!-- Error Alert -->
          <div v-if="error" class="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-sm text-destructive flex items-center gap-2">
            <Icon name="lucide:alert-circle" class="w-4 h-4 flex-shrink-0" />
            {{ error }}
          </div>

          <!-- Git Status Bar -->
          <div class="flex items-center justify-between bg-secondary/30 rounded-md p-3">
            <div class="flex items-center gap-4">
              <!-- Branch -->
              <div class="flex items-center gap-2">
                <Icon name="lucide:git-branch" class="w-4 h-4 text-muted-foreground" />
                <span class="text-sm font-mono">{{ gitStatus?.branch || 'main' }}</span>
              </div>
              
              <!-- Remote -->
              <div v-if="gitStatus?.hasRemote" class="flex items-center gap-2 text-sm text-muted-foreground">
                <Icon name="lucide:cloud" class="w-4 h-4" />
                <span class="truncate max-w-[200px]">{{ gitStatus.remoteUrl }}</span>
              </div>
              <button
                v-else
                class="flex items-center gap-1 text-sm text-primary hover:underline"
                @click="showRemoteInput = !showRemoteInput"
              >
                <Icon name="lucide:plus" class="w-3 h-3" />
                Add Remote
              </button>

              <!-- Ahead/Behind -->
              <div v-if="gitStatus?.hasRemote" class="flex items-center gap-2 text-sm">
                <span v-if="gitStatus.ahead > 0" class="text-green-400">
                  <Icon name="lucide:arrow-up" class="w-3 h-3 inline" /> {{ gitStatus.ahead }}
                </span>
                <span v-if="gitStatus.behind > 0" class="text-yellow-400">
                  <Icon name="lucide:arrow-down" class="w-3 h-3 inline" /> {{ gitStatus.behind }}
                </span>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex items-center gap-2">
              <UiButton
                v-if="!gitStatus?.isRepo"
                size="sm"
                variant="outline"
                @click="handleGitInit"
                :disabled="isLoading"
              >
                <Icon name="lucide:git-branch" class="w-4 h-4 mr-1" />
                Git Init
              </UiButton>
              <UiButton
                size="sm"
                variant="outline"
                @click="syncStore.refreshStatus"
                :disabled="isLoading"
              >
                <Icon name="lucide:refresh-cw" class="w-4 h-4" :class="{ 'animate-spin': isLoading }" />
              </UiButton>
            </div>
          </div>

          <!-- Remote URL Input -->
          <div v-if="showRemoteInput" class="flex items-center gap-2 bg-secondary/20 rounded-md p-2">
            <input
              v-model="remoteUrl"
              type="text"
              placeholder="git@github.com:user/repo.git"
              class="flex-1 bg-transparent border border-border rounded px-2 py-1 text-sm font-mono focus:outline-none focus:ring-1 focus:ring-primary"
            />
            <UiButton size="sm" @click="handleAddRemote" :disabled="!remoteUrl.trim()">
              Add
            </UiButton>
            <UiButton size="sm" variant="ghost" @click="showRemoteInput = false">
              Cancel
            </UiButton>
          </div>

          <!-- Tabs -->
          <div class="flex items-center gap-4 border-b border-border">
            <button
              :class="[
                'pb-2 text-sm font-medium border-b-2 -mb-px transition-colors',
                activeTab === 'changes' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'
              ]"
              @click="activeTab = 'changes'"
            >
              Changes
            </button>
            <button
              :class="[
                'pb-2 text-sm font-medium border-b-2 -mb-px transition-colors',
                activeTab === 'history' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'
              ]"
              @click="activeTab = 'history'"
            >
              History
            </button>
          </div>

          <!-- Changes Tab -->
          <div v-if="activeTab === 'changes'" class="flex-1 overflow-auto space-y-4">
            <!-- Sync Actions -->
            <div class="grid grid-cols-2 gap-4">
              <!-- Export Section -->
              <div class="border border-border rounded-md p-4">
                <div class="flex items-center justify-between mb-3">
                  <h4 class="font-medium flex items-center gap-2">
                    <Icon name="lucide:upload" class="w-4 h-4 text-primary" />
                    Export to Directory
                  </h4>
                  <UiButton size="sm" @click="handleExportAll" :disabled="isExporting">
                    <Icon v-if="isExporting" name="lucide:loader-2" class="w-4 h-4 mr-1 animate-spin" />
                    Export All
                  </UiButton>
                </div>
                <p class="text-xs text-muted-foreground mb-3">
                  Export collections, shareable environments, and global variables to YAML files.
                </p>
                
                <!-- Local Changes List -->
                <div v-if="syncStatus?.localChanges?.length" class="space-y-1">
                  <div
                    v-for="change in syncStatus.localChanges"
                    :key="change.resourceId"
                    class="flex items-center gap-2 text-sm py-1"
                  >
                    <Icon :name="getChangeIcon(change)" :class="['w-4 h-4', getChangeColor(change)]" />
                    <span class="capitalize text-muted-foreground">{{ change.resourceType.replace('_', ' ') }}:</span>
                    <span>{{ change.resourceName }}</span>
                  </div>
                </div>
                <div v-else class="text-sm text-muted-foreground text-center py-4">
                  No pending local changes
                </div>
              </div>

              <!-- Import Section -->
              <div class="border border-border rounded-md p-4">
                <div class="flex items-center justify-between mb-3">
                  <h4 class="font-medium flex items-center gap-2">
                    <Icon name="lucide:download" class="w-4 h-4 text-primary" />
                    Import from Directory
                  </h4>
                  <UiButton size="sm" @click="handleImportAll" :disabled="isImporting">
                    <Icon v-if="isImporting" name="lucide:loader-2" class="w-4 h-4 mr-1 animate-spin" />
                    Import All
                  </UiButton>
                </div>
                <p class="text-xs text-muted-foreground mb-3">
                  Import changes from YAML files. Existing secret values will be preserved.
                </p>
                
                <!-- External Changes List -->
                <div v-if="syncStatus?.externalChanges?.length" class="space-y-1">
                  <div
                    v-for="change in syncStatus.externalChanges"
                    :key="change.resourceId"
                    class="flex items-center gap-2 text-sm py-1"
                  >
                    <Icon :name="getChangeIcon(change)" :class="['w-4 h-4', getChangeColor(change)]" />
                    <span class="capitalize text-muted-foreground">{{ change.resourceType.replace('_', ' ') }}:</span>
                    <span>{{ change.resourceName }}</span>
                  </div>
                </div>
                <div v-else class="text-sm text-muted-foreground text-center py-4">
                  No external changes detected
                </div>
              </div>
            </div>

            <!-- Git Commit Section -->
            <div v-if="gitStatus?.isRepo" class="border border-border rounded-md p-4">
              <h4 class="font-medium flex items-center gap-2 mb-3">
                <Icon name="lucide:git-commit" class="w-4 h-4 text-primary" />
                Git Commit
              </h4>

              <!-- Uncommitted Changes -->
              <div v-if="gitStatus?.uncommittedChanges?.length" class="mb-4">
                <div class="text-xs text-muted-foreground mb-2">
                  {{ gitStatus.uncommittedChanges.length }} uncommitted change(s)
                </div>
                <div class="bg-secondary/20 rounded-md p-2 max-h-32 overflow-auto space-y-1">
                  <div
                    v-for="file in gitStatus.uncommittedChanges"
                    :key="file.path"
                    class="flex items-center gap-2 text-xs font-mono"
                  >
                    <Icon :name="getGitStatusIcon(file.status)" :class="['w-3 h-3', getGitStatusColor(file.status)]" />
                    <span>{{ file.path }}</span>
                  </div>
                </div>
              </div>
              <div v-else class="text-sm text-muted-foreground mb-4">
                No uncommitted changes
              </div>

              <!-- Commit Form -->
              <div class="flex items-center gap-2">
                <input
                  v-model="commitMessage"
                  type="text"
                  placeholder="Commit message..."
                  class="flex-1 bg-transparent border border-border rounded px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
                  @keydown.enter="handleCommit"
                />
                <UiButton
                  @click="handleCommit"
                  :disabled="!commitMessage.trim() || !hasUncommittedChanges || isCommitting"
                >
                  <Icon v-if="isCommitting" name="lucide:loader-2" class="w-4 h-4 mr-1 animate-spin" />
                  Commit
                </UiButton>
              </div>

              <!-- Push/Pull -->
              <div v-if="gitStatus?.hasRemote" class="flex items-center gap-2 mt-3 pt-3 border-t border-border">
                <UiButton
                  variant="outline"
                  size="sm"
                  @click="handlePull"
                  :disabled="isPulling || !canPull"
                >
                  <Icon v-if="isPulling" name="lucide:loader-2" class="w-4 h-4 mr-1 animate-spin" />
                  <Icon v-else name="lucide:arrow-down" class="w-4 h-4 mr-1" />
                  Pull
                  <span v-if="gitStatus.behind > 0" class="ml-1 text-xs">({{ gitStatus.behind }})</span>
                </UiButton>
                <UiButton
                  variant="outline"
                  size="sm"
                  @click="handlePush"
                  :disabled="isPushing || !canPush"
                >
                  <Icon v-if="isPushing" name="lucide:loader-2" class="w-4 h-4 mr-1 animate-spin" />
                  <Icon v-else name="lucide:arrow-up" class="w-4 h-4 mr-1" />
                  Push
                  <span v-if="gitStatus.ahead > 0" class="ml-1 text-xs">({{ gitStatus.ahead }})</span>
                </UiButton>
                <span class="text-xs text-muted-foreground ml-2">
                  Git authentication is your responsibility (SSH key or credential helper)
                </span>
              </div>
            </div>
          </div>

          <!-- History Tab -->
          <div v-if="activeTab === 'history'" class="flex-1 overflow-auto">
            <div v-if="gitCommits.length" class="space-y-2">
              <div
                v-for="commit in gitCommits"
                :key="commit.id"
                class="flex items-start gap-3 p-3 bg-secondary/20 rounded-md"
              >
                <Icon name="lucide:git-commit" class="w-4 h-4 text-muted-foreground mt-0.5" />
                <div class="flex-1 min-w-0">
                  <div class="text-sm truncate">{{ commit.message }}</div>
                  <div class="text-xs text-muted-foreground flex items-center gap-2 mt-1">
                    <span class="font-mono">{{ commit.id.slice(0, 7) }}</span>
                    <span>by {{ commit.author }}</span>
                    <span>{{ formatTimestamp(commit.timestamp) }}</span>
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="flex flex-col items-center justify-center h-full text-muted-foreground">
              <Icon name="lucide:history" class="w-12 h-12 mb-2" />
              <span class="text-sm">No commits yet</span>
            </div>
          </div>
        </template>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-between p-4 border-t border-border text-xs text-muted-foreground">
        <div v-if="syncStatus?.lastSync">
          Last sync: {{ new Date(syncStatus.lastSync * 1000).toLocaleString() }}
        </div>
        <div v-else>Never synced</div>
        <div>
          Secrets are excluded from sync
        </div>
      </div>
    </div>
  </div>
</template>
