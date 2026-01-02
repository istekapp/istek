<script setup lang="ts">
import type { Collection, RequestType, Variable, Environment, RequestTab } from '~/types'

interface SearchResult {
  id: string
  type: 'request' | 'collection' | 'variable' | 'environment' | 'history' | 'workspace' | 'integration' | 'tab'
  title: string
  subtitle?: string
  icon: string
  iconColor: string
  data: any
}

const appStore = useAppStore()
const variableStore = useVariableStore()
const workspaceStore = useWorkspaceStore()

const isOpen = ref(false)
const searchQuery = ref('')
const selectedIndex = ref(0)
const inputRef = ref<HTMLInputElement | null>(null)

// Get data from stores
const collections = computed(() => appStore.collections.value || [])
const history = computed(() => appStore.history.value || [])
const tabs = computed(() => appStore.tabs.value || [])
const globalVariables = computed(() => variableStore.globalVariables.value || [])
const environments = computed(() => variableStore.environments.value || [])
const secretProviders = computed(() => variableStore.secretProviders.value || [])
const workspaces = computed(() => workspaceStore.workspaces.value || [])

// Search results
const searchResults = computed<SearchResult[]>(() => {
  const query = searchQuery.value.toLowerCase().trim()
  if (!query) {
    // Show recent/suggested items when no query
    return getRecentItems()
  }
  
  const results: SearchResult[] = []
  
  // Search in open tabs FIRST (highest priority)
  for (const tab of tabs.value) {
    if (tab.type !== 'request') continue
    const reqTab = tab as RequestTab
    const request = reqTab.request
    const nameMatch = request.name?.toLowerCase().includes(query)
    const urlMatch = request.url?.toLowerCase().includes(query)
    
    if (nameMatch || urlMatch) {
      results.push({
        id: `tab-${tab.id}`,
        type: 'tab',
        title: request.name || request.url || 'Untitled',
        subtitle: `${request.method} · Open tab`,
        icon: 'lucide:app-window',
        iconColor: 'text-primary',
        data: { tabId: tab.id }
      })
    }
  }
  
  // Search in collections
  for (const collection of collections.value) {
    if (collection.name.toLowerCase().includes(query)) {
      results.push({
        id: `collection-${collection.id}`,
        type: 'collection',
        title: collection.name,
        subtitle: `${collection.requests?.length || 0} requests`,
        icon: 'lucide:folder',
        iconColor: 'text-yellow-500',
        data: collection
      })
    }
    
    // Search in requests within collections
    for (const request of collection.requests || []) {
      const nameMatch = request.name?.toLowerCase().includes(query)
      const urlMatch = request.url?.toLowerCase().includes(query)
      
      if (nameMatch || urlMatch) {
        results.push({
          id: `request-${collection.id}-${request.id}`,
          type: 'request',
          title: request.name || request.url || 'Untitled Request',
          subtitle: `${request.method} · ${collection.name}`,
          icon: getMethodIcon(request.method),
          iconColor: getMethodColor(request.method),
          data: { request, collectionId: collection.id }
        })
      }
    }
  }
  
  // Search in global variables
  for (const variable of globalVariables.value) {
    if (variable.key.toLowerCase().includes(query) || 
        variable.value?.toLowerCase().includes(query)) {
      results.push({
        id: `variable-${variable.id}`,
        type: 'variable',
        title: variable.key,
        subtitle: variable.isSecret ? '••••••' : variable.value,
        icon: 'lucide:variable',
        iconColor: 'text-purple-500',
        data: variable
      })
    }
  }
  
  // Search in environments
  for (const env of environments.value) {
    if (env.name.toLowerCase().includes(query)) {
      results.push({
        id: `environment-${env.id}`,
        type: 'environment',
        title: env.name,
        subtitle: `${env.variables?.length || 0} variables`,
        icon: 'lucide:layers',
        iconColor: 'text-cyan-500',
        data: env
      })
    }
    
    // Search in environment variables
    for (const variable of env.variables || []) {
      if (variable.key.toLowerCase().includes(query)) {
        results.push({
          id: `env-var-${env.id}-${variable.id}`,
          type: 'variable',
          title: variable.key,
          subtitle: `${env.name} · ${variable.isSecret ? '••••••' : variable.value}`,
          icon: 'lucide:variable',
          iconColor: 'text-purple-500',
          data: { variable, environmentId: env.id }
        })
      }
    }
  }
  
  // Search in history
  for (const item of history.value.slice(0, 50)) {
    const request = item.request
    const nameMatch = request.name?.toLowerCase().includes(query)
    const urlMatch = request.url?.toLowerCase().includes(query)
    
    if (nameMatch || urlMatch) {
      results.push({
        id: `history-${item.id}`,
        type: 'history',
        title: request.name || request.url || 'Untitled',
        subtitle: `${request.method} · ${formatTime(item.timestamp)}`,
        icon: 'lucide:history',
        iconColor: 'text-muted-foreground',
        data: item
      })
    }
  }
  
  // Search in workspaces
  for (const workspace of workspaces.value) {
    if (workspace.name.toLowerCase().includes(query)) {
      results.push({
        id: `workspace-${workspace.id}`,
        type: 'workspace',
        title: workspace.name,
        subtitle: workspace.syncPath ? `Sync: ${workspace.syncPath}` : 'No sync configured',
        icon: 'lucide:briefcase',
        iconColor: 'text-blue-500',
        data: workspace
      })
    }
  }
  
  // Search in integrations (secret providers)
  for (const provider of secretProviders.value) {
    if (provider.name.toLowerCase().includes(query) || 
        provider.type?.toLowerCase().includes(query)) {
      results.push({
        id: `integration-${provider.id}`,
        type: 'integration',
        title: provider.name,
        subtitle: `${provider.type} · ${provider.enabled ? 'Enabled' : 'Disabled'}`,
        icon: getIntegrationIcon(provider.type),
        iconColor: 'text-orange-500',
        data: provider
      })
    }
  }
  
  return results.slice(0, 25) // Limit results
})

// Get recent items when no search query
const getRecentItems = (): SearchResult[] => {
  const results: SearchResult[] = []
  
  // Recent history items
  for (const item of history.value.slice(0, 5)) {
    const request = item.request
    results.push({
      id: `history-${item.id}`,
      type: 'history',
      title: request.name || request.url || 'Untitled',
      subtitle: `${request.method} · ${formatTime(item.timestamp)}`,
      icon: 'lucide:history',
      iconColor: 'text-muted-foreground',
      data: item
    })
  }
  
  // Collections
  for (const collection of collections.value.slice(0, 3)) {
    results.push({
      id: `collection-${collection.id}`,
      type: 'collection',
      title: collection.name,
      subtitle: `${collection.requests?.length || 0} requests`,
      icon: 'lucide:folder',
      iconColor: 'text-yellow-500',
      data: collection
    })
  }
  
  return results
}

const getMethodIcon = (method: string) => {
  return 'lucide:arrow-right'
}

const getMethodColor = (method: string) => {
  switch (method?.toUpperCase()) {
    case 'GET': return 'text-green-500'
    case 'POST': return 'text-blue-500'
    case 'PUT': return 'text-yellow-500'
    case 'PATCH': return 'text-orange-500'
    case 'DELETE': return 'text-red-500'
    default: return 'text-muted-foreground'
  }
}

const getIntegrationIcon = (type: string) => {
  switch (type) {
    case 'aws': return 'lucide:cloud'
    case 'gcp': return 'lucide:cloud'
    case 'azure': return 'lucide:cloud'
    case 'vault': return 'lucide:lock'
    case '1password': return 'lucide:key'
    case 'bitwarden': return 'lucide:shield'
    default: return 'lucide:plug'
  }
}

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  
  if (diff < 60000) return 'Just now'
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`
  return date.toLocaleDateString()
}

const getTypeLabel = (type: string) => {
  switch (type) {
    case 'tab': return 'Open Tab'
    case 'request': return 'Request'
    case 'collection': return 'Collection'
    case 'variable': return 'Variable'
    case 'environment': return 'Environment'
    case 'history': return 'History'
    case 'workspace': return 'Workspace'
    case 'integration': return 'Integration'
    default: return type
  }
}

// Handle result selection
const selectResult = (result: SearchResult) => {
  switch (result.type) {
    case 'tab':
      // Switch to existing open tab
      appStore.setActiveTab(result.data.tabId)
      break
    case 'request':
      appStore.loadFromCollection(result.data.request, result.data.collectionId)
      break
    case 'collection':
      // Expand collection in sidebar - for now just switch to collections tab
      appStore.sidebarTab.value = 'collections'
      break
    case 'variable':
      variableStore.openVariableManager('variables')
      break
    case 'environment':
      variableStore.openVariableManager('environments')
      break
    case 'history':
      appStore.loadFromHistory(result.data)
      break
    case 'workspace':
      // Switch to this workspace
      workspaceStore.switchWorkspace(result.data.id)
      break
    case 'integration':
      // Open integrations tab in variable manager
      variableStore.openVariableManager('integrations')
      break
  }
  
  close()
}

// Keyboard navigation
const handleKeydown = (e: KeyboardEvent) => {
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      selectedIndex.value = Math.min(selectedIndex.value + 1, searchResults.value.length - 1)
      break
    case 'ArrowUp':
      e.preventDefault()
      selectedIndex.value = Math.max(selectedIndex.value - 1, 0)
      break
    case 'Enter':
      e.preventDefault()
      if (searchResults.value[selectedIndex.value]) {
        selectResult(searchResults.value[selectedIndex.value])
      }
      break
    case 'Escape':
      close()
      break
  }
}

// Reset selection when results change
watch(searchResults, () => {
  selectedIndex.value = 0
})

// Open/close
const open = () => {
  isOpen.value = true
  searchQuery.value = ''
  selectedIndex.value = 0
  nextTick(() => {
    inputRef.value?.focus()
  })
}

const close = () => {
  isOpen.value = false
  searchQuery.value = ''
}

// Global keyboard shortcut
onMounted(() => {
  const handleGlobalKeydown = (e: KeyboardEvent) => {
    // CMD+K or CMD+F to open
    if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'f')) {
      e.preventDefault()
      if (isOpen.value) {
        close()
      } else {
        open()
      }
    }
  }
  
  window.addEventListener('keydown', handleGlobalKeydown)
  onUnmounted(() => {
    window.removeEventListener('keydown', handleGlobalKeydown)
  })
})

// Expose open method for external trigger
defineExpose({ open })
</script>

<template>
  <Teleport to="body">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-[100] flex items-start justify-center pt-[15vh] bg-black/50"
      @click.self="close"
    >
      <div class="w-[600px] max-h-[60vh] bg-background border border-border rounded-xl shadow-2xl flex flex-col overflow-hidden">
        <!-- Search Input -->
        <div class="flex items-center gap-3 px-4 py-3 border-b border-border">
          <Icon name="lucide:search" class="w-5 h-5 text-muted-foreground flex-shrink-0" />
          <input
            ref="inputRef"
            v-model="searchQuery"
            type="text"
            placeholder="Search requests, collections, variables..."
            class="flex-1 bg-transparent text-base outline-none placeholder:text-muted-foreground"
            @keydown="handleKeydown"
          />
          <kbd class="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-secondary text-muted-foreground rounded">
            ESC
          </kbd>
        </div>
        
        <!-- Results -->
        <div class="flex-1 overflow-y-auto">
          <!-- Empty state -->
          <div v-if="searchResults.length === 0 && searchQuery" class="py-12 text-center text-muted-foreground">
            <Icon name="lucide:search-x" class="w-10 h-10 mx-auto mb-3 opacity-50" />
            <p class="text-sm">No results found for "{{ searchQuery }}"</p>
          </div>
          
          <!-- Results list -->
          <div v-else class="py-2">
            <div v-if="!searchQuery" class="px-3 py-1.5 text-xs text-muted-foreground uppercase tracking-wide">
              Recent
            </div>
            
            <button
              v-for="(result, index) in searchResults"
              :key="result.id"
              :class="[
                'w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors',
                index === selectedIndex ? 'bg-accent' : 'hover:bg-accent/50'
              ]"
              @click="selectResult(result)"
              @mouseenter="selectedIndex = index"
            >
              <!-- Icon -->
              <div :class="['flex-shrink-0', result.iconColor]">
                <Icon :name="result.icon" class="w-4 h-4" />
              </div>
              
              <!-- Content -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-medium truncate">{{ result.title }}</span>
                  <span class="text-xs px-1.5 py-0.5 rounded bg-secondary text-muted-foreground flex-shrink-0">
                    {{ getTypeLabel(result.type) }}
                  </span>
                </div>
                <div v-if="result.subtitle" class="text-sm text-muted-foreground truncate">
                  {{ result.subtitle }}
                </div>
              </div>
              
              <!-- Enter hint -->
              <Icon 
                v-if="index === selectedIndex" 
                name="lucide:corner-down-left" 
                class="w-4 h-4 text-muted-foreground flex-shrink-0" 
              />
            </button>
          </div>
        </div>
        
        <!-- Footer -->
        <div class="flex items-center justify-between px-4 py-2 border-t border-border bg-secondary/30 text-xs text-muted-foreground">
          <div class="flex items-center gap-4">
            <span class="flex items-center gap-1">
              <kbd class="px-1.5 py-0.5 bg-secondary rounded">↑</kbd>
              <kbd class="px-1.5 py-0.5 bg-secondary rounded">↓</kbd>
              to navigate
            </span>
            <span class="flex items-center gap-1">
              <kbd class="px-1.5 py-0.5 bg-secondary rounded">↵</kbd>
              to select
            </span>
          </div>
          <span class="flex items-center gap-1">
            <kbd class="px-1.5 py-0.5 bg-secondary rounded">⌘</kbd>
            <kbd class="px-1.5 py-0.5 bg-secondary rounded">K</kbd>
            to toggle
          </span>
        </div>
      </div>
    </div>
  </Teleport>
</template>
