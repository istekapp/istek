<script setup lang="ts">
import type { Variable, Environment, SecretProviderConfig, SecretProviderType, PlaygroundStatus } from '~/types'
import { invoke } from '@tauri-apps/api/core'

const variableStore = useVariableStore()
const {
  globalVariables,
  environments,
  activeEnvironmentId,
  activeEnvironment,
  secretProviders,
  showVariableManager,
  variableManagerTab,
  resolvedVariables,
  appTheme,
} = variableStore

// Theme handling
const applyTheme = (theme: 'dark' | 'light' | 'system') => {
  appTheme.value = theme
  
  if (theme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    document.documentElement.classList.toggle('dark', prefersDark)
  } else {
    document.documentElement.classList.toggle('dark', theme === 'dark')
  }
}

// Local state
const newEnvName = ref('')
const showNewEnvInput = ref(false)
const editingEnvId = ref<string | null>(null)
const editingEnvName = ref('')

// Secret provider form
const showProviderForm = ref(false)
const providerFormType = ref<SecretProviderType>('vault')
const providerFormName = ref('')
const providerFormConfig = ref<any>({})

// Test state for variables
const testingVariableId = ref<string | null>(null)
const variableTestResults = ref<Map<string, { success: boolean; message: string }>>(new Map())

// Test state for integrations  
const testingProviderId = ref<string | null>(null)
const providerTestResults = ref<Map<string, { success: boolean; message: string }>>(new Map())
const providerFormTestResult = ref<{ success: boolean; message: string } | null>(null)
const testingProviderForm = ref(false)

// Playground state
const playgroundStatus = ref<PlaygroundStatus | null>(null)
const playgroundLoading = ref(false)
const playgroundError = ref<string | null>(null)

// Load playground status when tab changes
watch(variableManagerTab, async (tab) => {
  if (tab === 'playground') {
    await loadPlaygroundStatus()
  }
})

const loadPlaygroundStatus = async () => {
  try {
    playgroundStatus.value = await invoke<PlaygroundStatus>('playground_status')
  } catch (e: any) {
    console.error('Failed to load playground status:', e)
  }
}

const togglePlayground = async () => {
  playgroundLoading.value = true
  playgroundError.value = null

  try {
    if (playgroundStatus.value?.running) {
      await invoke('playground_stop')
    } else {
      await invoke<PlaygroundStatus>('playground_start')
    }
    await loadPlaygroundStatus()
  } catch (e: any) {
    playgroundError.value = e.toString()
    console.error('Playground error:', e)
  } finally {
    playgroundLoading.value = false
  }
}

const copyPlaygroundUrl = async (url: string) => {
  try {
    await navigator.clipboard.writeText(url)
  } catch (e) {
    console.error('Failed to copy:', e)
  }
}

const playgroundEndpoints = computed(() => {
  if (!playgroundStatus.value?.running) return []
  
  return [
    { name: 'HTTP API', url: playgroundStatus.value.httpUrl, icon: 'lucide:globe', color: 'text-blue-500', bgColor: 'bg-blue-500/10' },
    { name: 'WebSocket', url: playgroundStatus.value.wsUrl, icon: 'lucide:radio', color: 'text-green-500', bgColor: 'bg-green-500/10' },
    { name: 'GraphQL', url: playgroundStatus.value.graphqlUrl, icon: 'lucide:hexagon', color: 'text-pink-500', bgColor: 'bg-pink-500/10' },
    { name: 'SSE', url: playgroundStatus.value.sseUrl, icon: 'lucide:activity', color: 'text-orange-400', bgColor: 'bg-orange-500/10' },
    { name: 'MQTT', url: playgroundStatus.value.mqttUrl, icon: 'lucide:radio-tower', color: 'text-purple-500', bgColor: 'bg-purple-500/10' },
    { name: 'gRPC', url: playgroundStatus.value.grpcUrl, icon: 'lucide:cpu', color: 'text-amber-500', bgColor: 'bg-amber-500/10' },
    { name: 'Unix Socket', url: playgroundStatus.value.unixSocket, icon: 'lucide:plug', color: 'text-gray-500', bgColor: 'bg-gray-500/10' },
    { name: 'OpenAPI Spec', url: playgroundStatus.value.openapiUrl, icon: 'lucide:file-json', color: 'text-cyan-500', bgColor: 'bg-cyan-500/10' },
  ].filter(e => e.url)
})

const closeModal = () => {
  variableStore.closeVariableManager()
}

// Check if a global variable is overridden in current environment
const isOverridden = (key: string) => {
  if (!activeEnvironment.value) return false
  return activeEnvironment.value.variables.some(v => v.key === key && v.enabled)
}

// Get the resolved value for a variable key
const getResolvedValue = (key: string) => {
  return resolvedVariables.value.get(key)?.value || ''
}

// Create override from global variable
const createOverrideFromGlobal = (globalVar: Variable) => {
  if (!activeEnvironment.value) return
  variableStore.addEnvironmentVariable(activeEnvironment.value.id, {
    key: globalVar.key,
    value: globalVar.value,
    isSecret: globalVar.isSecret,
  })
}

// Environment actions
const createEnvironment = () => {
  if (newEnvName.value.trim()) {
    variableStore.addEnvironment(newEnvName.value.trim())
    newEnvName.value = ''
    showNewEnvInput.value = false
  }
}

const startEditEnv = (env: Environment) => {
  editingEnvId.value = env.id
  editingEnvName.value = env.name
}

const saveEnvName = () => {
  if (editingEnvId.value && editingEnvName.value.trim()) {
    variableStore.updateEnvironment(editingEnvId.value, { name: editingEnvName.value.trim() })
  }
  editingEnvId.value = null
  editingEnvName.value = ''
}

// Handle shareable toggle for Git Sync
const syncStore = useSyncStore()
const handleShareableChange = async (envId: string, shareable: boolean) => {
  try {
    await syncStore.setEnvironmentShareable(envId, shareable)
    // Update local state
    const env = environments.value.find(e => e.id === envId)
    if (env) {
      variableStore.updateEnvironment(envId, { shareable })
    }
  } catch (e) {
    console.error('Failed to update shareable status:', e)
  }
}

// Provider form
const resetProviderForm = () => {
  providerFormType.value = 'vault'
  providerFormName.value = ''
  providerFormConfig.value = {}
  showProviderForm.value = false
}

const saveProvider = () => {
  if (!providerFormName.value.trim()) return
  
  variableStore.addSecretProvider({
    name: providerFormName.value.trim(),
    type: providerFormType.value,
    enabled: true,
    config: providerFormConfig.value,
  })
  resetProviderForm()
}

const getProviderIcon = (type: SecretProviderType) => {
  switch (type) {
    case 'vault': return 'lucide:vault'
    case '1password': return 'lucide:key-round'
    case 'bitwarden': return 'lucide:shield'
    case 'aws': return 'logos:aws'
    case 'gcp': return 'logos:google-cloud'
    case 'azure': return 'logos:microsoft-azure'
    default: return 'lucide:variable'
  }
}

const getProviderLabel = (type: SecretProviderType) => {
  switch (type) {
    case 'vault': return 'HashiCorp Vault'
    case '1password': return '1Password'
    case 'bitwarden': return 'Bitwarden'
    case 'aws': return 'AWS Secrets Manager'
    case 'gcp': return 'GCP Secret Manager'
    case 'azure': return 'Azure Key Vault'
    default: return 'Manual'
  }
}

const getProviderById = (id: string) => {
  return secretProviders.value.find(p => p.id === id)
}

const handleSourceChange = (variableId: string, source: string) => {
  if (source === 'manual') {
    variableStore.updateGlobalVariable(variableId, { secretProvider: undefined })
  } else {
    variableStore.updateGlobalVariable(variableId, { 
      secretProvider: { providerId: source, secretPath: '', secretKey: '' },
      isSecret: true 
    })
  }
}

const updateSecretProvider = (variableId: string, updates: Partial<{ secretPath: string; secretKey: string }>) => {
  const variable = globalVariables.value.find(v => v.id === variableId)
  if (variable?.secretProvider) {
    variableStore.updateGlobalVariable(variableId, {
      secretProvider: { ...variable.secretProvider, ...updates }
    })
  }
}

// Test a variable's secret path/key
const testVariableSecret = async (variable: Variable) => {
  if (!variable.secretProvider) return
  
  const provider = getProviderById(variable.secretProvider.providerId)
  if (!provider) {
    variableTestResults.value.set(variable.id, { 
      success: false, 
      message: 'Provider not found' 
    })
    return
  }

  testingVariableId.value = variable.id
  variableTestResults.value.delete(variable.id)

  try {
    const config = buildProviderConfig(provider, variable.secretProvider.secretPath, variable.secretProvider.secretKey)
    const result = await invoke<{ success: boolean; secrets: any[]; error?: string }>('test_secret_provider_connection', {
      provider: provider.type,
      config
    })

    if (result.success && result.secrets.length > 0) {
      const foundKey = result.secrets.find(s => s.key === variable.secretProvider?.secretKey)
      if (foundKey) {
        variableTestResults.value.set(variable.id, { 
          success: true, 
          message: 'Secret found!' 
        })
      } else {
        variableTestResults.value.set(variable.id, { 
          success: false, 
          message: `Key "${variable.secretProvider.secretKey}" not found in secret. Available: ${result.secrets.map(s => s.key).join(', ')}` 
        })
      }
    } else {
      variableTestResults.value.set(variable.id, { 
        success: false, 
        message: result.error || 'Secret not found' 
      })
    }
  } catch (err: any) {
    variableTestResults.value.set(variable.id, { 
      success: false, 
      message: err.toString() 
    })
  } finally {
    testingVariableId.value = null
  }
}

// Test provider integration connection (auth only, no secret fetch)
const testProviderConnection = async (provider: SecretProviderConfig) => {
  testingProviderId.value = provider.id
  providerTestResults.value.delete(provider.id)

  try {
    const config = buildProviderConfig(provider, '', '')
    console.log('=== DEBUG test_provider_auth ===')
    console.log('provider.type:', provider.type)
    console.log('config:', JSON.stringify(config, null, 2))
    const result = await invoke<{ success: boolean; message: string; identity?: string }>('test_provider_auth', {
      provider: provider.type,
      config
    })

    if (result.success) {
      providerTestResults.value.set(provider.id, { 
        success: true, 
        message: result.identity ? `Connected as ${result.identity}` : 'Connection successful!'
      })
    } else {
      providerTestResults.value.set(provider.id, { 
        success: false, 
        message: result.message || 'Connection failed' 
      })
    }
  } catch (err: any) {
    providerTestResults.value.set(provider.id, { 
      success: false, 
      message: err.toString() 
    })
  } finally {
    testingProviderId.value = null
  }
}

// Test provider form before saving (auth only)
const testProviderForm = async () => {
  testingProviderForm.value = true
  providerFormTestResult.value = null

  try {
    const config = buildProviderConfigFromForm()
    const result = await invoke<{ success: boolean; message: string; identity?: string }>('test_provider_auth', {
      provider: providerFormType.value,
      config
    })

    if (result.success) {
      providerFormTestResult.value = { 
        success: true, 
        message: result.identity ? `Connected as ${result.identity}` : 'Connection successful!'
      }
    } else {
      providerFormTestResult.value = { success: false, message: result.message || 'Connection failed' }
    }
  } catch (err: any) {
    providerFormTestResult.value = { success: false, message: err.toString() }
  } finally {
    testingProviderForm.value = false
  }
}

// Build config object for Tauri command
const buildProviderConfig = (provider: SecretProviderConfig, secretPath: string, secretKey: string) => {
  const config = provider.config || {}
  
  switch (provider.type) {
    case 'aws':
      return {
        awsRegion: config.region,
        awsAccessKeyId: config.accessKeyId,
        awsSecretAccessKey: config.secretAccessKey,
        awsSecretName: secretPath,
      }
    case 'gcp':
      return {
        gcpProjectId: config.projectId,
        gcpCredentialsJson: config.credentialsJson,
        gcpSecretName: secretPath,
      }
    case 'azure':
      return {
        azureVaultUrl: config.vaultUrl,
        azureTenantId: config.tenantId,
        azureClientId: config.clientId,
        azureClientSecret: config.clientSecret,
        azureSecretName: secretPath,
      }
    case 'vault':
      return {
        vaultAddress: config.address,
        vaultToken: config.token,
        vaultMountPath: config.mountPath,
        vaultNamespace: config.namespace,
        vaultSecretPath: secretPath,
      }
    case '1password':
      return {
        onepasswordServiceAccountToken: config.serviceAccountToken,
        onepasswordVaultId: config.vaultId,
        onepasswordItemName: secretPath,
      }
    case 'bitwarden':
      return {
        bitwardenServerUrl: config.serverUrl,
        bitwardenApiKey: config.apiKey,
        bitwardenOrganizationId: config.organizationId,
        bitwardenItemName: secretPath,
      }
    default:
      return {}
  }
}

// Build config from form inputs
const buildProviderConfigFromForm = () => {
  const config = providerFormConfig.value
  
  switch (providerFormType.value) {
    case 'aws':
      return {
        awsRegion: config.region,
        awsAccessKeyId: config.accessKeyId,
        awsSecretAccessKey: config.secretAccessKey,
        awsSecretName: 'test',
      }
    case 'gcp':
      return {
        gcpProjectId: config.projectId,
        gcpCredentialsJson: config.credentialsJson,
        gcpSecretName: 'test',
      }
    case 'azure':
      return {
        azureVaultUrl: config.vaultUrl,
        azureTenantId: config.tenantId,
        azureClientId: config.clientId,
        azureClientSecret: config.clientSecret,
        azureSecretName: 'test',
      }
    case 'vault':
      return {
        vaultAddress: config.address,
        vaultToken: config.token,
        vaultMountPath: config.mountPath,
        vaultNamespace: config.namespace,
        vaultSecretPath: 'test',
      }
    case '1password':
      return {
        onepasswordServiceAccountToken: config.serviceAccountToken,
        onepasswordVaultId: config.vaultId,
        onepasswordItemName: 'test',
      }
    case 'bitwarden':
      return {
        bitwardenServerUrl: config.serverUrl,
        bitwardenApiKey: config.apiKey,
        bitwardenOrganizationId: config.organizationId,
        bitwardenItemName: 'test',
      }
    default:
      return {}
  }
}

// Clear test result after timeout
const clearVariableTestResult = (variableId: string) => {
  setTimeout(() => {
    variableTestResults.value.delete(variableId)
  }, 5000)
}

// Watch for test results and auto-clear
watch(variableTestResults, (results) => {
  results.forEach((_, id) => {
    clearVariableTestResult(id)
  })
}, { deep: true })

</script>

<template>
  <Teleport to="body">
    <div
      v-if="showVariableManager"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      @click.self="closeModal"
    >
      <div class="w-[950px] h-[85vh] rounded-lg border border-border bg-card shadow-2xl flex flex-col">
        <!-- Header -->
        <div class="flex items-center justify-between border-b border-border px-6 py-4">
          <div class="flex items-center gap-3">
            <Icon name="lucide:settings" class="h-6 w-6 text-primary" />
            <h2 class="text-xl font-semibold">Settings</h2>
          </div>
          <UiButton variant="ghost" size="icon" @click="closeModal">
            <Icon name="lucide:x" class="h-5 w-5" />
          </UiButton>
        </div>

        <!-- Tabs -->
        <div class="flex border-b border-border px-6">
          <button
            v-for="tab in [
              { id: 'general', label: 'General', icon: 'lucide:sliders-horizontal' },
              { id: 'variables', label: 'Variables', icon: 'lucide:variable' },
              { id: 'environments', label: 'Environments', icon: 'lucide:layers' },
              { id: 'integrations', label: 'Integrations', icon: 'lucide:plug-zap' },
              { id: 'playground', label: 'Playground', icon: 'lucide:play-circle' },
            ]"
            :key="tab.id"
            :class="[
              'flex items-center gap-2 px-4 py-3 text-base font-medium transition-colors border-b-2 -mb-[2px]',
              variableManagerTab === tab.id
                ? 'border-primary text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            ]"
            @click="variableManagerTab = tab.id as any"
          >
            <Icon :name="tab.icon" class="h-5 w-5" />
            {{ tab.label }}
          </button>
        </div>

        <!-- Content -->
        <div class="flex-1 overflow-auto p-6">
          <!-- General Tab -->
          <div v-if="variableManagerTab === 'general'" class="space-y-8">
            <!-- Theme Section -->
            <div>
              <h3 class="text-lg font-semibold mb-1">Appearance</h3>
              <p class="text-sm text-muted-foreground mb-4">Customize how Istek looks</p>
              
              <!-- Theme -->
              <div class="flex items-center justify-between p-4 rounded-lg border border-border bg-background">
                <div class="flex items-center gap-3">
                  <div class="h-10 w-10 rounded-lg bg-muted flex items-center justify-center">
                    <Icon name="lucide:palette" class="h-5 w-5 text-muted-foreground" />
                  </div>
                  <div>
                    <div class="font-medium">Theme</div>
                    <div class="text-sm text-muted-foreground">Choose your preferred color scheme</div>
                  </div>
                </div>
                <div class="flex items-center gap-1 p-1 rounded-lg bg-muted">
                  <button
                    :class="[
                      'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                      appTheme === 'light' ? 'bg-background shadow-sm' : 'hover:bg-background/50'
                    ]"
                    @click="applyTheme('light')"
                  >
                    <Icon name="lucide:sun" class="h-4 w-4 inline mr-1.5" />
                    Light
                  </button>
                  <button
                    :class="[
                      'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                      appTheme === 'dark' ? 'bg-background shadow-sm' : 'hover:bg-background/50'
                    ]"
                    @click="applyTheme('dark')"
                  >
                    <Icon name="lucide:moon" class="h-4 w-4 inline mr-1.5" />
                    Dark
                  </button>
                  <button
                    :class="[
                      'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                      appTheme === 'system' ? 'bg-background shadow-sm' : 'hover:bg-background/50'
                    ]"
                    @click="applyTheme('system')"
                  >
                    <Icon name="lucide:monitor" class="h-4 w-4 inline mr-1.5" />
                    System
                  </button>
                </div>
              </div>
            </div>

            <!-- Keyboard Shortcuts Section -->
            <div>
              <h3 class="text-lg font-semibold mb-1">Keyboard Shortcuts</h3>
              <p class="text-sm text-muted-foreground mb-4">Quick access to common actions</p>
              
              <div class="grid grid-cols-2 gap-3">
                <div class="flex items-center justify-between p-3 rounded-lg bg-muted/50">
                  <span class="text-sm">Search Everywhere</span>
                  <div class="flex items-center gap-1 text-xs font-mono">
                    <kbd class="px-2 py-1 rounded bg-background border border-border">⌘</kbd>
                    <span>+</span>
                    <kbd class="px-2 py-1 rounded bg-background border border-border">K</kbd>
                  </div>
                </div>
                <div class="flex items-center justify-between p-3 rounded-lg bg-muted/50">
                  <span class="text-sm">Zoom In</span>
                  <div class="flex items-center gap-1 text-xs font-mono">
                    <kbd class="px-2 py-1 rounded bg-background border border-border">⌘</kbd>
                    <span>+</span>
                    <kbd class="px-2 py-1 rounded bg-background border border-border">+</kbd>
                  </div>
                </div>
                <div class="flex items-center justify-between p-3 rounded-lg bg-muted/50">
                  <span class="text-sm">Zoom Out</span>
                  <div class="flex items-center gap-1 text-xs font-mono">
                    <kbd class="px-2 py-1 rounded bg-background border border-border">⌘</kbd>
                    <span>+</span>
                    <kbd class="px-2 py-1 rounded bg-background border border-border">-</kbd>
                  </div>
                </div>
                <div class="flex items-center justify-between p-3 rounded-lg bg-muted/50">
                  <span class="text-sm">Reset Zoom</span>
                  <div class="flex items-center gap-1 text-xs font-mono">
                    <kbd class="px-2 py-1 rounded bg-background border border-border">⌘</kbd>
                    <span>+</span>
                    <kbd class="px-2 py-1 rounded bg-background border border-border">0</kbd>
                  </div>
                </div>
              </div>
            </div>

            <!-- About Section -->
            <div>
              <h3 class="text-lg font-semibold mb-1">About</h3>
              <p class="text-sm text-muted-foreground mb-4">Application information</p>
              
              <div class="p-4 rounded-lg border border-border bg-background">
                <div class="flex items-center gap-4">
                  <div class="h-16 w-16 rounded-xl bg-primary/10 flex items-center justify-center">
                    <Icon name="lucide:send" class="h-8 w-8 text-primary" />
                  </div>
                  <div>
                    <div class="text-xl font-bold">Istek</div>
                    <div class="text-sm text-muted-foreground">API Client for Developers</div>
                    <div class="text-xs text-muted-foreground mt-1">Version 0.1.0</div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Variables Tab -->
          <div v-else-if="variableManagerTab === 'variables'" class="space-y-6">
            <!-- Environment Selector Bar -->
            <div class="flex items-center justify-between p-4 rounded-lg bg-muted/50">
              <div class="flex items-center gap-4">
                <span class="text-base font-medium">Environment:</span>
                <div class="flex gap-2">
                  <button
                    v-for="env in environments"
                    :key="env.id"
                    :class="[
                      'px-4 py-2 rounded-md text-sm font-medium transition-all',
                      activeEnvironmentId === env.id
                        ? 'ring-2 ring-offset-2 ring-offset-card ring-primary shadow-sm'
                        : 'hover:opacity-80'
                    ]"
                    :style="{ backgroundColor: env.color + '30', color: env.color }"
                    @click="variableStore.setActiveEnvironment(env.id)"
                  >
                    {{ env.name }}
                    <span v-if="env.variables.length > 0" class="ml-1 opacity-70">({{ env.variables.length }})</span>
                  </button>
                </div>
              </div>
              <div class="text-sm text-muted-foreground">
                {{ resolvedVariables.size }} total variables
              </div>
            </div>

            <!-- How it works info -->
            <div class="flex items-start gap-3 p-4 rounded-lg border border-primary/30 bg-primary/5">
              <Icon name="lucide:info" class="h-5 w-5 text-primary mt-0.5" />
              <div class="text-sm">
                <p class="font-medium text-foreground">How variables work:</p>
                <p class="text-muted-foreground mt-1">
                  Define <strong>Global Variables</strong> as your defaults. Then add <strong>Environment Overrides</strong> only for values that differ per environment (e.g., API_URL might be different in Production vs Development).
                </p>
              </div>
            </div>

            <!-- Global Variables (Base/Default) -->
            <div>
              <div class="flex items-center justify-between mb-4">
                <div>
                  <h3 class="text-lg font-semibold flex items-center gap-2">
                    <Icon name="lucide:globe" class="h-5 w-5 text-muted-foreground" />
                    Global Variables
                    <span class="text-sm font-normal text-muted-foreground">(defaults)</span>
                  </h3>
                  <p class="text-sm text-muted-foreground">Base values used in all environments unless overridden</p>
                </div>
                <UiButton @click="variableStore.addGlobalVariable()">
                  <Icon name="lucide:plus" class="mr-2 h-4 w-4" />
                  Add Variable
                </UiButton>
              </div>
              
              <div v-if="globalVariables.length === 0" class="text-center py-8 text-muted-foreground border border-dashed border-border rounded-lg">
                <Icon name="lucide:variable" class="mx-auto h-12 w-12 opacity-50 mb-3" />
                <p>No global variables defined</p>
                <p class="text-sm">Start by adding variables like API_URL, AUTH_TOKEN, etc.</p>
              </div>
              
              <div v-else class="space-y-3">
                <div
                  v-for="variable in globalVariables"
                  :key="variable.id"
                  :class="[
                    'p-4 rounded-lg border bg-background transition-all',
                    isOverridden(variable.key) ? 'border-dashed border-muted-foreground/30 opacity-60' : 'border-border'
                  ]"
                >
                  <!-- Main row -->
                  <div class="flex items-center gap-3">
                    <input
                      type="checkbox"
                      :checked="variable.enabled"
                      class="h-5 w-5 rounded border-input accent-primary"
                      @change="variableStore.toggleGlobalVariable(variable.id)"
                    />
                    <UiInput
                      :model-value="variable.key"
                      placeholder="VARIABLE_NAME"
                      class="w-48 font-mono h-10"
                      @update:model-value="variableStore.updateGlobalVariable(variable.id, { key: $event })"
                    />
                    
                    <!-- Source selector -->
                    <select
                      :value="variable.secretProvider ? variable.secretProvider.providerId : 'manual'"
                      class="h-10 px-3 rounded-md border border-input bg-background text-sm w-40"
                      @change="handleSourceChange(variable.id, ($event.target as HTMLSelectElement).value)"
                    >
                      <option value="manual">Manual</option>
                      <option 
                        v-for="provider in secretProviders" 
                        :key="provider.id" 
                        :value="provider.id"
                        :disabled="!provider.enabled"
                      >
                        {{ provider.name }}
                      </option>
                    </select>
                    
                    <!-- Manual value input -->
                    <template v-if="!variable.secretProvider">
                      <div class="flex-1 relative">
                        <UiInput
                          :model-value="variable.value"
                          :type="variable.isSecret ? 'password' : 'text'"
                          placeholder="Enter value"
                          class="font-mono h-10 pr-10"
                          @update:model-value="variableStore.updateGlobalVariable(variable.id, { value: $event })"
                        />
                        <button
                          class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
                          @click="variableStore.updateGlobalVariable(variable.id, { isSecret: !variable.isSecret })"
                        >
                          <Icon :name="variable.isSecret ? 'lucide:eye-off' : 'lucide:eye'" class="h-4 w-4" />
                        </button>
                      </div>
                    </template>
                    
                    <!-- Provider secret inputs -->
                    <template v-else>
                      <UiInput
                        :model-value="variable.secretProvider.secretPath"
                        placeholder="secret/path"
                        class="w-36 font-mono h-10 text-sm"
                        @update:model-value="updateSecretProvider(variable.id, { secretPath: $event })"
                      />
                      <UiInput
                        :model-value="variable.secretProvider.secretKey"
                        placeholder="key"
                        class="w-28 font-mono h-10 text-sm"
                        @update:model-value="updateSecretProvider(variable.id, { secretKey: $event })"
                      />
                      <!-- Test secret button -->
                      <UiButton
                        variant="outline"
                        size="sm"
                        class="h-10 px-3"
                        :disabled="!variable.secretProvider.secretPath || !variable.secretProvider.secretKey || testingVariableId === variable.id"
                        @click="testVariableSecret(variable)"
                      >
                        <Icon 
                          v-if="testingVariableId === variable.id" 
                          name="lucide:loader-2" 
                          class="h-4 w-4 animate-spin" 
                        />
                        <Icon v-else name="lucide:play" class="h-4 w-4" />
                      </UiButton>
                    </template>
                    
                    <div class="flex items-center gap-1">
                      <!-- Test result indicator -->
                      <span
                        v-if="variableTestResults.get(variable.id)"
                        :class="[
                          'inline-flex items-center gap-1 px-2 py-1 text-xs rounded-full',
                          variableTestResults.get(variable.id)?.success 
                            ? 'bg-method-get/20 text-method-get' 
                            : 'bg-destructive/20 text-destructive'
                        ]"
                        :title="variableTestResults.get(variable.id)?.message"
                      >
                        <Icon 
                          :name="variableTestResults.get(variable.id)?.success ? 'lucide:check' : 'lucide:x'" 
                          class="h-3 w-3" 
                        />
                        {{ variableTestResults.get(variable.id)?.success ? 'OK' : 'Failed' }}
                      </span>
                      <span
                        v-else-if="isOverridden(variable.key)"
                        class="inline-flex items-center gap-1 px-2 py-1 text-xs rounded-full bg-protocol-mqtt/20 text-protocol-mqtt"
                      >
                        Overridden
                      </span>
                      <span
                        v-else-if="variable.secretProvider && !variableTestResults.get(variable.id)"
                        class="inline-flex items-center gap-1 px-2 py-1 text-xs rounded-full bg-purple-500/20 text-purple-500"
                      >
                        <Icon name="lucide:key" class="h-3 w-3" />
                        Secret
                      </span>
                      <UiButton
                        v-if="!isOverridden(variable.key) && activeEnvironment && variable.key"
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8 text-muted-foreground hover:text-foreground"
                        title="Create override for this environment"
                        @click="createOverrideFromGlobal(variable)"
                      >
                        <Icon name="lucide:copy-plus" class="h-4 w-4" />
                      </UiButton>
                      <UiButton
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8 text-muted-foreground hover:text-destructive"
                        @click="variableStore.deleteGlobalVariable(variable.id)"
                      >
                        <Icon name="lucide:trash-2" class="h-4 w-4" />
                      </UiButton>
                    </div>
                  </div>
                  
                  <!-- Provider hint + test result message -->
                  <div v-if="variable.secretProvider" class="mt-2 ml-8 flex items-center justify-between">
                    <div class="text-xs text-muted-foreground flex items-center gap-2">
                      <Icon :name="getProviderIcon(getProviderById(variable.secretProvider.providerId)?.type || 'manual')" class="h-4 w-4" />
                      <span>From {{ getProviderById(variable.secretProvider.providerId)?.name }}</span>
                      <span class="font-mono text-foreground/70">{{ variable.secretProvider.secretPath }}/{{ variable.secretProvider.secretKey }}</span>
                    </div>
                    <!-- Test result message -->
                    <div 
                      v-if="variableTestResults.get(variable.id) && !variableTestResults.get(variable.id)?.success"
                      class="text-xs text-destructive max-w-[300px] truncate"
                      :title="variableTestResults.get(variable.id)?.message"
                    >
                      {{ variableTestResults.get(variable.id)?.message }}
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Environment Overrides -->
            <div v-if="activeEnvironment">
              <div class="flex items-center justify-between mb-4">
                <div>
                  <h3 class="text-lg font-semibold flex items-center gap-2">
                    <span
                      class="w-4 h-4 rounded-full"
                      :style="{ backgroundColor: activeEnvironment.color }"
                    />
                    {{ activeEnvironment.name }} Overrides
                  </h3>
                  <p class="text-sm text-muted-foreground">Values that differ from globals in this environment</p>
                </div>
                <UiButton variant="outline" @click="variableStore.addEnvironmentVariable(activeEnvironment.id)">
                  <Icon name="lucide:plus" class="mr-2 h-4 w-4" />
                  Add Override
                </UiButton>
              </div>

              <div v-if="activeEnvironment.variables.length === 0" class="text-center py-6 text-muted-foreground border border-dashed rounded-lg" :style="{ borderColor: activeEnvironment.color + '50' }">
                <Icon name="lucide:layers" class="mx-auto h-10 w-10 opacity-50 mb-2" />
                <p>No overrides for {{ activeEnvironment.name }}</p>
                <p class="text-sm">Click the <Icon name="lucide:copy-plus" class="inline h-4 w-4" /> icon on a global variable to override it</p>
              </div>

              <div v-else class="space-y-2">
                <div
                  v-for="variable in activeEnvironment.variables"
                  :key="variable.id"
                  class="flex items-center gap-3 p-3 rounded-lg border-2"
                  :style="{ borderColor: activeEnvironment.color + '50', backgroundColor: activeEnvironment.color + '08' }"
                >
                  <input
                    type="checkbox"
                    :checked="variable.enabled"
                    class="h-5 w-5 rounded border-input accent-primary"
                    @change="variableStore.toggleEnvironmentVariable(activeEnvironment.id, variable.id)"
                  />
                  <UiInput
                    :model-value="variable.key"
                    placeholder="VARIABLE_NAME"
                    class="flex-1 font-mono h-10"
                    @update:model-value="variableStore.updateEnvironmentVariable(activeEnvironment.id, variable.id, { key: $event })"
                  />
                  <div class="flex-1 relative">
                    <UiInput
                      :model-value="variable.value"
                      :type="variable.isSecret ? 'password' : 'text'"
                      :placeholder="`Override value for ${activeEnvironment.name}`"
                      class="font-mono h-10 pr-10"
                      @update:model-value="variableStore.updateEnvironmentVariable(activeEnvironment.id, variable.id, { value: $event })"
                    />
                    <button
                      class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
                      @click="variableStore.updateEnvironmentVariable(activeEnvironment.id, variable.id, { isSecret: !variable.isSecret })"
                    >
                      <Icon :name="variable.isSecret ? 'lucide:eye-off' : 'lucide:eye'" class="h-4 w-4" />
                    </button>
                  </div>
                  <UiButton
                    variant="ghost"
                    size="icon"
                    class="text-muted-foreground hover:text-destructive"
                    @click="variableStore.deleteEnvironmentVariable(activeEnvironment.id, variable.id)"
                  >
                    <Icon name="lucide:trash-2" class="h-4 w-4" />
                  </UiButton>
                </div>
              </div>
            </div>

            <!-- Resolved Variables Preview -->
            <div v-if="resolvedVariables.size > 0" class="mt-6 pt-6 border-t border-border">
              <h3 class="text-sm font-semibold text-muted-foreground mb-3 flex items-center gap-2">
                <Icon name="lucide:eye" class="h-4 w-4" />
                Resolved Values Preview
                <span v-if="activeEnvironment" class="font-normal">({{ activeEnvironment.name }})</span>
              </h3>
              <div class="grid grid-cols-2 gap-2">
                <div
                  v-for="[key, variable] in resolvedVariables"
                  :key="key"
                  class="flex items-center gap-2 px-3 py-2 rounded bg-muted/50 font-mono text-sm"
                >
                  <span class="text-primary">{{ key }}</span>
                  <span class="text-muted-foreground">=</span>
                  <span v-if="variable.isSecret" class="text-muted-foreground">••••••••</span>
                  <span v-else class="truncate">{{ variable.value }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Environments Tab -->
          <div v-else-if="variableManagerTab === 'environments'" class="space-y-4">
            <div class="flex items-center justify-between mb-4">
              <div>
                <h3 class="text-lg font-semibold">Environments</h3>
                <p class="text-sm text-muted-foreground">Manage environments and see their override counts</p>
              </div>
              <UiButton v-if="!showNewEnvInput" @click="showNewEnvInput = true">
                <Icon name="lucide:plus" class="mr-2 h-4 w-4" />
                Add Environment
              </UiButton>
            </div>

            <!-- New Environment Input -->
            <div v-if="showNewEnvInput" class="flex items-center gap-3 p-4 rounded-lg border border-primary/50 bg-primary/5">
              <Icon name="lucide:layers" class="h-5 w-5 text-primary" />
              <UiInput
                v-model="newEnvName"
                placeholder="Environment name (e.g., QA, UAT)"
                class="flex-1 h-10"
                @keyup.enter="createEnvironment"
              />
              <UiButton @click="createEnvironment">Create</UiButton>
              <UiButton variant="ghost" @click="showNewEnvInput = false; newEnvName = ''">Cancel</UiButton>
            </div>

            <!-- Environment List -->
            <div class="space-y-3">
              <div
                v-for="env in environments"
                :key="env.id"
                class="p-4 rounded-lg border border-border"
                :style="{ borderLeftColor: env.color, borderLeftWidth: '4px' }"
              >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <div
                        class="w-4 h-4 rounded-full"
                        :style="{ backgroundColor: env.color }"
                      />
                      <template v-if="editingEnvId === env.id">
                        <UiInput
                          v-model="editingEnvName"
                          class="w-48 h-9"
                          @keyup.enter="saveEnvName"
                          @blur="saveEnvName"
                        />
                      </template>
                      <template v-else>
                        <span class="text-lg font-medium">{{ env.name }}</span>
                        <span
                          v-if="activeEnvironmentId === env.id"
                          class="px-2 py-0.5 text-xs rounded-full bg-primary/20 text-primary"
                        >
                          Active
                        </span>
                      </template>
                    </div>
                    <div class="flex items-center gap-2">
                      <!-- Shareable toggle for Git Sync -->
                      <label 
                        class="flex items-center gap-2 text-sm cursor-pointer"
                        :title="env.shareable ? 'This environment will be synced to Git (secrets excluded)' : 'Click to enable Git sync for this environment'"
                      >
                        <input
                          type="checkbox"
                          :checked="env.shareable"
                          class="h-4 w-4 rounded border-input accent-primary"
                          @change="handleShareableChange(env.id, ($event.target as HTMLInputElement).checked)"
                        />
                        <Icon name="lucide:git-branch" class="h-4 w-4" :class="env.shareable ? 'text-primary' : 'text-muted-foreground'" />
                        <span :class="env.shareable ? 'text-foreground' : 'text-muted-foreground'">Sync</span>
                      </label>
                      <span class="text-muted-foreground">|</span>
                      <span class="text-sm text-muted-foreground">
                        {{ env.variables.length }} {{ env.variables.length === 1 ? 'override' : 'overrides' }}
                      </span>
                      <UiButton
                        v-if="activeEnvironmentId !== env.id"
                        variant="outline"
                        size="sm"
                        @click="variableStore.setActiveEnvironment(env.id)"
                      >
                        Set Active
                      </UiButton>
                      <UiButton
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8"
                        title="Edit name"
                        @click="startEditEnv(env)"
                      >
                        <Icon name="lucide:pencil" class="h-4 w-4" />
                      </UiButton>
                      <UiButton
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8 text-muted-foreground hover:text-destructive"
                        title="Delete environment"
                        :disabled="environments.length <= 1"
                        @click="variableStore.deleteEnvironment(env.id)"
                      >
                        <Icon name="lucide:trash-2" class="h-4 w-4" />
                      </UiButton>
                    </div>
                  </div>

                <!-- Override preview -->
                <div v-if="env.variables.length > 0" class="mt-3 pt-3 border-t border-border">
                  <div class="text-xs text-muted-foreground mb-2">Overridden variables:</div>
                  <div class="flex flex-wrap gap-2">
                    <span
                      v-for="v in env.variables"
                      :key="v.id"
                      class="px-2 py-1 text-xs font-mono rounded"
                      :style="{ backgroundColor: env.color + '20', color: env.color }"
                    >
                      {{ v.key }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Integrations Tab -->
          <div v-else-if="variableManagerTab === 'integrations'" class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <div>
                <h3 class="text-lg font-semibold">Secret Provider Integrations</h3>
                <p class="text-sm text-muted-foreground">Connect to external secret managers to fetch sensitive values</p>
              </div>
              <UiButton v-if="!showProviderForm" @click="showProviderForm = true">
                <Icon name="lucide:plus" class="mr-2 h-4 w-4" />
                Add Integration
              </UiButton>
            </div>

            <!-- Provider Form -->
            <div v-if="showProviderForm" class="p-6 rounded-lg border border-primary/50 bg-primary/5 space-y-4">
              <h4 class="font-semibold">New Integration</h4>
              
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="text-sm font-medium mb-2 block">Provider Type</label>
                  <select
                    v-model="providerFormType"
                    class="w-full h-10 rounded-md border border-input bg-background px-3 text-base"
                  >
                    <option value="vault">HashiCorp Vault</option>
                    <option value="1password">1Password</option>
                    <option value="bitwarden">Bitwarden</option>
                    <option value="aws">AWS Secrets Manager</option>
                    <option value="gcp">GCP Secret Manager</option>
                    <option value="azure">Azure Key Vault</option>
                  </select>
                </div>
                <div>
                  <label class="text-sm font-medium mb-2 block">Name</label>
                  <UiInput v-model="providerFormName" placeholder="My Vault" class="h-10" />
                </div>
              </div>

              <!-- Vault Config -->
              <template v-if="providerFormType === 'vault'">
                <div class="grid grid-cols-2 gap-4">
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Vault Address</label>
                    <UiInput
                      v-model="providerFormConfig.address"
                      placeholder="https://vault.example.com"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Token</label>
                    <UiInput
                      v-model="providerFormConfig.token"
                      type="password"
                      placeholder="hvs.xxxxx"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Mount Path</label>
                    <UiInput
                      v-model="providerFormConfig.mountPath"
                      placeholder="secret"
                      class="h-10"
                    />
                  </div>
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Namespace (optional)</label>
                    <UiInput
                      v-model="providerFormConfig.namespace"
                      placeholder="admin"
                      class="h-10"
                    />
                  </div>
                </div>
              </template>

              <!-- 1Password Config -->
              <template v-else-if="providerFormType === '1password'">
                <div class="grid grid-cols-2 gap-4">
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Service Account Token</label>
                    <UiInput
                      v-model="providerFormConfig.serviceAccountToken"
                      type="password"
                      placeholder="ops_xxxxx"
                      class="h-10"
                    />
                  </div>
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Vault ID</label>
                    <UiInput
                      v-model="providerFormConfig.vaultId"
                      placeholder="xxxxx"
                      class="h-10"
                    />
                  </div>
                </div>
              </template>

              <!-- Bitwarden Config -->
              <template v-else-if="providerFormType === 'bitwarden'">
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="text-sm font-medium mb-2 block">Server URL</label>
                    <UiInput
                      v-model="providerFormConfig.serverUrl"
                      placeholder="https://bitwarden.example.com"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">API Key</label>
                    <UiInput
                      v-model="providerFormConfig.apiKey"
                      type="password"
                      placeholder="xxxxx"
                      class="h-10"
                    />
                  </div>
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Organization ID (optional)</label>
                    <UiInput
                      v-model="providerFormConfig.organizationId"
                      placeholder="xxxxx"
                      class="h-10"
                    />
                  </div>
                </div>
              </template>

              <!-- AWS Secrets Manager Config -->
              <template v-else-if="providerFormType === 'aws'">
                <div class="grid grid-cols-2 gap-4">
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Region</label>
                    <UiInput
                      v-model="providerFormConfig.region"
                      placeholder="us-east-1"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Access Key ID</label>
                    <UiInput
                      v-model="providerFormConfig.accessKeyId"
                      placeholder="AKIA..."
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Secret Access Key</label>
                    <UiInput
                      v-model="providerFormConfig.secretAccessKey"
                      type="password"
                      placeholder="••••••••"
                      class="h-10"
                    />
                  </div>
                </div>
              </template>

              <!-- GCP Secret Manager Config -->
              <template v-else-if="providerFormType === 'gcp'">
                <div class="grid grid-cols-2 gap-4">
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Project ID</label>
                    <UiInput
                      v-model="providerFormConfig.projectId"
                      placeholder="my-project-123"
                      class="h-10"
                    />
                  </div>
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Service Account JSON</label>
                    <textarea
                      v-model="providerFormConfig.credentialsJson"
                      class="w-full h-24 px-3 py-2 text-sm rounded-md border border-input bg-background font-mono resize-none"
                      placeholder='{"type": "service_account", ...}'
                    />
                  </div>
                </div>
              </template>

              <!-- Azure Key Vault Config -->
              <template v-else-if="providerFormType === 'azure'">
                <div class="grid grid-cols-2 gap-4">
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Key Vault URL</label>
                    <UiInput
                      v-model="providerFormConfig.vaultUrl"
                      placeholder="https://my-vault.vault.azure.net"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Tenant ID</label>
                    <UiInput
                      v-model="providerFormConfig.tenantId"
                      placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                      class="h-10"
                    />
                  </div>
                  <div>
                    <label class="text-sm font-medium mb-2 block">Client ID</label>
                    <UiInput
                      v-model="providerFormConfig.clientId"
                      placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                      class="h-10"
                    />
                  </div>
                  <div class="col-span-2">
                    <label class="text-sm font-medium mb-2 block">Client Secret</label>
                    <UiInput
                      v-model="providerFormConfig.clientSecret"
                      type="password"
                      placeholder="••••••••"
                      class="h-10"
                    />
                  </div>
                </div>
              </template>

              <!-- Test result for form -->
              <div 
                v-if="providerFormTestResult" 
                :class="[
                  'flex items-center gap-2 p-3 rounded-lg text-sm',
                  providerFormTestResult.success 
                    ? 'bg-method-get/10 text-method-get border border-method-get/30' 
                    : 'bg-destructive/10 text-destructive border border-destructive/30'
                ]"
              >
                <Icon 
                  :name="providerFormTestResult.success ? 'lucide:check-circle' : 'lucide:x-circle'" 
                  class="h-5 w-5 flex-shrink-0" 
                />
                <span>{{ providerFormTestResult.message }}</span>
              </div>

              <div class="flex justify-end gap-3 pt-4">
                <UiButton variant="outline" @click="resetProviderForm">Cancel</UiButton>
                <UiButton 
                  variant="outline" 
                  :disabled="testingProviderForm"
                  @click="testProviderForm"
                >
                  <Icon 
                    v-if="testingProviderForm" 
                    name="lucide:loader-2" 
                    class="mr-2 h-4 w-4 animate-spin" 
                  />
                  <Icon v-else name="lucide:plug" class="mr-2 h-4 w-4" />
                  Test Connection
                </UiButton>
                <UiButton @click="saveProvider">Add Integration</UiButton>
              </div>
            </div>

            <!-- Provider List -->
            <div v-if="secretProviders.length === 0 && !showProviderForm" class="text-center py-12 text-muted-foreground">
              <Icon name="lucide:plug-zap" class="mx-auto h-16 w-16 opacity-50 mb-4" />
              <p class="text-lg">No integrations configured</p>
              <p class="text-sm mt-2">Connect to secret managers to securely fetch sensitive values</p>
              <div class="flex flex-wrap justify-center gap-3 mt-6">
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-muted/50">
                  <Icon name="lucide:vault" class="h-5 w-5 text-protocol-mqtt" />
                  <span>HashiCorp Vault</span>
                </div>
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-muted/50">
                  <Icon name="lucide:key-round" class="h-5 w-5 text-protocol-ws" />
                  <span>1Password</span>
                </div>
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-muted/50">
                  <Icon name="lucide:shield" class="h-5 w-5 text-protocol-http" />
                  <span>Bitwarden</span>
                </div>
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-orange-500/10">
                  <Icon name="logos:aws" class="h-5 w-5" />
                  <span>AWS</span>
                </div>
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-500/10">
                  <Icon name="logos:google-cloud" class="h-5 w-5" />
                  <span>GCP</span>
                </div>
                <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-cyan-500/10">
                  <Icon name="logos:microsoft-azure" class="h-5 w-5" />
                  <span>Azure</span>
                </div>
              </div>
            </div>

            <div v-else class="space-y-3">
              <div
                v-for="provider in secretProviders"
                :key="provider.id"
                class="p-4 rounded-lg border border-border"
              >
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-4">
                    <div class="h-12 w-12 rounded-lg bg-muted flex items-center justify-center">
                      <Icon :name="getProviderIcon(provider.type)" class="h-6 w-6" />
                    </div>
                    <div>
                      <h4 class="font-medium">{{ provider.name }}</h4>
                      <p class="text-sm text-muted-foreground">{{ getProviderLabel(provider.type) }}</p>
                    </div>
                  </div>
                  <div class="flex items-center gap-3">
                    <!-- Test result indicator -->
                    <span
                      v-if="providerTestResults.get(provider.id)"
                      :class="[
                        'px-2 py-1 text-xs rounded-full',
                        providerTestResults.get(provider.id)?.success
                          ? 'bg-method-get/20 text-method-get'
                          : 'bg-destructive/20 text-destructive'
                      ]"
                    >
                      {{ providerTestResults.get(provider.id)?.success ? 'Connected' : 'Failed' }}
                    </span>
                    <span
                      v-else
                      :class="[
                        'px-2 py-1 text-xs rounded-full',
                        provider.enabled
                          ? 'bg-method-get/20 text-method-get'
                          : 'bg-muted text-muted-foreground'
                      ]"
                    >
                      {{ provider.enabled ? 'Enabled' : 'Disabled' }}
                    </span>
                    <!-- Test connection button -->
                    <UiButton
                      variant="outline"
                      size="sm"
                      class="h-8"
                      :disabled="testingProviderId === provider.id || !provider.enabled"
                      @click="testProviderConnection(provider)"
                    >
                      <Icon 
                        v-if="testingProviderId === provider.id" 
                        name="lucide:loader-2" 
                        class="mr-1 h-3 w-3 animate-spin" 
                      />
                      <Icon v-else name="lucide:plug" class="mr-1 h-3 w-3" />
                      Test
                    </UiButton>
                    <UiButton
                      variant="ghost"
                      size="icon"
                      class="h-8 w-8"
                      @click="variableStore.toggleSecretProvider(provider.id)"
                    >
                      <Icon :name="provider.enabled ? 'lucide:pause' : 'lucide:play'" class="h-4 w-4" />
                    </UiButton>
                    <UiButton
                      variant="ghost"
                      size="icon"
                      class="h-8 w-8 text-muted-foreground hover:text-destructive"
                      @click="variableStore.deleteSecretProvider(provider.id)"
                    >
                      <Icon name="lucide:trash-2" class="h-4 w-4" />
                    </UiButton>
                  </div>
                </div>
                <!-- Test result message -->
                <div 
                  v-if="providerTestResults.get(provider.id) && !providerTestResults.get(provider.id)?.success"
                  class="mt-3 p-2 rounded bg-destructive/10 text-destructive text-sm"
                >
                  {{ providerTestResults.get(provider.id)?.message }}
                </div>
              </div>
            </div>
          </div>

          <!-- Playground Tab -->
          <div v-else-if="variableManagerTab === 'playground'" class="space-y-6">
            <div class="flex items-center justify-between mb-4">
              <div>
                <h3 class="text-lg font-semibold">Playground Server</h3>
                <p class="text-sm text-muted-foreground">Start a local server with demo endpoints for testing all protocols</p>
              </div>
              <UiButton
                :variant="playgroundStatus?.running ? 'destructive' : 'default'"
                :disabled="playgroundLoading"
                @click="togglePlayground"
              >
                <Icon 
                  v-if="playgroundLoading" 
                  name="lucide:loader-2" 
                  class="h-4 w-4 mr-2 animate-spin" 
                />
                <Icon 
                  v-else 
                  :name="playgroundStatus?.running ? 'lucide:square' : 'lucide:play'" 
                  class="h-4 w-4 mr-2" 
                />
                {{ playgroundStatus?.running ? 'Stop Server' : 'Start Server' }}
              </UiButton>
            </div>

            <!-- Error message -->
            <div v-if="playgroundError" class="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-sm text-destructive flex items-center gap-2">
              <Icon name="lucide:alert-circle" class="w-4 h-4 flex-shrink-0" />
              {{ playgroundError }}
            </div>

            <!-- Status indicator -->
            <div 
              class="p-4 rounded-lg border"
              :class="playgroundStatus?.running ? 'border-green-500/30 bg-green-500/5' : 'border-border bg-muted/30'"
            >
              <div class="flex items-center gap-3">
                <div 
                  class="h-3 w-3 rounded-full"
                  :class="playgroundStatus?.running ? 'bg-green-500 animate-pulse' : 'bg-gray-400'"
                />
                <span class="font-medium">
                  {{ playgroundStatus?.running ? 'Server Running' : 'Server Stopped' }}
                </span>
              </div>
            </div>

            <!-- Endpoints when running -->
            <div v-if="playgroundStatus?.running" class="space-y-4">
              <h4 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">Available Endpoints</h4>
              
              <div class="grid grid-cols-2 gap-3">
                <div
                  v-for="endpoint in playgroundEndpoints"
                  :key="endpoint.name"
                  class="group flex items-center gap-3 p-4 rounded-lg border border-border bg-background hover:bg-accent/50 transition-colors"
                >
                  <div class="h-10 w-10 rounded-lg flex items-center justify-center" :class="endpoint.bgColor">
                    <Icon :name="endpoint.icon" :class="['h-5 w-5', endpoint.color]" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="font-medium">{{ endpoint.name }}</div>
                    <div class="text-sm text-muted-foreground truncate font-mono">
                      {{ endpoint.url }}
                    </div>
                  </div>
                  <button
                    class="p-2 rounded-md opacity-0 group-hover:opacity-100 hover:bg-accent transition-all"
                    title="Copy URL"
                    @click="copyPlaygroundUrl(endpoint.url!)"
                  >
                    <Icon name="lucide:copy" class="h-4 w-4 text-muted-foreground" />
                  </button>
                </div>
              </div>
            </div>

            <!-- Info when stopped -->
            <div v-else class="space-y-4">
              <div class="bg-muted/30 rounded-lg p-6">
                <h4 class="font-medium mb-3">What's included?</h4>
                <div class="grid grid-cols-2 gap-4 text-sm">
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:globe" class="h-4 w-4 text-blue-500" />
                    <span>HTTP REST API with CRUD endpoints</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:radio" class="h-4 w-4 text-green-500" />
                    <span>WebSocket for real-time messaging</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:hexagon" class="h-4 w-4 text-pink-500" />
                    <span>GraphQL with queries & mutations</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:activity" class="h-4 w-4 text-orange-400" />
                    <span>Server-Sent Events stream</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:radio-tower" class="h-4 w-4 text-purple-500" />
                    <span>MQTT broker for pub/sub</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Icon name="lucide:cpu" class="h-4 w-4 text-amber-500" />
                    <span>gRPC with reflection support</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div class="flex items-center justify-between border-t border-border px-6 py-4">
          <p v-if="variableManagerTab === 'variables'" class="text-sm text-muted-foreground">
            Use <code class="px-1.5 py-0.5 rounded bg-muted font-mono">&#123;&#123;VARIABLE_NAME&#125;&#125;</code> in URLs, headers, and body
          </p>
          <p v-else-if="variableManagerTab === 'general'" class="text-sm text-muted-foreground">
            Settings are saved automatically
          </p>
          <p v-else class="text-sm text-muted-foreground"></p>
          <UiButton variant="outline" @click="closeModal">Close</UiButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>
