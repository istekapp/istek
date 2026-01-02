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
  TestTab
} from '~/types'
import { generateId } from '~/lib/utils'

const props = defineProps<{
  tab: TestTab
}>()

const store = useAppStore()
const { collections } = store

// Test runner state
const isRunning = ref(false)
const currentRun = ref<TestRunSummary | null>(null)
const results = ref<TestResult[]>([])
const progress = ref({ current: 0, total: 0 })

// Configuration
const selectedCollectionId = ref<string | null>(props.tab.collectionId || null)
const selectedFolderId = ref<string | null>(null)
const stopOnFailure = ref(false)
const delayBetweenRequests = ref(100)

// Left panel tab
const leftTab = ref<'config' | 'history'>('config')

// Assertions & Extractions Configuration
const requestConfigs = ref<Map<string, { assertions: Assertion[], extractVariables: VariableExtraction[] }>>(new Map())
const expandedRequestId = ref<string | null>(null)

// History
const testRunHistory = ref<TestRunHistory[]>([])

// Initialize with collection from tab
watch(() => props.tab.collectionId, (newVal) => {
  if (newVal) {
    selectedCollectionId.value = newVal
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
  isRunning.value = false
}

const viewHistoryRun = (run: TestRunHistory) => {
  currentRun.value = run.summary
  results.value = run.summary.results
}

const deleteHistoryRun = async (runId: string) => {
  try {
    await invoke('delete_test_run', { id: runId })
    await loadHistory()
  } catch (e) {
    console.error('Failed to delete test run:', e)
  }
}

const clearHistory = async () => {
  try {
    await invoke('clear_test_runs')
    testRunHistory.value = []
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
            leftTab === 'history' ? 'border-b-2 border-primary text-foreground' : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="leftTab = 'history'"
        >
          <Icon name="lucide:history" class="h-4 w-4 mr-2 inline" />
          History
          <span v-if="testRunHistory.length > 0" class="ml-1.5 px-1.5 py-0.5 text-xs bg-muted text-muted-foreground rounded-full">
            {{ testRunHistory.length }}
          </span>
        </button>
      </div>
      
      <!-- Config Tab Content -->
      <template v-if="leftTab === 'config'">
        <!-- Collection Selector -->
        <div class="p-4 border-b border-border space-y-3">
          <div class="flex items-center gap-2">
            <label class="text-sm font-medium">Collection:</label>
            <select
              v-model="selectedCollectionId"
              class="flex-1 h-9 rounded-md border border-border bg-background px-3 text-sm"
              @change="selectedFolderId = null"
            >
              <option :value="null">Select a collection...</option>
              <option v-for="c in collections" :key="c.id" :value="c.id">
                {{ c.name }}
              </option>
            </select>
          </div>
          
          <div v-if="selectedCollection?.folders?.length" class="flex items-center gap-2">
            <label class="text-sm font-medium">Folder:</label>
            <select
              v-model="selectedFolderId"
              class="flex-1 h-9 rounded-md border border-border bg-background px-3 text-sm"
            >
              <option :value="null">All requests</option>
              <option v-for="f in selectedCollection.folders" :key="f.id" :value="f.id">
                {{ f.name }} ({{ f.requests.filter(r => r.protocol === 'http').length }})
              </option>
            </select>
          </div>
        </div>
        
        <!-- Options -->
        <div class="p-4 border-b border-border flex items-center gap-4">
          <label class="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" v-model="stopOnFailure" class="accent-primary" />
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
        
        <!-- Requests List -->
        <div class="flex-1 overflow-hidden flex flex-col">
          <div class="p-4 border-b border-border flex items-center justify-between">
            <h3 class="font-medium">Requests ({{ testableRequests.length }})</h3>
            <span class="text-xs text-muted-foreground">Click to configure</span>
          </div>
          
          <UiScrollArea class="flex-1">
            <div class="p-4 space-y-2">
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
                  <div class="text-xs text-muted-foreground font-mono truncate">
                    {{ request.url }}
                  </div>
                  
                  <!-- Assertions Section -->
                  <div>
                    <div class="flex items-center justify-between mb-2">
                      <h4 class="text-sm font-medium">Assertions</h4>
                      <button class="text-xs text-primary hover:underline" @click="addAssertion(request.id)">
                        + Add
                      </button>
                    </div>
                    
                    <div class="space-y-2">
                      <div
                        v-for="assertion in getRequestConfig(request.id).assertions"
                        :key="assertion.id"
                        class="flex items-center gap-2 p-2 bg-background rounded border border-border"
                      >
                        <input type="checkbox" v-model="assertion.enabled" class="accent-primary" />
                        
                        <select
                          v-model="assertion.type"
                          class="h-7 text-xs bg-background border border-border rounded px-2"
                        >
                          <option v-for="t in assertionTypes" :key="t.value" :value="t.value">
                            {{ t.label }}
                          </option>
                        </select>
                        
                        <!-- Status assertion -->
                        <template v-if="assertion.type === 'status'">
                          <span class="text-xs">=</span>
                          <UiInput v-model.number="assertion.expectedStatus" type="number" class="w-20 h-7 text-xs" placeholder="200" />
                        </template>
                        
                        <!-- Status Range assertion -->
                        <template v-if="assertion.type === 'status_range'">
                          <UiInput v-model.number="assertion.minStatus" type="number" class="w-16 h-7 text-xs" placeholder="200" />
                          <span class="text-xs">-</span>
                          <UiInput v-model.number="assertion.maxStatus" type="number" class="w-16 h-7 text-xs" placeholder="299" />
                        </template>
                        
                        <!-- JSONPath assertion -->
                        <template v-if="assertion.type === 'jsonpath'">
                          <UiInput v-model="assertion.jsonPath" class="flex-1 h-7 text-xs font-mono" placeholder="$.data.id" />
                          <select v-model="assertion.operator" class="h-7 text-xs bg-background border border-border rounded px-2">
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
                          <UiInput v-model="assertion.searchString" class="flex-1 h-7 text-xs" placeholder="search string" />
                        </template>
                        
                        <!-- Response Time assertion -->
                        <template v-if="assertion.type === 'response_time'">
                          <span class="text-xs">&lt;</span>
                          <UiInput v-model.number="assertion.maxTimeMs" type="number" class="w-20 h-7 text-xs" placeholder="5000" />
                          <span class="text-xs">ms</span>
                        </template>
                        
                        <!-- Header assertion -->
                        <template v-if="assertion.type === 'header'">
                          <UiInput v-model="assertion.headerName" class="w-32 h-7 text-xs" placeholder="Header-Name" />
                          <span class="text-xs">=</span>
                          <UiInput v-model="assertion.headerValue" class="flex-1 h-7 text-xs" placeholder="value (optional)" />
                        </template>
                        
                        <button class="p-1 text-muted-foreground hover:text-destructive" @click="removeAssertion(request.id, assertion.id)">
                          <Icon name="lucide:x" class="h-3 w-3" />
                        </button>
                      </div>
                      
                      <div v-if="getRequestConfig(request.id).assertions.length === 0" class="text-xs text-muted-foreground py-2">
                        No assertions. Default: status &lt; 400
                      </div>
                    </div>
                  </div>
                  
                  <!-- Variable Extraction Section -->
                  <div>
                    <div class="flex items-center justify-between mb-2">
                      <h4 class="text-sm font-medium">Extract Variables</h4>
                      <button class="text-xs text-primary hover:underline" @click="addExtraction(request.id)">
                        + Add
                      </button>
                    </div>
                    
                    <div class="space-y-2">
                      <div
                        v-for="extraction in getRequestConfig(request.id).extractVariables"
                        :key="extraction.id"
                        class="flex items-center gap-2 p-2 bg-background rounded border border-border"
                      >
                        <input type="checkbox" v-model="extraction.enabled" class="accent-primary" />
                        <UiInput v-model="extraction.variableName" class="w-32 h-7 text-xs" placeholder="variableName" />
                        <span class="text-xs">=</span>
                        <UiInput v-model="extraction.jsonPath" class="flex-1 h-7 text-xs font-mono" placeholder="$.data.id" />
                        <button class="p-1 text-muted-foreground hover:text-destructive" @click="removeExtraction(request.id, extraction.id)">
                          <Icon name="lucide:x" class="h-3 w-3" />
                        </button>
                      </div>
                      
                      <div v-if="getRequestConfig(request.id).extractVariables.length === 0" class="text-xs text-muted-foreground py-2">
                        Use {{variableName}} in subsequent requests.
                      </div>
                    </div>
                  </div>
                </div>
              </div>
              
              <div v-if="testableRequests.length === 0" class="text-center py-8 text-muted-foreground">
                <Icon name="lucide:inbox" class="h-8 w-8 mx-auto mb-2 opacity-50" />
                <p class="text-sm">Select a collection</p>
              </div>
            </div>
          </UiScrollArea>
        </div>
        
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
          <UiButton v-else variant="destructive" class="w-full" @click="stopTests">
            <Icon name="lucide:square" class="h-4 w-4 mr-2" />
            Stop
          </UiButton>
        </div>
      </template>
      
      <!-- History Tab Content -->
      <template v-else>
        <div class="p-4 border-b border-border flex items-center justify-between">
          <h3 class="font-medium">Test Run History</h3>
          <button v-if="testRunHistory.length > 0" class="text-xs text-destructive hover:underline" @click="clearHistory">
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
                    <span class="text-green-500">{{ run.summary.passed }}</span>/<span>{{ run.summary.total }}</span>
                  </div>
                  <div class="text-xs text-muted-foreground">{{ formatTime(run.summary.totalTime) }}</div>
                </div>
                <button class="p-1 text-muted-foreground hover:text-destructive" @click.stop="deleteHistoryRun(run.id)">
                  <Icon name="lucide:trash-2" class="h-4 w-4" />
                </button>
              </div>
            </div>
            
            <div v-if="testRunHistory.length === 0" class="p-8 text-center text-muted-foreground">
              <Icon name="lucide:history" class="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p class="text-sm">No test history yet</p>
            </div>
          </div>
        </UiScrollArea>
      </template>
    </div>
    
    <!-- Right Panel: Results -->
    <div class="flex w-1/2 flex-col overflow-hidden">
      <div class="p-4 border-b border-border">
        <h3 class="font-medium">Test Results</h3>
      </div>
      
      <!-- Summary -->
      <div v-if="currentRun" class="p-4 border-b border-border">
        <div class="flex items-center justify-between mb-4">
          <span class="text-sm font-medium">{{ currentRun.name }}</span>
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
        
        <div class="grid grid-cols-4 gap-3 text-center">
          <div class="p-2 rounded-lg bg-muted/50">
            <div class="text-xl font-bold">{{ currentRun.total }}</div>
            <div class="text-xs text-muted-foreground">Total</div>
          </div>
          <div class="p-2 rounded-lg bg-green-500/10">
            <div class="text-xl font-bold text-green-500">{{ currentRun.passed }}</div>
            <div class="text-xs text-muted-foreground">Passed</div>
          </div>
          <div class="p-2 rounded-lg bg-red-500/10">
            <div class="text-xl font-bold text-red-500">{{ currentRun.failed }}</div>
            <div class="text-xs text-muted-foreground">Failed</div>
          </div>
          <div class="p-2 rounded-lg bg-yellow-500/10">
            <div class="text-xl font-bold text-yellow-500">{{ currentRun.errors }}</div>
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
              <Icon :name="getStatusIcon(result.status)" :class="['h-5 w-5', getStatusColor(result.status)]" />
              <span :class="['font-mono text-sm font-semibold w-16', getMethodColor(result.method)]">
                {{ result.method }}
              </span>
              <span class="flex-1 text-sm font-medium truncate">{{ result.requestName }}</span>
              <span v-if="result.responseStatus" class="text-sm text-muted-foreground">{{ result.responseStatus }}</span>
              <span v-if="result.responseTime" class="text-xs text-muted-foreground">{{ result.responseTime }}ms</span>
            </div>
            
            <div class="mt-1 text-xs text-muted-foreground font-mono truncate pl-8">{{ result.url }}</div>
            
            <!-- Error message -->
            <div v-if="result.error" class="mt-2 text-xs text-red-500 pl-8">{{ result.error }}</div>
            
            <!-- Assertions -->
            <div v-if="result.assertions.length > 0" class="mt-2 pl-8 space-y-1">
              <div v-for="(assertion, i) in result.assertions" :key="i" class="flex items-center gap-2 text-xs">
                <Icon :name="assertion.passed ? 'lucide:check' : 'lucide:x'" :class="assertion.passed ? 'text-green-500' : 'text-red-500'" class="h-3 w-3" />
                <span class="text-muted-foreground">{{ assertion.name }}</span>
                <span v-if="!assertion.passed" class="text-red-400">(expected: {{ assertion.expected }}, got: {{ assertion.actual }})</span>
              </div>
            </div>
            
            <!-- Extracted Variables -->
            <div v-if="result.extractedVariables?.length" class="mt-2 pl-8">
              <div class="text-xs text-muted-foreground mb-1">Extracted Variables:</div>
              <div v-for="(extracted, i) in result.extractedVariables" :key="i" class="flex items-center gap-2 text-xs">
                <Icon :name="extracted.success ? 'lucide:variable' : 'lucide:alert-triangle'" :class="extracted.success ? 'text-blue-500' : 'text-yellow-500'" class="h-3 w-3" />
                <span class="font-mono text-primary">{{ extracted.variableName }}</span>
                <span class="text-muted-foreground">=</span>
                <span v-if="extracted.success" class="font-mono truncate max-w-[200px]">{{ extracted.value }}</span>
                <span v-else class="text-yellow-500">{{ extracted.error }}</span>
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
  </div>
</template>
