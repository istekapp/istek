<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { WebSocketRequest, WebSocketMessage } from '~/types'
import { generateId } from '~/lib/utils'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as WebSocketRequest)
const wsState = computed(() => activeTab.value.wsState!)

const messageInput = ref('')
const messagesContainer = ref<HTMLElement>()

// Listen for WebSocket messages
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await listen<WebSocketMessage>('ws-message', (event) => {
    if (event.payload.connection_id === wsState.value.connectionId) {
      store.addWsMessage(event.payload)
      nextTick(() => {
        messagesContainer.value?.scrollTo({ top: messagesContainer.value.scrollHeight, behavior: 'smooth' })
      })

      if (event.payload.type === 'close' || event.payload.type === 'error') {
        store.updateWsState({ connected: false, connectionId: null })
      }
    }
  })
})

onUnmounted(() => {
  unlisten?.()
})

const connect = async () => {
  if (!request.value.url) return
  
  store.setActiveLoading(true)
  try {
    const connectionId = generateId()
    const headers = request.value.headers
      .filter(h => h.enabled && h.key)
      .reduce((acc, h) => ({ ...acc, [h.key]: h.value }), {})
    
    await invoke('ws_connect', {
      connectionId,
      url: request.value.url,
      headers,
    })
    
    store.updateWsState({ connected: true, connectionId })
  } catch (error: any) {
    store.addWsMessage({
      id: generateId(),
      connection_id: '',
      direction: 'received',
      data: `Connection error: ${error}`,
      timestamp: Date.now(),
      type: 'error',
    } as any)
  } finally {
    store.setActiveLoading(false)
  }
}

const disconnect = async () => {
  if (wsState.value.connectionId) {
    await invoke('ws_disconnect', { connectionId: wsState.value.connectionId })
    store.updateWsState({ connected: false, connectionId: null })
  }
}

const sendMessage = async () => {
  if (!messageInput.value || !wsState.value.connectionId) return
  
  await invoke('ws_send', {
    connectionId: wsState.value.connectionId,
    message: messageInput.value,
    messageType: request.value.messageType,
  })
  
  messageInput.value = ''
}

const formatTime = (timestamp: number) => {
  return new Date(timestamp).toLocaleTimeString()
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- URL Bar -->
    <div class="flex items-center gap-2 border-b border-border p-3">
      <div class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1">
        <span class="text-xs font-mono text-green-500">WS</span>
      </div>
      
      <UiInput
        :model-value="request.url"
        placeholder="wss://echo.websocket.org"
        class="flex-1 font-mono"
        @update:model-value="store.updateActiveRequest({ url: $event })"
      />

      <UiButton
        v-if="!wsState.connected"
        :disabled="activeTab.isLoading || !request.url"
        @click="connect"
      >
        <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
        <Icon v-else name="lucide:plug" class="mr-2 h-4 w-4" />
        Connect
      </UiButton>
      <UiButton
        v-else
        variant="destructive"
        @click="disconnect"
      >
        <Icon name="lucide:plug-off" class="mr-2 h-4 w-4" />
        Disconnect
      </UiButton>
    </div>

    <!-- Connection Status -->
    <div class="flex items-center justify-between border-b border-border px-3 py-2">
      <div class="flex items-center gap-2">
        <span
          :class="[
            'h-2 w-2 rounded-full',
            wsState.connected ? 'bg-green-500' : 'bg-muted-foreground'
          ]"
        />
        <span class="text-sm text-muted-foreground">
          {{ wsState.connected ? 'Connected' : 'Disconnected' }}
        </span>
      </div>
      <UiButton v-if="wsState.messages.length > 0" variant="ghost" size="sm" @click="store.clearWsMessages()">
        <Icon name="lucide:trash-2" class="mr-1 h-3 w-3" />
        Clear
      </UiButton>
    </div>

    <!-- Messages -->
    <div ref="messagesContainer" class="flex-1 overflow-auto p-3 space-y-2">
      <div
        v-for="msg in wsState.messages"
        :key="msg.id"
        :class="[
          'rounded-lg p-3 max-w-[80%]',
          msg.direction === 'sent' 
            ? 'ml-auto bg-primary text-primary-foreground' 
            : 'bg-muted'
        ]"
      >
        <div class="flex items-center gap-2 mb-1">
          <Icon
            :name="msg.direction === 'sent' ? 'lucide:arrow-up-right' : 'lucide:arrow-down-left'"
            class="h-3 w-3"
          />
          <span class="text-xs opacity-70">
            {{ msg.type.toUpperCase() }} - {{ formatTime(msg.timestamp) }}
          </span>
        </div>
        <pre class="text-sm whitespace-pre-wrap break-all font-mono">{{ msg.data }}</pre>
      </div>
      
      <div v-if="wsState.messages.length === 0" class="flex items-center justify-center h-full text-muted-foreground">
        <div class="text-center">
          <Icon name="lucide:message-circle" class="mx-auto h-12 w-12 opacity-50" />
          <p class="mt-2">No messages yet</p>
          <p class="text-sm opacity-70">Connect and start sending messages</p>
        </div>
      </div>
    </div>

    <!-- Message Input -->
    <div class="border-t border-border p-3">
      <div class="flex items-center gap-2">
        <select
          :value="request.messageType"
          class="h-10 rounded-md border border-input bg-background px-2 text-sm"
          @change="store.updateActiveRequest({ messageType: ($event.target as HTMLSelectElement).value as any })"
        >
          <option value="text">Text</option>
          <option value="binary">Binary</option>
        </select>
        <UiInput
          v-model="messageInput"
          placeholder="Type a message..."
          class="flex-1"
          :disabled="!wsState.connected"
          @keyup.enter="sendMessage"
        />
        <UiButton :disabled="!wsState.connected || !messageInput" @click="sendMessage">
          <Icon name="lucide:send" class="h-4 w-4" />
        </UiButton>
      </div>
    </div>
  </div>
</template>
