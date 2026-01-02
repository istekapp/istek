<script setup lang="ts">
import type { CollectionFolder, Collection, AuthConfig, AuthType, KeyValue, Variable } from '~/types'
import { generateId } from '~/lib/utils'

const props = defineProps<{
  show: boolean
  folder?: CollectionFolder
  collection?: Collection
  isCollectionLevel?: boolean
}>()

const emit = defineEmits<{
  'update:show': [value: boolean]
  'save': [settings: { auth?: AuthConfig, headers?: KeyValue[], variables?: Variable[], baseUrl?: string }]
}>()

const activeSettingsTab = ref<'auth' | 'headers' | 'variables' | 'general'>('auth')

// Local state for editing
const authType = ref<AuthType>('none')
const authEnabled = ref(true)
const authEnabledWhen = ref('')

// Basic Auth
const basicUsername = ref('')
const basicPassword = ref('')

// Bearer Token
const bearerToken = ref('')
const bearerPrefix = ref('Bearer')

// API Key
const apiKeyName = ref('')
const apiKeyValue = ref('')
const apiKeyIn = ref<'header' | 'query'>('header')

// Headers
const headers = ref<KeyValue[]>([])

// Variables
const variables = ref<Variable[]>([])

// General
const baseUrl = ref('')

// Initialize from props
watch(() => props.show, (newVal) => {
  if (newVal) {
    const settings = props.isCollectionLevel 
      ? props.collection?.settings 
      : props.folder?.settings
    
    // Auth
    authType.value = settings?.auth?.type || 'none'
    authEnabled.value = settings?.auth?.enabled ?? true
    authEnabledWhen.value = settings?.auth?.enabledWhen || ''
    basicUsername.value = settings?.auth?.username || ''
    basicPassword.value = settings?.auth?.password || ''
    bearerToken.value = settings?.auth?.token || ''
    bearerPrefix.value = settings?.auth?.prefix || 'Bearer'
    apiKeyName.value = settings?.auth?.apiKeyName || ''
    apiKeyValue.value = settings?.auth?.apiKeyValue || ''
    apiKeyIn.value = settings?.auth?.apiKeyIn || 'header'
    
    // Headers
    headers.value = settings?.headers?.map(h => ({ ...h })) || []
    if (headers.value.length === 0) {
      headers.value = [{ id: generateId(), key: '', value: '', enabled: true }]
    }
    
    // Variables
    variables.value = settings?.variables?.map(v => ({ ...v })) || []
    if (variables.value.length === 0) {
      variables.value = [{ id: generateId(), key: '', value: '', enabled: true, isSecret: false }]
    }
    
    // General
    baseUrl.value = settings?.baseUrl || ''
  }
}, { immediate: true })

const authTypes: { value: AuthType, label: string }[] = [
  { value: 'none', label: 'No Auth' },
  { value: 'inherit', label: 'Inherit from Parent' },
  { value: 'basic', label: 'Basic Auth' },
  { value: 'bearer', label: 'Bearer Token' },
  { value: 'api-key', label: 'API Key' },
]

const addHeader = () => {
  headers.value.push({ id: generateId(), key: '', value: '', enabled: true })
}

const removeHeader = (id: string) => {
  headers.value = headers.value.filter(h => h.id !== id)
}

const addVariable = () => {
  variables.value.push({ id: generateId(), key: '', value: '', enabled: true, isSecret: false })
}

const removeVariable = (id: string) => {
  variables.value = variables.value.filter(v => v.id !== id)
}

const handleSave = () => {
  const auth: AuthConfig | undefined = authType.value !== 'none' ? {
    type: authType.value,
    enabled: authEnabled.value,
    enabledWhen: authEnabledWhen.value || undefined,
    username: authType.value === 'basic' ? basicUsername.value : undefined,
    password: authType.value === 'basic' ? basicPassword.value : undefined,
    token: authType.value === 'bearer' ? bearerToken.value : undefined,
    prefix: authType.value === 'bearer' ? bearerPrefix.value : undefined,
    apiKeyName: authType.value === 'api-key' ? apiKeyName.value : undefined,
    apiKeyValue: authType.value === 'api-key' ? apiKeyValue.value : undefined,
    apiKeyIn: authType.value === 'api-key' ? apiKeyIn.value : undefined,
  } : undefined

  const filteredHeaders = headers.value.filter(h => h.key.trim() !== '')
  const filteredVariables = variables.value.filter(v => v.key.trim() !== '')

  emit('save', {
    auth,
    headers: filteredHeaders.length > 0 ? filteredHeaders : undefined,
    variables: filteredVariables.length > 0 ? filteredVariables : undefined,
    baseUrl: baseUrl.value.trim() || undefined,
  })
  emit('update:show', false)
}

const close = () => {
  emit('update:show', false)
}

const title = computed(() => {
  if (props.isCollectionLevel) {
    return `Collection Settings: ${props.collection?.name || 'Untitled'}`
  }
  return `Folder Settings: ${props.folder?.name || 'Untitled'}`
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="show"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      @click.self="close"
    >
      <div class="w-full max-w-2xl rounded-lg border border-border bg-background shadow-xl">
        <!-- Header -->
        <div class="flex items-center justify-between border-b border-border px-6 py-4">
          <h2 class="text-lg font-semibold">{{ title }}</h2>
          <button
            class="rounded-md p-1 hover:bg-accent"
            @click="close"
          >
            <Icon name="lucide:x" class="h-5 w-5" />
          </button>
        </div>

        <!-- Tabs -->
        <div class="flex border-b border-border px-6">
          <button
            v-for="tab in ['auth', 'headers', 'variables', 'general'] as const"
            :key="tab"
            :class="[
              'px-4 py-3 text-sm font-medium capitalize transition-colors',
              activeSettingsTab === tab
                ? 'border-b-2 border-primary text-foreground'
                : 'text-muted-foreground hover:text-foreground'
            ]"
            @click="activeSettingsTab = tab"
          >
            {{ tab }}
          </button>
        </div>

        <!-- Content -->
        <div class="max-h-[60vh] overflow-y-auto p-6">
          <!-- Auth Tab -->
          <div v-if="activeSettingsTab === 'auth'" class="space-y-4">
            <div class="space-y-2">
              <label class="text-sm font-medium">Authentication Type</label>
              <select
                v-model="authType"
                class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
              >
                <option v-for="t in authTypes" :key="t.value" :value="t.value">
                  {{ t.label }}
                </option>
              </select>
            </div>

            <div v-if="authType !== 'none' && authType !== 'inherit'" class="space-y-4">
              <!-- Enable/Disable -->
              <div class="flex items-center gap-3">
                <input
                  id="auth-enabled"
                  v-model="authEnabled"
                  type="checkbox"
                  class="h-4 w-4 rounded border-input"
                />
                <label for="auth-enabled" class="text-sm">Enabled</label>
              </div>

              <!-- Conditional Enable -->
              <div class="space-y-2">
                <label class="text-sm font-medium">Enable When (Optional)</label>
                <input
                  v-model="authEnabledWhen"
                  type="text"
                  placeholder="e.g., {{ENV}} == 'production'"
                  class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
                />
                <p class="text-xs text-muted-foreground">Leave empty to always enable</p>
              </div>

              <!-- Basic Auth -->
              <template v-if="authType === 'basic'">
                <div class="space-y-2">
                  <label class="text-sm font-medium">Username</label>
                  <input
                    v-model="basicUsername"
                    type="text"
                    placeholder="Username"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm font-medium">Password</label>
                  <input
                    v-model="basicPassword"
                    type="password"
                    placeholder="Password"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                  />
                </div>
              </template>

              <!-- Bearer Token -->
              <template v-if="authType === 'bearer'">
                <div class="space-y-2">
                  <label class="text-sm font-medium">Token</label>
                  <input
                    v-model="bearerToken"
                    type="text"
                    placeholder="Token or {{VARIABLE}}"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm font-medium">Prefix</label>
                  <input
                    v-model="bearerPrefix"
                    type="text"
                    placeholder="Bearer"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                  />
                </div>
              </template>

              <!-- API Key -->
              <template v-if="authType === 'api-key'">
                <div class="space-y-2">
                  <label class="text-sm font-medium">Key Name</label>
                  <input
                    v-model="apiKeyName"
                    type="text"
                    placeholder="X-API-Key"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm font-medium">Key Value</label>
                  <input
                    v-model="apiKeyValue"
                    type="text"
                    placeholder="Value or {{VARIABLE}}"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
                  />
                </div>
                <div class="space-y-2">
                  <label class="text-sm font-medium">Add To</label>
                  <select
                    v-model="apiKeyIn"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm"
                  >
                    <option value="header">Header</option>
                    <option value="query">Query Parameter</option>
                  </select>
                </div>
              </template>
            </div>

            <div v-if="authType === 'inherit'" class="rounded-md bg-muted p-4 text-sm text-muted-foreground">
              <Icon name="lucide:info" class="inline h-4 w-4 mr-2" />
              Authentication will be inherited from the parent folder or collection.
            </div>
          </div>

          <!-- Headers Tab -->
          <div v-if="activeSettingsTab === 'headers'" class="space-y-3">
            <p class="text-sm text-muted-foreground">
              Headers defined here will be automatically added to all requests in this {{ isCollectionLevel ? 'collection' : 'folder' }}.
            </p>
            <div
              v-for="header in headers"
              :key="header.id"
              class="flex items-center gap-2"
            >
              <input
                type="checkbox"
                :checked="header.enabled"
                class="h-4 w-4 rounded border-input"
                @change="header.enabled = !header.enabled"
              />
              <input
                v-model="header.key"
                type="text"
                placeholder="Header name"
                class="flex-1 h-10 rounded-md border border-input bg-background px-3 text-sm"
              />
              <input
                v-model="header.value"
                type="text"
                placeholder="Value or {{VARIABLE}}"
                class="flex-1 h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
              />
              <button
                class="p-2 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive"
                @click="removeHeader(header.id)"
              >
                <Icon name="lucide:trash-2" class="h-4 w-4" />
              </button>
            </div>
            <button
              class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
              @click="addHeader"
            >
              <Icon name="lucide:plus" class="h-4 w-4" />
              Add Header
            </button>
          </div>

          <!-- Variables Tab -->
          <div v-if="activeSettingsTab === 'variables'" class="space-y-3">
            <p class="text-sm text-muted-foreground">
              Variables defined here are scoped to this {{ isCollectionLevel ? 'collection' : 'folder' }} and can override global/environment variables.
            </p>
            <div
              v-for="variable in variables"
              :key="variable.id"
              class="flex items-center gap-2"
            >
              <input
                type="checkbox"
                :checked="variable.enabled"
                class="h-4 w-4 rounded border-input"
                @change="variable.enabled = !variable.enabled"
              />
              <input
                v-model="variable.key"
                type="text"
                placeholder="Variable name"
                class="flex-1 h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
              />
              <input
                v-model="variable.value"
                :type="variable.isSecret ? 'password' : 'text'"
                placeholder="Value"
                class="flex-1 h-10 rounded-md border border-input bg-background px-3 text-sm"
              />
              <button
                :class="[
                  'p-2 rounded-md',
                  variable.isSecret ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
                ]"
                title="Toggle secret"
                @click="variable.isSecret = !variable.isSecret"
              >
                <Icon :name="variable.isSecret ? 'lucide:eye-off' : 'lucide:eye'" class="h-4 w-4" />
              </button>
              <button
                class="p-2 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive"
                @click="removeVariable(variable.id)"
              >
                <Icon name="lucide:trash-2" class="h-4 w-4" />
              </button>
            </div>
            <button
              class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
              @click="addVariable"
            >
              <Icon name="lucide:plus" class="h-4 w-4" />
              Add Variable
            </button>
          </div>

          <!-- General Tab -->
          <div v-if="activeSettingsTab === 'general'" class="space-y-4">
            <div class="space-y-2">
              <label class="text-sm font-medium">Base URL (Prefix)</label>
              <input
                v-model="baseUrl"
                type="text"
                placeholder="https://api.example.com/v1"
                class="w-full h-10 rounded-md border border-input bg-background px-3 text-sm font-mono"
              />
              <p class="text-xs text-muted-foreground">
                This URL will be prepended to all request URLs in this {{ isCollectionLevel ? 'collection' : 'folder' }}.
              </p>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div class="flex justify-end gap-3 border-t border-border px-6 py-4">
          <UiButton variant="outline" @click="close">
            Cancel
          </UiButton>
          <UiButton @click="handleSave">
            Save Settings
          </UiButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>
