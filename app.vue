<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { HttpRequest, HttpResponse, RequestTab, TestTab, MockTab, Collection, KeyValue } from '~/types'
import { resolveAuth, resolveHeaders, resolveBaseUrl, applyAuthToHeaders, getAuthQueryParams } from '~/lib/utils'

// Zoom level management - synced with variableStore
const variableStore = useVariableStore()
const { appZoom } = variableStore

const showZoomIndicator = ref(false)
const MIN_ZOOM = 0.5
const MAX_ZOOM = 2.0
const ZOOM_STEP = 0.1

let zoomIndicatorTimeout: ReturnType<typeof setTimeout> | null = null

const setZoom = async (level: number) => {
  const clampedLevel = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, level))
  // Round to avoid floating point issues
  appZoom.value = Math.round(clampedLevel * 10) / 10
  
  // Show indicator
  showZoomIndicator.value = true
  if (zoomIndicatorTimeout) clearTimeout(zoomIndicatorTimeout)
  zoomIndicatorTimeout = setTimeout(() => {
    showZoomIndicator.value = false
  }, 1500)
  
  try {
    const webview = getCurrentWebviewWindow()
    await webview.setZoom(appZoom.value)
  } catch (e) {
    console.error('Failed to set zoom:', e)
  }
}

const zoomIn = () => setZoom(appZoom.value + ZOOM_STEP)
const zoomOut = () => setZoom(appZoom.value - ZOOM_STEP)
const resetZoom = () => setZoom(1.0)

const zoomPercentage = computed(() => Math.round(appZoom.value * 100))

// Types for scripting
interface ScriptRequest {
  method: string
  url: string
  headers: Record<string, string>
  params: Record<string, string>
  body: string | null
}

interface ScriptResponse {
  status: number
  statusText: string
  headers: Record<string, string>
  body: string
  time: number
}

interface ScriptContext {
  request: ScriptRequest
  response?: ScriptResponse
  variables: Record<string, string>
  environment: string
}

interface ScriptResult {
  success: boolean
  error: string | null
  consoleOutput: string[]
  modifiedVariables: Record<string, string>
  modifiedHeaders: Record<string, string>
  abortRequest: boolean
}

const store = useAppStore()
const { activeTab, collections } = store

// Search modal ref
const searchModalRef = ref<{ open: () => void } | null>(null)
const openSearch = () => {
  searchModalRef.value?.open()
}

// Helper to check if current tab is a request tab
const isRequestTab = computed(() => activeTab.value.type === 'request' || !('type' in activeTab.value))
const isTestTab = computed(() => activeTab.value.type === 'test')
const isMockTab = computed(() => activeTab.value.type === 'mock')

// Get the active tab as RequestTab (with fallback for legacy tabs without type)
const activeRequestTab = computed(() => {
  const tab = activeTab.value
  if (tab.type === 'request' || !('type' in tab)) {
    return tab as RequestTab
  }
  return null
})

const activeTestTab = computed(() => {
  if (activeTab.value.type === 'test') {
    return activeTab.value as TestTab
  }
  return null
})

const activeMockTab = computed(() => {
  if (activeTab.value.type === 'mock') {
    return activeTab.value as MockTab
  }
  return null
})

// AbortController for cancelling requests
let abortController: AbortController | null = null

// Find which collection and folder a request belongs to (if any)
const findRequestContext = (requestId: string): { collection?: Collection, folderId?: string } => {
  for (const collection of collections.value) {
    // Check root-level requests
    if (collection.requests?.some(r => r.id === requestId)) {
      return { collection }
    }
    
    // Check folder requests
    if (collection.folders) {
      for (const folder of collection.folders) {
        if (folder.requests?.some(r => r.id === requestId)) {
          return { collection, folderId: folder.id }
        }
      }
    }
  }
  return {}
}

const sendHttpRequest = async () => {
  if (!activeRequestTab.value) return
  const request = activeRequestTab.value.request as HttpRequest
  
  if (!request.url) {
    return
  }

  // Create new abort controller for this request
  abortController = new AbortController()
  
  store.setActiveLoading(true)
  store.setActiveResponse(null)

  try {
    // Find the collection and folder context for inheritance
    const { collection, folderId } = findRequestContext(request.id)
    
    // Resolve inherited settings
    let resolvedAuth = request.auth
    let resolvedHeaders = request.headers
    let baseUrl = ''
    let additionalParams: KeyValue[] = []
    
    if (collection) {
      // Resolve auth from inheritance chain
      resolvedAuth = resolveAuth(collection, folderId || request.folderId, request.auth)
      
      // Resolve headers from inheritance chain
      resolvedHeaders = resolveHeaders(collection, folderId || request.folderId, request.headers)
      
      // Resolve base URL
      baseUrl = resolveBaseUrl(collection, folderId || request.folderId) || ''
      
      // Apply auth to headers if we have resolved auth
      if (resolvedAuth && resolvedAuth.enabled && resolvedAuth.type !== 'none' && resolvedAuth.type !== 'inherit') {
        resolvedHeaders = applyAuthToHeaders(resolvedAuth, resolvedHeaders, variableStore.interpolate)
        
        // Get query params from API key auth
        additionalParams = getAuthQueryParams(resolvedAuth, variableStore.interpolate)
      }
    }
    
    // Construct full URL with base URL prefix
    let fullUrl = request.url
    if (baseUrl && !request.url.startsWith('http://') && !request.url.startsWith('https://')) {
      // Ensure proper joining with slash
      const cleanBase = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl
      const cleanPath = request.url.startsWith('/') ? request.url : '/' + request.url
      fullUrl = cleanBase + cleanPath
    } else if (baseUrl && !request.url.startsWith('http')) {
      // URL doesn't have protocol but has a base URL
      fullUrl = baseUrl + (request.url.startsWith('/') ? '' : '/') + request.url
    }
    
    // Interpolate variables in request (using async for template functions)
    const interpolatedUrl = await variableStore.interpolateAsync(fullUrl)
    let interpolatedBody = await variableStore.interpolateAsync(request.body)
    
    // Prepare form data for multipart or url-encoded requests
    let formDataFields: Array<{ key: string; value: string; type: string; filePath?: string }> | null = null
    
    // Handle x-www-form-urlencoded - convert to string body
    if (request.bodyType === 'x-www-form-urlencoded' && request.urlEncodedData) {
      const formParts: string[] = []
      for (const field of request.urlEncodedData.filter(f => f.enabled && f.key)) {
        const key = await variableStore.interpolateAsync(field.key)
        const value = await variableStore.interpolateAsync(field.value)
        formParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
      }
      interpolatedBody = formParts.join('&')
    }
    
    // Handle multipart form-data - prepare fields for Rust backend
    if (request.bodyType === 'form-data' && request.formData) {
      formDataFields = []
      for (const field of request.formData.filter(f => f.enabled && f.key)) {
        const key = await variableStore.interpolateAsync(field.key)
        if (field.type === 'file') {
          formDataFields.push({
            key,
            value: field.fileName || '',
            type: 'file',
            filePath: field.value, // file path stored in value
          })
        } else {
          const value = await variableStore.interpolateAsync(field.value)
          formDataFields.push({ key, value, type: 'text' })
        }
      }
    }
    
    // Build interpolated headers
    const headerEntries: Record<string, string> = {}
    for (const h of resolvedHeaders.filter(h => h.enabled && h.key)) {
      const key = await variableStore.interpolateAsync(h.key)
      const value = await variableStore.interpolateAsync(h.value)
      headerEntries[key] = value
    }
    const interpolatedHeaders = headerEntries
    
    // Merge request params with auth query params
    const allParams = [...request.params, ...additionalParams]
    const paramEntries: Record<string, string> = {}
    for (const p of allParams.filter(p => p.enabled && p.key)) {
      const key = await variableStore.interpolateAsync(p.key)
      const value = await variableStore.interpolateAsync(p.value)
      paramEntries[key] = value
    }
    let interpolatedParams = paramEntries

    // Run pre-request script if present
    if (request.preRequestScript?.trim()) {
      const scriptContext: ScriptContext = {
        request: {
          method: request.method,
          url: interpolatedUrl,
          headers: interpolatedHeaders,
          params: interpolatedParams,
          body: request.bodyType !== 'none' ? interpolatedBody : null,
        },
        variables: Object.fromEntries(variableStore.resolvedVariables.value),
        environment: variableStore.activeEnvironment.value?.name || 'Default',
      }
      
      try {
        const scriptResult = await invoke<ScriptResult>('run_pre_request_script', {
          script: request.preRequestScript,
          context: scriptContext,
        })
        
        // Log console output
        if (scriptResult.consoleOutput.length > 0) {
          console.log('[Pre-request Script]', scriptResult.consoleOutput.join('\n'))
        }
        
        // Check for abort
        if (scriptResult.abortRequest) {
          store.setActiveLoading(false)
          store.setActiveResponse({
            status: 0,
            statusText: 'Aborted',
            headers: {},
            body: 'Request aborted by pre-request script',
            time: 0,
            size: 0,
          })
          return
        }
        
        // Apply modified headers
        for (const [key, value] of Object.entries(scriptResult.modifiedHeaders)) {
          interpolatedHeaders[key] = value
        }
        
        // Apply modified variables (store for future requests)
        for (const [key, value] of Object.entries(scriptResult.modifiedVariables)) {
          // Update the global variables or environment variables
          const existingVar = variableStore.globalVariables.value.find(v => v.key === key)
          if (existingVar) {
            await variableStore.updateGlobalVariable(existingVar.id, { value })
          } else {
            await variableStore.addGlobalVariable({ key, value })
          }
        }
        
        // Check for script error
        if (!scriptResult.success && scriptResult.error) {
          console.error('[Pre-request Script Error]', scriptResult.error)
        }
      } catch (e) {
        console.error('[Pre-request Script Error]', e)
      }
    }

    // Send request - use multipart for form-data with files
    let response: HttpResponse
    if (request.bodyType === 'form-data' && formDataFields) {
      response = await invoke<HttpResponse>('send_multipart_request', {
        method: request.method,
        url: interpolatedUrl,
        headers: interpolatedHeaders,
        params: interpolatedParams,
        formFields: formDataFields,
      })
    } else {
      response = await invoke<HttpResponse>('send_http_request', {
        method: request.method,
        url: interpolatedUrl,
        headers: interpolatedHeaders,
        params: interpolatedParams,
        body: request.bodyType !== 'none' ? interpolatedBody : null,
        bodyType: request.bodyType,
      })
    }

    // Run post-request script if present
    if (request.postRequestScript?.trim()) {
      const scriptContext: ScriptContext = {
        request: {
          method: request.method,
          url: interpolatedUrl,
          headers: interpolatedHeaders,
          params: interpolatedParams,
          body: request.bodyType !== 'none' ? interpolatedBody : null,
        },
        response: {
          status: response.status,
          statusText: response.statusText,
          headers: response.headers,
          body: response.body,
          time: response.time,
        },
        variables: Object.fromEntries(variableStore.resolvedVariables.value),
        environment: variableStore.activeEnvironment.value?.name || 'Default',
      }
      
      try {
        const scriptResult = await invoke<ScriptResult>('run_post_request_script', {
          script: request.postRequestScript,
          context: scriptContext,
        })
        
        // Log console output
        if (scriptResult.consoleOutput.length > 0) {
          console.log('[Post-request Script]', scriptResult.consoleOutput.join('\n'))
        }
        
        // Apply modified variables
        for (const [key, value] of Object.entries(scriptResult.modifiedVariables)) {
          const existingVar = variableStore.globalVariables.value.find(v => v.key === key)
          if (existingVar) {
            await variableStore.updateGlobalVariable(existingVar.id, { value })
          } else {
            await variableStore.addGlobalVariable({ key, value })
          }
        }
        
        // Check for script error
        if (!scriptResult.success && scriptResult.error) {
          console.error('[Post-request Script Error]', scriptResult.error)
        }
      } catch (e) {
        console.error('[Post-request Script Error]', e)
      }
    }

    store.setActiveResponse(response)
    store.addToHistory(request, response)
  } catch (error: any) {
    if (error.name !== 'AbortError') {
      store.setActiveResponse({
        status: 0,
        statusText: 'Error',
        headers: {},
        body: error.toString(),
        time: 0,
        size: 0,
      })
    }
  } finally {
    store.setActiveLoading(false)
    abortController = null
  }
}

const cancelRequest = () => {
  if (abortController) {
    abortController.abort()
  }
  store.setActiveLoading(false)
}

// Dark mode by default and load data from database
onMounted(async () => {
  document.documentElement.classList.add('dark')
  
  // Load persisted data from SQLite
  await Promise.all([
    store.loadDataFromDatabase(),
    variableStore.loadVariableDataFromDatabase(),
  ])
  
  // Keyboard shortcuts for zoom
  const handleZoomKeydown = (e: KeyboardEvent) => {
    // CMD/Ctrl + Plus (zoom in)
    if ((e.metaKey || e.ctrlKey) && (e.key === '=' || e.key === '+')) {
      e.preventDefault()
      zoomIn()
    }
    // CMD/Ctrl + Minus (zoom out)
    else if ((e.metaKey || e.ctrlKey) && e.key === '-') {
      e.preventDefault()
      zoomOut()
    }
    // CMD/Ctrl + 0 (reset zoom)
    else if ((e.metaKey || e.ctrlKey) && e.key === '0') {
      e.preventDefault()
      resetZoom()
    }
  }
  
  window.addEventListener('keydown', handleZoomKeydown)
  
  onUnmounted(() => {
    window.removeEventListener('keydown', handleZoomKeydown)
  })
})
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-background">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Main Content -->
    <main class="flex flex-1 flex-col overflow-hidden">
      <!-- Search Bar - Centered at top with Settings on right -->
      <div class="flex items-center py-3 border-b border-border bg-muted/30 px-4">
        <!-- Left spacer for balance -->
        <div class="w-10"></div>
        
        <!-- Centered Search Button -->
        <div class="flex-1 flex justify-center">
          <button
            class="flex items-center gap-3 h-11 px-4 w-[500px] text-sm text-muted-foreground hover:text-foreground bg-background border border-border hover:border-primary/50 rounded-lg transition-colors shadow-sm"
            title="Search Everywhere (⌘K)"
            @click="openSearch"
          >
            <Icon name="lucide:search" class="h-4 w-4" />
            <span class="flex-1 text-left">Search everywhere...</span>
            <div class="flex items-center gap-0.5 px-2 py-1 text-xs font-mono bg-secondary/80 rounded border border-border">
              <span>⌘</span>
              <span>+</span>
              <span>K</span>
            </div>
          </button>
        </div>
        
        <!-- Settings Button - Right aligned -->
        <button
          class="w-10 h-10 flex items-center justify-center rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title="Settings"
          @click="variableStore.openVariableManager('general')"
        >
          <Icon name="lucide:settings" class="h-5 w-5" />
        </button>
      </div>

      <!-- Header with Protocol Selector, Tabs, and Environment -->
      <div class="flex items-center border-b border-border">
        <div class="flex items-center gap-2 border-r border-border px-3 py-2">
          <ProtocolSelector />
        </div>
        <RequestTabs />
        <div class="flex items-center gap-2 border-l border-border px-3 py-2">
          <EnvironmentSelector />
          <UiButton
            variant="ghost"
            size="icon"
            class="h-9 w-9"
            title="Manage Variables"
            @click="variableStore.openVariableManager()"
          >
            <Icon name="lucide:settings-2" class="h-5 w-5" />
          </UiButton>
        </div>
      </div>

      <!-- Protocol-specific content -->
      <div class="flex flex-1 overflow-hidden">
        <!-- Test Tab -->
        <template v-if="isTestTab && activeTestTab">
          <PanelsTestPanel :tab="activeTestTab" class="flex-1" />
        </template>
        
        <!-- Mock Tab -->
        <template v-else-if="isMockTab && activeMockTab">
          <PanelsMockPanel :tab="activeMockTab" class="flex-1" />
        </template>
        
        <!-- Request Tabs -->
        <template v-else-if="isRequestTab && activeRequestTab">
          <!-- HTTP -->
          <template v-if="activeRequestTab.protocol === 'http'">
            <div class="flex w-1/2 flex-col overflow-hidden">
              <RequestPanel @send="sendHttpRequest" @cancel="cancelRequest" />
            </div>
            <div class="flex w-1/2 flex-col overflow-hidden">
              <ResponsePanel />
            </div>
          </template>

          <!-- WebSocket -->
          <template v-else-if="activeRequestTab.protocol === 'websocket'">
            <PanelsWebSocketPanel class="flex-1" />
          </template>

          <!-- GraphQL -->
          <template v-else-if="activeRequestTab.protocol === 'graphql'">
            <PanelsGraphQLPanel class="flex-1" />
          </template>

          <!-- gRPC -->
          <template v-else-if="activeRequestTab.protocol === 'grpc'">
            <PanelsGrpcPanel class="flex-1" />
          </template>

          <!-- MQTT -->
          <template v-else-if="activeRequestTab.protocol === 'mqtt'">
            <PanelsMqttPanel class="flex-1" />
          </template>

          <!-- Unix Socket -->
          <template v-else-if="activeRequestTab.protocol === 'unix-socket'">
            <PanelsUnixSocketPanel class="flex-1" />
          </template>

          <!-- MCP -->
          <template v-else-if="activeRequestTab.protocol === 'mcp'">
            <PanelsMcpPanel class="flex-1" />
          </template>

          <!-- SSE -->
          <template v-else-if="activeRequestTab.protocol === 'sse'">
            <PanelsSsePanel class="flex-1" />
          </template>
        </template>
      </div>
    </main>

    <!-- Variable Manager Modal -->
    <VariableManager />
    
    <!-- Workspace Modals -->
    <CreateWorkspaceModal />
    <CommitModal />
    <GitHistoryModal />
    
    <!-- Search Modal -->
    <SearchModal ref="searchModalRef" />
    
    <!-- Zoom Indicator -->
    <Transition name="fade">
      <div
        v-if="showZoomIndicator"
        class="fixed bottom-6 left-1/2 -translate-x-1/2 z-[100] bg-background/95 border border-border rounded-lg shadow-lg px-4 py-2 flex items-center gap-3"
        :style="{ zoom: 1 / appZoom }"
      >
        <Icon name="lucide:zoom-in" class="w-4 h-4 text-muted-foreground" />
        <span class="text-sm font-medium">{{ zoomPercentage }}%</span>
        <div class="flex items-center gap-1 text-xs text-muted-foreground">
          <span class="px-1.5 py-0.5 bg-secondary rounded">⌘</span>
          <span>+/-</span>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
