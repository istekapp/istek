<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { 
  MockServerConfig, 
  MockServerInfo, 
  MockEndpoint, 
  MockRequestLog,
  Collection,
  CollectionFolder,
  HttpRequest,
  HistoryItem
} from '~/types'
import { generateId } from '~/lib/utils'

const props = defineProps<{
  show: boolean
  selectedCollection?: Collection | null
}>()

const emit = defineEmits<{
  close: []
}>()

const store = useAppStore()
const { collections, history } = store

// Mock server state
const mockServers = ref<MockServerInfo[]>([])
const activeServerId = ref<string | null>(null)
const requestLogs = ref<MockRequestLog[]>([])
const isStarting = ref(false)

// New server form
const newServerName = ref('My Mock Server')
const newServerPort = ref(3333)
const selectedEndpoints = ref<MockEndpoint[]>([])

// Source selection
const sourceType = ref<'collection' | 'folder' | 'history'>('collection')
const selectedCollectionId = ref<string | null>(null)
const selectedFolderId = ref<string | null>(null)

// Watch for pre-selected collection from props
watch(() => props.selectedCollection, (newVal) => {
  if (newVal) {
    selectedCollectionId.value = newVal.id
    selectedFolderId.value = null
    sourceType.value = 'collection'
    newServerName.value = `${newVal.name} Mock Server`
  }
}, { immediate: true })

// Tabs
const activeTab = ref<'setup' | 'logs'>('setup')

// Listen for request logs
let unlisten: (() => void) | null = null

onMounted(async () => {
  // Load running servers
  await loadServers()
  
  // Listen for mock request logs
  unlisten = await listen<MockRequestLog>('mock-request-log', (event) => {
    requestLogs.value.unshift(event.payload)
    // Keep only last 100 logs
    if (requestLogs.value.length > 100) {
      requestLogs.value.pop()
    }
  })
})

onUnmounted(() => {
  unlisten?.()
})

const loadServers = async () => {
  try {
    mockServers.value = await invoke<MockServerInfo[]>('mock_server_list')
    if (mockServers.value.length > 0 && !activeServerId.value) {
      activeServerId.value = mockServers.value[0].id
    }
  } catch (error) {
    console.error('Failed to load mock servers:', error)
  }
}

const selectedCollection = computed(() => {
  return collections.value.find((c: Collection) => c.id === selectedCollectionId.value)
})

const selectedFolder = computed(() => {
  if (!selectedCollection.value || !selectedFolderId.value) return null
  return selectedCollection.value.folders?.find((f: CollectionFolder) => f.id === selectedFolderId.value)
})

// Get requests from selected source
const sourceRequests = computed(() => {
  if (sourceType.value === 'history') {
    return history.value
      .filter((h: HistoryItem) => h.request.protocol === 'http')
      .slice(0, 20) // Limit to last 20
      .map((h: HistoryItem) => ({
        request: h.request as HttpRequest,
        response: h.response
      }))
  }
  
  if (sourceType.value === 'folder' && selectedFolder.value) {
    return selectedFolder.value.requests
      .filter(r => r.protocol === 'http')
      .map(r => ({ request: r as HttpRequest, response: null }))
  }
  
  if (sourceType.value === 'collection' && selectedCollection.value) {
    const requests: { request: HttpRequest; response: any }[] = []
    
    // Add root requests
    selectedCollection.value.requests
      .filter(r => r.protocol === 'http')
      .forEach(r => requests.push({ request: r as HttpRequest, response: null }))
    
    // Add folder requests
    selectedCollection.value.folders?.forEach((folder: CollectionFolder) => {
      folder.requests
        .filter(r => r.protocol === 'http')
        .forEach(r => requests.push({ request: r as HttpRequest, response: null }))
    })
    
    return requests
  }
  
  return []
})

// Generate mock endpoints from requests
const generateEndpoints = () => {
  selectedEndpoints.value = sourceRequests.value.map(({ request, response }) => {
    // Parse URL to get path
    let path = '/'
    try {
      if (request.url.startsWith('http')) {
        const url = new URL(request.url)
        path = url.pathname
      } else if (request.url.startsWith('/')) {
        path = request.url.split('?')[0]
      }
    } catch {
      path = '/'
    }
    
    // Use response body if available, otherwise generate sample
    const responseBody = response?.body || JSON.stringify({
      message: `Mock response for ${request.method} ${path}`,
      timestamp: new Date().toISOString()
    }, null, 2)
    
    return {
      id: generateId(),
      method: request.method,
      path,
      responseStatus: response?.status || 200,
      responseHeaders: { 'Content-Type': 'application/json' },
      responseBody,
      delayMs: undefined
    }
  })
}

// Watch for source changes
watch([sourceType, selectedCollectionId, selectedFolderId], () => {
  generateEndpoints()
})

const startMockServer = async () => {
  if (selectedEndpoints.value.length === 0) {
    return
  }
  
  isStarting.value = true
  
  try {
    const config: MockServerConfig = {
      id: generateId(),
      name: newServerName.value,
      port: newServerPort.value,
      endpoints: selectedEndpoints.value
    }
    
    const info = await invoke<MockServerInfo>('mock_server_start', { config })
    mockServers.value.push(info)
    activeServerId.value = info.id
    activeTab.value = 'logs'
  } catch (error: any) {
    alert(`Failed to start mock server: ${error}`)
  } finally {
    isStarting.value = false
  }
}

const stopMockServer = async (serverId: string) => {
  try {
    await invoke('mock_server_stop', { serverId })
    mockServers.value = mockServers.value.filter(s => s.id !== serverId)
    if (activeServerId.value === serverId) {
      activeServerId.value = mockServers.value[0]?.id || null
    }
  } catch (error) {
    console.error('Failed to stop mock server:', error)
  }
}

const formatTimestamp = (timestamp: number) => {
  return new Date(timestamp).toLocaleTimeString()
}

const getStatusColor = (status: number) => {
  if (status >= 200 && status < 300) return 'text-green-400'
  if (status >= 300 && status < 400) return 'text-blue-400'
  if (status >= 400 && status < 500) return 'text-yellow-400'
  if (status >= 500) return 'text-red-400'
  return 'text-gray-400'
}

const getMethodColor = (method: string) => {
  const colors: Record<string, string> = {
    GET: 'text-method-get',
    POST: 'text-method-post',
    PUT: 'text-method-put',
    PATCH: 'text-method-patch',
    DELETE: 'text-method-delete',
  }
  return colors[method] || 'text-gray-400'
}

const clearLogs = () => {
  requestLogs.value = []
}

const activeServer = computed(() => {
  return mockServers.value.find(s => s.id === activeServerId.value)
})

const serverLogs = computed(() => {
  if (!activeServerId.value) return requestLogs.value
  return requestLogs.value.filter(l => l.serverId === activeServerId.value)
})
</script>

<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="emit('close')"
  >
    <div class="bg-background border border-border rounded-lg shadow-xl w-[900px] h-[700px] flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 rounded-lg bg-primary/10 flex items-center justify-center">
            <Icon name="lucide:server" class="h-5 w-5 text-primary" />
          </div>
          <div>
            <h2 class="text-lg font-semibold">Mock Server</h2>
            <p class="text-sm text-muted-foreground">Create mock APIs from your collections</p>
          </div>
        </div>
        <button
          class="p-2 rounded-lg hover:bg-accent transition-colors"
          @click="emit('close')"
        >
          <Icon name="lucide:x" class="h-5 w-5" />
        </button>
      </div>
      
      <!-- Tabs -->
      <div class="flex border-b border-border px-6">
        <button
          :class="[
            'px-4 py-3 text-sm font-medium transition-colors',
            activeTab === 'setup' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeTab = 'setup'"
        >
          <Icon name="lucide:settings" class="h-4 w-4 mr-2 inline" />
          Setup
        </button>
        <button
          :class="[
            'px-4 py-3 text-sm font-medium transition-colors',
            activeTab === 'logs' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeTab = 'logs'"
        >
          <Icon name="lucide:scroll-text" class="h-4 w-4 mr-2 inline" />
          Request Logs
          <span v-if="serverLogs.length > 0" class="ml-1.5 px-1.5 py-0.5 text-xs bg-primary/20 text-primary rounded-full">
            {{ serverLogs.length }}
          </span>
        </button>
      </div>
      
      <!-- Content -->
      <div class="flex-1 overflow-hidden">
        <!-- Setup Tab -->
        <div v-if="activeTab === 'setup'" class="h-full flex">
          <!-- Left: Source Selection -->
          <div class="w-80 border-r border-border flex flex-col">
            <div class="p-4 border-b border-border">
              <h3 class="font-medium mb-3">Source</h3>
              <div class="space-y-2">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    v-model="sourceType"
                    value="collection"
                    class="accent-primary"
                  />
                  <span class="text-sm">From Collection</span>
                </label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    v-model="sourceType"
                    value="history"
                    class="accent-primary"
                  />
                  <span class="text-sm">From History</span>
                </label>
              </div>
            </div>
            
            <!-- Collection/Folder Selection -->
            <div v-if="sourceType === 'collection'" class="flex-1 overflow-auto p-4">
              <div class="space-y-2">
                <div
                  v-for="collection in collections"
                  :key="collection.id"
                  class="rounded-lg border border-border overflow-hidden"
                >
                  <button
                    :class="[
                      'w-full flex items-center gap-2 p-3 text-left transition-colors',
                      selectedCollectionId === collection.id && !selectedFolderId ? 'bg-primary/10' : 'hover:bg-accent'
                    ]"
                    @click="selectedCollectionId = collection.id; selectedFolderId = null"
                  >
                    <Icon name="lucide:folder" class="h-4 w-4 text-muted-foreground" />
                    <span class="flex-1 text-sm font-medium">{{ collection.name }}</span>
                    <span class="text-xs text-muted-foreground">{{ collection.requests.length }} requests</span>
                  </button>
                  
                  <!-- Folders -->
                  <div v-if="collection.folders && collection.folders.length > 0" class="border-t border-border">
                    <button
                      v-for="folder in collection.folders"
                      :key="folder.id"
                      :class="[
                        'w-full flex items-center gap-2 p-3 pl-8 text-left transition-colors',
                        selectedFolderId === folder.id ? 'bg-primary/10' : 'hover:bg-accent'
                      ]"
                      @click="selectedCollectionId = collection.id; selectedFolderId = folder.id; sourceType = 'folder'"
                    >
                      <Icon name="lucide:folder-open" class="h-4 w-4 text-muted-foreground" />
                      <span class="flex-1 text-sm">{{ folder.name }}</span>
                      <span class="text-xs text-muted-foreground">{{ folder.requests.length }}</span>
                    </button>
                  </div>
                </div>
                
                <div v-if="collections.length === 0" class="text-center py-8 text-sm text-muted-foreground">
                  No collections yet
                </div>
              </div>
            </div>
            
            <!-- History info -->
            <div v-if="sourceType === 'history'" class="flex-1 p-4">
              <p class="text-sm text-muted-foreground">
                Using last {{ history.filter((h: HistoryItem) => h.request.protocol === 'http').length }} HTTP requests from history.
              </p>
            </div>
          </div>
          
          <!-- Right: Configuration -->
          <div class="flex-1 flex flex-col">
            <div class="p-4 space-y-4">
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="text-sm font-medium mb-1.5 block">Server Name</label>
                  <UiInput v-model="newServerName" placeholder="My Mock Server" />
                </div>
                <div>
                  <label class="text-sm font-medium mb-1.5 block">Port</label>
                  <UiInput v-model.number="newServerPort" type="number" placeholder="3333" />
                </div>
              </div>
              
              <div class="flex items-center justify-between">
                <h3 class="font-medium">Endpoints ({{ selectedEndpoints.length }})</h3>
                <UiButton
                  variant="outline"
                  size="sm"
                  @click="generateEndpoints"
                  :disabled="sourceRequests.length === 0"
                >
                  <Icon name="lucide:refresh-cw" class="h-4 w-4 mr-1.5" />
                  Regenerate
                </UiButton>
              </div>
            </div>
            
            <!-- Endpoints List -->
            <UiScrollArea class="flex-1 px-4">
              <div class="space-y-2 pb-4">
                <div
                  v-for="endpoint in selectedEndpoints"
                  :key="endpoint.id"
                  class="p-3 rounded-lg border border-border"
                >
                  <div class="flex items-center gap-3">
                    <span :class="['font-mono text-sm font-semibold', getMethodColor(endpoint.method)]">
                      {{ endpoint.method }}
                    </span>
                    <span class="font-mono text-sm flex-1 truncate">{{ endpoint.path }}</span>
                    <span class="text-xs text-muted-foreground">{{ endpoint.responseStatus }}</span>
                  </div>
                </div>
                
                <div v-if="selectedEndpoints.length === 0" class="text-center py-8 text-muted-foreground">
                  <Icon name="lucide:inbox" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p class="text-sm">Select a source to generate endpoints</p>
                </div>
              </div>
            </UiScrollArea>
            
            <!-- Start Button -->
            <div class="p-4 border-t border-border">
              <UiButton
                class="w-full"
                :disabled="selectedEndpoints.length === 0 || isStarting"
                @click="startMockServer"
              >
                <Icon v-if="isStarting" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
                <Icon v-else name="lucide:play" class="h-4 w-4 mr-2" />
                Start Mock Server on port {{ newServerPort }}
              </UiButton>
            </div>
          </div>
        </div>
        
        <!-- Logs Tab -->
        <div v-if="activeTab === 'logs'" class="h-full flex flex-col">
          <!-- Running Servers -->
          <div class="p-4 border-b border-border">
            <div class="flex items-center justify-between mb-3">
              <h3 class="font-medium">Running Servers</h3>
            </div>
            
            <div v-if="mockServers.length === 0" class="text-sm text-muted-foreground">
              No mock servers running
            </div>
            
            <div v-else class="flex flex-wrap gap-2">
              <div
                v-for="server in mockServers"
                :key="server.id"
                :class="[
                  'flex items-center gap-2 px-3 py-2 rounded-lg border transition-colors cursor-pointer',
                  activeServerId === server.id ? 'border-primary bg-primary/5' : 'border-border hover:border-primary/50'
                ]"
                @click="activeServerId = server.id"
              >
                <div class="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                <span class="text-sm font-medium">{{ server.name }}</span>
                <span class="text-xs text-muted-foreground">:{{ server.port }}</span>
                <button
                  class="p-1 rounded hover:bg-destructive/10 hover:text-destructive transition-colors"
                  @click.stop="stopMockServer(server.id)"
                >
                  <Icon name="lucide:square" class="h-3 w-3" />
                </button>
              </div>
            </div>
            
            <div v-if="activeServer" class="mt-3 p-2 rounded bg-muted/50">
              <code class="text-sm">http://localhost:{{ activeServer.port }}</code>
            </div>
          </div>
          
          <!-- Request Logs -->
          <div class="flex-1 flex flex-col min-h-0">
            <div class="flex items-center justify-between px-4 py-2 border-b border-border">
              <span class="text-sm font-medium text-muted-foreground">Request Logs</span>
              <button
                class="text-xs text-muted-foreground hover:text-foreground"
                @click="clearLogs"
              >
                Clear
              </button>
            </div>
            
            <UiScrollArea class="flex-1">
              <div class="divide-y divide-border">
                <div
                  v-for="log in serverLogs"
                  :key="log.id"
                  class="p-3 hover:bg-accent/50 transition-colors"
                >
                  <div class="flex items-center gap-3">
                    <span class="text-xs text-muted-foreground w-20">{{ formatTimestamp(log.timestamp) }}</span>
                    <span :class="['font-mono text-sm font-semibold w-16', getMethodColor(log.method)]">
                      {{ log.method }}
                    </span>
                    <span class="font-mono text-sm flex-1 truncate">{{ log.path }}</span>
                    <span :class="['text-sm font-medium', getStatusColor(log.responseStatus)]">
                      {{ log.responseStatus }}
                    </span>
                    <span class="text-xs text-muted-foreground">{{ log.responseTimeMs }}ms</span>
                  </div>
                  <div v-if="log.query" class="mt-1 text-xs text-muted-foreground font-mono">
                    ?{{ log.query }}
                  </div>
                </div>
                
                <div v-if="serverLogs.length === 0" class="p-8 text-center text-muted-foreground">
                  <Icon name="lucide:radio" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p class="text-sm">Waiting for requests...</p>
                  <p v-if="activeServer" class="text-xs mt-1">
                    Make requests to http://localhost:{{ activeServer.port }}
                  </p>
                </div>
              </div>
            </UiScrollArea>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
