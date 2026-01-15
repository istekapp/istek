<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { SseRequest, SseEvent, KeyValue } from '~/types'
import { generateId } from '~/lib/utils'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as SseRequest)
const sseState = computed(() => activeTab.value.sseState!)

const eventsContainer = ref<HTMLElement>()
const selectedEvent = ref<SseEvent | null>(null)

// Listen for SSE events
let unlistenConnection: (() => void) | null = null
let unlistenEvents: (() => void) | null = null

onMounted(async () => {
  // Listen for connection state changes
  unlistenConnection = await listen<{ connectionId: string; connected: boolean; error?: string }>('sse-connection', (event) => {
    if (event.payload.connectionId === sseState.value.connectionId) {
      store.updateSseState({ 
        connected: event.payload.connected,
        connectionId: event.payload.connected ? sseState.value.connectionId : null
      })
      
      if (event.payload.error) {
        store.addSseEvent({
          id: generateId(),
          eventType: 'error',
          data: event.payload.error,
          timestamp: Date.now(),
        })
      }
    }
  })
})

// Watch for connectionId changes to listen for events
watch(() => sseState.value.connectionId, async (connectionId) => {
  // Cleanup old listener
  if (unlistenEvents) {
    unlistenEvents()
    unlistenEvents = null
  }
  
  if (connectionId) {
    unlistenEvents = await listen<SseEvent>(`sse-event-${connectionId}`, (event) => {
      store.addSseEvent(event.payload)
      nextTick(() => {
        eventsContainer.value?.scrollTo({ top: eventsContainer.value.scrollHeight, behavior: 'smooth' })
      })
    })
  }
}, { immediate: true })

onUnmounted(() => {
  unlistenConnection?.()
  unlistenEvents?.()
})

const connect = async () => {
  if (!request.value.url) return
  
  store.setActiveLoading(true)
  try {
    const connectionId = generateId()
    const headers = request.value.headers
      .filter((h: KeyValue) => h.enabled && h.key)
      .reduce((acc: Record<string, string>, h: KeyValue) => ({ ...acc, [h.key]: h.value }), {})
    
    // Update state before connecting
    store.updateSseState({ connectionId, events: [] })
    
    await invoke('sse_connect', {
      connectionId,
      url: request.value.url,
      headers,
    })
  } catch (error: any) {
    store.addSseEvent({
      id: generateId(),
      eventType: 'error',
      data: `Connection error: ${error}`,
      timestamp: Date.now(),
    })
    store.updateSseState({ connected: false, connectionId: null })
  } finally {
    store.setActiveLoading(false)
  }
}

const disconnect = async () => {
  if (sseState.value.connectionId) {
    await invoke('sse_disconnect', { connectionId: sseState.value.connectionId })
    store.updateSseState({ connected: false, connectionId: null })
  }
}

const clearEvents = () => {
  store.updateSseState({ events: [] })
  selectedEvent.value = null
}

const formatTime = (timestamp: number) => {
  return new Date(timestamp).toLocaleTimeString()
}

const getEventTypeColor = (type: string) => {
  switch (type) {
    case 'message': return 'text-blue-400'
    case 'notification': return 'text-green-400'
    case 'update': return 'text-yellow-400'
    case 'alert': return 'text-red-400'
    case 'error': return 'text-red-500'
    case 'counter': return 'text-purple-400'
    case 'time': return 'text-cyan-400'
    default: return 'text-muted-foreground'
  }
}

const tryParseJson = (data: string) => {
  try {
    return JSON.stringify(JSON.parse(data), null, 2)
  } catch {
    return data
  }
}
</script>

<template>
  <div class="flex h-full">
    <!-- Left Panel: Config & Events List -->
    <div class="flex flex-col w-1/2 border-r border-border">
      <!-- URL Bar -->
      <div class="flex items-center gap-2 border-b border-border p-3">
        <div class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1">
          <span class="text-xs font-mono text-orange-500">SSE</span>
        </div>
        
        <UiVariableInput
          :model-value="request.url"
          placeholder="http://localhost:19510/sse/events"
          class="flex-1 font-mono text-sm h-9"
          @update:model-value="store.updateActiveRequest({ url: $event })"
        />
        
        <UiButton
          v-if="!sseState.connected"
          :disabled="!request.url || activeTab.isLoading"
          class="h-9 px-4"
          @click="connect"
        >
          <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
          <Icon v-else name="lucide:radio" class="mr-2 h-4 w-4" />
          Connect
        </UiButton>
        <UiButton
          v-else
          variant="destructive"
          class="h-9 px-4"
          @click="disconnect"
        >
          <Icon name="lucide:x" class="mr-2 h-4 w-4" />
          Disconnect
        </UiButton>
      </div>

      <!-- Headers -->
      <div class="border-b border-border p-3">
        <div class="flex items-center justify-between mb-2">
          <span class="text-sm font-medium">Headers</span>
          <UiButton variant="ghost" size="sm" @click="store.addSseHeader">
            <Icon name="lucide:plus" class="h-4 w-4" />
          </UiButton>
        </div>
        <div class="space-y-2 max-h-32 overflow-auto">
          <div
            v-for="header in request.headers"
            :key="header.id"
            class="flex items-center gap-2"
          >
            <input
              type="checkbox"
              :checked="header.enabled"
              class="h-4 w-4 rounded border-input"
              @change="store.toggleSseHeader(header.id)"
            />
            <UiHeaderKeyInput
              :model-value="header.key"
              placeholder="Header"
              class="flex-1 h-8 text-sm"
              @update:model-value="store.updateSseHeader(header.id, 'key', $event)"
            />
            <UiHeaderValueInput
              :model-value="header.value"
              :header-key="header.key"
              placeholder="Value"
              class="flex-1 h-8 text-sm"
              @update:model-value="store.updateSseHeader(header.id, 'value', $event)"
            />
            <UiButton
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              @click="store.removeSseHeader(header.id)"
            >
              <Icon name="lucide:x" class="h-4 w-4" />
            </UiButton>
          </div>
        </div>
      </div>

      <!-- Events List -->
      <div class="flex-1 flex flex-col min-h-0">
        <div class="flex items-center justify-between px-3 py-2 border-b border-border">
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium">Events</span>
            <span class="text-xs text-muted-foreground">({{ sseState.events.length }})</span>
            <div 
              v-if="sseState.connected" 
              class="flex items-center gap-1 text-xs text-green-500"
            >
              <span class="relative flex h-2 w-2">
                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
                <span class="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
              </span>
              Connected
            </div>
          </div>
          <UiButton variant="ghost" size="sm" @click="clearEvents">
            <Icon name="lucide:trash-2" class="h-4 w-4 mr-1" />
            Clear
          </UiButton>
        </div>
        
        <div ref="eventsContainer" class="flex-1 overflow-auto">
          <div
            v-if="sseState.events.length === 0"
            class="flex items-center justify-center h-full text-muted-foreground text-sm"
          >
            No events received yet
          </div>
          <div v-else class="divide-y divide-border">
            <div
              v-for="event in sseState.events"
              :key="event.id"
              :class="[
                'px-3 py-2 cursor-pointer hover:bg-accent/50 transition-colors',
                selectedEvent?.id === event.id && 'bg-accent'
              ]"
              @click="selectedEvent = event"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <span :class="['text-xs font-mono font-semibold', getEventTypeColor(event.eventType)]">
                    {{ event.eventType }}
                  </span>
                  <span v-if="event.eventId" class="text-xs text-muted-foreground">
                    #{{ event.eventId }}
                  </span>
                </div>
                <span class="text-xs text-muted-foreground">{{ formatTime(event.timestamp) }}</span>
              </div>
              <div class="text-xs text-muted-foreground truncate mt-1 font-mono">
                {{ event.data.substring(0, 100) }}{{ event.data.length > 100 ? '...' : '' }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Right Panel: Event Detail -->
    <div class="flex flex-col w-1/2">
      <div class="flex items-center justify-between px-3 py-2 border-b border-border">
        <span class="text-sm font-medium">Event Detail</span>
        <span v-if="selectedEvent" :class="['text-xs font-mono', getEventTypeColor(selectedEvent.eventType)]">
          {{ selectedEvent.eventType }}
        </span>
      </div>
      
      <div v-if="!selectedEvent" class="flex-1 flex items-center justify-center text-muted-foreground text-sm">
        Select an event to view details
      </div>
      
      <div v-else class="flex-1 overflow-auto p-3">
        <div class="space-y-3">
          <div class="grid grid-cols-2 gap-2 text-sm">
            <div>
              <span class="text-muted-foreground">Event Type:</span>
              <span :class="['ml-2 font-mono', getEventTypeColor(selectedEvent.eventType)]">
                {{ selectedEvent.eventType }}
              </span>
            </div>
            <div>
              <span class="text-muted-foreground">Time:</span>
              <span class="ml-2 font-mono">{{ formatTime(selectedEvent.timestamp) }}</span>
            </div>
            <div v-if="selectedEvent.eventId">
              <span class="text-muted-foreground">Event ID:</span>
              <span class="ml-2 font-mono">{{ selectedEvent.eventId }}</span>
            </div>
            <div v-if="selectedEvent.retry">
              <span class="text-muted-foreground">Retry:</span>
              <span class="ml-2 font-mono">{{ selectedEvent.retry }}ms</span>
            </div>
          </div>
          
          <div>
            <span class="text-sm text-muted-foreground">Data:</span>
            <pre class="mt-1 p-3 rounded-md bg-muted font-mono text-sm overflow-auto max-h-96">{{ tryParseJson(selectedEvent.data) }}</pre>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
