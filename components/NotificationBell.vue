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
}

interface AppNotification {
  id: string
  type: 'update' | 'license' | 'info'
  title: string
  description: string
  icon: string
  iconColor: string
  action?: { label: string; handler: () => void }
  dismissible: boolean
  timestamp: number
}

const DISMISSED_KEY = 'istek_dismissed_notifications'

const variableStore = useVariableStore()
const licenseStore = useLicenseStore()

const showDropdown = ref(false)
const dropdownRef = ref<HTMLElement | null>(null)
const notifications = ref<AppNotification[]>([])
const dismissedIds = ref<Set<string>>(new Set())

// Load dismissed IDs from localStorage
onMounted(() => {
  try {
    const stored = localStorage.getItem(DISMISSED_KEY)
    if (stored) {
      dismissedIds.value = new Set(JSON.parse(stored))
    }
  } catch (e) {
    // ignore
  }
})

function persistDismissed() {
  localStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissedIds.value]))
}

useClickOutside(dropdownRef, () => {
  showDropdown.value = false
})

const unreadCount = computed(() => notifications.value.length)

function addNotification(notification: AppNotification) {
  // Don't add if already dismissed or already present
  if (dismissedIds.value.has(notification.id)) return
  if (notifications.value.find(n => n.id === notification.id)) return
  notifications.value.unshift(notification)
}

function dismissNotification(id: string) {
  notifications.value = notifications.value.filter(n => n.id !== id)
  dismissedIds.value.add(id)
  persistDismissed()
}

function dismissAll() {
  for (const n of notifications.value) {
    dismissedIds.value.add(n.id)
  }
  notifications.value = []
  persistDismissed()
}

// Listen for update events
onMounted(async () => {
  await listen<UpdateInfo>('update-available', (event) => {
    if (event.payload.hasUpdate) {
      addNotification({
        id: 'update-available',
        type: 'update',
        title: `Update Available: ${event.payload.latestVersion}`,
        description: event.payload.publishedAt
          ? `Released ${formatDate(event.payload.publishedAt)}`
          : 'A new version of Istek is available.',
        icon: 'lucide:package',
        iconColor: 'text-primary',
        action: {
          label: 'Download',
          handler: () => window.open(event.payload.releaseUrl, '_blank'),
        },
        dismissible: true,
        timestamp: Date.now(),
      })
    }
  })

  // Check for update on mount
  try {
    const info = await invoke<UpdateInfo | null>('check_for_update', { force: false })
    if (info?.hasUpdate) {
      addNotification({
        id: 'update-available',
        type: 'update',
        title: `Update Available: ${info.latestVersion}`,
        description: info.publishedAt
          ? `Released ${formatDate(info.publishedAt)}`
          : 'A new version of Istek is available.',
        icon: 'lucide:package',
        iconColor: 'text-primary',
        action: {
          label: 'Download',
          handler: () => window.open(info.releaseUrl, '_blank'),
        },
        dismissible: true,
        timestamp: Date.now(),
      })
    }
  } catch (e) {
    // Silently fail
  }

  // License encouragement - show if not licensed
  if (!licenseStore.isLicensed.value) {
    addNotification({
      id: 'license-encouragement',
      type: 'license',
      title: 'Using Istek at work?',
      description: 'If you use Istek for commercial purposes, consider purchasing a license to support development.',
      icon: 'lucide:briefcase',
      iconColor: 'text-amber-500',
      action: {
        label: 'Get a License',
        handler: () => {
          dismissNotification('license-encouragement')
          showDropdown.value = false
          variableStore.openVariableManager('license')
        },
      },
      dismissible: true,
      timestamp: Date.now(),
    })
  }
})

function formatDate(dateStr: string): string {
  const date = new Date(dateStr)
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}
</script>

<template>
  <div ref="dropdownRef" class="relative">
    <!-- Bell Button -->
    <button
      class="relative w-10 h-10 flex items-center justify-center rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
      title="Notifications"
      @click="showDropdown = !showDropdown"
    >
      <Icon name="lucide:bell" class="h-5 w-5" />
      <!-- Badge -->
      <span
        v-if="unreadCount > 0"
        class="absolute top-1.5 right-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground"
      >
        {{ unreadCount > 9 ? '9+' : unreadCount }}
      </span>
    </button>

    <!-- Dropdown -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out"
      enter-from-class="opacity-0 scale-95 -translate-y-1"
      enter-to-class="opacity-100 scale-100 translate-y-0"
      leave-active-class="transition-all duration-150 ease-in"
      leave-from-class="opacity-100 scale-100 translate-y-0"
      leave-to-class="opacity-0 scale-95 -translate-y-1"
    >
      <div
        v-if="showDropdown"
        class="absolute right-0 top-full mt-2 w-80 bg-popover border border-border rounded-xl shadow-xl overflow-hidden z-50"
      >
        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border">
          <h3 class="text-sm font-semibold">Notifications</h3>
          <button
            v-if="notifications.length > 0"
            class="text-xs text-muted-foreground hover:text-foreground transition-colors"
            @click="dismissAll"
          >
            Clear all
          </button>
        </div>

        <!-- Notification List -->
        <div class="max-h-80 overflow-y-auto">
          <div v-if="notifications.length === 0" class="px-4 py-8 text-center">
            <Icon name="lucide:bell-off" class="h-8 w-8 mx-auto text-muted-foreground/40 mb-2" />
            <p class="text-sm text-muted-foreground">No notifications</p>
          </div>

          <div
            v-for="notification in notifications"
            :key="notification.id"
            class="px-4 py-3 border-b border-border last:border-b-0 hover:bg-accent/50 transition-colors"
          >
            <div class="flex gap-3">
              <!-- Icon -->
              <div class="shrink-0 mt-0.5">
                <div class="w-8 h-8 rounded-full bg-accent flex items-center justify-center">
                  <Icon :name="notification.icon" :class="['h-4 w-4', notification.iconColor]" />
                </div>
              </div>

              <!-- Content -->
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium leading-tight">{{ notification.title }}</p>
                <p class="text-xs text-muted-foreground mt-1 leading-relaxed">{{ notification.description }}</p>

                <!-- Action -->
                <div class="flex items-center gap-2 mt-2">
                  <button
                    v-if="notification.action"
                    class="px-2.5 py-1 text-xs font-medium bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition-colors"
                    @click="notification.action!.handler()"
                  >
                    {{ notification.action.label }}
                  </button>
                  <button
                    v-if="notification.dismissible"
                    class="px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent rounded-md transition-colors"
                    @click="dismissNotification(notification.id)"
                  >
                    Dismiss
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>
