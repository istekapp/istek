<script setup lang="ts">
import type { GrpcRequest, GrpcResponse, KeyValue } from '~/types'
import { invoke } from '@tauri-apps/api/core'

const store = useAppStore()
const variableStore = useVariableStore()
const { activeTab } = store

const request = computed(() => (activeTab.value as any).request as GrpcRequest)
const response = computed(() => (activeTab.value as any).response as GrpcResponse | null)
const isLoading = computed(() => (activeTab.value as any).isLoading)

// Service discovery state
interface GrpcMethodInfo {
  name: string
  fullName: string
  inputType: string
  outputType: string
  clientStreaming: boolean
  serverStreaming: boolean
  inputSchema?: any
}

interface GrpcServiceInfo {
  name: string
  fullName: string
  methods: GrpcMethodInfo[]
}

interface GrpcDiscoveryResult {
  success: boolean
  services: GrpcServiceInfo[]
  error?: string
  source: string
}

const discoveredServices = ref<GrpcServiceInfo[]>([])
const isDiscovering = ref(false)
const discoveryError = ref<string | null>(null)
const discoverySource = ref<string>('') // 'reflection' or 'proto'

// Proto file upload
const protoContent = ref('')
const showProtoUpload = ref(false)
const protoFileInput = ref<HTMLInputElement | null>(null)

// Selected service and method
const selectedService = ref<string>('')
const selectedMethod = ref<string>('')

const selectedServiceInfo = computed(() => 
  discoveredServices.value.find(s => s.fullName === selectedService.value)
)

const selectedMethodInfo = computed(() =>
  selectedServiceInfo.value?.methods.find(m => m.name === selectedMethod.value)
)

// Tabs for request configuration
const activeRequestTab = ref<'message' | 'metadata'>('message')

// Discover services via reflection
const discoverViaReflection = async () => {
  if (!request.value.url) {
    discoveryError.value = 'Please enter a server URL'
    return
  }
  
  isDiscovering.value = true
  discoveryError.value = null
  discoveredServices.value = []
  
  try {
    const url = variableStore.interpolate(request.value.url)
    const result = await invoke<GrpcDiscoveryResult>('grpc_discover_services', { url })
    
    if (result.success) {
      discoveredServices.value = result.services
      discoverySource.value = result.source
      
      // Auto-select first service and method if available
      if (result.services.length > 0) {
        selectedService.value = result.services[0].fullName
        if (result.services[0].methods.length > 0) {
          selectedMethod.value = result.services[0].methods[0].name
          updateRequestServiceMethod()
        }
      }
    } else {
      discoveryError.value = result.error || 'Failed to discover services'
    }
  } catch (e: any) {
    discoveryError.value = e.toString()
  } finally {
    isDiscovering.value = false
  }
}

// Parse proto file
const parseProtoFile = async () => {
  if (!protoContent.value.trim()) {
    discoveryError.value = 'Please provide proto file content'
    return
  }
  
  isDiscovering.value = true
  discoveryError.value = null
  discoveredServices.value = []
  
  try {
    const result = await invoke<GrpcDiscoveryResult>('grpc_parse_proto', { 
      protoContent: protoContent.value 
    })
    
    if (result.success) {
      discoveredServices.value = result.services
      discoverySource.value = result.source
      showProtoUpload.value = false
      
      // Auto-select first service and method
      if (result.services.length > 0) {
        selectedService.value = result.services[0].fullName
        if (result.services[0].methods.length > 0) {
          selectedMethod.value = result.services[0].methods[0].name
          updateRequestServiceMethod()
        }
      }
    } else {
      discoveryError.value = result.error || 'Failed to parse proto file'
    }
  } catch (e: any) {
    discoveryError.value = e.toString()
  } finally {
    isDiscovering.value = false
  }
}

// Handle proto file upload
const handleProtoFileUpload = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  
  try {
    protoContent.value = await file.text()
  } catch (e: any) {
    discoveryError.value = `Failed to read file: ${e.toString()}`
  }
  
  // Reset input
  if (target) target.value = ''
}

// Update request with selected service/method
const updateRequestServiceMethod = () => {
  store.updateActiveRequest({
    service: selectedService.value,
    method: selectedMethod.value,
  })
  
  // Generate sample message from schema if available
  if (selectedMethodInfo.value?.inputSchema) {
    const sampleMessage = generateSampleFromSchema(selectedMethodInfo.value.inputSchema)
    store.updateActiveRequest({
      message: JSON.stringify(sampleMessage, null, 2)
    })
  }
}

// Generate sample JSON from schema
const generateSampleFromSchema = (schema: any): any => {
  if (!schema || schema.type !== 'object') return {}
  
  const result: any = {}
  const properties = schema.properties || {}
  
  for (const [key, prop] of Object.entries(properties)) {
    const propSchema = prop as any
    switch (propSchema.type) {
      case 'string':
        result[key] = ''
        break
      case 'integer':
        result[key] = 0
        break
      case 'number':
        result[key] = 0.0
        break
      case 'boolean':
        result[key] = false
        break
      case 'object':
        result[key] = {}
        break
      default:
        result[key] = null
    }
  }
  
  return result
}

// Watch for service/method changes
watch(selectedService, () => {
  // Reset method when service changes
  if (selectedServiceInfo.value?.methods.length) {
    selectedMethod.value = selectedServiceInfo.value.methods[0].name
  } else {
    selectedMethod.value = ''
  }
  updateRequestServiceMethod()
})

watch(selectedMethod, () => {
  updateRequestServiceMethod()
})

// Metadata helpers
const addMetadata = () => {
  const currentMetadata = request.value.metadata || []
  store.updateActiveRequest({
    metadata: [...currentMetadata, { 
      id: crypto.randomUUID(), 
      key: '', 
      value: '', 
      enabled: true 
    }]
  })
}

const updateMetadata = (id: string, field: 'key' | 'value', value: string) => {
  store.updateActiveRequest({
    metadata: request.value.metadata.map(m =>
      m.id === id ? { ...m, [field]: value } : m
    )
  })
}

const toggleMetadata = (id: string) => {
  store.updateActiveRequest({
    metadata: request.value.metadata.map(m =>
      m.id === id ? { ...m, enabled: !m.enabled } : m
    )
  })
}

const removeMetadata = (id: string) => {
  store.updateActiveRequest({
    metadata: request.value.metadata.filter(m => m.id !== id)
  })
}

// Send gRPC request
const sendRequest = async () => {
  if (!request.value.url || !request.value.service || !request.value.method) {
    return
  }
  
  store.setActiveLoading(true)
  store.setActiveResponse(null)
  
  try {
    const url = variableStore.interpolate(request.value.url)
    const message = variableStore.interpolate(request.value.message)
    
    // Build metadata map
    const metadata: Record<string, string> = {}
    for (const m of request.value.metadata || []) {
      if (m.enabled && m.key) {
        metadata[variableStore.interpolate(m.key)] = variableStore.interpolate(m.value)
      }
    }
    
    const startTime = Date.now()
    
    const result = await invoke<{
      success: boolean
      data?: any
      error?: string
      statusCode: number
      statusMessage: string
      metadata: Record<string, string>
      timeMs: number
    }>('grpc_call', {
      url,
      service: request.value.service,
      method: request.value.method,
      message,
      metadata,
    })
    
    const response: GrpcResponse = {
      data: result.data,
      metadata: result.metadata,
      status: result.statusCode,
      statusMessage: result.statusMessage,
      time: result.timeMs,
    }
    
    store.setActiveResponse(response)
    store.addToHistory(request.value, response)
  } catch (e: any) {
    const errorResponse: GrpcResponse = {
      data: { error: e.toString() },
      metadata: {},
      status: -1,
      statusMessage: e.toString(),
      time: 0,
    }
    store.setActiveResponse(errorResponse)
  } finally {
    store.setActiveLoading(false)
  }
}

// Get streaming badge
const getStreamingBadge = (method: GrpcMethodInfo) => {
  if (method.clientStreaming && method.serverStreaming) return 'Bidirectional'
  if (method.clientStreaming) return 'Client Stream'
  if (method.serverStreaming) return 'Server Stream'
  return 'Unary'
}

// Format response data as JSON string
const formatResponseData = (data: any): string => {
  try {
    return JSON.stringify(data, null, 2)
  } catch {
    return String(data)
  }
}

// Copy response to clipboard
const copyResponse = async () => {
  if (response.value?.data) {
    try {
      await navigator.clipboard.writeText(formatResponseData(response.value.data))
    } catch (e) {
      console.error('Failed to copy:', e)
    }
  }
}
</script>

<template>
  <div class="flex h-full">
    <!-- Left Panel: Request -->
    <div class="flex flex-col w-1/2 border-r border-border overflow-hidden">
    <!-- URL and Discovery -->
    <div class="border-b border-border p-4 space-y-4">
      <!-- Server URL -->
      <div class="flex gap-2">
        <UiVariableInput
          :model-value="request.url"
          placeholder="localhost:50051 or grpc://localhost:50051"
          class="flex-1"
          @update:model-value="store.updateActiveRequest({ url: $event })"
        />
        <UiButton
          :disabled="isDiscovering || !request.url"
          @click="discoverViaReflection"
        >
          <Icon v-if="isDiscovering" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
          <Icon v-else name="lucide:search" class="h-4 w-4 mr-2" />
          Discover
        </UiButton>
        <UiButton
          variant="outline"
          @click="showProtoUpload = !showProtoUpload"
        >
          <Icon name="lucide:file-text" class="h-4 w-4 mr-2" />
          Proto
        </UiButton>
      </div>
      
      <!-- Proto file upload panel -->
      <div v-if="showProtoUpload" class="border border-border rounded-lg p-4 space-y-3">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">Upload Proto File</span>
          <button @click="showProtoUpload = false">
            <Icon name="lucide:x" class="h-4 w-4 text-muted-foreground" />
          </button>
        </div>
        
        <input
          ref="protoFileInput"
          type="file"
          accept=".proto"
          class="hidden"
          @change="handleProtoFileUpload"
        />
        
        <div class="flex gap-2">
          <UiButton
            variant="outline"
            size="sm"
            @click="protoFileInput?.click()"
          >
            <Icon name="lucide:upload" class="h-4 w-4 mr-2" />
            Choose File
          </UiButton>
          <span class="text-sm text-muted-foreground self-center">
            Or paste proto content below
          </span>
        </div>
        
        <textarea
          v-model="protoContent"
          class="w-full h-32 p-3 text-sm font-mono bg-muted rounded-md border border-input resize-none"
          placeholder="syntax = 'proto3';&#10;&#10;service MyService {&#10;  rpc MyMethod (Request) returns (Response);&#10;}"
        />
        
        <UiButton
          :disabled="isDiscovering || !protoContent.trim()"
          @click="parseProtoFile"
        >
          <Icon v-if="isDiscovering" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
          Parse Proto
        </UiButton>
      </div>
      
      <!-- Discovery Error -->
      <div v-if="discoveryError" class="text-sm text-destructive p-2 bg-destructive/10 rounded">
        {{ discoveryError }}
      </div>
      
      <!-- Service and Method Selection -->
      <div v-if="discoveredServices.length > 0" class="flex gap-4">
        <div class="flex-1">
          <label class="text-xs text-muted-foreground mb-1 block">Service</label>
          <select
            v-model="selectedService"
            class="w-full h-10 px-3 rounded-md border border-input bg-background text-sm"
          >
            <option v-for="service in discoveredServices" :key="service.fullName" :value="service.fullName">
              {{ service.fullName }}
            </option>
          </select>
        </div>
        
        <div class="flex-1">
          <label class="text-xs text-muted-foreground mb-1 block">Method</label>
          <select
            v-model="selectedMethod"
            class="w-full h-10 px-3 rounded-md border border-input bg-background text-sm"
          >
            <option 
              v-for="method in selectedServiceInfo?.methods" 
              :key="method.name" 
              :value="method.name"
            >
              {{ method.name }} ({{ getStreamingBadge(method) }})
            </option>
          </select>
        </div>
        
        <div class="self-end">
          <UiButton
            :disabled="isLoading || !selectedMethod"
            @click="sendRequest"
          >
            <Icon v-if="isLoading" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
            <Icon v-else name="lucide:send" class="h-4 w-4 mr-2" />
            Send
          </UiButton>
        </div>
      </div>
      
      <!-- Discovery source badge -->
      <div v-if="discoverySource" class="flex items-center gap-2">
        <span class="text-xs px-2 py-1 rounded bg-muted text-muted-foreground">
          <Icon 
            :name="discoverySource === 'reflection' ? 'lucide:radio' : 'lucide:file-text'" 
            class="h-3 w-3 inline mr-1" 
          />
          Discovered via {{ discoverySource }}
        </span>
        <span class="text-xs text-muted-foreground">
          {{ discoveredServices.length }} service(s), 
          {{ discoveredServices.reduce((sum, s) => sum + s.methods.length, 0) }} method(s)
        </span>
      </div>
    </div>
    
    <!-- Request Configuration -->
    <div v-if="discoveredServices.length > 0" class="flex-1 overflow-hidden flex flex-col">
      <!-- Tabs -->
      <div class="flex border-b border-border">
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            activeRequestTab === 'message' 
              ? 'border-b-2 border-primary text-foreground' 
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeRequestTab = 'message'"
        >
          Message
        </button>
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            activeRequestTab === 'metadata' 
              ? 'border-b-2 border-primary text-foreground' 
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeRequestTab = 'metadata'"
        >
          Metadata
          <span v-if="request.metadata?.filter(m => m.enabled && m.key).length" class="ml-1 text-xs text-muted-foreground">
            ({{ request.metadata.filter(m => m.enabled && m.key).length }})
          </span>
        </button>
      </div>
      
      <!-- Tab Content -->
      <div class="flex-1 overflow-auto p-4">
        <!-- Message Tab -->
        <div v-if="activeRequestTab === 'message'" class="h-full">
          <div v-if="selectedMethodInfo" class="mb-2 text-xs text-muted-foreground">
            Input: {{ selectedMethodInfo.inputType }} → Output: {{ selectedMethodInfo.outputType }}
          </div>
          <CodeEditor
            :model-value="request.message"
            language="json"
            :placeholder="'{\n  \n}'"
            class="h-64"
            @update:model-value="store.updateActiveRequest({ message: $event })"
          />
        </div>
        
        <!-- Metadata Tab -->
        <div v-else-if="activeRequestTab === 'metadata'" class="space-y-2">
          <div class="text-xs text-muted-foreground mb-2">
            Custom metadata (headers) to send with the request
          </div>
          
          <div
            v-for="meta in request.metadata"
            :key="meta.id"
            class="flex items-center gap-2"
          >
            <input
              type="checkbox"
              :checked="meta.enabled"
              class="h-4 w-4"
              @change="toggleMetadata(meta.id)"
            />
            <UiVariableInput
              :model-value="meta.key"
              placeholder="Key"
              class="flex-1"
              @update:model-value="updateMetadata(meta.id, 'key', $event)"
            />
            <UiVariableInput
              :model-value="meta.value"
              placeholder="Value"
              class="flex-1"
              @update:model-value="updateMetadata(meta.id, 'value', $event)"
            />
            <UiButton
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              @click="removeMetadata(meta.id)"
            >
              <Icon name="lucide:x" class="h-4 w-4" />
            </UiButton>
          </div>
          
          <UiButton variant="outline" size="sm" @click="addMetadata">
            <Icon name="lucide:plus" class="h-4 w-4 mr-2" />
            Add Metadata
          </UiButton>
        </div>
      </div>
    </div>
    
    <!-- Empty State -->
    <div v-else class="flex-1 flex items-center justify-center text-center p-8">
      <div class="max-w-md space-y-4">
        <Icon name="lucide:cpu" class="h-12 w-12 mx-auto text-muted-foreground" />
        <h3 class="text-lg font-medium">Connect to a gRPC Server</h3>
        <p class="text-sm text-muted-foreground">
          Enter a server URL and click "Discover" to auto-detect services via reflection,
          or upload a .proto file to define the service schema manually.
        </p>
        <div class="flex gap-2 justify-center">
          <div class="text-xs px-2 py-1 rounded bg-muted text-muted-foreground">
            <Icon name="lucide:radio" class="h-3 w-3 inline mr-1" />
            Server Reflection
          </div>
          <div class="text-xs px-2 py-1 rounded bg-muted text-muted-foreground">
            <Icon name="lucide:file-text" class="h-3 w-3 inline mr-1" />
            Proto File
          </div>
        </div>
      </div>
    </div>
    </div>
    
    <!-- Right Panel: Response -->
    <div class="flex flex-col w-1/2 overflow-hidden">
      <!-- Response Header -->
      <div class="flex items-center justify-between border-b border-border px-4 py-2">
        <span class="text-sm font-medium">Response</span>
        <div v-if="response" class="flex items-center gap-3 text-xs">
          <span :class="response.status === 0 ? 'text-green-500' : 'text-red-500'">
            Status: {{ response.status === 0 ? 'OK' : `Error (${response.status})` }}
          </span>
          <span class="text-muted-foreground">{{ response.time }}ms</span>
        </div>
      </div>
      
      <!-- Response Content -->
      <div class="flex-1 overflow-auto">
        <!-- Loading State -->
        <div v-if="isLoading" class="flex items-center justify-center h-full">
          <div class="text-center space-y-2">
            <Icon name="lucide:loader-2" class="h-8 w-8 animate-spin mx-auto text-muted-foreground" />
            <p class="text-sm text-muted-foreground">Sending request...</p>
          </div>
        </div>
        
        <!-- No Response State -->
        <div v-else-if="!response" class="flex items-center justify-center h-full">
          <div class="text-center space-y-2">
            <Icon name="lucide:inbox" class="h-8 w-8 mx-auto text-muted-foreground" />
            <p class="text-sm text-muted-foreground">No response yet</p>
            <p class="text-xs text-muted-foreground">Discover services and send a request to see the response</p>
          </div>
        </div>
        
        <!-- Response Data -->
        <div v-else class="p-4 space-y-4">
          <!-- Status Message -->
          <div v-if="response.statusMessage && response.status !== 0" class="p-3 bg-destructive/10 border border-destructive/20 rounded-md">
            <p class="text-sm text-destructive">{{ response.statusMessage }}</p>
          </div>
          
          <!-- Response Data -->
          <div v-if="response.data">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs text-muted-foreground uppercase tracking-wider">Data</span>
              <button 
                class="text-xs text-muted-foreground hover:text-foreground"
                @click="copyResponse"
              >
                <Icon name="lucide:copy" class="h-3 w-3 inline mr-1" />
                Copy
              </button>
            </div>
            <CodeEditor
              :model-value="formatResponseData(response.data)"
              language="json"
              :readonly="true"
              class="h-64"
            />
          </div>
          
          <!-- Response Metadata -->
          <div v-if="response.metadata && Object.keys(response.metadata).length > 0">
            <span class="text-xs text-muted-foreground uppercase tracking-wider block mb-2">Metadata</span>
            <div class="bg-muted rounded-md p-3 font-mono text-xs space-y-1">
              <div v-for="(value, key) in response.metadata" :key="key" class="flex">
                <span class="text-muted-foreground w-48 flex-shrink-0">{{ key }}:</span>
                <span class="text-foreground break-all">{{ value }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
