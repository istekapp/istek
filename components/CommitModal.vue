<script setup lang="ts">
import type { GitFileChange } from '~/types'

const workspaceStore = useWorkspaceStore()

const showModal = computed({
  get: () => workspaceStore.showCommitModal.value,
  set: (val) => workspaceStore.showCommitModal.value = val
})

const commitMessage = ref('')
const isCommitting = ref(false)
const isPushing = ref(false)
const error = ref<string | null>(null)

// Selected files for commit
const selectedFiles = ref<Set<string>>(new Set())

// Expanded folders
const expandedFolders = ref<Set<string>>(new Set())

// Create local computed refs that reference the store's useState refs
const currentBranch = computed(() => workspaceStore.gitStatus.value?.branch || 'main')
const uncommittedChanges = computed(() => workspaceStore.gitStatus.value?.uncommittedChanges || [])
const hasRemote = computed(() => workspaceStore.gitStatus.value?.hasRemote || false)

// Group files by folder
interface FileTreeNode {
  name: string
  path: string
  isFolder: boolean
  status?: 'new' | 'modified' | 'deleted' | 'renamed'
  children?: FileTreeNode[]
}

const fileTree = computed(() => {
  const tree: Map<string, FileTreeNode> = new Map()
  
  for (const file of uncommittedChanges.value) {
    // Skip empty paths and filter out trailing slashes
    const cleanPath = file.path.replace(/\/+$/, '')
    if (!cleanPath) continue
    
    const parts = cleanPath.split('/').filter(p => p.length > 0)
    if (parts.length === 0) continue
    
    let currentPath = ''
    
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      const isLast = i === parts.length - 1
      const parentPath = currentPath
      currentPath = currentPath ? `${currentPath}/${part}` : part
      
      if (!tree.has(currentPath)) {
        tree.set(currentPath, {
          name: part,
          path: currentPath,
          isFolder: !isLast,
          status: isLast ? file.status : undefined,
          children: []
        })
      }
      
      // Add to parent's children
      if (parentPath && tree.has(parentPath)) {
        const parent = tree.get(parentPath)!
        if (!parent.children!.find(c => c.path === currentPath)) {
          parent.children!.push(tree.get(currentPath)!)
        }
      }
    }
  }
  
  // Get root level items
  const rootItems: FileTreeNode[] = []
  for (const [path, node] of tree) {
    if (!path.includes('/')) {
      rootItems.push(node)
    }
  }
  
  return rootItems
})

// Get all file paths in a folder (recursive)
const getFilesInFolder = (node: FileTreeNode): string[] => {
  if (!node.isFolder) {
    return [node.path]
  }
  const files: string[] = []
  for (const child of node.children || []) {
    files.push(...getFilesInFolder(child))
  }
  return files
}

// Check if all files in a folder are selected
const isFolderFullySelected = (node: FileTreeNode): boolean => {
  const files = getFilesInFolder(node)
  return files.length > 0 && files.every(f => selectedFiles.value.has(f))
}

// Check if some (but not all) files in a folder are selected
const isFolderPartiallySelected = (node: FileTreeNode): boolean => {
  const files = getFilesInFolder(node)
  const selectedCount = files.filter(f => selectedFiles.value.has(f)).length
  return selectedCount > 0 && selectedCount < files.length
}

// Toggle folder selection
const toggleFolder = (node: FileTreeNode) => {
  const files = getFilesInFolder(node)
  const allSelected = isFolderFullySelected(node)
  
  if (allSelected) {
    // Deselect all
    for (const file of files) {
      selectedFiles.value.delete(file)
    }
  } else {
    // Select all
    for (const file of files) {
      selectedFiles.value.add(file)
    }
  }
  // Trigger reactivity
  selectedFiles.value = new Set(selectedFiles.value)
}

// Toggle file selection
const toggleFile = (path: string) => {
  if (selectedFiles.value.has(path)) {
    selectedFiles.value.delete(path)
  } else {
    selectedFiles.value.add(path)
  }
  // Trigger reactivity
  selectedFiles.value = new Set(selectedFiles.value)
}

// Toggle folder expansion
const toggleExpand = (path: string) => {
  if (expandedFolders.value.has(path)) {
    expandedFolders.value.delete(path)
  } else {
    expandedFolders.value.add(path)
  }
  expandedFolders.value = new Set(expandedFolders.value)
}

// Select all / Deselect all
const selectAll = () => {
  for (const file of uncommittedChanges.value) {
    selectedFiles.value.add(file.path)
  }
  selectedFiles.value = new Set(selectedFiles.value)
}

const deselectAll = () => {
  selectedFiles.value.clear()
  selectedFiles.value = new Set(selectedFiles.value)
}

const allSelected = computed(() => 
  uncommittedChanges.value.length > 0 && 
  uncommittedChanges.value.every(f => selectedFiles.value.has(f.path))
)

const someSelected = computed(() => selectedFiles.value.size > 0)

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

const getStatusLabel = (status: string) => {
  switch (status) {
    case 'new': return 'untracked'
    case 'modified': return 'modified'
    case 'deleted': return 'deleted'
    case 'renamed': return 'renamed'
    default: return status
  }
}

const handleCommit = async () => {
  if (!commitMessage.value.trim() || selectedFiles.value.size === 0) return
  
  try {
    isCommitting.value = true
    error.value = null
    
    // TODO: Pass selected files to git commit
    await workspaceStore.gitCommit(commitMessage.value.trim())
    
    commitMessage.value = ''
    selectedFiles.value = new Set()
    showModal.value = false
  } catch (e) {
    error.value = String(e)
  } finally {
    isCommitting.value = false
  }
}

const handleCommitAndPush = async () => {
  if (!commitMessage.value.trim() || selectedFiles.value.size === 0) return
  
  try {
    isCommitting.value = true
    error.value = null
    
    await workspaceStore.gitCommit(commitMessage.value.trim())
    
    isPushing.value = true
    await workspaceStore.gitPush()
    
    commitMessage.value = ''
    selectedFiles.value = new Set()
    showModal.value = false
  } catch (e) {
    error.value = String(e)
  } finally {
    isCommitting.value = false
    isPushing.value = false
  }
}

const handleClose = () => {
  commitMessage.value = ''
  error.value = null
  showModal.value = false
}

// Refresh status when modal opens and auto-select all files
watch(showModal, (show) => {
  if (show) {
    workspaceStore.refreshGitStatus()
  }
})

// Auto-select all files when changes are loaded, and expand all folders
watch(uncommittedChanges, (changes) => {
  if (changes.length > 0) {
    // Select all files by default
    for (const file of changes) {
      selectedFiles.value.add(file.path)
    }
    selectedFiles.value = new Set(selectedFiles.value)
    
    // Expand all folders by default
    for (const file of changes) {
      const parts = file.path.split('/')
      let currentPath = ''
      for (let i = 0; i < parts.length - 1; i++) {
        currentPath = currentPath ? `${currentPath}/${parts[i]}` : parts[i]
        expandedFolders.value.add(currentPath)
      }
    }
    expandedFolders.value = new Set(expandedFolders.value)
  }
}, { immediate: true })
</script>

<template>
  <div
    v-if="showModal"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="handleClose"
  >
    <div class="bg-background border border-border rounded-lg w-[600px] max-h-[80vh] flex flex-col shadow-xl">
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-lg font-semibold">Commit Changes</h2>
        <button
          class="p-1 hover:bg-accent rounded"
          @click="handleClose"
        >
          <Icon name="lucide:x" class="w-5 h-5" />
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-auto p-4 space-y-4">
        <!-- Error Alert -->
        <div v-if="error" class="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-sm text-destructive flex items-center gap-2">
          <Icon name="lucide:alert-circle" class="w-4 h-4 flex-shrink-0" />
          {{ error }}
        </div>

        <!-- Changed Files Tree -->
        <div v-if="uncommittedChanges.length > 0" class="space-y-1">
          <!-- Select All / Deselect All -->
          <div class="flex items-center justify-between pb-2 border-b border-border mb-2">
            <div class="flex items-center gap-2">
              <button
                class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
                @click="allSelected ? deselectAll() : selectAll()"
              >
                <div 
                  :class="[
                    'w-4 h-4 rounded border flex items-center justify-center transition-colors',
                    allSelected ? 'bg-primary border-primary' : someSelected ? 'bg-primary/50 border-primary' : 'border-border'
                  ]"
                >
                  <Icon v-if="allSelected" name="lucide:check" class="w-3 h-3 text-primary-foreground" />
                  <Icon v-else-if="someSelected" name="lucide:minus" class="w-3 h-3 text-primary-foreground" />
                </div>
                <span>{{ allSelected ? 'Deselect all' : 'Select all' }}</span>
              </button>
            </div>
            <span class="text-xs text-muted-foreground">
              {{ selectedFiles.size }} of {{ uncommittedChanges.length }} selected
            </span>
          </div>

          <!-- File Tree -->
          <template v-for="node in fileTree" :key="node.path">
            <FileTreeItem 
              :node="node"
              :depth="0"
              :selected-files="selectedFiles"
              :expanded-folders="expandedFolders"
              :is-folder-fully-selected="isFolderFullySelected"
              :is-folder-partially-selected="isFolderPartiallySelected"
              :toggle-folder="toggleFolder"
              :toggle-file="toggleFile"
              :toggle-expand="toggleExpand"
              :get-status-icon="getStatusIcon"
              :get-status-color="getStatusColor"
              :get-status-label="getStatusLabel"
            />
          </template>
        </div>
        
        <div v-else class="text-center py-8 text-muted-foreground">
          <Icon name="lucide:check-circle" class="w-12 h-12 mx-auto mb-2" />
          <p class="text-sm">No uncommitted changes</p>
        </div>

        <!-- Commit Message -->
        <div class="space-y-2">
          <input
            v-model="commitMessage"
            type="text"
            placeholder="Commit message..."
            class="w-full bg-secondary/30 border border-border rounded-md px-3 py-3 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
            :disabled="selectedFiles.size === 0"
            @keydown.enter.meta="handleCommit"
          />
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-between p-4 border-t border-border">
        <div class="flex items-center gap-2">
          <span class="text-xs font-mono bg-secondary px-2 py-1 rounded">{{ currentBranch }}</span>
          <span v-if="hasRemote" class="text-xs text-green-500 flex items-center gap-1">
            <Icon name="lucide:cloud" class="w-3 h-3" />
            Remote configured
          </span>
        </div>
        
        <div class="flex items-center gap-2">
          <UiButton 
            variant="outline"
            @click="handleCommit"
            :disabled="!commitMessage.trim() || selectedFiles.size === 0 || isCommitting"
          >
            <Icon v-if="isCommitting && !isPushing" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
            Commit
          </UiButton>
          
          <UiButton 
            @click="handleCommitAndPush"
            :disabled="!commitMessage.trim() || selectedFiles.size === 0 || isCommitting"
          >
            <Icon v-if="isPushing" name="lucide:loader-2" class="w-4 h-4 mr-2 animate-spin" />
            <Icon v-else name="lucide:upload" class="w-4 h-4 mr-2" />
            Commit & Push
          </UiButton>
        </div>
      </div>
    </div>
  </div>
</template>
