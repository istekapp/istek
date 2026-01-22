import { invoke } from '@tauri-apps/api/core'

interface EncryptionStatus {
  enabled: boolean
  workspaceId: string
}

export interface SensitiveValue {
  key: string
  encryptedValue: string
  description?: string
  createdAt: number
  updatedAt: number
}

export const useWorkspaceEncryption = () => {
  // State
  const isEncryptionEnabled = useState<boolean>('workspaceEncryptionEnabled', () => false)
  const showMasterKeySetup = useState<boolean>('showMasterKeySetup', () => false)
  const pendingSensitiveAction = useState<{ key: string; value: string } | null>('pendingSensitiveAction', () => null)
  const masterKeyForDisplay = useState<string | null>('masterKeyForDisplay', () => null)
  
  // Callback to execute after encryption is enabled
  const onEncryptionEnabledCallback = useState<(() => Promise<void>) | null>('onEncryptionEnabledCallback', () => null)
  
  // Sensitive values store (encrypted values from storage)
  const sensitiveValues = useState<SensitiveValue[]>('sensitiveValues', () => [])
  const sensitiveValuesLoaded = useState<boolean>('sensitiveValuesLoaded', () => false)
  
  // Get current workspace ID
  const getCurrentWorkspaceId = (): string => {
    // Get from workspace store or use default
    const workspaceStore = useWorkspaceStore()
    const id = workspaceStore.activeWorkspace.value?.id || 'default'
    console.log('[Encryption] getCurrentWorkspaceId:', id)
    return id
  }

  // Check if encryption is enabled for current workspace
  const checkEncryptionStatus = async (): Promise<boolean> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      const status = await invoke<EncryptionStatus>('sensitive_check_encryption_status', { workspaceId })
      isEncryptionEnabled.value = status.enabled
      return status.enabled
    } catch (e) {
      console.error('Failed to check encryption status:', e)
      return false
    }
  }

  // Generate a new master key (called during setup)
  const generateMasterKey = async (): Promise<string> => {
    try {
      const key = await invoke<string>('sensitive_generate_master_key')
      masterKeyForDisplay.value = key
      return key
    } catch (e) {
      console.error('Failed to generate master key:', e)
      throw e
    }
  }

  // Store the master key in keychain (after user confirms they saved it)
  const storeMasterKey = async (masterKey: string): Promise<void> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      console.log('[Encryption] Storing master key for workspace:', workspaceId)
      await invoke('sensitive_store_master_key', { workspaceId, masterKey })
      console.log('[Encryption] Master key stored in keychain')
      isEncryptionEnabled.value = true
      masterKeyForDisplay.value = null
      
      // If there was a pending callback, execute it now
      if (onEncryptionEnabledCallback.value) {
        console.log('[Encryption] Executing pending callback...')
        const callback = onEncryptionEnabledCallback.value
        onEncryptionEnabledCallback.value = null
        pendingSensitiveAction.value = null
        try {
          await callback()
          console.log('[Encryption] Pending callback executed successfully')
        } catch (callbackError) {
          console.error('[Encryption] Pending callback failed:', callbackError)
          // Don't re-throw - the key is already stored
        }
      }
    } catch (e) {
      console.error('[Encryption] Failed to store master key:', e)
      throw e
    }
  }

  // Import an existing master key (for team members)
  const importMasterKey = async (masterKey: string): Promise<void> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      // Validate the key by storing it
      await invoke('sensitive_store_master_key', { workspaceId, masterKey })
      isEncryptionEnabled.value = true
      
      // If there was a pending callback, execute it now
      if (onEncryptionEnabledCallback.value) {
        const callback = onEncryptionEnabledCallback.value
        onEncryptionEnabledCallback.value = null
        pendingSensitiveAction.value = null
        await callback()
      }
    } catch (e) {
      console.error('Failed to import master key:', e)
      throw e
    }
  }

  // Delete master key (disable encryption)
  const deleteMasterKey = async (): Promise<void> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      await invoke('sensitive_delete_master_key', { workspaceId })
      isEncryptionEnabled.value = false
    } catch (e) {
      console.error('Failed to delete master key:', e)
      throw e
    }
  }

  // Encrypt a value
  const encryptValue = async (key: string, value: string): Promise<string> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      return await invoke<string>('sensitive_encrypt', { workspaceId, key, value })
    } catch (e) {
      console.error('Failed to encrypt value:', e)
      throw e
    }
  }

  // Decrypt a value
  const decryptValue = async (key: string, encryptedValue: string): Promise<string> => {
    try {
      const workspaceId = getCurrentWorkspaceId()
      return await invoke<string>('sensitive_decrypt', { workspaceId, key, encryptedValue })
    } catch (e) {
      console.error('Failed to decrypt value:', e)
      throw e
    }
  }

  // Main entry point: encrypt and store a sensitive value
  // Returns the encrypted value to be stored in workspace
  const encryptAndStore = async (key: string, value: string): Promise<string> => {
    // Check if encryption is enabled
    const enabled = await checkEncryptionStatus()
    
    if (!enabled) {
      // Store the pending action and show setup dialog
      pendingSensitiveAction.value = { key, value }
      showMasterKeySetup.value = true
      throw new Error('ENCRYPTION_NOT_ENABLED')
    }
    
    return await encryptValue(key, value)
  }

  // Process $sensitive() function - decrypt on demand
  const processSensitiveFunction = async (key: string, encryptedValue: string): Promise<string> => {
    const enabled = await checkEncryptionStatus()
    
    if (!enabled) {
      // Show dialog to import master key
      showMasterKeySetup.value = true
      throw new Error('ENCRYPTION_NOT_ENABLED')
    }
    
    return await decryptValue(key, encryptedValue)
  }

  // Cancel setup
  const cancelSetup = () => {
    showMasterKeySetup.value = false
    pendingSensitiveAction.value = null
    masterKeyForDisplay.value = null
    onEncryptionEnabledCallback.value = null
  }

  // ============ Sensitive Values CRUD ============
  
  // Load sensitive values from storage
  const loadSensitiveValues = async (): Promise<SensitiveValue[]> => {
    try {
      const values = await invoke<SensitiveValue[]>('get_sensitive_values', {})
      sensitiveValues.value = values
      sensitiveValuesLoaded.value = true
      return values
    } catch (e) {
      console.error('Failed to load sensitive values:', e)
      return []
    }
  }

  // Get a single sensitive value by key
  const getSensitiveValue = (key: string): SensitiveValue | undefined => {
    return sensitiveValues.value.find(v => v.key === key)
  }

  // Add or update a sensitive value (encrypts and stores)
  const saveSensitiveValue = async (key: string, plainValue: string, description?: string): Promise<void> => {
    // Check if encryption is enabled first (use cached value if available)
    let enabled = isEncryptionEnabled.value
    if (!enabled) {
      enabled = await checkEncryptionStatus()
    }
    
    if (!enabled) {
      // Store pending action and show setup dialog
      pendingSensitiveAction.value = { key, value: plainValue }
      showMasterKeySetup.value = true
      throw new Error('ENCRYPTION_NOT_ENABLED')
    }
    
    // Encrypt the value
    const encryptedValue = await encryptValue(key, plainValue)
    
    // Get existing or create new
    const existing = sensitiveValues.value.find(v => v.key === key)
    const now = Date.now()
    
    const sensitiveValue: SensitiveValue = {
      key,
      encryptedValue,
      description,
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    }
    
    // Save to storage
    await invoke('save_sensitive_value', { value: sensitiveValue })
    
    // Update local state
    if (existing) {
      sensitiveValues.value = sensitiveValues.value.map(v => 
        v.key === key ? sensitiveValue : v
      )
    } else {
      sensitiveValues.value = [...sensitiveValues.value, sensitiveValue]
    }
  }

  // Delete a sensitive value
  const removeSensitiveValue = async (key: string): Promise<void> => {
    await invoke('delete_sensitive_value', { key })
    sensitiveValues.value = sensitiveValues.value.filter(v => v.key !== key)
  }

  // Get decrypted value for a key
  const getDecryptedValue = async (key: string): Promise<string | null> => {
    const sensitiveValue = sensitiveValues.value.find(v => v.key === key)
    if (!sensitiveValue) {
      return null
    }
    
    try {
      return await decryptValue(key, sensitiveValue.encryptedValue)
    } catch (e) {
      console.error(`Failed to decrypt sensitive value '${key}':`, e)
      return null
    }
  }

  return {
    // State
    isEncryptionEnabled,
    showMasterKeySetup,
    masterKeyForDisplay,
    pendingSensitiveAction,
    onEncryptionEnabledCallback,
    sensitiveValues,
    sensitiveValuesLoaded,
    
    // Actions
    checkEncryptionStatus,
    generateMasterKey,
    storeMasterKey,
    importMasterKey,
    deleteMasterKey,
    encryptValue,
    decryptValue,
    encryptAndStore,
    processSensitiveFunction,
    cancelSetup,
    
    // Sensitive Values CRUD
    loadSensitiveValues,
    getSensitiveValue,
    saveSensitiveValue,
    removeSensitiveValue,
    getDecryptedValue,
  }
}
