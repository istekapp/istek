<script setup lang="ts">
import type { HttpMethod, KeyValue } from '~/types'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const store = useAppStore()
const { activeTab, collections } = store

const requestTab = ref<'params' | 'headers' | 'body' | 'pre-script' | 'post-script'>('params')
const codeEditorRef = ref<InstanceType<typeof CodeEditor> | null>(null)
const isBodyValid = ref(true)
const bodyErrors = ref<string[]>([])
const validationErrors = ref<string[]>([])
const showCopied = ref(false)

// File selection for form-data
const selectFile = async (fieldId: string) => {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
    })
    if (selected) {
      const filePath = selected as string
      const fileName = filePath.split('/').pop() || filePath.split('\\').pop() || 'file'
      // Get file info - we'll estimate size as 0 for now, actual size will be read when sending
      store.updateFormDataFile(fieldId, filePath, fileName, 0, 'application/octet-stream')
    }
  } catch (e) {
    console.error('Failed to select file:', e)
  }
}

// Format file size for display
const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return ''
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

// Check for missing required params
const missingRequiredParams = computed(() => {
  return activeTab.value.request.params.filter(
    (p: KeyValue) => p.required && p.enabled && (!p.value || p.value.trim() === '')
  )
})

const hasValidationErrors = computed(() => {
  return missingRequiredParams.value.length > 0
})

const methods: { value: HttpMethod; label: string }[] = [
  { value: 'GET', label: 'GET' },
  { value: 'POST', label: 'POST' },
  { value: 'PUT', label: 'PUT' },
  { value: 'PATCH', label: 'PATCH' },
  { value: 'DELETE', label: 'DELETE' },
  { value: 'HEAD', label: 'HEAD' },
  { value: 'OPTIONS', label: 'OPTIONS' },
]

const bodyTypes = [
  { value: 'none', label: 'None' },
  { value: 'form-data', label: 'Form Data' },
  { value: 'x-www-form-urlencoded', label: 'URL Encoded' },
  { value: 'json', label: 'JSON' },
  { value: 'xml', label: 'XML' },
  { value: 'html', label: 'HTML' },
  { value: 'raw', label: 'Raw Text' },
]

// Check body type category
const isMultipartFormData = computed(() => activeTab.value.request.bodyType === 'form-data')
const isUrlEncodedData = computed(() => activeTab.value.request.bodyType === 'x-www-form-urlencoded')

const editorLanguage = computed(() => {
  switch (activeTab.value.request.bodyType) {
    case 'json': return 'json'
    case 'xml': return 'xml'
    case 'html': return 'html'
    default: return 'text'
  }
})

const emit = defineEmits<{
  send: []
  cancel: []
}>()

const showSaveMenu = ref(false)
const saveMenuRef = useClickOutside(() => {
  showSaveMenu.value = false
})

const handleSaveClick = async () => {
  // If request has a source (came from collection), save directly
  if (store.activeTabHasSource.value) {
    const saved = await store.saveActiveRequest()
    if (saved) {
      return // Saved in place, no menu needed
    }
  }
  // Otherwise show menu to pick collection
  showSaveMenu.value = !showSaveMenu.value
}

const saveToOriginal = async () => {
  await store.saveActiveRequest()
  showSaveMenu.value = false
}

const saveToCollection = (collectionId: string) => {
  store.saveToCollection(collectionId)
  showSaveMenu.value = false
}

const copyAsCurl = async () => {
  try {
    const request = activeTab.value.request
    const curlCommand = await invoke<string>('generate_curl_command', {
      method: request.method,
      url: request.url,
      headers: request.headers.map((h: KeyValue) => ({ key: h.key, value: h.value, enabled: h.enabled })),
      params: request.params.map((p: KeyValue) => ({ key: p.key, value: p.value, enabled: p.enabled })),
      body: request.body || null,
      bodyType: request.bodyType,
    })
    
    await navigator.clipboard.writeText(curlCommand)
    showCopied.value = true
    setTimeout(() => {
      showCopied.value = false
    }, 2000)
  } catch (e) {
    console.error('Failed to copy as curl:', e)
  }
}

const handleValidation = (valid: boolean, errors: string[]) => {
  isBodyValid.value = valid
  bodyErrors.value = errors
}

const formatBody = () => {
  codeEditorRef.value?.formatJson()
}

const minifyBody = () => {
  codeEditorRef.value?.minifyJson()
}

// Handle curl paste - parse and fill request fields
const handleCurlPaste = async (curl: string) => {
  try {
    const parsed = await invoke<{
      method: string
      url: string
      headers: Array<{ key: string; value: string; enabled: boolean }>
      body: string | null
      bodyType: string
      formData: Array<{ key: string; value: string; enabled: boolean }> | null
    }>('parse_curl_command', { curl })
    
    // Update method
    store.updateActiveRequest({ method: parsed.method as HttpMethod })
    
    // Update URL
    store.updateActiveRequest({ url: parsed.url })
    
    // Update headers - clear existing non-empty headers and add parsed ones
    const existingHeaders = activeTab.value.request.headers
    // Keep only empty headers
    for (const h of existingHeaders) {
      if (h.key || h.value) {
        store.removeHeader(h.id)
      }
    }
    // Add parsed headers
    for (const header of parsed.headers) {
      store.addHeader()
      const newHeaders = activeTab.value.request.headers
      const lastHeader = newHeaders[newHeaders.length - 1]
      store.updateHeader(lastHeader.id, 'key', header.key)
      store.updateHeader(lastHeader.id, 'value', header.value)
    }
    
    // Update body type first
    store.updateActiveRequest({ 
      bodyType: parsed.bodyType as 'none' | 'json' | 'xml' | 'html' | 'raw' | 'form-data' | 'x-www-form-urlencoded'
    })
    
    // Handle form data body types
    if (parsed.formData && parsed.formData.length > 0) {
      if (parsed.bodyType === 'x-www-form-urlencoded') {
        // Clear existing url encoded data
        const existingData = activeTab.value.request.urlEncodedData || []
        for (const f of existingData) {
          if (f.key || f.value) {
            store.removeUrlEncodedData(f.id)
          }
        }
        // Add parsed data
        for (const field of parsed.formData) {
          store.addUrlEncodedData()
          const newData = activeTab.value.request.urlEncodedData || []
          const lastField = newData[newData.length - 1]
          store.updateUrlEncodedData(lastField.id, 'key', field.key)
          store.updateUrlEncodedData(lastField.id, 'value', field.value)
        }
      } else {
        // form-data (multipart)
        const existingFormData = activeTab.value.request.formData || []
        for (const f of existingFormData) {
          if (f.key || f.value) {
            store.removeFormData(f.id)
          }
        }
        // Add parsed form data
        for (const field of parsed.formData) {
          store.addFormData()
          const newFormData = activeTab.value.request.formData || []
          const lastField = newFormData[newFormData.length - 1]
          store.updateFormData(lastField.id, 'key', field.key)
          store.updateFormData(lastField.id, 'value', field.value)
        }
      }
      // Switch to body tab
      requestTab.value = 'body'
    } else if (parsed.body) {
      // Regular body (JSON, XML, etc.)
      store.updateActiveRequest({ body: parsed.body })
      // Switch to body tab
      requestTab.value = 'body'
    }
    
  } catch (e) {
    console.error('Failed to parse curl command:', e)
    // If parsing fails, just paste the text as URL
    store.updateActiveRequest({ url: curl })
  }
}

// Auto-set Content-Type header when body type changes
watch(() => activeTab.value.request.bodyType, (newType) => {
  const contentTypeMap: Record<string, string> = {
    json: 'application/json',
    xml: 'application/xml',
    html: 'text/html',
    'x-www-form-urlencoded': 'application/x-www-form-urlencoded',
    'form-data': 'multipart/form-data',
  }
  
  if (contentTypeMap[newType]) {
    const existingHeader = activeTab.value.request.headers.find(
      h => h.key.toLowerCase() === 'content-type'
    )
    
    if (!existingHeader) {
      store.addHeader()
      const newHeaders = [...activeTab.value.request.headers]
      const lastHeader = newHeaders[newHeaders.length - 1]
      store.updateHeader(lastHeader.id, 'key', 'Content-Type')
      store.updateHeader(lastHeader.id, 'value', contentTypeMap[newType])
    }
  }
})
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- URL Bar -->
    <div class="flex items-center gap-3 border-b border-border p-4">
      <select
        :value="activeTab.request.method"
        :class="[
          'h-11 rounded-md border border-input bg-background px-3 font-mono text-base font-semibold focus:outline-none focus:ring-2 focus:ring-ring',
          `method-${activeTab.request.method.toLowerCase()}`
        ]"
        @change="store.updateActiveRequest({ method: ($event.target as HTMLSelectElement).value as HttpMethod })"
      >
        <option v-for="m in methods" :key="m.value" :value="m.value">
          {{ m.label }}
        </option>
      </select>

      <UiVariableInput
        :model-value="activeTab.request.url"
        placeholder="Enter request URL (e.g., {{API_URL}}/users) - or paste a curl command"
        class="flex-1 font-mono text-base h-11"
        @update:model-value="store.updateActiveRequest({ url: $event })"
        @keyup.enter="emit('send')"
        @paste-curl="handleCurlPaste"
      />

      <div class="relative group">
        <UiButton
          v-if="!activeTab.isLoading"
          :disabled="(!isBodyValid && activeTab.request.bodyType === 'json') || hasValidationErrors"
          class="h-11 px-4 text-base gap-2"
          @click="emit('send')"
        >
          <Icon v-if="hasValidationErrors" name="lucide:alert-triangle" class="h-5 w-5" />
          <Icon v-else name="lucide:play" class="h-5 w-5 fill-current" />
          <span>Run</span>
        </UiButton>
        <UiButton
          v-else
          variant="destructive"
          class="h-11 px-4 text-base gap-2"
          @click="emit('cancel')"
        >
          <Icon name="lucide:square" class="h-5 w-5 fill-current" />
          <span>Stop</span>
        </UiButton>
        <!-- Validation error tooltip -->
        <div
          v-if="hasValidationErrors && !activeTab.isLoading"
          class="absolute left-1/2 top-full z-50 mt-2 -translate-x-1/2 whitespace-nowrap rounded-md bg-destructive px-3 py-2 text-sm text-destructive-foreground opacity-0 group-hover:opacity-100 transition-opacity"
        >
          Missing required: {{ missingRequiredParams.map((p: KeyValue) => p.key).join(', ') }}
        </div>
      </div>

      <div ref="saveMenuRef" class="relative">
        <UiButton variant="outline" class="h-11 px-4 gap-2" @click="handleSaveClick">
          <Icon name="lucide:save" class="h-5 w-5" />
          <span>Save</span>
        </UiButton>
        <div
          v-if="showSaveMenu"
          class="absolute right-0 top-full z-10 mt-1 w-56 rounded-md border border-border bg-popover p-1 shadow-lg"
        >
          <!-- Save to original location option (if from collection) -->
          <button
            v-if="store.activeTabHasSource.value"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent text-primary font-medium"
            @click="saveToOriginal"
          >
            <Icon name="lucide:save" class="h-4 w-4" />
            Update in place
          </button>
          <div v-if="store.activeTabHasSource.value" class="my-1 border-t border-border" />
          
          <!-- Save to collection options -->
          <div class="px-2 py-1 text-xs text-muted-foreground">Save to collection:</div>
          <div v-if="collections.length === 0" class="p-2 text-center text-sm text-muted-foreground">
            No collections yet
          </div>
          <button
            v-for="collection in collections"
            :key="collection.id"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent"
            @click="saveToCollection(collection.id)"
          >
            <Icon name="lucide:folder" class="h-4 w-4" />
            {{ collection.name }}
          </button>
        </div>
      </div>
    </div>

    <!-- Request Config Tabs -->
    <div class="flex border-b border-border overflow-x-auto">
      <button
        v-for="tab in [
          { id: 'params', label: 'Params' },
          { id: 'headers', label: 'Headers' },
          { id: 'body', label: 'Body' },
          { id: 'pre-script', label: 'Pre-request' },
          { id: 'post-script', label: 'Post-request' },
        ]"
        :key="tab.id"
        :class="[
          'px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors',
          requestTab === tab.id
            ? 'border-b-2 border-primary text-foreground'
            : 'text-muted-foreground hover:text-foreground'
        ]"
        @click="requestTab = tab.id as typeof requestTab"
      >
        {{ tab.label }}
        <span
          v-if="tab.id === 'params' && activeTab.request.params.filter((p: KeyValue) => p.key).length > 0"
          :class="[
            'ml-1.5 rounded-full px-2 py-0.5 text-xs',
            hasValidationErrors ? 'bg-destructive/20 text-destructive' : 'bg-primary/20'
          ]"
        >
          {{ activeTab.request.params.filter((p: KeyValue) => p.key).length }}
        </span>
        <Icon
          v-if="tab.id === 'params' && hasValidationErrors"
          name="lucide:alert-circle"
          class="ml-1 h-3.5 w-3.5 text-destructive"
        />
        <span
          v-if="tab.id === 'headers' && activeTab.request.headers.filter(h => h.key).length > 0"
          class="ml-1.5 rounded-full bg-primary/20 px-2 py-0.5 text-xs"
        >
          {{ activeTab.request.headers.filter(h => h.key).length }}
        </span>
        <span
          v-if="tab.id === 'body' && !isBodyValid && activeTab.request.bodyType === 'json'"
          class="ml-1.5 text-destructive"
        >
          <Icon name="lucide:alert-circle" class="h-3.5 w-3.5" />
        </span>
        <Icon
          v-if="tab.id === 'pre-script' && activeTab.request.preRequestScript?.trim()"
          name="lucide:code"
          class="ml-1 h-3.5 w-3.5 text-method-post"
        />
        <Icon
          v-if="tab.id === 'post-script' && activeTab.request.postRequestScript?.trim()"
          name="lucide:code"
          class="ml-1 h-3.5 w-3.5 text-method-post"
        />
      </button>
    </div>

    <!-- Tab Content -->
    <div class="flex-1 overflow-auto">
      <!-- Params -->
      <div v-if="requestTab === 'params'" class="p-4 space-y-3">
        <div
          v-for="param in activeTab.request.params"
          :key="param.id"
          class="group"
        >
          <div class="flex items-center gap-3">
            <input
              type="checkbox"
              :checked="param.enabled"
              class="h-5 w-5 rounded border-input accent-primary"
              @change="store.toggleParam(param.id)"
            />
            <div class="relative flex-1">
              <UiInput
                :model-value="param.key"
                placeholder="Key"
                :class="[
                  'h-10 text-base',
                  param.required ? 'pr-16' : ''
                ]"
                @update:model-value="store.updateParam(param.id, 'key', $event)"
              />
              <span
                v-if="param.required"
                class="absolute right-2 top-1/2 -translate-y-1/2 rounded bg-destructive/20 px-1.5 py-0.5 text-xs font-medium text-destructive"
              >
                required
              </span>
            </div>
            <div class="relative flex-1">
              <UiVariableInput
                :model-value="param.value"
                :placeholder="param.required ? 'Value (required)' : 'Value'"
                :class="[
                  'h-10 text-base',
                  param.required && param.enabled && (!param.value || param.value.trim() === '') ? 'border-destructive focus:ring-destructive' : ''
                ]"
                @update:model-value="store.updateParam(param.id, 'value', $event)"
              />
            </div>
            <UiButton
              variant="ghost"
              size="icon"
              class="h-10 w-10 text-muted-foreground hover:text-destructive"
              @click="store.removeParam(param.id)"
            >
              <Icon name="lucide:trash-2" class="h-5 w-5" />
            </UiButton>
          </div>
          <!-- Description tooltip -->
          <div
            v-if="param.description"
            class="ml-8 mt-1 text-xs text-muted-foreground"
          >
            {{ param.description }}
          </div>
        </div>
        <UiButton variant="outline" class="h-10 text-base" @click="store.addParam()">
          <Icon name="lucide:plus" class="mr-2 h-5 w-5" />
          Add Parameter
        </UiButton>
      </div>

      <!-- Headers -->
      <div v-else-if="requestTab === 'headers'" class="p-4 space-y-3">
        <div
          v-for="header in activeTab.request.headers"
          :key="header.id"
          class="flex items-center gap-3"
        >
          <input
            type="checkbox"
            :checked="header.enabled"
            class="h-5 w-5 rounded border-input accent-primary"
            @change="store.toggleHeader(header.id)"
          />
          <UiHeaderKeyInput
            :model-value="header.key"
            placeholder="Header name"
            class="flex-1 h-10 text-base"
            @update:model-value="store.updateHeader(header.id, 'key', $event)"
          />
          <UiHeaderValueInput
            :model-value="header.value"
            :header-key="header.key"
            placeholder="Value"
            class="flex-1 h-10 text-base"
            @update:model-value="store.updateHeader(header.id, 'value', $event)"
          />
          <UiButton
            variant="ghost"
            size="icon"
            class="h-10 w-10 text-muted-foreground hover:text-destructive"
            @click="store.removeHeader(header.id)"
          >
            <Icon name="lucide:trash-2" class="h-5 w-5" />
          </UiButton>
        </div>
        <UiButton variant="outline" class="h-10 text-base" @click="store.addHeader()">
          <Icon name="lucide:plus" class="mr-2 h-5 w-5" />
          Add Header
        </UiButton>
      </div>

      <!-- Pre-request Script -->
      <div v-else-if="requestTab === 'pre-script'" class="flex flex-col h-full">
        <div class="p-4 border-b border-border bg-muted/30">
          <div class="flex items-center gap-2 mb-2">
            <Icon name="lucide:code" class="h-4 w-4 text-method-post" />
            <span class="font-medium text-sm">Pre-request Script</span>
          </div>
          <p class="text-xs text-muted-foreground">
            Runs before the request is sent. Use <code class="px-1 py-0.5 rounded bg-muted">istek.variables.set()</code> to set variables, 
            <code class="px-1 py-0.5 rounded bg-muted">istek.request.setHeader()</code> to add headers, or 
            <code class="px-1 py-0.5 rounded bg-muted">istek.abort()</code> to cancel the request.
          </p>
        </div>
        <div class="flex-1 p-4">
          <ClientOnly>
            <CodeEditor
              :model-value="activeTab.request.preRequestScript || ''"
              language="text"
              min-height="250px"
              placeholder="// Example: istek.variables.set('timestamp', Date.now().toString())"
              @update:model-value="store.updateActiveRequest({ preRequestScript: $event })"
            />
          </ClientOnly>
        </div>
      </div>

      <!-- Post-request Script -->
      <div v-else-if="requestTab === 'post-script'" class="flex flex-col h-full">
        <div class="p-4 border-b border-border bg-muted/30">
          <div class="flex items-center gap-2 mb-2">
            <Icon name="lucide:code" class="h-4 w-4 text-method-post" />
            <span class="font-medium text-sm">Post-request Script</span>
          </div>
          <p class="text-xs text-muted-foreground">
            Runs after the response is received. Use <code class="px-1 py-0.5 rounded bg-muted">istek.response.json()</code> to parse the response, 
            <code class="px-1 py-0.5 rounded bg-muted">istek.response.status</code> to check status, or 
            <code class="px-1 py-0.5 rounded bg-muted">istek.variables.set()</code> to store values for later requests.
          </p>
        </div>
        <div class="flex-1 p-4">
          <ClientOnly>
            <CodeEditor
              :model-value="activeTab.request.postRequestScript || ''"
              language="text"
              min-height="250px"
              placeholder="// Example: const data = istek.response.json(); istek.variables.set('token', data.token)"
              @update:model-value="store.updateActiveRequest({ postRequestScript: $event })"
            />
          </ClientOnly>
        </div>
      </div>

      <!-- Body -->
      <div v-else-if="requestTab === 'body'" class="flex flex-col h-full">
        <div class="flex items-center justify-between gap-3 p-4 border-b border-border">
          <div class="flex items-center gap-3">
            <span class="text-base text-muted-foreground">Content Type:</span>
            <UiSelect
              :model-value="activeTab.request.bodyType"
              :options="bodyTypes"
              class="w-36 h-10"
              @update:model-value="store.updateActiveRequest({ bodyType: $event as any })"
            />
          </div>
          
          <!-- JSON Actions -->
          <div v-if="activeTab.request.bodyType === 'json'" class="flex items-center gap-2">
            <UiButton variant="ghost" class="h-9 text-sm" @click="formatBody">
              <Icon name="lucide:align-left" class="mr-1.5 h-4 w-4" />
              Format
            </UiButton>
            <UiButton variant="ghost" class="h-9 text-sm" @click="minifyBody">
              <Icon name="lucide:minimize-2" class="mr-1.5 h-4 w-4" />
              Minify
            </UiButton>
            <div v-if="!isBodyValid" class="flex items-center gap-1.5 text-destructive text-sm ml-2">
              <Icon name="lucide:alert-circle" class="h-4 w-4" />
              Invalid JSON
            </div>
            <div v-else-if="activeTab.request.body.trim()" class="flex items-center gap-1.5 text-method-get text-sm ml-2">
              <Icon name="lucide:check-circle" class="h-4 w-4" />
              Valid JSON
            </div>
          </div>
        </div>
        
        <!-- Multipart Form Data (with file support) -->
        <div v-if="isMultipartFormData" class="flex-1 overflow-auto p-4 space-y-2">
          <div
            v-for="item in (activeTab.request.formData || [])"
            :key="item.id"
            class="flex items-center gap-3"
          >
            <UiCheckbox
              :checked="item.enabled"
              @update:checked="store.toggleFormData(item.id)"
            />
            <UiVariableInput
              :model-value="item.key"
              placeholder="Key"
              class="w-48 h-10"
              @update:model-value="store.updateFormData(item.id, 'key', $event)"
            />
            <!-- Type selector -->
            <select
              :value="item.type"
              class="h-10 rounded-md border border-input bg-background px-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
              @change="store.updateFormData(item.id, 'type', ($event.target as HTMLSelectElement).value)"
            >
              <option value="text">Text</option>
              <option value="file">File</option>
            </select>
            <!-- Text input -->
            <UiVariableInput
              v-if="item.type === 'text'"
              :model-value="item.value"
              placeholder="Value"
              class="flex-1 h-10"
              @update:model-value="store.updateFormData(item.id, 'value', $event)"
            />
            <!-- File input -->
            <div v-else class="flex-1 flex items-center gap-2">
              <UiButton
                variant="outline"
                class="h-10"
                @click="selectFile(item.id)"
              >
                <Icon name="lucide:upload" class="mr-2 h-4 w-4" />
                Choose File
              </UiButton>
              <span v-if="item.fileName" class="text-sm text-muted-foreground truncate max-w-48">
                {{ item.fileName }}
                <span v-if="item.fileSize" class="opacity-70">({{ formatFileSize(item.fileSize) }})</span>
              </span>
              <span v-else class="text-sm text-muted-foreground">No file selected</span>
            </div>
            <UiButton
              variant="ghost"
              size="icon"
              class="h-10 w-10 shrink-0"
              :disabled="(activeTab.request.formData || []).length <= 1"
              @click="store.removeFormData(item.id)"
            >
              <Icon name="lucide:trash-2" class="h-5 w-5" />
            </UiButton>
          </div>
          <UiButton variant="outline" class="h-10 text-base" @click="store.addFormData()">
            <Icon name="lucide:plus" class="mr-2 h-5 w-5" />
            Add Field
          </UiButton>
        </div>
        
        <!-- URL Encoded Data -->
        <div v-else-if="isUrlEncodedData" class="flex-1 overflow-auto p-4 space-y-2">
          <div
            v-for="item in (activeTab.request.urlEncodedData || [])"
            :key="item.id"
            class="flex items-center gap-3"
          >
            <UiCheckbox
              :checked="item.enabled"
              @update:checked="store.toggleUrlEncodedData(item.id)"
            />
            <UiVariableInput
              :model-value="item.key"
              placeholder="Key"
              class="flex-1 h-10"
              @update:model-value="store.updateUrlEncodedData(item.id, 'key', $event)"
            />
            <UiVariableInput
              :model-value="item.value"
              placeholder="Value"
              class="flex-1 h-10"
              @update:model-value="store.updateUrlEncodedData(item.id, 'value', $event)"
            />
            <UiButton
              variant="ghost"
              size="icon"
              class="h-10 w-10 shrink-0"
              :disabled="(activeTab.request.urlEncodedData || []).length <= 1"
              @click="store.removeUrlEncodedData(item.id)"
            >
              <Icon name="lucide:trash-2" class="h-5 w-5" />
            </UiButton>
          </div>
          <UiButton variant="outline" class="h-10 text-base" @click="store.addUrlEncodedData()">
            <Icon name="lucide:plus" class="mr-2 h-5 w-5" />
            Add Field
          </UiButton>
        </div>
        
        <!-- Code Editor Body (JSON, XML, HTML, Raw) -->
        <div v-else-if="activeTab.request.bodyType !== 'none'" class="flex-1 p-4">
          <ClientOnly>
            <CodeEditor
              ref="codeEditorRef"
              :model-value="activeTab.request.body"
              :language="editorLanguage"
              min-height="300px"
              @update:model-value="store.updateActiveRequest({ body: $event })"
              @validation="handleValidation"
            />
          </ClientOnly>
        </div>
        <div v-else class="flex flex-1 items-center justify-center text-muted-foreground">
          <div class="text-center">
            <Icon name="lucide:file-x" class="mx-auto h-14 w-14 opacity-50" />
            <p class="mt-3 text-lg">This request does not have a body</p>
            <p class="text-base opacity-70">Select a content type to add a request body</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
