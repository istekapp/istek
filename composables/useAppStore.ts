import type { 
  ProtocolType, HttpRequest, HttpResponse, WebSocketRequest, WebSocketMessage, WebSocketState,
  GraphQLRequest, GraphQLResponse, GrpcRequest, GrpcResponse, MqttRequest, MqttMessage, MqttState,
  UnixSocketRequest, McpRequest, McpState, SseRequest, SseEvent, SseState,
  Collection, HistoryItem, Tab, RequestTab, TestTab, MockTab, HttpMethod, KeyValue, FormDataField, RequestType, ResponseType
} from '~/types'
import { generateId } from '~/lib/utils'
import { invoke } from '@tauri-apps/api/core'

// Helper to refresh git status after collection changes
async function refreshGitStatusIfEnabled() {
  try {
    const workspaceStore = useWorkspaceStore()
    if (workspaceStore.hasSyncEnabled.value) {
      await workspaceStore.refreshGitStatus()
    }
  } catch (e) {
    console.error('Failed to refresh git status:', e)
  }
}

const createEmptyKeyValue = (): KeyValue => ({
  id: generateId(),
  key: '',
  value: '',
  enabled: true,
})

const createEmptyFormDataField = (): FormDataField => ({
  id: generateId(),
  key: '',
  value: '',
  type: 'text',
  enabled: true,
})

const createHttpRequest = (): HttpRequest => ({
  id: generateId(),
  name: 'New Request',
  protocol: 'http',
  method: 'GET',
  url: '',
  headers: [createEmptyKeyValue()],
  params: [createEmptyKeyValue()],
  body: '',
  bodyType: 'none',
  formData: [createEmptyFormDataField()],
  urlEncodedData: [createEmptyKeyValue()],
})

const createWebSocketRequest = (): WebSocketRequest => ({
  id: generateId(),
  name: 'New WebSocket',
  protocol: 'websocket',
  url: 'wss://',
  headers: [createEmptyKeyValue()],
  message: '',
  messageType: 'text',
})

const createGraphQLRequest = (): GraphQLRequest => ({
  id: generateId(),
  name: 'New GraphQL',
  protocol: 'graphql',
  url: '',
  headers: [createEmptyKeyValue()],
  query: `query {
  
}`,
  variables: '{}',
  operationName: '',
})

const createGrpcRequest = (): GrpcRequest => ({
  id: generateId(),
  name: 'New gRPC',
  protocol: 'grpc',
  url: '',
  service: '',
  method: '',
  protoFile: '',
  message: '{}',
  metadata: [createEmptyKeyValue()],
})

const createMqttRequest = (): MqttRequest => ({
  id: generateId(),
  name: 'New MQTT',
  protocol: 'mqtt',
  broker: '',
  port: 1883,
  clientId: `istek-${generateId().slice(0, 8)}`,
  username: '',
  password: '',
  topic: '',
  message: '',
  qos: 0,
  retain: false,
  useTls: false,
})

const createUnixSocketRequest = (): UnixSocketRequest => ({
  id: generateId(),
  name: 'New Unix Socket',
  protocol: 'unix-socket',
  socketPath: '/var/run/docker.sock',
  method: 'GET',
  path: '/',
  headers: [createEmptyKeyValue()],
  body: '',
})

const createMcpRequest = (): McpRequest => ({
  id: generateId(),
  name: 'New MCP',
  protocol: 'mcp',
  transportType: 'stdio',
  command: '',
  args: [],
  env: {},
  toolInput: '{}',
})

const createSseRequest = (): SseRequest => ({
  id: generateId(),
  name: 'New SSE',
  protocol: 'sse',
  url: '',
  headers: [createEmptyKeyValue()],
  withCredentials: false,
})

const createRequestByProtocol = (protocol: ProtocolType): RequestType => {
  switch (protocol) {
    case 'http': return createHttpRequest()
    case 'websocket': return createWebSocketRequest()
    case 'graphql': return createGraphQLRequest()
    case 'grpc': return createGrpcRequest()
    case 'mqtt': return createMqttRequest()
    case 'unix-socket': return createUnixSocketRequest()
    case 'mcp': return createMcpRequest()
    case 'sse': return createSseRequest()
    default: return createHttpRequest()
  }
}

const createNewRequestTab = (protocol: ProtocolType = 'http'): RequestTab => ({
  id: generateId(),
  type: 'request',
  protocol,
  request: createRequestByProtocol(protocol),
  response: null,
  isLoading: false,
  isDirty: false,
  wsState: protocol === 'websocket' ? { connected: false, messages: [], connectionId: null } : undefined,
  mqttState: protocol === 'mqtt' ? { connected: false, messages: [], subscribedTopics: [], connectionId: null } : undefined,
  mcpState: protocol === 'mcp' ? { connected: false, tools: [], resources: [], prompts: [], connectionId: null } : undefined,
  sseState: protocol === 'sse' ? { connected: false, events: [], connectionId: null } : undefined,
})

const createNewTestTab = (collectionId: string | undefined, collectionName: string): TestTab => ({
  id: generateId(),
  type: 'test',
  name: `Test: ${collectionName}`,
  collectionId,
  collectionName,
})

const createNewMockTab = (collectionId: string | undefined, collectionName: string): MockTab => ({
  id: generateId(),
  type: 'mock',
  name: `Mock: ${collectionName}`,
  collectionId,
  collectionName,
})

export const useAppStore = () => {
  // Tabs
  const tabs = useState<Tab[]>('tabs', () => [createNewRequestTab('http')])
  const activeTabId = useState<string>('activeTabId', () => tabs.value[0].id)
  
  // Sidebar
  const sidebarTab = useState<'history' | 'collections'>('sidebarTab', () => 'history')
  const sidebarCollapsed = useState<boolean>('sidebarCollapsed', () => false)
  
  // Collections & History
  const collections = useState<Collection[]>('collections', () => [])
  const history = useState<HistoryItem[]>('history', () => [])
  
  // Loading state for initial data load
  const isDataLoaded = useState<boolean>('isDataLoaded', () => false)

  // Computed
  const activeTab = computed(() => 
    tabs.value.find(t => t.id === activeTabId.value) || tabs.value[0]
  )

  // Tab actions
  const addTab = (protocol: ProtocolType = 'http') => {
    const newTab = createNewRequestTab(protocol)
    tabs.value = [...tabs.value, newTab]
    activeTabId.value = newTab.id
  }

  const addTestTab = (collectionId: string | undefined, collectionName: string) => {
    // Check if a test tab for this collection already exists
    const existingTab = tabs.value.find(
      t => t.type === 'test' && (t as TestTab).collectionId === collectionId
    )
    if (existingTab) {
      activeTabId.value = existingTab.id
      return
    }
    
    const newTab = createNewTestTab(collectionId, collectionName)
    tabs.value = [...tabs.value, newTab]
    activeTabId.value = newTab.id
  }

  const addMockTab = (collectionId: string | undefined, collectionName: string) => {
    // Check if a mock tab for this collection already exists
    const existingTab = tabs.value.find(
      t => t.type === 'mock' && (t as MockTab).collectionId === collectionId
    )
    if (existingTab) {
      activeTabId.value = existingTab.id
      return
    }
    
    const newTab = createNewMockTab(collectionId, collectionName)
    tabs.value = [...tabs.value, newTab]
    activeTabId.value = newTab.id
  }

  const closeTab = (tabId: string) => {
    if (tabs.value.length === 1) {
      tabs.value = [createNewRequestTab('http')]
      activeTabId.value = tabs.value[0].id
      return
    }
    
    const index = tabs.value.findIndex(t => t.id === tabId)
    tabs.value = tabs.value.filter(t => t.id !== tabId)
    
    if (activeTabId.value === tabId) {
      activeTabId.value = tabs.value[Math.max(0, index - 1)].id
    }
  }

  const setActiveTab = (tabId: string) => {
    activeTabId.value = tabId
  }

  const changeProtocol = (protocol: ProtocolType) => {
    const currentTab = activeTab.value
    if (currentTab.type !== 'request') return
    
    tabs.value = tabs.value.map(tab => 
      tab.id === activeTabId.value
        ? { 
            ...createNewRequestTab(protocol),
            id: tab.id,
          }
        : tab
    )
  }

  const updateActiveRequest = (updates: Partial<RequestType>) => {
    tabs.value = tabs.value.map(tab => 
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, request: { ...(tab as RequestTab).request, ...updates } as RequestType, isDirty: true }
        : tab
    )
  }

  const setActiveResponse = (response: ResponseType | null) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, response }
        : tab
    )
  }

  const setActiveLoading = (loading: boolean) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, isLoading: loading }
        : tab
    )
  }

  // WebSocket state
  const updateWsState = (updates: Partial<WebSocketState>) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, wsState: { ...(tab as RequestTab).wsState!, ...updates } }
        : tab
    )
  }

  const addWsMessage = (message: WebSocketMessage) => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.wsState) {
          return { ...tab, wsState: { ...reqTab.wsState, messages: [...reqTab.wsState.messages, message] } }
        }
      }
      return tab
    })
  }

  const clearWsMessages = () => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.wsState) {
          return { ...tab, wsState: { ...reqTab.wsState, messages: [] } }
        }
      }
      return tab
    })
  }

  // MQTT state
  const updateMqttState = (updates: Partial<MqttState>) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, mqttState: { ...(tab as RequestTab).mqttState!, ...updates } }
        : tab
    )
  }

  const addMqttMessage = (message: MqttMessage) => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.mqttState) {
          return { ...tab, mqttState: { ...reqTab.mqttState, messages: [...reqTab.mqttState.messages, message] } }
        }
      }
      return tab
    })
  }

  const clearMqttMessages = () => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.mqttState) {
          return { ...tab, mqttState: { ...reqTab.mqttState, messages: [] } }
        }
      }
      return tab
    })
  }

  // MCP state
  const updateMcpState = (updates: Partial<McpState>) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, mcpState: { ...(tab as RequestTab).mcpState!, ...updates } }
        : tab
    )
  }

  // SSE state
  const updateSseState = (updates: Partial<SseState>) => {
    tabs.value = tabs.value.map(tab =>
      tab.id === activeTabId.value && tab.type === 'request'
        ? { ...tab, sseState: { ...(tab as RequestTab).sseState!, ...updates } }
        : tab
    )
  }

  const addSseEvent = (event: SseEvent) => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.sseState) {
          return { ...tab, sseState: { ...reqTab.sseState, events: [...reqTab.sseState.events, event] } }
        }
      }
      return tab
    })
  }

  const clearSseEvents = () => {
    tabs.value = tabs.value.map(tab => {
      if (tab.id === activeTabId.value && tab.type === 'request') {
        const reqTab = tab as RequestTab
        if (reqTab.sseState) {
          return { ...tab, sseState: { ...reqTab.sseState, events: [] } }
        }
      }
      return tab
    })
  }

  // SSE header helpers
  const addSseHeader = () => {
    const currentTab = activeTab.value
    if (currentTab.type !== 'request' || currentTab.protocol !== 'sse') return
    
    const request = currentTab.request as SseRequest
    updateActiveRequest({ headers: [...request.headers, createEmptyKeyValue()] })
  }

  const updateSseHeader = (id: string, field: 'key' | 'value', value: string) => {
    const currentTab = activeTab.value
    if (currentTab.type !== 'request' || currentTab.protocol !== 'sse') return
    
    const request = currentTab.request as SseRequest
    updateActiveRequest({
      headers: request.headers.map(h => h.id === id ? { ...h, [field]: value } : h)
    })
  }

  const toggleSseHeader = (id: string) => {
    const currentTab = activeTab.value
    if (currentTab.type !== 'request' || currentTab.protocol !== 'sse') return
    
    const request = currentTab.request as SseRequest
    updateActiveRequest({
      headers: request.headers.map(h => h.id === id ? { ...h, enabled: !h.enabled } : h)
    })
  }

  const removeSseHeader = (id: string) => {
    const currentTab = activeTab.value
    if (currentTab.type !== 'request' || currentTab.protocol !== 'sse') return
    
    const request = currentTab.request as SseRequest
    updateActiveRequest({
      headers: request.headers.filter(h => h.id !== id)
    })
  }

  // Header/Param helpers (for HTTP-like protocols)
  const addHeader = () => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest | WebSocketRequest | GraphQLRequest | UnixSocketRequest
    if ('headers' in request) {
      updateActiveRequest({
        headers: [...request.headers, createEmptyKeyValue()]
      })
    }
  }

  const updateHeader = (id: string, field: 'key' | 'value', value: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('headers' in request) {
      updateActiveRequest({
        headers: request.headers.map(h =>
          h.id === id ? { ...h, [field]: value } : h
        )
      })
    }
  }

  const toggleHeader = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('headers' in request) {
      updateActiveRequest({
        headers: request.headers.map(h =>
          h.id === id ? { ...h, enabled: !h.enabled } : h
        )
      })
    }
  }

  const removeHeader = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('headers' in request) {
      updateActiveRequest({
        headers: request.headers.filter(h => h.id !== id)
      })
    }
  }

  const addParam = () => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('params' in request) {
      updateActiveRequest({
        params: [...request.params, createEmptyKeyValue()]
      })
    }
  }

  const updateParam = (id: string, field: 'key' | 'value', value: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('params' in request) {
      updateActiveRequest({
        params: request.params.map(p =>
          p.id === id ? { ...p, [field]: value } : p
        )
      })
    }
  }

  const toggleParam = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('params' in request) {
      updateActiveRequest({
        params: request.params.map(p =>
          p.id === id ? { ...p, enabled: !p.enabled } : p
        )
      })
    }
  }

  const removeParam = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if ('params' in request) {
      updateActiveRequest({
        params: request.params.filter(p => p.id !== id)
      })
    }
  }

  // Form Data helpers (for form-data body type with file support)
  const addFormData = () => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    const currentFormData = request.formData || []
    updateActiveRequest({
      formData: [...currentFormData, createEmptyFormDataField()]
    })
  }

  const updateFormData = (id: string, field: 'key' | 'value' | 'type', value: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.formData) {
      updateActiveRequest({
        formData: request.formData.map(f =>
          f.id === id ? { ...f, [field]: value } : f
        )
      })
    }
  }

  const updateFormDataFile = (id: string, filePath: string, fileName: string, fileSize: number, mimeType: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.formData) {
      updateActiveRequest({
        formData: request.formData.map(f =>
          f.id === id ? { ...f, value: filePath, fileName, fileSize, mimeType } : f
        )
      })
    }
  }

  const toggleFormData = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.formData) {
      updateActiveRequest({
        formData: request.formData.map(f =>
          f.id === id ? { ...f, enabled: !f.enabled } : f
        )
      })
    }
  }

  const removeFormData = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.formData) {
      updateActiveRequest({
        formData: request.formData.filter(f => f.id !== id)
      })
    }
  }

  // URL Encoded Data helpers (for x-www-form-urlencoded body type)
  const addUrlEncodedData = () => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    const currentData = request.urlEncodedData || []
    updateActiveRequest({
      urlEncodedData: [...currentData, createEmptyKeyValue()]
    })
  }

  const updateUrlEncodedData = (id: string, field: 'key' | 'value', value: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.urlEncodedData) {
      updateActiveRequest({
        urlEncodedData: request.urlEncodedData.map(f =>
          f.id === id ? { ...f, [field]: value } : f
        )
      })
    }
  }

  const toggleUrlEncodedData = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.urlEncodedData) {
      updateActiveRequest({
        urlEncodedData: request.urlEncodedData.map(f =>
          f.id === id ? { ...f, enabled: !f.enabled } : f
        )
      })
    }
  }

  const removeUrlEncodedData = (id: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    const request = (tab as RequestTab).request as HttpRequest
    if (request.urlEncodedData) {
      updateActiveRequest({
        urlEncodedData: request.urlEncodedData.filter(f => f.id !== id)
      })
    }
  }

  // History
  const addToHistory = async (request: RequestType, response: ResponseType | null) => {
    const item: HistoryItem = {
      id: generateId(),
      request: { ...request },
      response,
      timestamp: Date.now(),
    }
    
    history.value = [item, ...history.value.slice(0, 99)]
    
    // Persist to database
    try {
      await invoke('save_history_item', { item })
    } catch (e) {
      console.error('Failed to save history item:', e)
    }
  }

  const clearHistory = async () => {
    history.value = []
    
    // Persist to database
    try {
      await invoke('clear_history')
    } catch (e) {
      console.error('Failed to clear history:', e)
    }
  }

  const deleteHistoryItem = async (itemId: string) => {
    history.value = history.value.filter(h => h.id !== itemId)
    
    // Persist to database - we'll save by clearing and re-adding all
    // For now, just update the local state (items are persisted on add)
    try {
      await invoke('delete_history_item', { id: itemId })
    } catch (e) {
      // If delete command doesn't exist, that's okay - item is removed from memory
      console.error('Failed to delete history item:', e)
    }
  }

  const loadFromHistory = (item: HistoryItem) => {
    // Check if this history item is already open in a tab
    const existingTab = tabs.value.find(t => {
      if (t.type !== 'request') return false
      const reqTab = t as RequestTab
      return reqTab.sourceHistoryId === item.id
    })
    
    if (existingTab) {
      // Switch to existing tab
      activeTabId.value = existingTab.id
      return
    }
    
    // Create new tab
    const protocol = item.request.protocol || 'http'
    const newTab = createNewRequestTab(protocol)
    newTab.request = { ...item.request, id: generateId() }
    // Track the source history item
    newTab.sourceHistoryId = item.id
    tabs.value = [...tabs.value, newTab]
    activeTabId.value = newTab.id
  }

  // Collections
  const addCollection = async (name: string, protocolType?: ProtocolType) => {
    // Use active tab's protocol if not specified
    const currentProtocol = protocolType || (activeTab.value?.type === 'request' ? (activeTab.value as RequestTab).protocol : 'http')
    
    const collection: Collection = { 
      id: generateId(), 
      name, 
      requests: [], 
      protocolType: currentProtocol,
      createdAt: Date.now() 
    }
    collections.value = [...collections.value, collection]
    
    // Persist to database
    try {
      await invoke('save_collection', { collection })
      await refreshGitStatusIfEnabled()
    } catch (e) {
      console.error('Failed to save collection:', e)
    }
  }

  const saveToCollection = async (collectionId: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    
    const updatedCollections = collections.value.map(c =>
      c.id === collectionId
        ? { ...c, requests: [...c.requests, { ...(tab as RequestTab).request, id: generateId() }] }
        : c
    )
    collections.value = updatedCollections
    
    // Persist to database
    const collection = updatedCollections.find(c => c.id === collectionId)
    if (collection) {
      try {
        await invoke('save_collection', { collection })
        await refreshGitStatusIfEnabled()
      } catch (e) {
        console.error('Failed to save collection:', e)
      }
    }
  }

  // Update existing request in its original collection
  const updateInCollection = async (collectionId: string, requestId: string) => {
    const tab = activeTab.value
    if (tab.type !== 'request') return
    
    const reqTab = tab as RequestTab
    const updatedCollections = collections.value.map(c => {
      if (c.id !== collectionId) return c
      
      return {
        ...c,
        requests: c.requests.map(r => 
          r.id === requestId ? { ...reqTab.request, id: requestId } : r
        )
      }
    })
    collections.value = updatedCollections
    
    // Mark tab as not dirty
    tabs.value = tabs.value.map(t =>
      t.id === activeTabId.value ? { ...t, isDirty: false } : t
    )
    
    // Persist to database
    const collection = updatedCollections.find(c => c.id === collectionId)
    if (collection) {
      try {
        await invoke('save_collection', { collection })
        await refreshGitStatusIfEnabled()
      } catch (e) {
        console.error('Failed to update collection:', e)
      }
    }
  }

  // Check if active tab has a source (came from a collection)
  const activeTabHasSource = computed(() => {
    const tab = activeTab.value
    if (tab.type !== 'request') return false
    const reqTab = tab as RequestTab
    return !!(reqTab.sourceCollectionId && reqTab.sourceRequestId)
  })

  // Save active request - either update in place or show collection picker
  const saveActiveRequest = async () => {
    const tab = activeTab.value
    if (tab.type !== 'request') return false
    
    const reqTab = tab as RequestTab
    if (reqTab.sourceCollectionId && reqTab.sourceRequestId) {
      await updateInCollection(reqTab.sourceCollectionId, reqTab.sourceRequestId)
      return true // Saved in place
    }
    return false // Need to pick collection
  }

  const loadFromCollection = (request: RequestType, collectionId?: string) => {
    // Check if this request is already open in a tab
    if (collectionId && request.id) {
      const existingTab = tabs.value.find(t => {
        if (t.type !== 'request') return false
        const reqTab = t as RequestTab
        return reqTab.sourceCollectionId === collectionId && reqTab.sourceRequestId === request.id
      })
      
      if (existingTab) {
        // Switch to existing tab
        activeTabId.value = existingTab.id
        return
      }
    }
    
    // Create new tab
    const protocol = request.protocol || 'http'
    const newTab = createNewRequestTab(protocol)
    newTab.request = { ...request }
    // Track the source collection and request for saving back
    if (collectionId) {
      newTab.sourceCollectionId = collectionId
      newTab.sourceRequestId = request.id
    }
    tabs.value = [...tabs.value, newTab]
    activeTabId.value = newTab.id
  }

  const deleteCollection = async (collectionId: string) => {
    collections.value = collections.value.filter(c => c.id !== collectionId)
    
    // Persist to database
    try {
      await invoke('delete_collection', { id: collectionId })
      await refreshGitStatusIfEnabled()
    } catch (e) {
      console.error('Failed to delete collection:', e)
    }
  }

  const updateCollection = async (collection: Collection) => {
    // Update in store
    const index = collections.value.findIndex(c => c.id === collection.id)
    if (index !== -1) {
      collections.value[index] = collection
      collections.value = [...collections.value]
    }
    
    // Persist to database
    try {
      await invoke('save_collection', { collection })
      await refreshGitStatusIfEnabled()
    } catch (e) {
      console.error('Failed to update collection:', e)
    }
  }

  const deleteRequestFromCollection = async (collectionId: string, requestId: string) => {
    const updatedCollections = collections.value.map(c =>
      c.id === collectionId
        ? { ...c, requests: c.requests.filter(r => r.id !== requestId) }
        : c
    )
    collections.value = updatedCollections
    
    // Persist to database
    const collection = updatedCollections.find(c => c.id === collectionId)
    if (collection) {
      try {
        await invoke('save_collection', { collection })
        await refreshGitStatusIfEnabled()
      } catch (e) {
        console.error('Failed to save collection:', e)
      }
    }
  }

  const deleteRequestFromFolder = async (collectionId: string, folderId: string, requestId: string) => {
    const updatedCollections = collections.value.map(c => {
      if (c.id !== collectionId) return c
      
      const updatedFolders = c.folders?.map(f => {
        if (f.id !== folderId) return f
        return { ...f, requests: f.requests?.filter(r => r.id !== requestId) || [] }
      })
      
      return { ...c, folders: updatedFolders }
    })
    collections.value = updatedCollections
    
    // Persist to database
    const collection = updatedCollections.find(c => c.id === collectionId)
    if (collection) {
      try {
        await invoke('save_collection', { collection })
        await refreshGitStatusIfEnabled()
      } catch (e) {
        console.error('Failed to save collection:', e)
      }
    }
  }
  
  // Load data from database
  const loadDataFromDatabase = async () => {
    if (isDataLoaded.value) return
    
    try {
      const data = await invoke<{
        workspaces: { id: string; name: string; syncPath?: string; isDefault: boolean; createdAt: number }[]
        activeWorkspaceId: string | null
        collections: Collection[]
        history: HistoryItem[]
      }>('load_app_data')
      
      // Set workspace data via workspace store
      const workspaceStore = useWorkspaceStore()
      workspaceStore.setWorkspaces({
        workspaces: data.workspaces,
        activeWorkspaceId: data.activeWorkspaceId
      })
      
      if (data.collections.length > 0) {
        collections.value = data.collections
      }
      if (data.history.length > 0) {
        history.value = data.history
      }
      
      isDataLoaded.value = true
    } catch (e) {
      console.error('Failed to load app data:', e)
    }
  }

  // Set workspace-specific data (called when switching workspaces)
  const setWorkspaceData = (data: { collections: Collection[]; history: HistoryItem[] }) => {
    collections.value = data.collections || []
    history.value = data.history || []
  }

  return {
    // State
    tabs,
    activeTabId,
    activeTab,
    sidebarTab,
    sidebarCollapsed,
    collections,
    history,
    isDataLoaded,
    
    // Tab actions
    addTab,
    addTestTab,
    addMockTab,
    closeTab,
    setActiveTab,
    changeProtocol,
    updateActiveRequest,
    setActiveResponse,
    setActiveLoading,
    
    // WebSocket state
    updateWsState,
    addWsMessage,
    clearWsMessages,
    
    // MQTT state
    updateMqttState,
    addMqttMessage,
    clearMqttMessages,
    
    // MCP state
    updateMcpState,
    
    // SSE state
    updateSseState,
    addSseEvent,
    clearSseEvents,
    addSseHeader,
    updateSseHeader,
    toggleSseHeader,
    removeSseHeader,
    
    // Header/Param actions
    addHeader,
    updateHeader,
    toggleHeader,
    removeHeader,
    addParam,
    updateParam,
    toggleParam,
    removeParam,
    
    // Form Data actions (multipart/form-data)
    addFormData,
    updateFormData,
    updateFormDataFile,
    toggleFormData,
    removeFormData,
    
    // URL Encoded Data actions (x-www-form-urlencoded)
    addUrlEncodedData,
    updateUrlEncodedData,
    toggleUrlEncodedData,
    removeUrlEncodedData,
    
    // History actions
    addToHistory,
    clearHistory,
    deleteHistoryItem,
    loadFromHistory,
    
    // Collection actions
    addCollection,
    saveToCollection,
    updateInCollection,
    updateCollection,
    loadFromCollection,
    deleteCollection,
    deleteRequestFromCollection,
    deleteRequestFromFolder,
    activeTabHasSource,
    saveActiveRequest,
    
    // Database actions
    loadDataFromDatabase,
    setWorkspaceData,
  }
}
