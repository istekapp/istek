import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export type LicenseStatus = 'valid' | 'invalid' | 'none'

interface LicenseState {
  key: string | null
  status: LicenseStatus
  token: string | null
  deviceId: string | null      // Fingerprint
  registeredDeviceId: number | null  // Server device ID
}

interface EvaluationInfo {
  startedAt: number | null
  expiredDismissed: boolean
}

const STORAGE_KEY = 'istek_license'
const API_BASE = 'http://localhost:8080/api/v1'
// TODO: revert to 30 days before release
const EVALUATION_PERIOD_DAYS = 1 / 24 // 1 hour for testing

export const useLicenseStore = () => {
  // State
  const state = useState<LicenseState>('licenseState', () => ({
    key: null,
    status: 'none',
    token: null,
    deviceId: null,
    registeredDeviceId: null
  }))
  
  const isActivating = useState<boolean>('licenseActivating', () => false)
  const error = useState<string | null>('licenseError', () => null)

  // Evaluation state
  const evaluationInfo = useState<EvaluationInfo>('evaluationInfo', () => ({
    startedAt: null,
    expiredDismissed: false,
  }))

  // Computed
  const isLicensed = computed(() => state.value.status === 'valid')

  const evaluationDaysLeft = computed(() => {
    if (!evaluationInfo.value.startedAt) return EVALUATION_PERIOD_DAYS
    const elapsedDays = (Date.now() / 1000 - evaluationInfo.value.startedAt) / 86400
    const daysLeft = EVALUATION_PERIOD_DAYS - elapsedDays
    return Math.max(0, daysLeft)
  })

  const isEvaluationExpired = computed(() => evaluationDaysLeft.value <= 0)

  const shouldShowEvaluationDialog = computed(() =>
    isEvaluationExpired.value && !isLicensed.value && !evaluationInfo.value.expiredDismissed
  )

  // Actions
  async function init() {
    // Load from localStorage
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      try {
        const parsed = JSON.parse(stored)
        state.value = { ...state.value, ...parsed }
      } catch (e) {
        console.error('Failed to parse license storage', e)
      }
    }
    
    // Get device fingerprint if missing
    if (!state.value.deviceId) {
      try {
        const fingerprint = await invoke<string>('get_device_fingerprint')
        state.value.deviceId = fingerprint
        persist()
      } catch (e) {
        console.error('Failed to get device fingerprint:', e)
        // Fallback for dev
        state.value.deviceId = 'dev-' + Math.random().toString(36).slice(2)
        persist()
      }
    }

    // Validate with server if we have a registered device
    if (state.value.registeredDeviceId) {
      await validateDevice()
    }

    // Initialize evaluation period tracking
    try {
      const info = await invoke<EvaluationInfo>('get_evaluation_info')
      evaluationInfo.value = info
    } catch (e) {
      console.error('Failed to get evaluation info:', e)
    }
  }

  async function dismissEvaluationExpired() {
    evaluationInfo.value.expiredDismissed = true
    try {
      await invoke('dismiss_evaluation_expired')
    } catch (e) {
      console.error('Failed to dismiss evaluation:', e)
    }
  }

  async function validateDevice() {
    if (!state.value.registeredDeviceId) {
      return
    }

    try {
      const response = await fetch(`${API_BASE}/devices/${state.value.registeredDeviceId}`)
      
      if (!response.ok) {
        // Device was revoked
        console.log('Device was revoked - clearing license')
        clearLicense()
        return
      }

      // Device is still valid
      state.value.status = 'valid'
    } catch (e) {
      // Network error - keep current state if we have a valid license locally
      console.error('Network error during validation:', e)
      if (state.value.token && state.value.registeredDeviceId) {
        state.value.status = 'valid'
      }
    }
  }

  async function activateLicense(key: string) {
    isActivating.value = true
    error.value = null
    
    try {
      // Get device name
      let deviceName = 'Unknown Device'
      try {
        deviceName = await invoke<string>('get_device_name')
      } catch (e) {
        console.error('Failed to get device name:', e)
      }
      
      const response = await fetch(`${API_BASE}/licenses/activate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: key,
          device_fingerprint: state.value.deviceId,
          device_name: deviceName
        })
      })
      
      const data = await response.json()
      
      if (!response.ok) {
        throw new Error(data.error || 'Activation failed')
      }
      
      state.value.key = key
      state.value.token = data.token
      state.value.registeredDeviceId = data.device_id
      state.value.status = 'valid'
      
      persist()
      return true
    } catch (e: any) {
      error.value = e.message
      state.value.status = 'invalid'
      return false
    } finally {
      isActivating.value = false
    }
  }

  function clearLicense() {
    state.value.key = null
    state.value.token = null
    state.value.registeredDeviceId = null
    state.value.status = 'none'
    persist()
  }

  function persist() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      key: state.value.key,
      token: state.value.token,
      deviceId: state.value.deviceId,
      registeredDeviceId: state.value.registeredDeviceId
    }))
  }

  return {
    state,
    isLicensed,
    isActivating,
    error,
    evaluationInfo,
    evaluationDaysLeft,
    isEvaluationExpired,
    shouldShowEvaluationDialog,
    init,
    activateLicense,
    clearLicense,
    dismissEvaluationExpired,
  }
}
