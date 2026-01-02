<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'

interface CommitFile {
  path: string
  status: 'new' | 'modified' | 'deleted' | 'renamed'
}

interface Commit {
  id: string
  message: string
  author: string
  timestamp: number
}

const workspaceStore = useWorkspaceStore()

const showModal = computed({
  get: () => workspaceStore.showHistoryModal.value,
  set: (val) => workspaceStore.showHistoryModal.value = val
})

const isLoading = ref(false)
const error = ref<string | null>(null)

const commits = computed(() => workspaceStore.gitCommits.value || [])
const currentBranch = computed(() => workspaceStore.gitStatus.value?.branch || 'main')

// Selected commit and file
const selectedCommit = ref<Commit | null>(null)
const selectedFile = ref<string | null>(null)
const commitFiles = ref<CommitFile[]>([])
const fileDiff = ref<string>('')
const isLoadingFiles = ref(false)
const isLoadingDiff = ref(false)

const formatDate = (timestamp: number) => {
  const date = new Date(timestamp * 1000)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  
  if (diff < 3600000) {
    const mins = Math.floor(diff / 60000)
    return `${mins}m ago`
  }
  
  if (diff < 86400000) {
    const hours = Math.floor(diff / 3600000)
    return `${hours}h ago`
  }
  
  if (diff < 604800000) {
    const days = Math.floor(diff / 86400000)
    return `${days}d ago`
  }
  
  return date.toLocaleDateString('en-US', { 
    month: 'short', 
    day: 'numeric',
    year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined
  })
}

const getStatusIcon = (status: string) => {
  switch (status) {
    case 'new': return 'lucide:plus'
    case 'modified': return 'lucide:pencil'
    case 'deleted': return 'lucide:trash-2'
    case 'renamed': return 'lucide:arrow-right'
    default: return 'lucide:file'
  }
}

const getStatusColor = (status: string) => {
  switch (status) {
    case 'new': return 'text-green-400'
    case 'modified': return 'text-yellow-400'
    case 'deleted': return 'text-red-400'
    case 'renamed': return 'text-blue-400'
    default: return 'text-muted-foreground'
  }
}

const selectCommit = async (commit: Commit) => {
  selectedCommit.value = commit
  selectedFile.value = null
  fileDiff.value = ''
  commitFiles.value = []
  
  isLoadingFiles.value = true
  try {
    const files = await invoke<CommitFile[]>('git_get_commit_files', { commitId: commit.id })
    commitFiles.value = files
  } catch (e) {
    console.error('Failed to get commit files:', e)
  } finally {
    isLoadingFiles.value = false
  }
}

const selectFile = async (file: CommitFile) => {
  if (!selectedCommit.value) return
  
  selectedFile.value = file.path
  fileDiff.value = ''
  
  isLoadingDiff.value = true
  try {
    const diff = await invoke<string>('git_get_file_diff', { 
      commitId: selectedCommit.value.id,
      filePath: file.path 
    })
    fileDiff.value = diff
  } catch (e) {
    console.error('Failed to get file diff:', e)
    fileDiff.value = 'Failed to load diff'
  } finally {
    isLoadingDiff.value = false
  }
}

const parseDiffLines = computed(() => {
  if (!fileDiff.value) return []
  
  return fileDiff.value.split('\n').map((line, index) => {
    let type: 'add' | 'remove' | 'context' = 'context'
    if (line.startsWith('+')) type = 'add'
    else if (line.startsWith('-')) type = 'remove'
    
    return {
      number: index + 1,
      content: line,
      type
    }
  })
})

const handleClose = () => {
  selectedCommit.value = null
  selectedFile.value = null
  commitFiles.value = []
  fileDiff.value = ''
  showModal.value = false
}

// Load commits when modal opens
watch(showModal, async (show) => {
  if (show) {
    isLoading.value = true
    error.value = null
    try {
      await workspaceStore.refreshGitStatus()
      // Auto-select first commit if available
      if (commits.value.length > 0) {
        selectCommit(commits.value[0])
      }
    } catch (e) {
      error.value = String(e)
    } finally {
      isLoading.value = false
    }
  }
})
</script>

<template>
  <div
    v-if="showModal"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="handleClose"
  >
    <div class="bg-background border border-border rounded-lg w-[95vw] max-w-[1400px] h-[85vh] flex flex-col shadow-xl">
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-border flex-shrink-0">
        <div class="flex items-center gap-3">
          <Icon name="lucide:history" class="w-5 h-5 text-muted-foreground" />
          <h2 class="text-lg font-semibold">Commit History</h2>
          <span class="text-xs font-mono bg-secondary px-2 py-1 rounded">{{ currentBranch }}</span>
        </div>
        <button class="p-1 hover:bg-accent rounded" @click="handleClose">
          <Icon name="lucide:x" class="w-5 h-5" />
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 flex overflow-hidden">
        <!-- Left Panel: Commits & Files -->
        <div class="w-[350px] border-r border-border flex flex-col overflow-hidden flex-shrink-0">
          <!-- Commits List -->
          <div class="flex-1 overflow-y-auto">
            <!-- Loading -->
            <div v-if="isLoading" class="flex items-center justify-center py-12">
              <Icon name="lucide:loader-2" class="w-6 h-6 animate-spin text-muted-foreground" />
            </div>

            <!-- Empty state -->
            <div v-else-if="commits.length === 0" class="text-center py-12 text-muted-foreground">
              <Icon name="lucide:git-commit" class="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p class="text-sm">No commits yet</p>
            </div>

            <!-- Commit list -->
            <div v-else class="divide-y divide-border">
              <button
                v-for="commit in commits"
                :key="commit.id"
                :class="[
                  'w-full text-left p-3 hover:bg-secondary/50 transition-colors',
                  selectedCommit?.id === commit.id ? 'bg-secondary' : ''
                ]"
                @click="selectCommit(commit)"
              >
                <div class="flex items-start gap-2">
                  <div class="mt-0.5 p-1 bg-secondary rounded flex-shrink-0">
                    <Icon name="lucide:git-commit" class="w-3 h-3 text-muted-foreground" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium truncate">{{ commit.message }}</p>
                    <div class="flex items-center gap-2 mt-1 text-xs text-muted-foreground">
                      <span class="font-mono">{{ commit.id.substring(0, 7) }}</span>
                      <span>{{ formatDate(commit.timestamp) }}</span>
                    </div>
                  </div>
                </div>
                
                <!-- Files in commit (when selected) -->
                <div v-if="selectedCommit?.id === commit.id && commitFiles.length > 0" class="mt-2 ml-6 space-y-1">
                  <div v-if="isLoadingFiles" class="py-2">
                    <Icon name="lucide:loader-2" class="w-4 h-4 animate-spin text-muted-foreground" />
                  </div>
                  <button
                    v-else
                    v-for="file in commitFiles"
                    :key="file.path"
                    :class="[
                      'flex items-center gap-2 w-full text-left py-1 px-2 rounded text-xs hover:bg-accent transition-colors',
                      selectedFile === file.path ? 'bg-accent' : ''
                    ]"
                    @click.stop="selectFile(file)"
                  >
                    <Icon :name="getStatusIcon(file.status)" :class="['w-3 h-3', getStatusColor(file.status)]" />
                    <span class="truncate font-mono">{{ file.path.split('/').pop() }}</span>
                  </button>
                </div>
              </button>
            </div>
          </div>
        </div>

        <!-- Right Panel: Diff View -->
        <div class="flex-1 flex flex-col overflow-hidden">
          <!-- File path header -->
          <div v-if="selectedFile" class="px-4 py-2 border-b border-border bg-secondary/30 flex-shrink-0">
            <span class="text-sm font-mono text-muted-foreground">{{ selectedFile }}</span>
          </div>
          
          <!-- Diff content -->
          <div class="flex-1 overflow-auto">
            <!-- No commit selected -->
            <div v-if="!selectedCommit" class="flex items-center justify-center h-full text-muted-foreground">
              <div class="text-center">
                <Icon name="lucide:git-commit" class="w-12 h-12 mx-auto mb-3 opacity-30" />
                <p class="text-sm">Select a commit to view changes</p>
              </div>
            </div>
            
            <!-- Commit selected but no file -->
            <div v-else-if="!selectedFile" class="flex items-center justify-center h-full text-muted-foreground">
              <div class="text-center">
                <Icon name="lucide:file-diff" class="w-12 h-12 mx-auto mb-3 opacity-30" />
                <p class="text-sm">Select a file to view diff</p>
                <p class="text-xs mt-1">{{ commitFiles.length }} file(s) changed</p>
              </div>
            </div>
            
            <!-- Loading diff -->
            <div v-else-if="isLoadingDiff" class="flex items-center justify-center h-full">
              <Icon name="lucide:loader-2" class="w-6 h-6 animate-spin text-muted-foreground" />
            </div>
            
            <!-- Diff view -->
            <div v-else class="font-mono text-sm">
              <div
                v-for="line in parseDiffLines"
                :key="line.number"
                :class="[
                  'px-4 py-0.5 border-l-2',
                  line.type === 'add' ? 'bg-green-500/10 border-green-500 text-green-400' : '',
                  line.type === 'remove' ? 'bg-red-500/10 border-red-500 text-red-400' : '',
                  line.type === 'context' ? 'border-transparent text-muted-foreground' : ''
                ]"
              >
                <span class="select-none inline-block w-8 text-right mr-4 text-muted-foreground/50">{{ line.number }}</span>
                <span class="whitespace-pre">{{ line.content }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-end p-4 border-t border-border flex-shrink-0">
        <UiButton variant="outline" @click="handleClose">
          Close
        </UiButton>
      </div>
    </div>
  </div>
</template>
