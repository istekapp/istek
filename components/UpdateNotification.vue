<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

interface UpdateInfo {
  currentVersion: string
  latestVersion: string
  hasUpdate: boolean
  releaseUrl: string
  releaseNotes: string | null
  publishedAt: string | null
  downloadSize?: string
}

const updateInfo = ref<UpdateInfo | null>(null)
const showDropdown = ref(false)
const isChecking = ref(false)
const dismissed = ref(false)

// Close dropdown when clicking outside
const dropdownRef = ref<HTMLElement | null>(null)

// Click outside handler
function handleClickOutside(event: MouseEvent) {
  if (dropdownRef.value && !dropdownRef.value.contains(event.target as Node)) {
    showDropdown.value = false
  }
}

// Listen for update events from backend and setup click outside
onMounted(async () => {
  // Click outside listener
  document.addEventListener('click', handleClickOutside)
  
  // Listen for update-available events
  await listen<UpdateInfo>('update-available', (event) => {
    updateInfo.value = event.payload
    dismissed.value = false
  })
  
  // Also check manually on mount (in case we missed the event)
  await checkForUpdate()
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})

async function checkForUpdate(force = false) {
  try {
    isChecking.value = true
    const info = await invoke<UpdateInfo | null>('check_for_update', { force })
    if (info) {
      updateInfo.value = info
      dismissed.value = false
    }
  } catch (e) {
    console.error('Failed to check for updates:', e)
  } finally {
    isChecking.value = false
  }
}

function toggleDropdown() {
  showDropdown.value = !showDropdown.value
}

function skip() {
  dismissed.value = true
  showDropdown.value = false
  invoke('dismiss_update')
}

function later() {
  showDropdown.value = false
}

function downloadUpdate() {
  if (updateInfo.value?.releaseUrl) {
    window.open(updateInfo.value.releaseUrl, '_blank')
  }
}

function formatDate(dateStr: string | null): string {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  return date.toLocaleDateString(undefined, { 
    year: 'numeric', 
    month: 'short', 
    day: 'numeric' 
  })
}

// Show the pill if there's an update and not dismissed
const showPill = computed(() => {
  return updateInfo.value?.hasUpdate && !dismissed.value
})

// Expose for manual check from settings
defineExpose({ checkForUpdate })
</script>

<template>
  <!-- Update Pill - Fixed position at top center -->
  <div
    v-if="showPill"
    ref="dropdownRef"
    class="fixed top-3 left-1/2 -translate-x-1/2 z-[200]"
  >
    <!-- The Pill Button -->
    <button
      class="flex items-center gap-2 px-3 py-1.5 bg-primary text-primary-foreground rounded-full text-sm font-medium shadow-lg hover:bg-primary/90 transition-all border border-primary-foreground/20"
      @click="toggleDropdown"
    >
      <Icon name="lucide:package" class="w-4 h-4" />
      <span>Update Available: {{ updateInfo?.latestVersion }}</span>
      <span class="text-primary-foreground/70">({{ formatDate(updateInfo?.publishedAt) }})</span>
      <Icon 
        name="lucide:chevron-down" 
        class="w-4 h-4 transition-transform" 
        :class="{ 'rotate-180': showDropdown }"
      />
    </button>

    <!-- Dropdown Panel -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out"
      enter-from-class="opacity-0 scale-95 -translate-y-2"
      enter-to-class="opacity-100 scale-100 translate-y-0"
      leave-active-class="transition-all duration-150 ease-in"
      leave-from-class="opacity-100 scale-100 translate-y-0"
      leave-to-class="opacity-0 scale-95 -translate-y-2"
    >
      <div
        v-if="showDropdown"
        class="absolute top-full left-1/2 -translate-x-1/2 mt-2 w-80 bg-popover border border-border rounded-xl shadow-xl overflow-hidden"
      >
        <!-- Header -->
        <div class="p-4 border-b border-border">
          <h3 class="font-semibold text-base">Update Available</h3>
          
          <!-- Version Info -->
          <div class="mt-3 space-y-1.5 text-sm">
            <div class="flex justify-between">
              <span class="text-muted-foreground">Version:</span>
              <span class="font-medium">{{ updateInfo?.latestVersion }}</span>
            </div>
            <div v-if="updateInfo?.downloadSize" class="flex justify-between">
              <span class="text-muted-foreground">Size:</span>
              <span>{{ updateInfo.downloadSize }}</span>
            </div>
            <div v-if="updateInfo?.publishedAt" class="flex justify-between">
              <span class="text-muted-foreground">Released:</span>
              <span>{{ formatDate(updateInfo.publishedAt) }}</span>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="p-3 flex items-center gap-2 bg-muted/30">
          <button
            class="px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground hover:bg-accent rounded-md transition-colors"
            @click="skip"
          >
            Skip
          </button>
          <button
            class="px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground hover:bg-accent rounded-md transition-colors"
            @click="later"
          >
            Later
          </button>
          <button
            class="flex-1 px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition-colors font-medium"
            @click="downloadUpdate"
          >
            Download Update
          </button>
        </div>

        <!-- Release Notes Link -->
        <div class="px-4 py-2.5 border-t border-border">
          <a
            :href="updateInfo?.releaseUrl"
            target="_blank"
            class="flex items-center justify-between text-sm text-muted-foreground hover:text-foreground transition-colors group"
          >
            <div class="flex items-center gap-2">
              <Icon name="lucide:file-text" class="w-4 h-4" />
              <span>Changes Since This Release</span>
            </div>
            <Icon name="lucide:external-link" class="w-3.5 h-3.5 opacity-0 group-hover:opacity-100 transition-opacity" />
          </a>
        </div>
      </div>
    </Transition>
  </div>
</template>
