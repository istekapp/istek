<script setup lang="ts">
import type { RequestType, HttpRequest, WebSocketRequest, GraphQLRequest, MqttRequest, UnixSocketRequest, HistoryItem, Collection, CollectionFolder, FolderSettings } from '~/types'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'

const store = useAppStore()
const { sidebarTab, sidebarCollapsed, history, collections } = store

const newCollectionName = ref('')
const showNewCollection = ref(false)
const showImportMenu = ref(false)
const isImporting = ref(false)
const importError = ref<string | null>(null)
const fileInputRef = ref<HTMLInputElement | null>(null)
const importType = ref<'openapi' | 'postman'>('openapi')
const showUrlImport = ref(false)
const importUrl = ref('')
// showMockServer is no longer needed as we use tabs now
const showSecretProviders = ref(false)
const isExporting = ref(false)
const exportError = ref<string | null>(null)

// Folder Settings Dialog state
const showFolderSettings = ref(false)
const settingsFolder = ref<CollectionFolder | undefined>()
const settingsCollection = ref<Collection | undefined>()
const isCollectionSettings = ref(false)

// Collection menu state
const activeCollectionMenu = ref<string | null>(null)
const menuPosition = ref({ top: 0, left: 0 })

const toggleCollectionMenu = (e: Event, collectionId: string) => {
  e.stopPropagation()
  
  if (activeCollectionMenu.value === collectionId) {
    activeCollectionMenu.value = null
    return
  }
  
  // Calculate position based on the button clicked
  const button = e.currentTarget as HTMLElement
  const rect = button.getBoundingClientRect()
  menuPosition.value = {
    top: rect.bottom + 4,
    left: rect.right - 192 // 192px = w-48
  }
  activeCollectionMenu.value = collectionId
}

const closeCollectionMenu = () => {
  activeCollectionMenu.value = null
}

const getActiveCollection = () => {
  return collections.value.find((c: Collection) => c.id === activeCollectionMenu.value)
}

const handleRunTests = (collection: Collection) => {
  closeCollectionMenu()
  store.addTestTab(collection.id, collection.name)
}

const handleStartMockServer = (collection: Collection) => {
  closeCollectionMenu()
  store.addMockTab(collection.id, collection.name)
}

const handleDeleteCollection = async (collectionId: string) => {
  closeCollectionMenu()
  await store.deleteCollection(collectionId)
}

// Export collection as YAML
const handleExportYaml = async (collection: Collection) => {
  closeCollectionMenu()
  isExporting.value = true
  exportError.value = null
  
  try {
    // Get YAML content from backend
    const yamlContent = await invoke<string>('export_collection_yaml', { collection })
    
    // Open save dialog
    const filePath = await save({
      defaultPath: `${collection.name.replace(/[^a-zA-Z0-9-_]/g, '_')}.yaml`,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }]
    })
    
    if (filePath) {
      // Write file using Tauri fs
      const { writeTextFile } = await import('@tauri-apps/plugin-fs')
      await writeTextFile(filePath, yamlContent)
    }
  } catch (error: any) {
    exportError.value = error.toString()
    console.error('Failed to export collection:', error)
  } finally {
    isExporting.value = false
  }
}

// Import collection from YAML file
const handleImportYaml = async () => {
  showImportMenu.value = false
  isImporting.value = true
  importError.value = null
  
  try {
    // Open file dialog
    const filePath = await open({
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
      multiple: false
    })
    
    if (filePath && typeof filePath === 'string') {
      // Read file content
      const { readTextFile } = await import('@tauri-apps/plugin-fs')
      const yamlContent = await readTextFile(filePath)
      
      // Import via backend
      const collection = await invoke<Collection>('import_collection_yaml', { yamlContent })
      
      // Add to store
      collections.value = [...collections.value, collection]
      
      // Save to database
      try {
        await invoke('save_collection', { collection })
        
        // Sync to filesystem if enabled
        const workspaceStore = useWorkspaceStore()
        if (workspaceStore.hasSyncEnabled.value) {
          await invoke('sync_export_collections')
          await workspaceStore.refreshGitStatus()
        }
      } catch (e) {
        console.error('Failed to save imported collection:', e)
      }
    }
  } catch (error: any) {
    importError.value = error.toString()
    console.error('Failed to import YAML:', error)
  } finally {
    isImporting.value = false
  }
}

// Open settings dialog for collection
const handleCollectionSettings = (collection: Collection) => {
  closeCollectionMenu()
  settingsCollection.value = collection
  settingsFolder.value = undefined
  isCollectionSettings.value = true
  showFolderSettings.value = true
}

// Open settings dialog for folder
const handleFolderSettings = (e: Event, collection: Collection, folder: CollectionFolder) => {
  e.stopPropagation()
  settingsCollection.value = collection
  settingsFolder.value = folder
  isCollectionSettings.value = false
  showFolderSettings.value = true
}

// Save folder/collection settings
const handleSaveSettings = async (settings: FolderSettings) => {
  if (isCollectionSettings.value && settingsCollection.value) {
    // Update collection settings
    const updatedCollection = {
      ...settingsCollection.value,
      settings
    }
    
    // Update in store
    const index = collections.value.findIndex((c: Collection) => c.id === settingsCollection.value!.id)
    if (index !== -1) {
      collections.value[index] = updatedCollection
      collections.value = [...collections.value]
    }
    
    // Save to database
    try {
      await invoke('save_collection', { collection: updatedCollection })
      
      // Sync to filesystem if enabled
      const workspaceStore = useWorkspaceStore()
      if (workspaceStore.hasSyncEnabled.value) {
        await invoke('sync_export_collections')
        await workspaceStore.refreshGitStatus()
      }
    } catch (e) {
      console.error('Failed to save collection settings:', e)
    }
  } else if (settingsFolder.value && settingsCollection.value) {
    // Update folder settings
    const updatedFolder = {
      ...settingsFolder.value,
      settings
    }
    
    // Find and update folder in collection
    const collectionIndex = collections.value.findIndex((c: Collection) => c.id === settingsCollection.value!.id)
    if (collectionIndex !== -1) {
      const collection = collections.value[collectionIndex]
      const folderIndex = collection.folders?.findIndex((f: CollectionFolder) => f.id === settingsFolder.value!.id) ?? -1
      
      if (folderIndex !== -1 && collection.folders) {
        collection.folders[folderIndex] = updatedFolder
        collections.value[collectionIndex] = { ...collection }
        collections.value = [...collections.value]
        
        // Save to database
        try {
          await invoke('save_collection', { collection: collections.value[collectionIndex] })
          
          // Sync to filesystem if enabled
          const workspaceStore = useWorkspaceStore()
          if (workspaceStore.hasSyncEnabled.value) {
            await invoke('sync_export_collections')
            await workspaceStore.refreshGitStatus()
          }
        } catch (e) {
          console.error('Failed to save folder settings:', e)
        }
      }
    }
  }
  
  showFolderSettings.value = false
}

// Close collection menu when clicking outside
const handleClickOutsideMenu = (event: MouseEvent) => {
  if (activeCollectionMenu.value) {
    const target = event.target as HTMLElement
    if (!target.closest('.collection-menu')) {
      closeCollectionMenu()
    }
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutsideMenu)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutsideMenu)
})

// Collapsed state for collections and folders
const collapsedCollections = ref<Set<string>>(new Set())
const collapsedFolders = ref<Set<string>>(new Set())

// Initialize all collections as collapsed
watch(collections, (newCollections) => {
  newCollections.forEach((c: Collection) => {
    if (!collapsedCollections.value.has(c.id)) {
      collapsedCollections.value.add(c.id)
    }
  })
}, { immediate: true })

const toggleCollection = (collectionId: string) => {
  if (collapsedCollections.value.has(collectionId)) {
    collapsedCollections.value.delete(collectionId)
  } else {
    collapsedCollections.value.add(collectionId)
  }
  // Force reactivity
  collapsedCollections.value = new Set(collapsedCollections.value)
}

const toggleFolder = (folderId: string) => {
  if (collapsedFolders.value.has(folderId)) {
    collapsedFolders.value.delete(folderId)
  } else {
    collapsedFolders.value.add(folderId)
  }
  // Force reactivity
  collapsedFolders.value = new Set(collapsedFolders.value)
}

const isCollectionCollapsed = (collectionId: string) => collapsedCollections.value.has(collectionId)
const isFolderCollapsed = (folderId: string) => collapsedFolders.value.has(folderId)

// Get total request count including folders
const getTotalRequestCount = (collection: Collection) => {
  let count = collection.requests?.length || 0
  if (collection.folders) {
    count += collection.folders.reduce((sum, folder) => sum + (folder.requests?.length || 0), 0)
  }
  return count
}

// Get current active protocol from the active tab
const activeProtocol = computed(() => {
  const tab = store.activeTab.value
  if (tab?.type === 'request') {
    return (tab as any).protocol || 'http'
  }
  return 'http'
})

// Filter collections by the active protocol
const filteredCollections = computed(() => {
  return collections.value.filter((c: Collection) => {
    // Default to 'http' if protocolType is not set (backward compatibility)
    const collectionProtocol = c.protocolType || 'http'
    return collectionProtocol === activeProtocol.value
  })
})

// Filter history by the active protocol (this will be used as base for other filters)
const protocolFilteredHistory = computed(() => {
  return history.value.filter((item: HistoryItem) => {
    // Get protocol from the request, default to 'http'
    const itemProtocol = item.request?.protocol || 'http'
    return itemProtocol === activeProtocol.value
  })
})

const createCollection = () => {
  if (newCollectionName.value.trim()) {
    store.addCollection(newCollectionName.value.trim())
    newCollectionName.value = ''
    showNewCollection.value = false
  }
}

const triggerImport = (type: 'openapi' | 'postman') => {
  importType.value = type
  showImportMenu.value = false
  fileInputRef.value?.click()
}

const triggerUrlImport = () => {
  showImportMenu.value = false
  showUrlImport.value = true
  importUrl.value = ''
}

const importFromUrl = async () => {
  if (!importUrl.value.trim()) return
  
  isImporting.value = true
  importError.value = null
  
  try {
    const result = await invoke<{
      success: boolean
      collection: Collection | null
      error: string | null
      requestCount: number
    }>('import_from_url', { url: importUrl.value.trim() })
    
    if (result.success && result.collection) {
      collections.value = [...collections.value, result.collection]
      
      try {
        await invoke('save_collection', { collection: result.collection })
        
        // Sync to filesystem if enabled
        const workspaceStore = useWorkspaceStore()
        if (workspaceStore.hasSyncEnabled.value) {
          await invoke('sync_export_collections')
          await workspaceStore.refreshGitStatus()
        }
      } catch (e) {
        console.error('Failed to save imported collection:', e)
      }
      
      showUrlImport.value = false
      importUrl.value = ''
    } else {
      importError.value = result.error || 'Failed to import from URL'
    }
  } catch (error: any) {
    importError.value = error.toString()
  } finally {
    isImporting.value = false
  }
}

const handleFileImport = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  
  isImporting.value = true
  importError.value = null
  
  try {
    const content = await file.text()
    const fileName = file.name
    
    let result: {
      success: boolean
      collection: Collection | null
      error: string | null
      requestCount: number
    }
    
    if (importType.value === 'postman') {
      result = await invoke('import_postman', { content, fileName })
    } else {
      result = await invoke('import_openapi', { content, fileName })
    }
    
    if (result.success && result.collection) {
      // Add the imported collection to the store
      collections.value = [...collections.value, result.collection]
      
      // Save to database
      try {
        await invoke('save_collection', { collection: result.collection })
        
        // Sync to filesystem if enabled
        const workspaceStore = useWorkspaceStore()
        if (workspaceStore.hasSyncEnabled.value) {
          await invoke('sync_export_collections')
          await workspaceStore.refreshGitStatus()
        }
      } catch (e) {
        console.error('Failed to save imported collection:', e)
      }
    } else {
      importError.value = result.error || 'Failed to import collection'
    }
  } catch (error: any) {
    importError.value = error.toString()
  } finally {
    isImporting.value = false
    // Reset file input
    if (target) target.value = ''
  }
}

// Close import menu on outside click
const importMenuRef = ref<HTMLElement | null>(null)
watch(showImportMenu, () => {
  const handleClickOutside = (event: MouseEvent) => {
    if (importMenuRef.value && !importMenuRef.value.contains(event.target as Node)) {
      showImportMenu.value = false
    }
  }
  if (showImportMenu.value) {
    setTimeout(() => document.addEventListener('click', handleClickOutside), 0)
  }
})

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  
  if (diff < 60000) return 'Just now'
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`
  if (diff < 172800000) return '1d ago'
  if (diff < 604800000) return `${Math.floor(diff / 86400000)}d ago`
  return date.toLocaleDateString()
}

// Add to Collection modal state
const showAddToCollectionModal = ref(false)
const selectedHistoryItem = ref<HistoryItem | null>(null)
const expandedCollectionsInModal = ref<Set<string>>(new Set())

const openAddToCollection = (e: Event, item: HistoryItem) => {
  e.stopPropagation()
  selectedHistoryItem.value = item
  expandedCollectionsInModal.value = new Set()
  showAddToCollectionModal.value = true
}

const toggleCollectionInModal = (collectionId: string) => {
  if (expandedCollectionsInModal.value.has(collectionId)) {
    expandedCollectionsInModal.value.delete(collectionId)
  } else {
    expandedCollectionsInModal.value.add(collectionId)
  }
  expandedCollectionsInModal.value = new Set(expandedCollectionsInModal.value)
}

const addToCollection = async (collectionId: string, folderId?: string) => {
  if (!selectedHistoryItem.value) return
  
  const request = { ...selectedHistoryItem.value.request }
  
  // Find the collection
  const collection = collections.value.find((c: Collection) => c.id === collectionId)
  if (!collection) return
  
  if (folderId) {
    // Add to folder
    const folder = collection.folders?.find((f: CollectionFolder) => f.id === folderId)
    if (folder) {
      if (!folder.requests) folder.requests = []
      folder.requests.push(request)
    }
  } else {
    // Add to collection root
    if (!collection.requests) collection.requests = []
    collection.requests.push(request)
  }
  
  // Update collection in store
  collections.value = [...collections.value]
  
  // Save to database
  try {
    await invoke('save_collection', { collection })
    
    // Sync to filesystem if enabled
    const workspaceStore = useWorkspaceStore()
    if (workspaceStore.hasSyncEnabled.value) {
      await invoke('sync_export_collections')
      await workspaceStore.refreshGitStatus()
    }
  } catch (e) {
    console.error('Failed to save collection:', e)
  }
  
  showAddToCollectionModal.value = false
  selectedHistoryItem.value = null
}

const deleteHistoryItem = async (e: Event, itemId: string) => {
  e.stopPropagation()
  await store.deleteHistoryItem(itemId)
}

// Request menu state (for collection requests)
const activeRequestMenu = ref<{ collectionId: string; folderId?: string; requestId: string } | null>(null)
const requestMenuPosition = ref({ top: 0, left: 0 })

// Rename dialog state
const showRenameDialog = ref(false)
const renameRequestName = ref('')
const renameTarget = ref<{ collectionId: string; folderId?: string; requestId: string } | null>(null)

const toggleRequestMenu = (e: Event, collectionId: string, requestId: string, folderId?: string) => {
  e.stopPropagation()
  
  const key = `${collectionId}-${folderId || 'root'}-${requestId}`
  const currentKey = activeRequestMenu.value 
    ? `${activeRequestMenu.value.collectionId}-${activeRequestMenu.value.folderId || 'root'}-${activeRequestMenu.value.requestId}`
    : null
  
  if (currentKey === key) {
    activeRequestMenu.value = null
    return
  }
  
  const button = e.currentTarget as HTMLElement
  const rect = button.getBoundingClientRect()
  requestMenuPosition.value = {
    top: rect.bottom + 4,
    left: rect.right - 120
  }
  activeRequestMenu.value = { collectionId, folderId, requestId }
}

const closeRequestMenu = () => {
  activeRequestMenu.value = null
}

const handleRenameRequest = () => {
  if (!activeRequestMenu.value) return
  
  // Find the request
  const collection = collections.value.find((c: Collection) => c.id === activeRequestMenu.value!.collectionId)
  if (!collection) return
  
  let request: RequestType | undefined
  if (activeRequestMenu.value.folderId) {
    const folder = collection.folders?.find((f: CollectionFolder) => f.id === activeRequestMenu.value!.folderId)
    request = folder?.requests?.find((r: RequestType) => r.id === activeRequestMenu.value!.requestId)
  } else {
    request = collection.requests?.find((r: RequestType) => r.id === activeRequestMenu.value!.requestId)
  }
  
  if (request) {
    renameTarget.value = { ...activeRequestMenu.value }
    renameRequestName.value = request.name || ''
    showRenameDialog.value = true
  }
  closeRequestMenu()
}

const saveRename = async () => {
  if (!renameTarget.value || !renameRequestName.value.trim()) return
  
  const collection = collections.value.find((c: Collection) => c.id === renameTarget.value!.collectionId)
  if (!collection) return
  
  if (renameTarget.value.folderId) {
    const folder = collection.folders?.find((f: CollectionFolder) => f.id === renameTarget.value!.folderId)
    const request = folder?.requests?.find((r: RequestType) => r.id === renameTarget.value!.requestId)
    if (request) {
      request.name = renameRequestName.value.trim()
    }
  } else {
    const request = collection.requests?.find((r: RequestType) => r.id === renameTarget.value!.requestId)
    if (request) {
      request.name = renameRequestName.value.trim()
    }
  }
  
  collections.value = [...collections.value]
  
  // Save to database
  try {
    await invoke('save_collection', { collection })
    
    const workspaceStore = useWorkspaceStore()
    if (workspaceStore.hasSyncEnabled.value) {
      await invoke('sync_export_collections')
      await workspaceStore.refreshGitStatus()
    }
  } catch (e) {
    console.error('Failed to save collection:', e)
  }
  
  showRenameDialog.value = false
  renameTarget.value = null
  renameRequestName.value = ''
}

const handleDeleteRequestFromMenu = async () => {
  if (!activeRequestMenu.value) return
  
  const { collectionId, folderId, requestId } = activeRequestMenu.value
  closeRequestMenu()
  
  if (folderId) {
    await store.deleteRequestFromFolder(collectionId, folderId, requestId)
  } else {
    await store.deleteRequestFromCollection(collectionId, requestId)
  }
}

// Close request menu when clicking outside
const handleClickOutsideRequestMenu = (event: MouseEvent) => {
  if (activeRequestMenu.value) {
    const target = event.target as HTMLElement
    if (!target.closest('.request-menu')) {
      closeRequestMenu()
    }
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutsideRequestMenu)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutsideRequestMenu)
})

// Get day group label for history item
const getDayGroup = (timestamp: number): string => {
  const date = new Date(timestamp)
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const yesterday = new Date(today.getTime() - 86400000)
  const itemDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  
  if (itemDate.getTime() === today.getTime()) return 'Today'
  if (itemDate.getTime() === yesterday.getTime()) return 'Yesterday'
  
  // Check if within this week
  const weekAgo = new Date(today.getTime() - 7 * 86400000)
  if (itemDate.getTime() > weekAgo.getTime()) {
    return date.toLocaleDateString('en-US', { weekday: 'long' })
  }
  
  // Older dates
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

// History search and collapse state
const historySearchQuery = ref('')
const collapsedHistoryGroups = ref<Set<string>>(new Set())

const toggleHistoryGroup = (group: string) => {
  if (collapsedHistoryGroups.value.has(group)) {
    collapsedHistoryGroups.value.delete(group)
  } else {
    collapsedHistoryGroups.value.add(group)
  }
  collapsedHistoryGroups.value = new Set(collapsedHistoryGroups.value)
}

const isHistoryGroupCollapsed = (group: string) => collapsedHistoryGroups.value.has(group)

// Filter history items based on protocol and search query
const filteredHistory = computed(() => {
  // First filter by protocol
  const protocolFiltered = protocolFilteredHistory.value
  
  // Then filter by search query if present
  if (!historySearchQuery.value.trim()) return protocolFiltered
  
  const query = historySearchQuery.value.toLowerCase()
  return protocolFiltered.filter(item => {
    const url = getRequestUrl(item.request).toLowerCase()
    const method = item.request.method?.toLowerCase() || ''
    const name = item.request.name?.toLowerCase() || ''
    return url.includes(query) || method.includes(query) || name.includes(query)
  })
})

// Group history items by day
const groupedHistory = computed(() => {
  const groups: Map<string, HistoryItem[]> = new Map()
  
  for (const item of filteredHistory.value) {
    const group = getDayGroup(item.timestamp)
    if (!groups.has(group)) {
      groups.set(group, [])
    }
    groups.get(group)!.push(item)
  }
  
  return groups
})

// Get display URL for any request type
const getRequestUrl = (request: RequestType): string => {
  switch (request.protocol) {
    case 'http':
      return (request as HttpRequest).url || 'No URL'
    case 'websocket':
      return (request as WebSocketRequest).url || 'No URL'
    case 'graphql':
      return (request as GraphQLRequest).url || 'No URL'
    case 'mqtt':
      const mqtt = request as MqttRequest
      return mqtt.broker ? `${mqtt.broker}:${mqtt.port}` : 'No broker'
    case 'unix-socket':
      const unix = request as UnixSocketRequest
      return `${unix.socketPath}${unix.path}` || 'No path'
    default:
      return 'Unknown'
  }
}

// Get response status indicator
const getResponseStatus = (item: HistoryItem): { status: number | null; success: boolean } => {
  if (!item.response) return { status: null, success: false }
  
  // HTTP and Unix Socket responses have status
  if ('status' in item.response) {
    return {
      status: item.response.status,
      success: item.response.status < 400
    }
  }
  
  // GraphQL responses check for errors
  if ('errors' in item.response && item.response.errors?.length) {
    return { status: null, success: false }
  }
  
  return { status: null, success: true }
}
</script>

<template>
  <aside
    :class="[
      'flex flex-col border-r border-border bg-card transition-all duration-300',
      sidebarCollapsed ? 'w-14' : 'w-80'
    ]"
  >
    <!-- Header with Workspace Selector -->
    <div class="flex h-14 items-center justify-between border-b border-border px-3">
      <div v-if="!sidebarCollapsed" class="flex-1 mr-2">
        <WorkspaceSelector />
      </div>
      <UiButton
        variant="ghost"
        size="icon"
        class="h-9 w-9 flex-shrink-0"
        @click="sidebarCollapsed = !sidebarCollapsed"
      >
        <Icon :name="sidebarCollapsed ? 'lucide:panel-left' : 'lucide:panel-left-close'" class="h-5 w-5" />
      </UiButton>
    </div>

    <template v-if="!sidebarCollapsed">
      <!-- Tab switcher -->
      <div class="flex border-b border-border">
        <button
          :class="[
            'flex-1 px-4 py-3 text-base font-medium transition-colors',
            sidebarTab === 'history' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="sidebarTab = 'history'"
        >
          <Icon name="lucide:history" class="mr-2 inline h-5 w-5" />
          History
        </button>
        <button
          :class="[
            'flex-1 px-4 py-3 text-base font-medium transition-colors',
            sidebarTab === 'collections' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="sidebarTab = 'collections'"
        >
          <Icon name="lucide:folder" class="mr-2 inline h-5 w-5" />
          Collections
        </button>
      </div>

      <!-- Content -->
      <UiScrollArea class="flex-1">
        <!-- History Tab -->
        <div v-if="sidebarTab === 'history'" class="p-3 flex flex-col h-full">
          <!-- Search box -->
          <div class="relative mb-3">
            <Icon name="lucide:search" class="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              v-model="historySearchQuery"
              type="text"
              placeholder="Filter history..."
              class="w-full h-9 pl-8 pr-3 text-sm rounded-md border border-border bg-background placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
            <button
              v-if="historySearchQuery"
              class="absolute right-2 top-1/2 -translate-y-1/2 p-0.5 rounded hover:bg-accent"
              @click="historySearchQuery = ''"
            >
              <Icon name="lucide:x" class="w-3.5 h-3.5 text-muted-foreground" />
            </button>
          </div>
          
          <div v-if="history.length === 0" class="p-4 text-center text-base text-muted-foreground">
            No history yet
          </div>
          <div v-else-if="protocolFilteredHistory.length === 0" class="p-4 text-center text-base text-muted-foreground">
            No {{ activeProtocol.toUpperCase() }} requests in history
          </div>
          <div v-else-if="filteredHistory.length === 0" class="p-4 text-center text-base text-muted-foreground">
            No matching requests
          </div>
          <div v-else class="space-y-1 flex-1 overflow-y-auto">
            <div class="flex items-center justify-between px-2 py-1 mb-2">
              <span class="text-sm text-muted-foreground">
                {{ filteredHistory.length }}{{ filteredHistory.length !== protocolFilteredHistory.length ? ` / ${protocolFilteredHistory.length}` : '' }} requests
              </span>
              <button
                class="text-sm text-muted-foreground hover:text-destructive"
                @click="store.clearHistory()"
              >
                Clear
              </button>
            </div>
            
            <!-- Grouped by day -->
            <template v-for="[group, items] in groupedHistory" :key="group">
              <!-- Day header (clickable for collapse) -->
              <button
                class="w-full flex items-center gap-1.5 px-2 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide hover:bg-accent/50 rounded transition-colors"
                @click="toggleHistoryGroup(group)"
              >
                <Icon 
                  :name="isHistoryGroupCollapsed(group) ? 'lucide:chevron-right' : 'lucide:chevron-down'" 
                  class="w-3.5 h-3.5" 
                />
                <span class="flex-1 text-left">{{ group }}</span>
                <span class="text-[10px] font-normal opacity-70">({{ items.length }})</span>
              </button>
              
              <!-- Items in this day (collapsible) -->
              <template v-if="!isHistoryGroupCollapsed(group)">
                <div
                  v-for="item in items"
                  :key="item.id"
                  class="group relative w-full rounded-md px-2 py-2 text-left hover:bg-accent transition-colors cursor-pointer"
                  :title="getRequestUrl(item.request)"
                  @click="store.loadFromHistory(item)"
                >
                  <div class="flex items-center gap-2">
                    <ProtocolBadge :request="item.request" size="sm" />
                    <span class="flex-1 truncate text-sm">
                      {{ getRequestUrl(item.request) }}
                    </span>
                  </div>
                  
                  <!-- Hover actions: Add to Collection + Delete -->
                  <div class="absolute right-0 top-0 bottom-0 opacity-0 group-hover:opacity-100 transition-opacity flex items-center gap-1 pl-6 pr-1 bg-gradient-to-l from-accent via-accent to-transparent z-10">
                    <button
                      class="p-1.5 rounded-md bg-primary/20 border border-primary/30 text-primary hover:bg-primary/30 hover:border-primary/50 transition-colors"
                      title="Add to Collection"
                      @click="openAddToCollection($event, item)"
                    >
                      <Icon name="lucide:folder-plus" class="w-4 h-4" />
                    </button>
                    <button
                      class="p-1.5 rounded-md bg-destructive/20 border border-destructive/30 text-destructive hover:bg-destructive/30 hover:border-destructive/50 transition-colors"
                      title="Delete"
                      @click="deleteHistoryItem($event, item.id)"
                    >
                      <Icon name="lucide:trash-2" class="w-4 h-4" />
                    </button>
                  </div>
                </div>
              </template>
            </template>
          </div>
        </div>

        <!-- Collections Tab -->
        <div v-else class="p-3">
          <!-- Hidden file input -->
          <input
            ref="fileInputRef"
            type="file"
            accept=".json,.yaml,.yml"
            class="hidden"
            @change="handleFileImport"
          />
          
          <div class="mb-3 space-y-2">
            <!-- New Collection / Import buttons -->
            <div v-if="!showNewCollection && !showUrlImport" class="flex gap-2">
              <UiButton
                variant="outline"
                class="flex-1 h-10"
                @click="showNewCollection = true"
              >
                <Icon name="lucide:plus" class="mr-2 h-5 w-5" />
                New
              </UiButton>
              <div ref="importMenuRef" class="relative">
                <UiButton
                  variant="outline"
                  class="h-10"
                  :disabled="isImporting"
                  @click.stop="showImportMenu = !showImportMenu"
                >
                  <Icon v-if="isImporting" name="lucide:loader-2" class="mr-2 h-5 w-5 animate-spin" />
                  <Icon v-else name="lucide:download" class="mr-2 h-5 w-5" />
                  Import
                </UiButton>
                <!-- Import dropdown menu -->
                <div
                  v-if="showImportMenu"
                  class="absolute right-0 top-full z-50 mt-1 w-52 rounded-md border border-border bg-popover p-1 shadow-md"
                >
                  <div class="px-2 py-1.5 text-xs font-medium text-muted-foreground">From File</div>
                  <button
                    class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
                    @click="triggerImport('openapi')"
                  >
                    <Icon name="lucide:file-json" class="h-4 w-4" />
                    OpenAPI / Swagger
                  </button>
                  <button
                    class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
                    @click="triggerImport('postman')"
                  >
                    <Icon name="lucide:box" class="h-4 w-4" />
                    Postman Collection
                  </button>
                  <button
                    class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
                    @click="handleImportYaml"
                  >
                    <Icon name="lucide:file-code" class="h-4 w-4" />
                    Istek YAML
                  </button>
                  <div class="my-1 border-t border-border"></div>
                  <div class="px-2 py-1.5 text-xs font-medium text-muted-foreground">From URL</div>
                  <button
                    class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
                    @click="triggerUrlImport"
                  >
                    <Icon name="lucide:link" class="h-4 w-4" />
                    Import from URL
                  </button>
                </div>
              </div>
            </div>
            
            <!-- New collection input -->
            <div v-if="showNewCollection" class="flex gap-2">
              <UiInput
                v-model="newCollectionName"
                placeholder="Collection name"
                class="h-10"
                @keyup.enter="createCollection"
              />
              <UiButton class="h-10 w-10 p-0" @click="createCollection">
                <Icon name="lucide:check" class="h-5 w-5" />
              </UiButton>
              <UiButton variant="ghost" class="h-10 w-10 p-0" @click="showNewCollection = false">
                <Icon name="lucide:x" class="h-5 w-5" />
              </UiButton>
            </div>
            
            <!-- URL import input -->
            <div v-if="showUrlImport" class="space-y-2">
              <div class="flex gap-2">
                <UiInput
                  v-model="importUrl"
                  placeholder="https://api.example.com/swagger.json"
                  class="h-10"
                  :disabled="isImporting"
                  @keyup.enter="importFromUrl"
                />
              </div>
              <div class="flex gap-2">
                <UiButton 
                  class="flex-1 h-9" 
                  :disabled="isImporting || !importUrl.trim()"
                  @click="importFromUrl"
                >
                  <Icon v-if="isImporting" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
                  <Icon v-else name="lucide:download" class="mr-2 h-4 w-4" />
                  {{ isImporting ? 'Importing...' : 'Import' }}
                </UiButton>
                <UiButton variant="ghost" class="h-9 w-9 p-0" @click="showUrlImport = false">
                  <Icon name="lucide:x" class="h-5 w-5" />
                </UiButton>
              </div>
              <p class="text-xs text-muted-foreground">
                Supports OpenAPI 3.0, Swagger 2.0, and Postman collections
              </p>
            </div>
            
            <!-- Import error message -->
            <div v-if="importError" class="rounded-md bg-destructive/10 p-2 text-sm text-destructive">
              {{ importError }}
              <button class="ml-2 underline" @click="importError = null">Dismiss</button>
            </div>
          </div>

          <div v-if="collections.length === 0" class="p-4 text-center text-base text-muted-foreground">
            No collections yet
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="collection in filteredCollections"
              :key="collection.id"
              class="rounded-md border border-border"
            >
              <!-- Collection header -->
              <div class="flex w-full items-center gap-2 p-3 hover:bg-accent/50 transition-colors rounded-t-md">
                <button
                  class="flex flex-1 items-center gap-2"
                  @click="toggleCollection(collection.id)"
                >
                  <Icon 
                    :name="isCollectionCollapsed(collection.id) ? 'lucide:chevron-right' : 'lucide:chevron-down'" 
                    class="h-4 w-4 text-muted-foreground transition-transform" 
                  />
                  <Icon name="lucide:folder" class="h-5 w-5 text-muted-foreground" />
                  <span class="flex-1 text-left text-base font-medium truncate">{{ collection.name }}</span>
                  <span class="text-sm text-muted-foreground shrink-0">({{ getTotalRequestCount(collection) }})</span>
                </button>
                <!-- Three-dots menu -->
                <div class="relative collection-menu shrink-0">
                  <UiButton
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 text-muted-foreground hover:text-foreground"
                    @click="toggleCollectionMenu($event, collection.id)"
                  >
                    <Icon name="lucide:more-vertical" class="h-4 w-4" />
                  </UiButton>
                </div>
              </div>
              
              <!-- Collection content (collapsible) -->
              <div v-if="!isCollectionCollapsed(collection.id)" class="border-t border-border">
                <!-- Folders -->
                <div v-if="collection.folders && collection.folders.length > 0">
                  <div
                    v-for="folder in collection.folders"
                    :key="folder.id"
                    class="border-b border-border last:border-b-0"
                  >
                    <!-- Folder header -->
                    <div class="group flex w-full items-center gap-2 py-2 pl-6 pr-3 hover:bg-accent/50 transition-colors">
                      <button
                        class="flex flex-1 items-center gap-2"
                        @click="toggleFolder(folder.id)"
                      >
                        <Icon 
                          :name="isFolderCollapsed(folder.id) ? 'lucide:chevron-right' : 'lucide:chevron-down'" 
                          class="h-3 w-3 text-muted-foreground" 
                        />
                        <Icon name="lucide:folder-open" class="h-4 w-4 text-muted-foreground" />
                        <span class="flex-1 text-left text-sm font-medium">{{ folder.name }}</span>
                        <span class="text-xs text-muted-foreground">({{ folder.requests?.length || 0 }})</span>
                      </button>
                      <button
                        class="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-accent text-muted-foreground hover:text-foreground transition-opacity"
                        title="Folder settings"
                        @click="handleFolderSettings($event, collection, folder)"
                      >
                        <Icon name="lucide:settings" class="h-3.5 w-3.5" />
                      </button>
                    </div>
                    
                    <!-- Folder requests -->
                    <div v-if="!isFolderCollapsed(folder.id) && folder.requests && folder.requests.length > 0">
                      <div
                        v-for="request in folder.requests"
                        :key="request.id"
                        class="group flex w-full items-center gap-2 py-2 pl-12 pr-3 text-left hover:bg-accent transition-colors cursor-pointer"
                        @click="store.loadFromCollection(request, collection.id)"
                      >
                        <ProtocolBadge :request="request" size="sm" />
                        <span class="flex-1 truncate text-sm">{{ request.name }}</span>
                        <div class="relative request-menu">
                          <button
                            class="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-secondary text-muted-foreground hover:text-foreground transition-opacity"
                            @click.stop="toggleRequestMenu($event, collection.id, request.id, folder.id)"
                          >
                            <Icon name="lucide:more-vertical" class="h-4 w-4" />
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                
                <!-- Root-level requests (not in folders) -->
                <div v-if="collection.requests && collection.requests.length > 0">
                  <div
                    v-for="request in collection.requests"
                    :key="request.id"
                    class="group flex w-full items-center gap-2 py-2 pl-6 pr-3 text-left hover:bg-accent transition-colors cursor-pointer"
                    @click="store.loadFromCollection(request, collection.id)"
                  >
                    <ProtocolBadge :request="request" size="sm" />
                    <span class="flex-1 truncate text-sm">{{ request.name }}</span>
                    <div class="relative request-menu">
                      <button
                        class="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-secondary text-muted-foreground hover:text-foreground transition-opacity"
                        @click.stop="toggleRequestMenu($event, collection.id, request.id)"
                      >
                        <Icon name="lucide:more-vertical" class="h-4 w-4" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </UiScrollArea>
    </template>
    
    <!-- Bottom Actions (Git only) -->
    <div v-if="!sidebarCollapsed" class="border-t border-border">
      <!-- Git Status Bar -->
      <GitStatusBar />
    </div>
    
    <!-- Secret Providers Modal -->
    <SecretProviders :show="showSecretProviders" @close="showSecretProviders = false" />
    
    <!-- Folder/Collection Settings Dialog -->
    <FolderSettingsDialog
      v-model:show="showFolderSettings"
      :folder="settingsFolder"
      :collection="settingsCollection"
      :is-collection-level="isCollectionSettings"
      @save="handleSaveSettings"
    />
    
    <!-- Teleported Collection Menu -->
    <Teleport to="body">
      <div
        v-if="activeCollectionMenu && getActiveCollection()"
        class="fixed z-[100] w-48 rounded-md border border-border bg-popover p-1 shadow-lg collection-menu"
        :style="{ top: menuPosition.top + 'px', left: menuPosition.left + 'px' }"
      >
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
          @click="handleCollectionSettings(getActiveCollection()!)"
        >
          <Icon name="lucide:settings" class="h-4 w-4 text-muted-foreground" />
          Settings
        </button>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
          @click="handleRunTests(getActiveCollection()!)"
        >
          <Icon name="lucide:play-circle" class="h-4 w-4 text-method-get" />
          Run Tests
        </button>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
          @click="handleStartMockServer(getActiveCollection()!)"
        >
          <Icon name="lucide:server" class="h-4 w-4 text-method-post" />
          Start Mock Server
        </button>
        <div class="my-1 border-t border-border"></div>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
          :disabled="isExporting"
          @click="handleExportYaml(getActiveCollection()!)"
        >
          <Icon v-if="isExporting" name="lucide:loader-2" class="h-4 w-4 animate-spin" />
          <Icon v-else name="lucide:file-output" class="h-4 w-4 text-muted-foreground" />
          Export as YAML
        </button>
        <div class="my-1 border-t border-border"></div>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm text-destructive hover:bg-destructive/10"
          @click="handleDeleteCollection(activeCollectionMenu!)"
        >
          <Icon name="lucide:trash-2" class="h-4 w-4" />
          Delete
        </button>
      </div>
    </Teleport>
    
    <!-- Request Menu (Rename/Delete) -->
    <Teleport to="body">
      <div
        v-if="activeRequestMenu"
        class="fixed z-[100] w-32 rounded-md border border-border bg-popover p-1 shadow-lg request-menu"
        :style="{ top: requestMenuPosition.top + 'px', left: requestMenuPosition.left + 'px' }"
      >
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm hover:bg-accent"
          @click="handleRenameRequest"
        >
          <Icon name="lucide:pencil" class="h-4 w-4 text-muted-foreground" />
          Rename
        </button>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-2 text-sm text-destructive hover:bg-destructive/10"
          @click="handleDeleteRequestFromMenu"
        >
          <Icon name="lucide:trash-2" class="h-4 w-4" />
          Delete
        </button>
      </div>
    </Teleport>
    
    <!-- Rename Request Dialog -->
    <Teleport to="body">
      <div
        v-if="showRenameDialog"
        class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
        @click.self="showRenameDialog = false"
      >
        <div class="bg-background border border-border rounded-lg w-[350px] shadow-xl">
          <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">Rename Request</h3>
            <button class="p-1 hover:bg-accent rounded" @click="showRenameDialog = false">
              <Icon name="lucide:x" class="w-4 h-4" />
            </button>
          </div>
          <div class="p-4">
            <input
              v-model="renameRequestName"
              type="text"
              placeholder="Request name"
              class="w-full h-10 px-3 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-1 focus:ring-primary"
              @keyup.enter="saveRename"
            />
          </div>
          <div class="flex justify-end gap-2 p-4 pt-0">
            <UiButton variant="outline" size="sm" @click="showRenameDialog = false">Cancel</UiButton>
            <UiButton size="sm" @click="saveRename">Save</UiButton>
          </div>
        </div>
      </div>
    </Teleport>
    
    <!-- Add to Collection Modal -->
    <Teleport to="body">
      <div
        v-if="showAddToCollectionModal"
        class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
        @click.self="showAddToCollectionModal = false"
      >
        <div class="bg-background border border-border rounded-lg w-[400px] max-h-[500px] flex flex-col shadow-xl">
          <!-- Header -->
          <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">Add to Collection</h3>
            <button class="p-1 hover:bg-accent rounded" @click="showAddToCollectionModal = false">
              <Icon name="lucide:x" class="w-4 h-4" />
            </button>
          </div>
          
          <!-- Content -->
          <div class="flex-1 overflow-y-auto p-2">
            <div v-if="collections.length === 0" class="p-4 text-center text-muted-foreground">
              <Icon name="lucide:folder" class="w-10 h-10 mx-auto mb-2 opacity-50" />
              <p class="text-sm">No collections yet</p>
              <p class="text-xs mt-1">Create a collection first</p>
            </div>
            
            <div v-else class="space-y-1">
              <div
                v-for="collection in filteredCollections"
                :key="collection.id"
                class="rounded-md"
              >
                <!-- Collection row -->
                <div class="flex items-center gap-1">
                  <button
                    v-if="collection.folders && collection.folders.length > 0"
                    class="p-1 hover:bg-accent rounded"
                    @click="toggleCollectionInModal(collection.id)"
                  >
                    <Icon 
                      :name="expandedCollectionsInModal.has(collection.id) ? 'lucide:chevron-down' : 'lucide:chevron-right'" 
                      class="w-4 h-4 text-muted-foreground" 
                    />
                  </button>
                  <div v-else class="w-6"></div>
                  
                  <button
                    class="flex-1 flex items-center gap-2 px-2 py-2 rounded hover:bg-accent text-left transition-colors"
                    @click="addToCollection(collection.id)"
                  >
                    <Icon name="lucide:folder" class="w-4 h-4 text-muted-foreground" />
                    <span class="text-sm font-medium truncate">{{ collection.name }}</span>
                    <span class="text-xs text-muted-foreground ml-auto">(root)</span>
                  </button>
                </div>
                
                <!-- Folders -->
                <div v-if="expandedCollectionsInModal.has(collection.id) && collection.folders" class="ml-6 space-y-1 mt-1">
                  <button
                    v-for="folder in collection.folders"
                    :key="folder.id"
                    class="w-full flex items-center gap-2 px-2 py-2 rounded hover:bg-accent text-left transition-colors"
                    @click="addToCollection(collection.id, folder.id)"
                  >
                    <Icon name="lucide:folder-open" class="w-4 h-4 text-muted-foreground" />
                    <span class="text-sm truncate">{{ folder.name }}</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Teleport>
  </aside>
</template>
