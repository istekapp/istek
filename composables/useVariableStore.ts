import type { Variable, Environment, SecretProviderConfig, VariableScope } from '~/types'
import { generateId } from '~/lib/utils'
import { invoke } from '@tauri-apps/api/core'

const ENV_COLORS = [
  '#86efac', // green
  '#93c5fd', // blue
  '#fde047', // yellow
  '#d8b4fe', // purple
  '#fdba74', // orange
  '#f9a8d4', // pink
  '#a5f3fc', // cyan
  '#fca5a5', // red
]

const createVariable = (overrides: Partial<Variable> = {}): Variable => ({
  id: generateId(),
  key: '',
  value: '',
  description: '',
  isSecret: false,
  enabled: true,
  ...overrides,
})

const createEnvironment = (name: string, color?: string): Environment => ({
  id: generateId(),
  name,
  color: color || ENV_COLORS[Math.floor(Math.random() * ENV_COLORS.length)],
  variables: [],
  createdAt: Date.now(),
})

export const useVariableStore = () => {
  // Global variables
  const globalVariables = useState<Variable[]>('globalVariables', () => [])
  
  // Environments
  const environments = useState<Environment[]>('environments', () => [])
  
  // Active environment
  const activeEnvironmentId = useState<string | null>('activeEnvironmentId', () => null)
  
  // Secret providers
  const secretProviders = useState<SecretProviderConfig[]>('secretProviders', () => [])
  
  // UI State
  const showVariableManager = useState<boolean>('showVariableManager', () => false)
  const showIntegrations = useState<boolean>('showIntegrations', () => false)
  const variableManagerTab = useState<'general' | 'variables' | 'environments' | 'integrations'>('variableManagerTab', () => 'general')
  
  // Settings State
  const appTheme = useState<'dark' | 'light' | 'system'>('appTheme', () => 'dark')
  const appZoom = useState<number>('appZoom', () => 1.0)
  
  // Loading state
  const isVariableDataLoaded = useState<boolean>('isVariableDataLoaded', () => false)
  
  // Cache for fetched secret values (providerId:secretPath:secretKey -> value)
  const secretCache = useState<Map<string, { value: string; fetchedAt: number }>>('secretCache', () => new Map())
  const SECRET_CACHE_TTL = 5 * 60 * 1000 // 5 minutes
  
  // Track which secrets are being fetched to avoid duplicate requests
  const fetchingSecrets = useState<Set<string>>('fetchingSecrets', () => new Set())
  
  // Resolved secret values (variableId -> fetched value)
  const resolvedSecretValues = useState<Map<string, string>>('resolvedSecretValues', () => new Map())

  // Computed
  const activeEnvironment = computed(() =>
    environments.value.find(e => e.id === activeEnvironmentId.value) || null
  )

  // Get all resolved variables (global + environment overrides)
  const resolvedVariables = computed(() => {
    const vars = new Map<string, Variable>()
    
    // First add global variables
    for (const v of globalVariables.value) {
      if (v.enabled && v.key) {
        // If variable has a secret provider and we have a cached value, use it
        if (v.secretProvider && resolvedSecretValues.value.has(v.id)) {
          vars.set(v.key, { ...v, value: resolvedSecretValues.value.get(v.id)! })
        } else {
          vars.set(v.key, v)
        }
      }
    }
    
    // Then override with environment-specific variables
    if (activeEnvironment.value) {
      for (const v of activeEnvironment.value.variables) {
        if (v.enabled && v.key) {
          if (v.secretProvider && resolvedSecretValues.value.has(v.id)) {
            vars.set(v.key, { ...v, value: resolvedSecretValues.value.get(v.id)! })
          } else {
            vars.set(v.key, v)
          }
        }
      }
    }
    
    return vars
  })
  
  // Fetch secret value from provider
  const fetchSecretValue = async (variable: Variable): Promise<string | null> => {
    if (!variable.secretProvider) return null
    
    const provider = secretProviders.value.find(p => p.id === variable.secretProvider!.providerId)
    if (!provider || !provider.enabled) return null
    
    const cacheKey = `${provider.id}:${variable.secretProvider.secretPath}:${variable.secretProvider.secretKey}`
    
    // Check cache first
    const cached = secretCache.value.get(cacheKey)
    if (cached && Date.now() - cached.fetchedAt < SECRET_CACHE_TTL) {
      return cached.value
    }
    
    // Avoid duplicate fetches
    if (fetchingSecrets.value.has(cacheKey)) {
      // Wait for ongoing fetch
      await new Promise(resolve => setTimeout(resolve, 100))
      const cachedAfterWait = secretCache.value.get(cacheKey)
      return cachedAfterWait?.value || null
    }
    
    fetchingSecrets.value.add(cacheKey)
    
    try {
      const config = buildProviderConfig(provider, variable.secretProvider.secretPath, variable.secretProvider.secretKey)
      const result = await invoke<{ success: boolean; secrets: Array<{ key: string; value: string }>; error?: string }>('test_secret_provider_connection', {
        provider: provider.type,
        config
      })
      
      if (result.success && result.secrets.length > 0) {
        // Find the specific key in the secrets
        const secret = result.secrets.find(s => s.key === variable.secretProvider!.secretKey)
        if (secret) {
          // Cache the value
          secretCache.value.set(cacheKey, { value: secret.value, fetchedAt: Date.now() })
          return secret.value
        }
        // If key not found but we have secrets, maybe the secret itself is the value (single value secret)
        if (result.secrets.length === 1) {
          secretCache.value.set(cacheKey, { value: result.secrets[0].value, fetchedAt: Date.now() })
          return result.secrets[0].value
        }
      }
      
      console.warn(`Failed to fetch secret for ${variable.key}:`, result.error)
      return null
    } catch (e) {
      console.error(`Error fetching secret for ${variable.key}:`, e)
      return null
    } finally {
      fetchingSecrets.value.delete(cacheKey)
    }
  }
  
  // Build provider config for Tauri command
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
  
  // Refresh all secret values from providers
  const refreshSecretValues = async () => {
    const allVariables = [
      ...globalVariables.value,
      ...(activeEnvironment.value?.variables || [])
    ]
    
    const secretVariables = allVariables.filter(v => v.enabled && v.secretProvider)
    
    for (const variable of secretVariables) {
      const value = await fetchSecretValue(variable)
      if (value !== null) {
        resolvedSecretValues.value.set(variable.id, value)
      }
    }
    
    // Trigger reactivity
    resolvedSecretValues.value = new Map(resolvedSecretValues.value)
  }
  
  // Clear secret cache (useful when provider config changes)
  const clearSecretCache = () => {
    secretCache.value.clear()
    resolvedSecretValues.value.clear()
  }

  // Template function pattern: {{fn.name(args)}} or {{$fn.name(args)}}
  const templateFunctionPattern = /\{\{\s*\$?([\w.]+)\((.*?)\)\s*\}\}/g
  
  // Process template functions asynchronously
  const processTemplateFunction = async (fnName: string, args: string): Promise<string> => {
    try {
      // Parse arguments (simple comma-separated, respecting quotes)
      const parsedArgs = args
        .split(/,(?=(?:[^"']*["'][^"']*["'])*[^"']*$)/)
        .map(arg => arg.trim().replace(/^["']|["']$/g, ''))
        .filter(arg => arg.length > 0)
      
      switch (fnName) {
        // Hash functions
        case 'hash.md5': {
          const result = await invoke<{ hex: string; base64: string }>('hash_md5', { input: parsedArgs[0] || '' })
          return result.hex
        }
        case 'hash.sha1': {
          const result = await invoke<{ hex: string; base64: string }>('hash_sha1', { input: parsedArgs[0] || '' })
          return result.hex
        }
        case 'hash.sha256': {
          const result = await invoke<{ hex: string; base64: string }>('hash_sha256', { input: parsedArgs[0] || '' })
          return result.hex
        }
        case 'hash.sha512': {
          const result = await invoke<{ hex: string; base64: string }>('hash_sha512', { input: parsedArgs[0] || '' })
          return result.hex
        }
        
        // HMAC functions
        case 'hmac.sha256': {
          const result = await invoke<{ hex: string; base64: string }>('hmac_sha256', { 
            input: parsedArgs[0] || '', 
            key: parsedArgs[1] || '' 
          })
          return result.hex
        }
        case 'hmac.sha512': {
          const result = await invoke<{ hex: string; base64: string }>('hmac_sha512', { 
            input: parsedArgs[0] || '', 
            key: parsedArgs[1] || '' 
          })
          return result.hex
        }
        
        // Encoding functions
        case 'base64.encode':
        case 'encode.base64': {
          return await invoke<string>('encode_base64', { input: parsedArgs[0] || '' })
        }
        case 'base64.decode':
        case 'decode.base64': {
          return await invoke<string>('decode_base64', { input: parsedArgs[0] || '' })
        }
        case 'url.encode':
        case 'encode.url': {
          return await invoke<string>('encode_url', { input: parsedArgs[0] || '' })
        }
        case 'url.decode':
        case 'decode.url': {
          return await invoke<string>('decode_url', { input: parsedArgs[0] || '' })
        }
        
        // Encryption (keychain) functions
        case 'encrypt': {
          // encrypt(key) - retrieves value from keychain
          return await invoke<string>('encrypt_retrieve', { key: parsedArgs[0] || '' })
        }
        
        // Utility functions
        case 'uuid': {
          return await invoke<string>('generate_uuid', {})
        }
        case 'timestamp': {
          return String(await invoke<number>('timestamp_now', {}))
        }
        case 'timestamp.ms': {
          return String(await invoke<number>('timestamp_now_ms', {}))
        }
        
        // Random functions
        case 'random.int': {
          const min = parseInt(parsedArgs[0]) || 0
          const max = parseInt(parsedArgs[1]) || 100
          return String(await invoke<number>('random_int', { min, max }))
        }
        case 'random.float': {
          const min = parseFloat(parsedArgs[0]) || 0
          const max = parseFloat(parsedArgs[1]) || 1
          return String(await invoke<number>('random_float', { min, max }))
        }
        case 'random.string': {
          const length = parseInt(parsedArgs[0]) || 16
          const charset = parsedArgs[1] || null
          return await invoke<string>('random_string', { length, charset })
        }
        case 'random.hex': {
          const length = parseInt(parsedArgs[0]) || 16
          return await invoke<string>('random_hex', { length })
        }
        
        default:
          return `{{${fnName}(${args})}}`
      }
    } catch (e) {
      console.error(`Template function error (${fnName}):`, e)
      return `{{ERROR: ${fnName}}}`
    }
  }

  // Interpolate variables in a string ({{VAR_NAME}} syntax)
  // This is the synchronous version for simple variable replacement
  const interpolate = (text: string): string => {
    if (!text) return text
    
    // First, replace simple variables
    let result = text.replace(/\{\{([^}(]+)\}\}/g, (match, varName) => {
      const trimmedName = varName.trim()
      // Skip if it looks like a function call
      if (trimmedName.includes('.') && !resolvedVariables.value.has(trimmedName)) {
        return match
      }
      const variable = resolvedVariables.value.get(trimmedName)
      return variable ? variable.value : match
    })
    
    return result
  }
  
  // Async interpolation that handles template functions and fetches secrets
  const interpolateAsync = async (text: string): Promise<string> => {
    if (!text) return text
    
    // First, fetch any needed secrets for variables used in the text
    const varNames = extractVariableNames(text)
    for (const varName of varNames) {
      // Find the variable
      const allVariables = [
        ...globalVariables.value,
        ...(activeEnvironment.value?.variables || [])
      ]
      const variable = allVariables.find(v => v.key === varName && v.enabled)
      
      // If variable has a secret provider and we don't have a cached value, fetch it
      if (variable?.secretProvider && !resolvedSecretValues.value.has(variable.id)) {
        const value = await fetchSecretValue(variable)
        if (value !== null) {
          resolvedSecretValues.value.set(variable.id, value)
          // Trigger reactivity
          resolvedSecretValues.value = new Map(resolvedSecretValues.value)
        }
      }
    }
    
    // Now do synchronous variable replacement (with updated resolvedVariables)
    let result = interpolate(text)
    
    // Then process template functions
    const matches = [...result.matchAll(templateFunctionPattern)]
    for (const match of matches) {
      const [fullMatch, fnName, args] = match
      const replacement = await processTemplateFunction(fnName, args)
      result = result.replace(fullMatch, replacement)
    }
    
    return result
  }

  // Check if text contains variables
  const hasVariables = (text: string): boolean => {
    return /\{\{[^}]+\}\}/.test(text)
  }

  // Extract variable names from text
  const extractVariableNames = (text: string): string[] => {
    const matches = text.match(/\{\{([^}]+)\}\}/g) || []
    return matches.map(m => m.slice(2, -2).trim())
  }

  // Get unresolved variables in text
  const getUnresolvedVariables = (text: string): string[] => {
    const names = extractVariableNames(text)
    return names.filter(name => !resolvedVariables.value.has(name))
  }

  // ============ Global Variable Actions ============
  const addGlobalVariable = async (variable?: Partial<Variable>) => {
    const newVar = createVariable(variable)
    globalVariables.value = [...globalVariables.value, newVar]
    
    // Persist to database
    try {
      await invoke('save_global_variable', { variable: newVar })
    } catch (e) {
      console.error('Failed to save global variable:', e)
    }
  }

  const updateGlobalVariable = async (id: string, updates: Partial<Variable>) => {
    globalVariables.value = globalVariables.value.map(v =>
      v.id === id ? { ...v, ...updates } : v
    )
    
    // Persist to database
    const updated = globalVariables.value.find(v => v.id === id)
    if (updated) {
      try {
        await invoke('save_global_variable', { variable: updated })
      } catch (e) {
        console.error('Failed to update global variable:', e)
      }
    }
  }

  const deleteGlobalVariable = async (id: string) => {
    globalVariables.value = globalVariables.value.filter(v => v.id !== id)
    
    // Persist to database
    try {
      await invoke('delete_global_variable', { id })
    } catch (e) {
      console.error('Failed to delete global variable:', e)
    }
  }

  const toggleGlobalVariable = async (id: string) => {
    globalVariables.value = globalVariables.value.map(v =>
      v.id === id ? { ...v, enabled: !v.enabled } : v
    )
    
    // Persist to database
    const updated = globalVariables.value.find(v => v.id === id)
    if (updated) {
      try {
        await invoke('save_global_variable', { variable: updated })
      } catch (e) {
        console.error('Failed to toggle global variable:', e)
      }
    }
  }

  // ============ Environment Actions ============
  const addEnvironment = async (name: string) => {
    const newEnv = createEnvironment(name)
    environments.value = [...environments.value, newEnv]
    
    // Persist to database
    try {
      await invoke('save_environment', { environment: newEnv })
    } catch (e) {
      console.error('Failed to save environment:', e)
    }
    
    return newEnv
  }

  const updateEnvironment = async (id: string, updates: Partial<Environment>) => {
    environments.value = environments.value.map(e =>
      e.id === id ? { ...e, ...updates } : e
    )
    
    // Persist to database
    const updated = environments.value.find(e => e.id === id)
    if (updated) {
      try {
        await invoke('save_environment', { environment: updated })
      } catch (e) {
        console.error('Failed to update environment:', e)
      }
    }
  }

  const deleteEnvironment = async (id: string) => {
    environments.value = environments.value.filter(e => e.id !== id)
    if (activeEnvironmentId.value === id) {
      activeEnvironmentId.value = environments.value[0]?.id || null
    }
    
    // Persist to database
    try {
      await invoke('delete_environment', { id })
      await invoke('save_active_environment_id', { id: activeEnvironmentId.value })
    } catch (e) {
      console.error('Failed to delete environment:', e)
    }
  }

  const setActiveEnvironment = async (id: string | null) => {
    activeEnvironmentId.value = id
    
    // Persist to database
    try {
      await invoke('save_active_environment_id', { id })
    } catch (e) {
      console.error('Failed to save active environment:', e)
    }
    
    // Refresh secret values for the new environment
    await refreshSecretValues()
  }

  const duplicateEnvironment = async (id: string) => {
    const source = environments.value.find(e => e.id === id)
    if (!source) return null
    
    const newEnv: Environment = {
      ...createEnvironment(`${source.name} (Copy)`),
      variables: source.variables.map(v => ({ ...v, id: generateId() })),
    }
    environments.value = [...environments.value, newEnv]
    
    // Persist to database
    try {
      await invoke('save_environment', { environment: newEnv })
    } catch (e) {
      console.error('Failed to save duplicated environment:', e)
    }
    
    return newEnv
  }

  // ============ Environment Variable Actions ============
  const addEnvironmentVariable = async (envId: string, variable?: Partial<Variable>) => {
    environments.value = environments.value.map(e =>
      e.id === envId
        ? { ...e, variables: [...e.variables, createVariable(variable)] }
        : e
    )
    
    // Persist to database
    const updated = environments.value.find(e => e.id === envId)
    if (updated) {
      try {
        await invoke('save_environment', { environment: updated })
      } catch (e) {
        console.error('Failed to save environment variable:', e)
      }
    }
  }

  const updateEnvironmentVariable = async (envId: string, varId: string, updates: Partial<Variable>) => {
    environments.value = environments.value.map(e =>
      e.id === envId
        ? {
            ...e,
            variables: e.variables.map(v =>
              v.id === varId ? { ...v, ...updates } : v
            ),
          }
        : e
    )
    
    // Persist to database
    const updated = environments.value.find(e => e.id === envId)
    if (updated) {
      try {
        await invoke('save_environment', { environment: updated })
      } catch (e) {
        console.error('Failed to update environment variable:', e)
      }
    }
  }

  const deleteEnvironmentVariable = async (envId: string, varId: string) => {
    environments.value = environments.value.map(e =>
      e.id === envId
        ? { ...e, variables: e.variables.filter(v => v.id !== varId) }
        : e
    )
    
    // Persist to database
    const updated = environments.value.find(e => e.id === envId)
    if (updated) {
      try {
        await invoke('save_environment', { environment: updated })
      } catch (e) {
        console.error('Failed to delete environment variable:', e)
      }
    }
  }

  const toggleEnvironmentVariable = async (envId: string, varId: string) => {
    environments.value = environments.value.map(e =>
      e.id === envId
        ? {
            ...e,
            variables: e.variables.map(v =>
              v.id === varId ? { ...v, enabled: !v.enabled } : v
            ),
          }
        : e
    )
    
    // Persist to database
    const updated = environments.value.find(e => e.id === envId)
    if (updated) {
      try {
        await invoke('save_environment', { environment: updated })
      } catch (e) {
        console.error('Failed to toggle environment variable:', e)
      }
    }
  }

  // ============ Secret Provider Actions ============
  const addSecretProvider = async (provider: Omit<SecretProviderConfig, 'id' | 'createdAt'>) => {
    const newProvider: SecretProviderConfig = {
      ...provider,
      id: generateId(),
      createdAt: Date.now(),
    }
    secretProviders.value = [...secretProviders.value, newProvider]
    
    // Persist to database
    try {
      await invoke('save_secret_provider', { 
        provider: {
          ...newProvider,
          providerType: newProvider.type
        }
      })
    } catch (e) {
      console.error('Failed to save secret provider:', e)
    }
    
    return newProvider
  }

  const updateSecretProvider = async (id: string, updates: Partial<SecretProviderConfig>) => {
    secretProviders.value = secretProviders.value.map(p =>
      p.id === id ? { ...p, ...updates } : p
    )
    
    // Persist to database
    const updated = secretProviders.value.find(p => p.id === id)
    if (updated) {
      try {
        await invoke('save_secret_provider', { 
          provider: {
            ...updated,
            providerType: updated.type
          }
        })
      } catch (e) {
        console.error('Failed to update secret provider:', e)
      }
    }
  }

  const deleteSecretProvider = async (id: string) => {
    secretProviders.value = secretProviders.value.filter(p => p.id !== id)
    
    // Persist to database
    try {
      await invoke('delete_secret_provider', { id })
    } catch (e) {
      console.error('Failed to delete secret provider:', e)
    }
  }

  const toggleSecretProvider = async (id: string) => {
    secretProviders.value = secretProviders.value.map(p =>
      p.id === id ? { ...p, enabled: !p.enabled } : p
    )
    
    // Persist to database
    const updated = secretProviders.value.find(p => p.id === id)
    if (updated) {
      try {
        await invoke('save_secret_provider', { 
          provider: {
            ...updated,
            providerType: updated.type
          }
        })
      } catch (e) {
        console.error('Failed to toggle secret provider:', e)
      }
    }
  }

  // ============ UI Actions ============
  const openVariableManager = (tab?: 'general' | 'variables' | 'environments' | 'integrations') => {
    if (tab) variableManagerTab.value = tab
    showVariableManager.value = true
  }

  const closeVariableManager = () => {
    showVariableManager.value = false
  }
  
  // ============ Workspace Switch Actions ============
  // Set workspace-specific data (called when switching workspaces)
  const setWorkspaceData = (data: { 
    globalVariables: Variable[]
    environments: Environment[]
    activeEnvironmentId: string | null
  }) => {
    globalVariables.value = data.globalVariables || []
    environments.value = data.environments || []
    activeEnvironmentId.value = data.activeEnvironmentId
    
    // Clear secret cache when switching workspaces
    clearSecretCache()
    
    // If no environments, create defaults
    if (environments.value.length === 0) {
      environments.value = [
        { ...createEnvironment('Development', '#86efac'), isDefault: true },
        { ...createEnvironment('Staging', '#fde047') },
        { ...createEnvironment('Production', '#fca5a5') },
      ]
      activeEnvironmentId.value = environments.value[0].id
    }
    
    // If no active environment set, use the first one
    if (!activeEnvironmentId.value && environments.value.length > 0) {
      const defaultEnv = environments.value.find(e => e.isDefault) || environments.value[0]
      activeEnvironmentId.value = defaultEnv.id
    }
    
    // Refresh secret values for the new workspace
    refreshSecretValues()
  }

  // ============ Database Actions ============
  const loadVariableDataFromDatabase = async () => {
    if (isVariableDataLoaded.value) return
    
    try {
      const data = await invoke<{
        globalVariables: Variable[]
        environments: Environment[]
        secretProviders: Array<{
          id: string
          name: string
          providerType: string
          enabled: boolean
          config: any
          createdAt: number
        }>
        activeEnvironmentId: string | null
      }>('load_app_data')
      
      if (data.globalVariables.length > 0) {
        globalVariables.value = data.globalVariables
      }
      
      if (data.environments.length > 0) {
        environments.value = data.environments
      } else {
        // Create default environments if none exist
        const defaultEnvs = [
          { ...createEnvironment('Development', '#86efac'), isDefault: true },
          { ...createEnvironment('Staging', '#fde047') },
          { ...createEnvironment('Production', '#fca5a5') },
        ]
        environments.value = defaultEnvs
        // Save default environments
        try {
          await invoke('save_all_environments', { environments: defaultEnvs })
        } catch (e) {
          console.error('Failed to save default environments:', e)
        }
      }
      
      if (data.secretProviders.length > 0) {
        // Map providerType back to type
        console.log('=== DEBUG Loading secretProviders ===')
        console.log('Raw data:', JSON.stringify(data.secretProviders, null, 2))
        secretProviders.value = data.secretProviders.map(p => {
          console.log('Provider p:', JSON.stringify(p, null, 2))
          console.log('p.providerType:', p.providerType)
          return {
            ...p,
            type: p.providerType as SecretProviderConfig['type']
          }
        })
        console.log('Mapped secretProviders:', JSON.stringify(secretProviders.value, null, 2))
      }
      
      if (data.activeEnvironmentId) {
        activeEnvironmentId.value = data.activeEnvironmentId
      } else if (environments.value.length > 0) {
        // Set default active environment
        const defaultEnv = environments.value.find(e => e.isDefault) || environments.value[0]
        activeEnvironmentId.value = defaultEnv.id
      }
      
      isVariableDataLoaded.value = true
      
      // Fetch secret values for variables with secret providers
      await refreshSecretValues()
    } catch (e) {
      console.error('Failed to load variable data:', e)
      
      // Still create default environments on error
      if (environments.value.length === 0) {
        environments.value = [
          { ...createEnvironment('Development', '#86efac'), isDefault: true },
          { ...createEnvironment('Staging', '#fde047') },
          { ...createEnvironment('Production', '#fca5a5') },
        ]
        activeEnvironmentId.value = environments.value[0].id
      }
    }
  }

  return {
    // State
    globalVariables,
    environments,
    activeEnvironmentId,
    activeEnvironment,
    secretProviders,
    resolvedVariables,
    resolvedSecretValues,
    showVariableManager,
    showIntegrations,
    variableManagerTab,
    isVariableDataLoaded,
    
    // Variable utilities
    interpolate,
    interpolateAsync,
    hasVariables,
    extractVariableNames,
    getUnresolvedVariables,
    
    // Secret utilities
    fetchSecretValue,
    refreshSecretValues,
    clearSecretCache,
    
    // Global variable actions
    addGlobalVariable,
    updateGlobalVariable,
    deleteGlobalVariable,
    toggleGlobalVariable,
    
    // Environment actions
    addEnvironment,
    updateEnvironment,
    deleteEnvironment,
    setActiveEnvironment,
    duplicateEnvironment,
    
    // Environment variable actions
    addEnvironmentVariable,
    updateEnvironmentVariable,
    deleteEnvironmentVariable,
    toggleEnvironmentVariable,
    
    // Secret provider actions
    addSecretProvider,
    updateSecretProvider,
    deleteSecretProvider,
    toggleSecretProvider,
    
    // UI actions
    openVariableManager,
    closeVariableManager,
    
    // Settings state
    appTheme,
    appZoom,
    
    // Database actions
    loadVariableDataFromDatabase,
    
    // Workspace switch actions
    setWorkspaceData,
  }
}
