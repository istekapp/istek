<script setup lang="ts">
import { formatBytes, formatDuration, getStatusColor, tryParseJson } from '~/lib/utils'
import { invoke } from '@tauri-apps/api/core'

const store = useAppStore()
const { activeTab } = store

const responseTab = ref<'body' | 'headers' | 'code'>('body')
const copied = ref(false)
const selectedLanguage = ref('curl')
const generatedCode = ref('')
const isGenerating = ref(false)

const codeLanguages = [
  { value: 'curl', label: 'cURL', editorLang: 'shell' },
  { value: 'python', label: 'Python (requests)', editorLang: 'python' },
  { value: 'javascript', label: 'JavaScript (fetch)', editorLang: 'javascript' },
  { value: 'go', label: 'Go', editorLang: 'go' },
  { value: 'rust', label: 'Rust (reqwest)', editorLang: 'rust' },
  { value: 'java', label: 'Java (HttpClient)', editorLang: 'java' },
  { value: 'csharp', label: 'C# (HttpClient)', editorLang: 'csharp' },
  { value: 'php', label: 'PHP (cURL)', editorLang: 'php' },
  { value: 'ruby', label: 'Ruby (Net::HTTP)', editorLang: 'ruby' },
]

const codeEditorLanguage = computed(() => {
  const lang = codeLanguages.find(l => l.value === selectedLanguage.value)
  return lang?.editorLang || 'text'
})

// Generate code when language changes or tab opens
const generateCode = async () => {
  if (responseTab.value !== 'code') return
  
  isGenerating.value = true
  try {
    const request = activeTab.value.request
    const code = await invoke<string>('generate_code_snippet', {
      language: selectedLanguage.value,
      method: request.method,
      url: request.url,
      headers: request.headers.filter((h: any) => h.enabled && h.key).map((h: any) => ({ key: h.key, value: h.value })),
      params: request.params.filter((p: any) => p.enabled && p.key).map((p: any) => ({ key: p.key, value: p.value })),
      body: request.body || null,
      bodyType: request.bodyType,
    })
    generatedCode.value = code
  } catch (e) {
    console.error('Failed to generate code:', e)
    generatedCode.value = `// Error generating code: ${e}`
  } finally {
    isGenerating.value = false
  }
}

watch(selectedLanguage, generateCode)
watch(responseTab, (tab) => {
  if (tab === 'code') generateCode()
})

const copyCode = async () => {
  await navigator.clipboard.writeText(generatedCode.value)
  copied.value = true
  setTimeout(() => copied.value = false, 2000)
}

const formattedBody = computed(() => {
  if (!activeTab.value.response) return ''
  return tryParseJson(activeTab.value.response.body)
})

const headerEntries = computed(() => {
  if (!activeTab.value.response) return []
  return Object.entries(activeTab.value.response.headers)
})

const contentType = computed(() => {
  if (!activeTab.value.response) return 'text'
  const ct = activeTab.value.response.headers['content-type'] || 
             activeTab.value.response.headers['Content-Type'] || ''
  
  if (ct.includes('json')) return 'json'
  if (ct.includes('xml')) return 'xml'
  if (ct.includes('html')) return 'html'
  return 'text'
})

const isJson = computed(() => {
  if (!activeTab.value.response) return false
  try {
    JSON.parse(activeTab.value.response.body)
    return true
  } catch {
    return false
  }
})

const responseLanguage = computed(() => {
  // If it looks like JSON, treat it as JSON regardless of content-type
  if (isJson.value) return 'json'
  return contentType.value
})

const copyToClipboard = async () => {
  if (activeTab.value.response) {
    await navigator.clipboard.writeText(activeTab.value.response.body)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  }
}

const downloadResponse = () => {
  if (!activeTab.value.response) return
  
  const blob = new Blob([activeTab.value.response.body], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `response-${Date.now()}.${responseLanguage.value === 'json' ? 'json' : 'txt'}`
  a.click()
  URL.revokeObjectURL(url)
}

const getStatusBadgeClass = (status: number) => {
  if (status >= 200 && status < 300) return 'bg-method-get/20 text-method-get border-method-get/30'
  if (status >= 300 && status < 400) return 'bg-method-post/20 text-method-post border-method-post/30'
  if (status >= 400 && status < 500) return 'bg-protocol-mqtt/20 text-protocol-mqtt border-protocol-mqtt/30'
  if (status >= 500) return 'bg-method-delete/20 text-method-delete border-method-delete/30'
  return 'bg-muted text-muted-foreground'
}
</script>

<template>
  <div class="flex flex-col border-l border-border h-full">
    <!-- Response Header -->
    <div class="flex h-14 items-center justify-between border-b border-border px-4">
      <span class="text-base font-medium">Response</span>
      <div v-if="activeTab.response" class="flex items-center gap-3">
        <span
          :class="[
            'inline-flex items-center rounded-md border px-2.5 py-1 text-sm font-semibold',
            getStatusBadgeClass(activeTab.response.status)
          ]"
        >
          {{ activeTab.response.status }} {{ activeTab.response.statusText }}
        </span>
        <span class="text-sm text-muted-foreground flex items-center gap-1.5">
          <Icon name="lucide:clock" class="h-4 w-4" />
          {{ formatDuration(activeTab.response.time) }}
        </span>
        <span class="text-sm text-muted-foreground flex items-center gap-1.5">
          <Icon name="lucide:hard-drive" class="h-4 w-4" />
          {{ formatBytes(activeTab.response.size) }}
        </span>
      </div>
    </div>

    <!-- Loading State -->
    <div v-if="activeTab.isLoading" class="flex flex-1 items-center justify-center">
      <div class="text-center">
        <div class="relative">
          <Icon name="lucide:loader-2" class="mx-auto h-12 w-12 animate-spin text-primary" />
        </div>
        <p class="mt-4 text-base text-muted-foreground">Sending request...</p>
        <p class="mt-1 text-sm text-muted-foreground/70">Please wait</p>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else-if="!activeTab.response" class="flex flex-1 items-center justify-center">
      <div class="text-center max-w-xs">
        <div class="mx-auto h-20 w-20 rounded-full bg-muted/50 flex items-center justify-center">
          <Icon name="lucide:arrow-right-circle" class="h-10 w-10 text-muted-foreground/50" />
        </div>
        <p class="mt-4 text-lg font-medium text-foreground">No Response Yet</p>
        <p class="mt-2 text-base text-muted-foreground">
          Enter a URL and click Send to make a request
        </p>
        <div class="mt-4 text-sm text-muted-foreground/70 space-y-1">
          <p>Tip: Press Enter in the URL field to send quickly</p>
        </div>
      </div>
    </div>

    <!-- Response Content -->
    <template v-else>
      <!-- Response Tabs & Actions -->
      <div class="flex items-center justify-between border-b border-border px-4">
        <div class="flex">
          <button
            v-for="tab in ['body', 'headers', 'code'] as const"
            :key="tab"
            :class="[
              'px-5 py-3 text-base font-medium capitalize transition-colors',
              responseTab === tab
                ? 'border-b-2 border-primary text-foreground'
                : 'text-muted-foreground hover:text-foreground'
            ]"
            @click="responseTab = tab"
          >
            {{ tab }}
            <span
              v-if="tab === 'headers'"
              class="ml-1.5 rounded-full bg-muted px-2 py-0.5 text-sm"
            >
              {{ headerEntries.length }}
            </span>
            <span
              v-if="tab === 'body' && responseLanguage !== 'text'"
              class="ml-1.5 rounded bg-muted px-2 py-0.5 text-xs uppercase"
            >
              {{ responseLanguage }}
            </span>
          </button>
        </div>
        <div v-if="responseTab !== 'code'" class="flex items-center gap-2">
          <UiButton variant="ghost" class="h-9 text-sm" @click="copyToClipboard">
            <Icon :name="copied ? 'lucide:check' : 'lucide:copy'" class="mr-1.5 h-4 w-4" />
            {{ copied ? 'Copied!' : 'Copy' }}
          </UiButton>
          <UiButton variant="ghost" class="h-9 text-sm" @click="downloadResponse">
            <Icon name="lucide:download" class="mr-1.5 h-4 w-4" />
            Save
          </UiButton>
        </div>
        <div v-else class="flex items-center gap-2">
          <select
            v-model="selectedLanguage"
            class="h-9 rounded-md border border-input bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option v-for="lang in codeLanguages" :key="lang.value" :value="lang.value">
              {{ lang.label }}
            </option>
          </select>
          <UiButton variant="ghost" class="h-9 text-sm" @click="copyCode">
            <Icon :name="copied ? 'lucide:check' : 'lucide:copy'" class="mr-1.5 h-4 w-4" />
            {{ copied ? 'Copied!' : 'Copy' }}
          </UiButton>
        </div>
      </div>

      <!-- Response Body -->
      <div v-if="responseTab === 'body'" class="flex-1 overflow-hidden min-h-0">
        <ClientOnly>
          <CodeEditor
            :model-value="formattedBody"
            :language="responseLanguage"
            :readonly="true"
            min-height="100%"
            class="h-full"
          />
        </ClientOnly>
      </div>

      <!-- Response Headers -->
      <UiScrollArea v-else-if="responseTab === 'headers'" class="flex-1">
        <div class="divide-y divide-border">
          <div
            v-for="[key, value] in headerEntries"
            :key="key"
            class="flex gap-4 px-4 py-3 hover:bg-muted/50 transition-colors"
          >
            <span class="w-52 shrink-0 font-mono text-sm font-medium text-primary">{{ key }}</span>
            <span class="text-sm text-muted-foreground break-all font-mono">{{ value }}</span>
          </div>
        </div>
        <div v-if="headerEntries.length === 0" class="p-4 text-center text-base text-muted-foreground">
          No headers in response
        </div>
      </UiScrollArea>

      <!-- Generated Code -->
      <div v-else-if="responseTab === 'code'" class="flex-1 overflow-hidden min-h-0">
        <div v-if="isGenerating" class="flex items-center justify-center h-full">
          <Icon name="lucide:loader-2" class="h-8 w-8 animate-spin text-primary" />
        </div>
        <ClientOnly v-else>
          <CodeEditor
            :model-value="generatedCode"
            :language="codeEditorLanguage"
            :readonly="true"
            min-height="100%"
            class="h-full"
          />
        </ClientOnly>
      </div>
    </template>
  </div>
</template>
