<script setup lang="ts">
interface FileTreeNode {
  name: string
  path: string
  isFolder: boolean
  status?: 'new' | 'modified' | 'deleted' | 'renamed'
  children?: FileTreeNode[]
}

const props = defineProps<{
  node: FileTreeNode
  depth: number
  selectedFiles: Set<string>
  expandedFolders: Set<string>
  isFolderFullySelected: (node: FileTreeNode) => boolean
  isFolderPartiallySelected: (node: FileTreeNode) => boolean
  toggleFolder: (node: FileTreeNode) => void
  toggleFile: (path: string) => void
  toggleExpand: (path: string) => void
  getStatusIcon: (status: string) => string
  getStatusColor: (status: string) => string
  getStatusLabel: (status: string) => string
}>()

const isExpanded = computed(() => props.expandedFolders.has(props.node.path))
const isSelected = computed(() => {
  if (props.node.isFolder) {
    return props.isFolderFullySelected(props.node)
  }
  return props.selectedFiles.has(props.node.path)
})
const isPartiallySelected = computed(() => {
  if (props.node.isFolder) {
    return props.isFolderPartiallySelected(props.node)
  }
  return false
})

const handleCheckboxClick = () => {
  if (props.node.isFolder) {
    props.toggleFolder(props.node)
  } else {
    props.toggleFile(props.node.path)
  }
}

const handleExpandClick = () => {
  if (props.node.isFolder) {
    props.toggleExpand(props.node.path)
  }
}
</script>

<template>
  <div>
    <!-- Current Node -->
    <div 
      :class="[
        'flex items-center gap-2 py-1.5 px-2 rounded hover:bg-secondary/50 cursor-pointer group',
      ]"
      :style="{ paddingLeft: `${depth * 16 + 8}px` }"
    >
      <!-- Checkbox -->
      <button
        class="flex-shrink-0"
        @click.stop="handleCheckboxClick"
      >
        <div 
          :class="[
            'w-4 h-4 rounded border flex items-center justify-center transition-colors',
            isSelected ? 'bg-primary border-primary' : isPartiallySelected ? 'bg-primary/50 border-primary' : 'border-border hover:border-primary/50'
          ]"
        >
          <Icon v-if="isSelected" name="lucide:check" class="w-3 h-3 text-primary-foreground" />
          <Icon v-else-if="isPartiallySelected" name="lucide:minus" class="w-3 h-3 text-primary-foreground" />
        </div>
      </button>

      <!-- Expand/Collapse for folders -->
      <button
        v-if="node.isFolder"
        class="flex-shrink-0 p-0.5 hover:bg-secondary rounded"
        @click.stop="handleExpandClick"
      >
        <Icon 
          :name="isExpanded ? 'lucide:chevron-down' : 'lucide:chevron-right'" 
          class="w-3 h-3 text-muted-foreground" 
        />
      </button>
      <span v-else class="w-4" />

      <!-- Icon -->
      <Icon 
        v-if="node.isFolder"
        :name="isExpanded ? 'lucide:folder-open' : 'lucide:folder'"
        class="w-4 h-4 text-yellow-500 flex-shrink-0"
      />
      <Icon 
        v-else
        :name="getStatusIcon(node.status || '')"
        :class="['w-4 h-4 flex-shrink-0', getStatusColor(node.status || '')]"
      />

      <!-- Name -->
      <span 
        class="text-sm flex-1 truncate font-mono"
        @click="node.isFolder ? handleExpandClick() : handleCheckboxClick()"
      >
        {{ node.name }}
      </span>

      <!-- Status badge for files -->
      <span 
        v-if="!node.isFolder && node.status"
        class="text-xs px-2 py-0.5 rounded bg-secondary text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity"
      >
        {{ getStatusLabel(node.status) }}
      </span>

      <!-- File count for folders -->
      <span 
        v-if="node.isFolder && node.children"
        class="text-xs text-muted-foreground"
      >
        {{ node.children.length }}
      </span>
    </div>

    <!-- Children (if expanded) -->
    <template v-if="node.isFolder && isExpanded && node.children">
      <FileTreeItem
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :depth="depth + 1"
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
</template>
