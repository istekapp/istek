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
  HistoryItem,
  MockTab
} from '~/types'
import { generateId } from '~/lib/utils'

const props = defineProps<{
  tab: MockTab
}>()

const store = useAppStore()
const { collections, history } = store

// Mock server state
const mockServers = ref<MockServerInfo[]>([])
const activeServerId = ref<string | null>(null)
const requestLogs = ref<MockRequestLog[]>([])
const isStarting = ref(false)
const isGenerating = ref(false)

// New server form
const newServerName = ref('My Mock Server')
const newServerPort = ref(3333)
const selectedEndpoints = ref<MockEndpoint[]>([])

// Source selection
const sourceType = ref<'collection' | 'folder' | 'history'>('collection')
const selectedCollectionId = ref<string | null>(props.tab.collectionId || null)
const selectedFolderId = ref<string | null>(null)

// Left panel tab
const leftTab = ref<'config' | 'servers'>('config')

// Initialize with collection from tab
watch(() => props.tab.collectionId, (newVal) => {
  if (newVal) {
    selectedCollectionId.value = newVal
    const collection = collections.value.find((c: Collection) => c.id === newVal)
    if (collection) {
      newServerName.value = `${collection.name} Mock Server`
    }
  }
}, { immediate: true })

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
  
  // Generate endpoints if collection is already selected
  if (selectedCollectionId.value) {
    await generateEndpoints()
  }
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
const generateEndpoints = async () => {
  isGenerating.value = true
  const endpoints: MockEndpoint[] = []
  
  try {
  for (const { request, response } of sourceRequests.value) {
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
    
    let responseBody: string
    
    // If we have a response schema from OpenAPI/Swagger, generate fake data from it
    if (request.responseSchema) {
      try {
        responseBody = await invoke<string>('generate_mock_response', { 
          schema: request.responseSchema 
        })
      } catch (e) {
        console.error('Failed to generate mock response:', e)
        responseBody = JSON.stringify({
          message: `Mock response for ${request.method} ${path}`,
          timestamp: new Date().toISOString()
        }, null, 2)
      }
    } else if (response?.body) {
      // Use existing response body if available
      responseBody = response.body
    } else {
      // Fallback to generic response
      responseBody = JSON.stringify({
        message: `Mock response for ${request.method} ${path}`,
        timestamp: new Date().toISOString()
      }, null, 2)
    }
    
    endpoints.push({
      id: generateId(),
      method: request.method,
      path,
      responseStatus: response?.status || 200,
      responseHeaders: { 'Content-Type': 'application/json' },
      responseBody,
      delayMs: undefined
    })
  }
  
  selectedEndpoints.value = endpoints
  } finally {
    isGenerating.value = false
  }
}

// Watch for source changes
watch([sourceType, selectedCollectionId, selectedFolderId], async () => {
  await generateEndpoints()
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
    leftTab.value = 'servers'
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
  <div class="flex h-full">
    <!-- Left Panel: Configuration -->
    <div class="flex w-1/2 flex-col border-r border-border overflow-hidden">
      <!-- Left Panel Tabs -->
      <div class="flex border-b border-border px-4">
        <button
          :class="[
            'px-4 py-3 text-sm font-medium transition-colors',
            leftTab === 'config' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="leftTab = 'config'"
        >
          <Icon name="lucide:settings" class="h-4 w-4 mr-2 inline" />
          Configuration
        </button>
        <button
          :class="[
            'px-4 py-3 text-sm font-medium transition-colors',
            leftTab === 'servers' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="leftTab = 'servers'"
        >
          <Icon name="lucide:server" class="h-4 w-4 mr-2 inline" />
          Servers
          <span v-if="mockServers.length > 0" class="ml-1.5 px-1.5 py-0.5 text-xs bg-green-500/20 text-green-500 rounded-full">
            {{ mockServers.length }}
          </span>
        </button>
      </div>
      
      <!-- Config Tab Content -->
      <template v-if="leftTab === 'config'">
        <!-- Source Selection -->
        <div class="p-4 border-b border-border space-y-3">
          <h3 class="font-medium">Source</h3>
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
        
        <!-- Collection/Folder Selector -->
        <div v-if="sourceType === 'collection'" class="p-4 border-b border-border space-y-3">
          <div class="flex items-center gap-2">
            <label class="text-sm font-medium">Collection:</label>
            <UiSelect
              :model-value="selectedCollectionId || ''"
              :options="[{ value: '', label: 'Select a collection...' }, ...collections.map((c: any) => ({ value: c.id, label: c.name }))]"
              class="flex-1 h-9 text-sm"
              placeholder="Select a collection..."
              @update:model-value="selectedCollectionId = $event || null; selectedFolderId = null"
            />
          </div>
          
          <div v-if="selectedCollection?.folders?.length" class="flex items-center gap-2">
            <label class="text-sm font-medium">Folder:</label>
            <UiSelect
              :model-value="selectedFolderId || ''"
              :options="[{ value: '', label: 'All requests' }, ...selectedCollection.folders.map((f: any) => ({ value: f.id, label: `${f.name} (${f.requests.filter((r: any) => r.protocol === 'http').length})` }))]"
              class="flex-1 h-9 text-sm"
              @update:model-value="selectedFolderId = $event || null; sourceType = 'folder'"
            />
          </div>
        </div>
        
        <!-- History info -->
        <div v-if="sourceType === 'history'" class="p-4 border-b border-border">
          <p class="text-sm text-muted-foreground">
            Using last {{ history.filter((h: HistoryItem) => h.request.protocol === 'http').slice(0, 20).length }} HTTP requests from history.
          </p>
        </div>
        
        <!-- Server Configuration -->
        <div class="p-4 border-b border-border space-y-3">
          <h3 class="font-medium">Server Configuration</h3>
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
        </div>
        
        <!-- Endpoints Preview -->
        <div class="flex-1 overflow-hidden flex flex-col">
          <div class="p-4 border-b border-border flex items-center justify-between">
            <h3 class="font-medium">Endpoints ({{ selectedEndpoints.length }})</h3>
            <UiButton
              variant="outline"
              size="sm"
              @click="generateEndpoints"
              :disabled="sourceRequests.length === 0 || isGenerating"
            >
              <Icon 
                :name="isGenerating ? 'lucide:loader-2' : 'lucide:refresh-cw'" 
                :class="['h-4 w-4 mr-1.5', isGenerating && 'animate-spin']" 
              />
              {{ isGenerating ? 'Generating...' : 'Regenerate' }}
            </UiButton>
          </div>
          
          <UiScrollArea class="flex-1">
            <div class="p-4 space-y-2">
              <div
                v-for="(endpoint, index) in selectedEndpoints"
                :key="endpoint.id"
                class="p-3 rounded-lg border border-border"
              >
                <div class="flex items-center gap-3">
                  <span :class="['font-mono text-sm font-semibold w-16', getMethodColor(endpoint.method)]">
                    {{ endpoint.method }}
                  </span>
                  <span class="font-mono text-sm flex-1 truncate">{{ endpoint.path }}</span>
                  <span 
                    v-if="sourceRequests[index]?.request?.responseSchema" 
                    class="text-xs px-1.5 py-0.5 bg-green-500/10 text-green-500 rounded"
                    title="Response generated from OpenAPI schema"
                  >
                    Schema
                  </span>
                  <span class="text-xs text-muted-foreground">{{ endpoint.responseStatus }}</span>
                </div>
              </div>
              
              <div v-if="selectedEndpoints.length === 0" class="text-center py-8 text-muted-foreground">
                <Icon name="lucide:inbox" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                <p class="text-sm">Select a source to generate endpoints</p>
              </div>
            </div>
          </UiScrollArea>
        </div>
        
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
      </template>
      
      <!-- Servers Tab Content -->
      <template v-else>
        <div class="p-4 border-b border-border">
          <h3 class="font-medium">Running Servers</h3>
        </div>
        
        <UiScrollArea class="flex-1">
          <div class="p-4">
            <div v-if="mockServers.length === 0" class="text-center py-8 text-muted-foreground">
              <Icon name="lucide:server-off" class="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p class="text-sm">No mock servers running</p>
              <p class="text-xs mt-1">Start a server from the Configuration tab</p>
            </div>
            
            <div v-else class="space-y-3">
              <div
                v-for="server in mockServers"
                :key="server.id"
                :class="[
                  'p-4 rounded-lg border transition-colors cursor-pointer',
                  activeServerId === server.id ? 'border-primary bg-primary/5' : 'border-border hover:border-primary/50'
                ]"
                @click="activeServerId = server.id"
              >
                <div class="flex items-center gap-3">
                  <div class="h-3 w-3 rounded-full bg-green-500 animate-pulse" />
                  <div class="flex-1">
                    <div class="font-medium">{{ server.name }}</div>
                    <div class="text-sm text-muted-foreground">
                      <code>http://localhost:{{ server.port }}</code>
                    </div>
                  </div>
                  <div class="text-right">
                    <div class="text-sm">{{ server.endpointCount }} endpoints</div>
                  </div>
                  <UiButton
                    variant="ghost"
                    size="icon"
                    class="h-8 w-8 text-destructive hover:bg-destructive/10"
                    @click.stop="stopMockServer(server.id)"
                  >
                    <Icon name="lucide:square" class="h-4 w-4" />
                  </UiButton>
                </div>
              </div>
            </div>
          </div>
        </UiScrollArea>
      </template>
    </div>
    
    <!-- Right Panel: Request Logs -->
    <div class="flex w-1/2 flex-col overflow-hidden">
      <div class="p-4 border-b border-border flex items-center justify-between">
        <div class="flex items-center gap-3">
          <h3 class="font-medium">Request Logs</h3>
          <span v-if="activeServer" class="text-xs text-muted-foreground">
            {{ activeServer.name }}
          </span>
        </div>
        <button
          v-if="serverLogs.length > 0"
          class="text-xs text-muted-foreground hover:text-foreground"
          @click="clearLogs"
        >
          Clear
        </button>
      </div>
      
      <!-- Server URL -->
      <div v-if="activeServer" class="px-4 py-2 border-b border-border bg-muted/30">
        <code class="text-sm">http://localhost:{{ activeServer.port }}</code>
      </div>
      
      <!-- Logs -->
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
            <div v-if="log.query" class="mt-1 text-xs text-muted-foreground font-mono pl-20">
              ?{{ log.query }}
            </div>
            <div v-if="log.matchedEndpoint" class="mt-1 text-xs text-green-500 pl-20">
              Matched: {{ log.matchedEndpoint }}
            </div>
          </div>
          
          <div v-if="serverLogs.length === 0" class="p-8 text-center text-muted-foreground">
            <Icon name="lucide:radio" class="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p class="text-sm">Waiting for requests...</p>
            <p v-if="activeServer" class="text-xs mt-1">
              Make requests to http://localhost:{{ activeServer.port }}
            </p>
            <p v-else class="text-xs mt-1">
              Start a mock server to see requests here
            </p>
          </div>
        </div>
      </UiScrollArea>
    </div>
  </div>
</template>
