<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { 
  TestRunConfig, 
  TestRunSummary, 
  TestResult, 
  TestProgressEvent,
  TestRequest,
  Collection,
  CollectionFolder,
  HttpRequest,
  Assertion,
  AssertionType,
  VariableExtraction,
  TestRunHistory,
  ExtractedVariable
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
const { collections } = store

// Test runner state
const isRunning = ref(false)
const currentRun = ref<TestRunSummary | null>(null)
const results = ref<TestResult[]>([])
const progress = ref({ current: 0, total: 0 })

// Configuration
const selectedCollectionId = ref<string | null>(null)
const selectedFolderId = ref<string | null>(null)
const stopOnFailure = ref(false)
const delayBetweenRequests = ref(100)

// Tabs
const activeTab = ref<'setup' | 'results' | 'history'>('setup')

// Assertions & Extractions Configuration
const requestConfigs = ref<Map<string, { assertions: Assertion[], extractVariables: VariableExtraction[] }>>(new Map())
const expandedRequestId = ref<string | null>(null)

// History
const testRunHistory = ref<TestRunHistory[]>([])
const selectedHistoryRun = ref<TestRunHistory | null>(null)

// Watch for pre-selected collection from props
watch(() => props.selectedCollection, (newVal) => {
  if (newVal) {
    selectedCollectionId.value = newVal.id
    selectedFolderId.value = null
  }
}, { immediate: true })

// Load test run history
const loadHistory = async () => {
  try {
    testRunHistory.value = await invoke<TestRunHistory[]>('load_test_runs')
  } catch (e) {
    console.error('Failed to load test run history:', e)
  }
}

// Listen for test progress
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await listen<TestProgressEvent>('test-progress', (event) => {
    progress.value = { current: event.payload.current, total: event.payload.total }
    results.value.push(event.payload.result)
  })
  
  await loadHistory()
})

onUnmounted(() => {
  unlisten?.()
})

const selectedCollection = computed(() => {
  return collections.value.find((c: Collection) => c.id === selectedCollectionId.value)
})

const selectedFolder = computed(() => {
  if (!selectedCollection.value || !selectedFolderId.value) return null
  return selectedCollection.value.folders?.find((f: CollectionFolder) => f.id === selectedFolderId.value)
})

// Get HTTP requests from selected source
const testableRequests = computed(() => {
  if (selectedFolderId.value && selectedFolder.value) {
    return selectedFolder.value.requests.filter(r => r.protocol === 'http') as HttpRequest[]
  }
  
  if (selectedCollection.value) {
    const requests: HttpRequest[] = []
    
    // Add root requests
    selectedCollection.value.requests
      .filter(r => r.protocol === 'http')
      .forEach(r => requests.push(r as HttpRequest))
    
    // Add folder requests
    selectedCollection.value.folders?.forEach((folder: CollectionFolder) => {
      folder.requests
        .filter(r => r.protocol === 'http')
        .forEach(r => requests.push(r as HttpRequest))
    })
    
    return requests
  }
  
  return []
})

// Get or initialize request config
const getRequestConfig = (requestId: string) => {
  if (!requestConfigs.value.has(requestId)) {
    requestConfigs.value.set(requestId, { assertions: [], extractVariables: [] })
  }
  return requestConfigs.value.get(requestId)!
}

// Assertion management
const addAssertion = (requestId: string) => {
  const config = getRequestConfig(requestId)
  config.assertions.push({
    id: generateId(),
    type: 'status' as AssertionType,
    enabled: true,
    expectedStatus: 200
  })
}

const removeAssertion = (requestId: string, assertionId: string) => {
  const config = getRequestConfig(requestId)
  config.assertions = config.assertions.filter(a => a.id !== assertionId)
}

const updateAssertion = (requestId: string, assertionId: string, updates: Partial<Assertion>) => {
  const config = getRequestConfig(requestId)
  const assertion = config.assertions.find(a => a.id === assertionId)
  if (assertion) {
    Object.assign(assertion, updates)
  }
}

// Variable extraction management
const addExtraction = (requestId: string) => {
  const config = getRequestConfig(requestId)
  config.extractVariables.push({
    id: generateId(),
    variableName: '',
    jsonPath: '$.data',
    enabled: true
  })
}

const removeExtraction = (requestId: string, extractionId: string) => {
  const config = getRequestConfig(requestId)
  config.extractVariables = config.extractVariables.filter(e => e.id !== extractionId)
}

const convertToTestRequest = (request: HttpRequest): TestRequest => {
  const config = requestConfigs.value.get(request.id)
  return {
    id: request.id,
    name: request.name,
    method: request.method,
    url: request.url,
    headers: request.headers,
    params: request.params,
    body: request.body,
    bodyType: request.bodyType,
    assertions: config?.assertions.filter(a => a.enabled),
    extractVariables: config?.extractVariables.filter(e => e.enabled && e.variableName)
  }
}

const runTests = async () => {
  if (testableRequests.value.length === 0) return
  
  isRunning.value = true
  results.value = []
  progress.value = { current: 0, total: testableRequests.value.length }
  activeTab.value = 'results'
  
  try {
    const config: TestRunConfig = {
      id: generateId(),
      name: selectedCollection.value?.name || 'Test Run',
      requests: testableRequests.value.map(convertToTestRequest),
      stopOnFailure: stopOnFailure.value,
      delayBetweenRequests: delayBetweenRequests.value
    }
    
    currentRun.value = await invoke<TestRunSummary>('run_collection_tests', { config })
    
    // Save to history
    const historyEntry: TestRunHistory = {
      id: generateId(),
      runId: currentRun.value.runId,
      collectionId: selectedCollectionId.value || undefined,
      collectionName: selectedCollection.value?.name || 'Test Run',
      timestamp: Date.now(),
      summary: currentRun.value
    }
    
    await invoke('save_test_run', { testRun: historyEntry })
    await loadHistory()
    
  } catch (error: any) {
    console.error('Test run failed:', error)
  } finally {
    isRunning.value = false
  }
}

const stopTests = () => {
  // TODO: Implement cancel functionality
  isRunning.value = false
}

const viewHistoryRun = (run: TestRunHistory) => {
  selectedHistoryRun.value = run
  currentRun.value = run.summary
  results.value = run.summary.results
  activeTab.value = 'results'
}

const deleteHistoryRun = async (runId: string) => {
  try {
    await invoke('delete_test_run', { id: runId })
    await loadHistory()
    if (selectedHistoryRun.value?.id === runId) {
      selectedHistoryRun.value = null
    }
  } catch (e) {
    console.error('Failed to delete test run:', e)
  }
}

const clearHistory = async () => {
  try {
    await invoke('clear_test_runs')
    testRunHistory.value = []
    selectedHistoryRun.value = null
  } catch (e) {
    console.error('Failed to clear history:', e)
  }
}

const getStatusIcon = (status: string) => {
  switch (status) {
    case 'passed': return 'lucide:check-circle'
    case 'failed': return 'lucide:x-circle'
    case 'error': return 'lucide:alert-circle'
    case 'running': return 'lucide:loader-2'
    default: return 'lucide:circle'
  }
}

const getStatusColor = (status: string) => {
  switch (status) {
    case 'passed': return 'text-green-500'
    case 'failed': return 'text-red-500'
    case 'error': return 'text-yellow-500'
    case 'running': return 'text-blue-500 animate-spin'
    default: return 'text-gray-400'
  }
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

const passRate = computed(() => {
  if (!currentRun.value || currentRun.value.total === 0) return 0
  return Math.round((currentRun.value.passed / currentRun.value.total) * 100)
})

const formatTime = (ms: number) => {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

const formatDate = (timestamp: number) => {
  return new Date(timestamp).toLocaleString()
}

const assertionTypes: { value: AssertionType; label: string }[] = [
  { value: 'status', label: 'Status Code' },
  { value: 'status_range', label: 'Status Range' },
  { value: 'jsonpath', label: 'JSONPath' },
  { value: 'contains', label: 'Contains' },
  { value: 'response_time', label: 'Response Time' },
  { value: 'header', label: 'Header' },
]
</script>

<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="emit('close')"
  >
    <div class="bg-background border border-border rounded-lg shadow-xl w-[1000px] h-[750px] flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 rounded-lg bg-green-500/10 flex items-center justify-center">
            <Icon name="lucide:play-circle" class="h-5 w-5 text-green-500" />
          </div>
          <div>
            <h2 class="text-lg font-semibold">Test Runner</h2>
            <p class="text-sm text-muted-foreground">Run tests against your API collection</p>
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
            activeTab === 'results' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeTab = 'results'"
        >
          <Icon name="lucide:list-checks" class="h-4 w-4 mr-2 inline" />
          Results
          <span v-if="results.length > 0" class="ml-1.5 px-1.5 py-0.5 text-xs bg-primary/20 text-primary rounded-full">
            {{ results.length }}
          </span>
        </button>
        <button
          :class="[
            'px-4 py-3 text-sm font-medium transition-colors',
            activeTab === 'history' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeTab = 'history'"
        >
          <Icon name="lucide:history" class="h-4 w-4 mr-2 inline" />
          History
          <span v-if="testRunHistory.length > 0" class="ml-1.5 px-1.5 py-0.5 text-xs bg-muted text-muted-foreground rounded-full">
            {{ testRunHistory.length }}
          </span>
        </button>
      </div>
      
      <!-- Content -->
      <div class="flex-1 overflow-hidden">
        <!-- Setup Tab -->
        <div v-if="activeTab === 'setup'" class="h-full flex">
          <!-- Left: Collection Selection -->
          <div class="w-72 border-r border-border flex flex-col">
            <div class="p-4 border-b border-border">
              <h3 class="font-medium mb-3">Select Collection</h3>
            </div>
            
            <UiScrollArea class="flex-1 p-4">
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
                    <span class="flex-1 text-sm font-medium truncate">{{ collection.name }}</span>
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
                      @click="selectedCollectionId = collection.id; selectedFolderId = folder.id"
                    >
                      <Icon name="lucide:folder-open" class="h-4 w-4 text-muted-foreground" />
                      <span class="flex-1 text-sm truncate">{{ folder.name }}</span>
                      <span class="text-xs text-muted-foreground">{{ folder.requests.filter(r => r.protocol === 'http').length }}</span>
                    </button>
                  </div>
                </div>
                
                <div v-if="collections.length === 0" class="text-center py-8 text-sm text-muted-foreground">
                  No collections yet
                </div>
              </div>
            </UiScrollArea>
          </div>
          
          <!-- Right: Configuration & Requests -->
          <div class="flex-1 flex flex-col">
            <div class="p-4 space-y-4 border-b border-border">
              <h3 class="font-medium">Configuration</h3>
              
              <div class="flex items-center gap-4">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    v-model="stopOnFailure"
                    class="accent-primary"
                  />
                  <span class="text-sm">Stop on failure</span>
                </label>
                
                <div class="flex items-center gap-2">
                  <span class="text-sm text-muted-foreground">Delay:</span>
                  <UiInput
                    v-model.number="delayBetweenRequests"
                    type="number"
                    class="w-20 h-8"
                    min="0"
                    max="5000"
                  />
                  <span class="text-sm text-muted-foreground">ms</span>
                </div>
              </div>
            </div>
            
            <div class="p-4 border-b border-border">
              <div class="flex items-center justify-between">
                <h3 class="font-medium">Requests to Test ({{ testableRequests.length }})</h3>
                <span class="text-xs text-muted-foreground">Click to configure assertions</span>
              </div>
            </div>
            
            <!-- Requests Preview with Assertions -->
            <UiScrollArea class="flex-1 px-4">
              <div class="space-y-2 py-4">
                <div
                  v-for="request in testableRequests"
                  :key="request.id"
                  class="rounded-lg border border-border overflow-hidden"
                >
                  <!-- Request Header -->
                  <button
                    class="w-full p-3 flex items-center gap-3 hover:bg-accent/50 transition-colors"
                    @click="expandedRequestId = expandedRequestId === request.id ? null : request.id"
                  >
                    <Icon 
                      :name="expandedRequestId === request.id ? 'lucide:chevron-down' : 'lucide:chevron-right'" 
                      class="h-4 w-4 text-muted-foreground" 
                    />
                    <span :class="['font-mono text-sm font-semibold w-16', getMethodColor(request.method)]">
                      {{ request.method }}
                    </span>
                    <span class="text-sm flex-1 truncate text-left">{{ request.name }}</span>
                    <span class="text-xs text-muted-foreground">
                      {{ getRequestConfig(request.id).assertions.length }} assertions
                    </span>
                  </button>
                  
                  <!-- Expanded Configuration -->
                  <div v-if="expandedRequestId === request.id" class="border-t border-border bg-muted/30 p-4 space-y-4">
                    <div class="text-xs text-muted-foreground font-mono truncate mb-3">
                      {{ request.url }}
                    </div>
                    
                    <!-- Assertions Section -->
                    <div>
                      <div class="flex items-center justify-between mb-2">
                        <h4 class="text-sm font-medium">Assertions</h4>
                        <button
                          class="text-xs text-primary hover:underline"
                          @click="addAssertion(request.id)"
                        >
                          + Add Assertion
                        </button>
                      </div>
                      
                      <div class="space-y-2">
                        <div
                          v-for="assertion in getRequestConfig(request.id).assertions"
                          :key="assertion.id"
                          class="flex items-center gap-2 p-2 bg-background rounded border border-border"
                        >
                          <input
                            type="checkbox"
                            v-model="assertion.enabled"
                            class="accent-primary"
                          />
                          
                          <select
                            v-model="assertion.type"
                            class="h-7 text-xs bg-background border border-border rounded px-2"
                            @change="updateAssertion(request.id, assertion.id, { type: assertion.type })"
                          >
                            <option v-for="t in assertionTypes" :key="t.value" :value="t.value">
                              {{ t.label }}
                            </option>
                          </select>
                          
                          <!-- Status assertion -->
                          <template v-if="assertion.type === 'status'">
                            <span class="text-xs">=</span>
                            <UiInput
                              v-model.number="assertion.expectedStatus"
                              type="number"
                              class="w-20 h-7 text-xs"
                              placeholder="200"
                            />
                          </template>
                          
                          <!-- Status Range assertion -->
                          <template v-if="assertion.type === 'status_range'">
                            <UiInput
                              v-model.number="assertion.minStatus"
                              type="number"
                              class="w-16 h-7 text-xs"
                              placeholder="200"
                            />
                            <span class="text-xs">-</span>
                            <UiInput
                              v-model.number="assertion.maxStatus"
                              type="number"
                              class="w-16 h-7 text-xs"
                              placeholder="299"
                            />
                          </template>
                          
                          <!-- JSONPath assertion -->
                          <template v-if="assertion.type === 'jsonpath'">
                            <UiInput
                              v-model="assertion.jsonPath"
                              class="flex-1 h-7 text-xs font-mono"
                              placeholder="$.data.id"
                            />
                            <select
                              v-model="assertion.operator"
                              class="h-7 text-xs bg-background border border-border rounded px-2"
                            >
                              <option value="equals">equals</option>
                              <option value="not_equals">not equals</option>
                              <option value="contains">contains</option>
                              <option value="exists">exists</option>
                              <option value="not_exists">not exists</option>
                            </select>
                            <UiInput
                              v-if="assertion.operator !== 'exists' && assertion.operator !== 'not_exists'"
                              v-model="assertion.expectedValue"
                              class="w-24 h-7 text-xs"
                              placeholder="value"
                            />
                          </template>
                          
                          <!-- Contains assertion -->
                          <template v-if="assertion.type === 'contains'">
                            <UiInput
                              v-model="assertion.searchString"
                              class="flex-1 h-7 text-xs"
                              placeholder="search string"
                            />
                          </template>
                          
                          <!-- Response Time assertion -->
                          <template v-if="assertion.type === 'response_time'">
                            <span class="text-xs">&lt;</span>
                            <UiInput
                              v-model.number="assertion.maxTimeMs"
                              type="number"
                              class="w-20 h-7 text-xs"
                              placeholder="5000"
                            />
                            <span class="text-xs">ms</span>
                          </template>
                          
                          <!-- Header assertion -->
                          <template v-if="assertion.type === 'header'">
                            <UiInput
                              v-model="assertion.headerName"
                              class="w-32 h-7 text-xs"
                              placeholder="Header-Name"
                            />
                            <span class="text-xs">=</span>
                            <UiInput
                              v-model="assertion.headerValue"
                              class="flex-1 h-7 text-xs"
                              placeholder="value (optional)"
                            />
                          </template>
                          
                          <button
                            class="p-1 text-muted-foreground hover:text-destructive"
                            @click="removeAssertion(request.id, assertion.id)"
                          >
                            <Icon name="lucide:x" class="h-3 w-3" />
                          </button>
                        </div>
                        
                        <div v-if="getRequestConfig(request.id).assertions.length === 0" class="text-xs text-muted-foreground py-2">
                          No assertions configured. Default: status &lt; 400
                        </div>
                      </div>
                    </div>
                    
                    <!-- Variable Extraction Section -->
                    <div>
                      <div class="flex items-center justify-between mb-2">
                        <h4 class="text-sm font-medium">Extract Variables</h4>
                        <button
                          class="text-xs text-primary hover:underline"
                          @click="addExtraction(request.id)"
                        >
                          + Add Extraction
                        </button>
                      </div>
                      
                      <div class="space-y-2">
                        <div
                          v-for="extraction in getRequestConfig(request.id).extractVariables"
                          :key="extraction.id"
                          class="flex items-center gap-2 p-2 bg-background rounded border border-border"
                        >
                          <input
                            type="checkbox"
                            v-model="extraction.enabled"
                            class="accent-primary"
                          />
                          <UiInput
                            v-model="extraction.variableName"
                            class="w-32 h-7 text-xs"
                            placeholder="variableName"
                          />
                          <span class="text-xs">=</span>
                          <UiInput
                            v-model="extraction.jsonPath"
                            class="flex-1 h-7 text-xs font-mono"
                            placeholder="$.data.id"
                          />
                          <button
                            class="p-1 text-muted-foreground hover:text-destructive"
                            @click="removeExtraction(request.id, extraction.id)"
                          >
                            <Icon name="lucide:x" class="h-3 w-3" />
                          </button>
                        </div>
                        
                        <div v-if="getRequestConfig(request.id).extractVariables.length === 0" class="text-xs text-muted-foreground py-2">
                          No variables to extract. Use {{variableName}} in subsequent requests.
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                
                <div v-if="testableRequests.length === 0" class="text-center py-8 text-muted-foreground">
                  <Icon name="lucide:inbox" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p class="text-sm">Select a collection to run tests</p>
                </div>
              </div>
            </UiScrollArea>
            
            <!-- Run Button -->
            <div class="p-4 border-t border-border">
              <UiButton
                v-if="!isRunning"
                class="w-full"
                :disabled="testableRequests.length === 0"
                @click="runTests"
              >
                <Icon name="lucide:play" class="h-4 w-4 mr-2" />
                Run {{ testableRequests.length }} Tests
              </UiButton>
              <UiButton
                v-else
                variant="destructive"
                class="w-full"
                @click="stopTests"
              >
                <Icon name="lucide:square" class="h-4 w-4 mr-2" />
                Stop
              </UiButton>
            </div>
          </div>
        </div>
        
        <!-- Results Tab -->
        <div v-if="activeTab === 'results'" class="h-full flex flex-col">
          <!-- Summary -->
          <div v-if="currentRun" class="p-4 border-b border-border">
            <div class="flex items-center justify-between mb-4">
              <h3 class="font-medium">{{ currentRun.name }}</h3>
              <span class="text-sm text-muted-foreground">{{ formatTime(currentRun.totalTime) }}</span>
            </div>
            
            <!-- Progress bar -->
            <div class="h-2 bg-muted rounded-full overflow-hidden mb-4">
              <div 
                class="h-full transition-all duration-300"
                :class="currentRun.failed > 0 || currentRun.errors > 0 ? 'bg-red-500' : 'bg-green-500'"
                :style="{ width: `${passRate}%` }"
              />
            </div>
            
            <div class="grid grid-cols-4 gap-4 text-center">
              <div class="p-3 rounded-lg bg-muted/50">
                <div class="text-2xl font-bold">{{ currentRun.total }}</div>
                <div class="text-xs text-muted-foreground">Total</div>
              </div>
              <div class="p-3 rounded-lg bg-green-500/10">
                <div class="text-2xl font-bold text-green-500">{{ currentRun.passed }}</div>
                <div class="text-xs text-muted-foreground">Passed</div>
              </div>
              <div class="p-3 rounded-lg bg-red-500/10">
                <div class="text-2xl font-bold text-red-500">{{ currentRun.failed }}</div>
                <div class="text-xs text-muted-foreground">Failed</div>
              </div>
              <div class="p-3 rounded-lg bg-yellow-500/10">
                <div class="text-2xl font-bold text-yellow-500">{{ currentRun.errors }}</div>
                <div class="text-xs text-muted-foreground">Errors</div>
              </div>
            </div>
          </div>
          
          <!-- Running indicator -->
          <div v-if="isRunning" class="px-4 py-2 bg-blue-500/10 border-b border-border">
            <div class="flex items-center gap-2 text-sm">
              <Icon name="lucide:loader-2" class="h-4 w-4 animate-spin text-blue-500" />
              <span>Running test {{ progress.current }} of {{ progress.total }}...</span>
            </div>
          </div>
          
          <!-- Results List -->
          <UiScrollArea class="flex-1">
            <div class="divide-y divide-border">
              <div
                v-for="result in results"
                :key="result.requestId"
                class="p-4 hover:bg-accent/50 transition-colors"
              >
                <div class="flex items-center gap-3">
                  <Icon 
                    :name="getStatusIcon(result.status)" 
                    :class="['h-5 w-5', getStatusColor(result.status)]"
                  />
                  <span :class="['font-mono text-sm font-semibold w-16', getMethodColor(result.method)]">
                    {{ result.method }}
                  </span>
                  <span class="flex-1 text-sm font-medium truncate">{{ result.requestName }}</span>
                  <span v-if="result.responseStatus" class="text-sm text-muted-foreground">
                    {{ result.responseStatus }}
                  </span>
                  <span v-if="result.responseTime" class="text-xs text-muted-foreground">
                    {{ result.responseTime }}ms
                  </span>
                </div>
                
                <div class="mt-1 text-xs text-muted-foreground font-mono truncate pl-8">
                  {{ result.url }}
                </div>
                
                <!-- Error message -->
                <div v-if="result.error" class="mt-2 text-xs text-red-500 pl-8">
                  {{ result.error }}
                </div>
                
                <!-- Assertions -->
                <div v-if="result.assertions.length > 0" class="mt-2 pl-8 space-y-1">
                  <div
                    v-for="(assertion, i) in result.assertions"
                    :key="i"
                    class="flex items-center gap-2 text-xs"
                  >
                    <Icon 
                      :name="assertion.passed ? 'lucide:check' : 'lucide:x'" 
                      :class="assertion.passed ? 'text-green-500' : 'text-red-500'"
                      class="h-3 w-3"
                    />
                    <span class="text-muted-foreground">{{ assertion.name }}</span>
                    <span v-if="!assertion.passed" class="text-red-400">
                      (expected: {{ assertion.expected }}, got: {{ assertion.actual }})
                    </span>
                  </div>
                </div>
                
                <!-- Extracted Variables -->
                <div v-if="result.extractedVariables && result.extractedVariables.length > 0" class="mt-2 pl-8">
                  <div class="text-xs text-muted-foreground mb-1">Extracted Variables:</div>
                  <div class="space-y-1">
                    <div
                      v-for="(extracted, i) in result.extractedVariables"
                      :key="i"
                      class="flex items-center gap-2 text-xs"
                    >
                      <Icon 
                        :name="extracted.success ? 'lucide:variable' : 'lucide:alert-triangle'" 
                        :class="extracted.success ? 'text-blue-500' : 'text-yellow-500'"
                        class="h-3 w-3"
                      />
                      <span class="font-mono text-primary">{{ extracted.variableName }}</span>
                      <span class="text-muted-foreground">=</span>
                      <span v-if="extracted.success" class="font-mono truncate max-w-[200px]">{{ extracted.value }}</span>
                      <span v-else class="text-yellow-500">{{ extracted.error }}</span>
                    </div>
                  </div>
                </div>
              </div>
              
              <div v-if="results.length === 0 && !isRunning" class="p-8 text-center text-muted-foreground">
                <Icon name="lucide:flask-conical" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                <p class="text-sm">No test results yet</p>
                <p class="text-xs mt-1">Run tests to see results here</p>
              </div>
            </div>
          </UiScrollArea>
        </div>
        
        <!-- History Tab -->
        <div v-if="activeTab === 'history'" class="h-full flex flex-col">
          <div class="p-4 border-b border-border flex items-center justify-between">
            <h3 class="font-medium">Test Run History</h3>
            <button
              v-if="testRunHistory.length > 0"
              class="text-xs text-destructive hover:underline"
              @click="clearHistory"
            >
              Clear All
            </button>
          </div>
          
          <UiScrollArea class="flex-1">
            <div class="divide-y divide-border">
              <div
                v-for="run in testRunHistory"
                :key="run.id"
                class="p-4 hover:bg-accent/50 transition-colors cursor-pointer"
                @click="viewHistoryRun(run)"
              >
                <div class="flex items-center gap-3">
                  <div 
                    class="h-8 w-8 rounded-lg flex items-center justify-center"
                    :class="run.summary.failed > 0 || run.summary.errors > 0 ? 'bg-red-500/10' : 'bg-green-500/10'"
                  >
                    <Icon 
                      :name="run.summary.failed > 0 || run.summary.errors > 0 ? 'lucide:x-circle' : 'lucide:check-circle'"
                      :class="run.summary.failed > 0 || run.summary.errors > 0 ? 'text-red-500' : 'text-green-500'"
                      class="h-4 w-4"
                    />
                  </div>
                  <div class="flex-1">
                    <div class="font-medium text-sm">{{ run.collectionName }}</div>
                    <div class="text-xs text-muted-foreground">{{ formatDate(run.timestamp) }}</div>
                  </div>
                  <div class="text-right">
                    <div class="text-sm">
                      <span class="text-green-500">{{ run.summary.passed }}</span>
                      <span class="text-muted-foreground">/</span>
                      <span>{{ run.summary.total }}</span>
                    </div>
                    <div class="text-xs text-muted-foreground">{{ formatTime(run.summary.totalTime) }}</div>
                  </div>
                  <button
                    class="p-1 text-muted-foreground hover:text-destructive"
                    @click.stop="deleteHistoryRun(run.id)"
                  >
                    <Icon name="lucide:trash-2" class="h-4 w-4" />
                  </button>
                </div>
              </div>
              
              <div v-if="testRunHistory.length === 0" class="p-8 text-center text-muted-foreground">
                <Icon name="lucide:history" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                <p class="text-sm">No test history yet</p>
                <p class="text-xs mt-1">Your test runs will appear here</p>
              </div>
            </div>
          </UiScrollArea>
        </div>
      </div>
    </div>
  </div>
</template>
