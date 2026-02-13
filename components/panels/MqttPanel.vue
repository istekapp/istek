<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { MqttRequest, MqttMessage } from '~/types'
import { generateId } from '~/lib/utils'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as MqttRequest)
const mqttState = computed(() => activeTab.value.mqttState!)

const subscribeTopicInput = ref('')
const publishMessage = ref('')
const publishTopic = ref('')
const messagesContainer = ref<HTMLElement>()

// Listen for MQTT messages
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await listen<MqttMessage>('mqtt-message', (event) => {
    if (event.payload.connection_id === mqttState.value.connectionId) {
      store.addMqttMessage(event.payload)
      nextTick(() => {
        messagesContainer.value?.scrollTo({ top: messagesContainer.value.scrollHeight, behavior: 'smooth' })
      })

      if (event.payload.type === 'disconnect' || event.payload.type === 'error') {
        store.updateMqttState({ connected: false, connectionId: null })
      }
    }
  })
})

onUnmounted(() => {
  unlisten?.()
})

const connect = async () => {
  if (!request.value.broker) return
  
  store.setActiveLoading(true)
  try {
    const connectionId = generateId()
    
    await invoke('mqtt_connect', {
      connectionId,
      broker: request.value.broker,
      port: request.value.port,
      clientId: request.value.clientId,
      username: request.value.username || null,
      password: request.value.password || null,
      useTls: request.value.useTls,
    })
    
    store.updateMqttState({ connected: true, connectionId })
  } catch (error: any) {
    store.addMqttMessage({
      id: generateId(),
      connection_id: '',
      topic: '',
      payload: `Connection error: ${error}`,
      qos: 0,
      retained: false,
      timestamp: Date.now(),
      direction: 'received',
      type: 'error',
    } as any)
  } finally {
    store.setActiveLoading(false)
  }
}

const disconnect = async () => {
  if (mqttState.value.connectionId) {
    await invoke('mqtt_disconnect', { connectionId: mqttState.value.connectionId })
    store.updateMqttState({ connected: false, connectionId: null, subscribedTopics: [] })
  }
}

const subscribe = async () => {
  if (!subscribeTopicInput.value || !mqttState.value.connectionId) return
  
  await invoke('mqtt_subscribe', {
    connectionId: mqttState.value.connectionId,
    topic: subscribeTopicInput.value,
    qos: request.value.qos,
  })
  
  store.updateMqttState({
    subscribedTopics: [...mqttState.value.subscribedTopics, subscribeTopicInput.value]
  })
  subscribeTopicInput.value = ''
}

const publish = async () => {
  if (!publishMessage.value || !publishTopic.value || !mqttState.value.connectionId) return
  
  await invoke('mqtt_publish', {
    connectionId: mqttState.value.connectionId,
    topic: publishTopic.value,
    payload: publishMessage.value,
    qos: request.value.qos,
    retain: request.value.retain,
  })
  
  publishMessage.value = ''
}

const formatTime = (timestamp: number) => {
  return new Date(timestamp).toLocaleTimeString()
}
</script>

<template>
  <div class="flex h-full">
    <!-- Left Panel - Connection -->
    <div class="flex w-80 flex-col border-r border-border">
      <div class="border-b border-border p-3">
        <h3 class="font-medium mb-3">Connection</h3>
        <div class="space-y-2">
          <UiInput
            :model-value="request.broker"
            placeholder="Broker (e.g., broker.hivemq.com)"
            @update:model-value="store.updateActiveRequest({ broker: $event })"
          />
          <div class="flex gap-2">
            <UiInput
              :model-value="String(request.port)"
              type="number"
              placeholder="Port"
              class="w-24"
              @update:model-value="store.updateActiveRequest({ port: parseInt($event) || 1883 })"
            />
            <UiInput
              :model-value="request.clientId"
              placeholder="Client ID"
              class="flex-1"
              @update:model-value="store.updateActiveRequest({ clientId: $event })"
            />
          </div>
          <UiInput
            :model-value="request.username"
            placeholder="Username (optional)"
            @update:model-value="store.updateActiveRequest({ username: $event })"
          />
          <UiInput
            :model-value="request.password"
            type="password"
            placeholder="Password (optional)"
            @update:model-value="store.updateActiveRequest({ password: $event })"
          />
          <div class="flex items-center gap-4">
            <label class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                :checked="request.useTls"
                class="rounded"
                @change="store.updateActiveRequest({ useTls: ($event.target as HTMLInputElement).checked })"
              />
              Use TLS
            </label>
            <UiSelect
              :model-value="String(request.qos)"
              :options="[{ value: '0', label: 'QoS 0' }, { value: '1', label: 'QoS 1' }, { value: '2', label: 'QoS 2' }]"
              class="h-8 w-24 text-sm"
              @update:model-value="store.updateActiveRequest({ qos: parseInt($event) as 0 | 1 | 2 })"
            />
          </div>
          
          <UiButton
            v-if="!mqttState.connected"
            :disabled="activeTab.isLoading || !request.broker"
            class="w-full"
            @click="connect"
          >
            <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
            <Icon v-else name="lucide:plug" class="mr-2 h-4 w-4" />
            Connect
          </UiButton>
          <UiButton v-else variant="destructive" class="w-full" @click="disconnect">
            <Icon name="lucide:plug-off" class="mr-2 h-4 w-4" />
            Disconnect
          </UiButton>
        </div>
      </div>

      <!-- Subscribe -->
      <div class="border-b border-border p-3">
        <h3 class="font-medium mb-2">Subscribe</h3>
        <div class="flex gap-2">
          <UiInput
            v-model="subscribeTopicInput"
            placeholder="Topic (e.g., test/#)"
            class="flex-1"
            :disabled="!mqttState.connected"
            @keyup.enter="subscribe"
          />
          <UiButton :disabled="!mqttState.connected || !subscribeTopicInput" size="icon" @click="subscribe">
            <Icon name="lucide:plus" class="h-4 w-4" />
          </UiButton>
        </div>
        <div v-if="mqttState.subscribedTopics.length > 0" class="mt-2 flex flex-wrap gap-1">
          <span
            v-for="topic in mqttState.subscribedTopics"
            :key="topic"
            class="rounded bg-muted px-2 py-0.5 text-xs"
          >
            {{ topic }}
          </span>
        </div>
      </div>

      <!-- Publish -->
      <div class="p-3">
        <h3 class="font-medium mb-2">Publish</h3>
        <div class="space-y-2">
          <UiInput
            v-model="publishTopic"
            placeholder="Topic"
            :disabled="!mqttState.connected"
          />
          <textarea
            v-model="publishMessage"
            placeholder="Message"
            class="w-full rounded-md border border-input bg-background p-2 text-sm min-h-20"
            :disabled="!mqttState.connected"
          />
          <div class="flex items-center justify-between">
            <label class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                :checked="request.retain"
                class="rounded"
                @change="store.updateActiveRequest({ retain: ($event.target as HTMLInputElement).checked })"
              />
              Retain
            </label>
            <UiButton :disabled="!mqttState.connected || !publishMessage || !publishTopic" @click="publish">
              <Icon name="lucide:send" class="mr-2 h-4 w-4" />
              Publish
            </UiButton>
          </div>
        </div>
      </div>
    </div>

    <!-- Right Panel - Messages -->
    <div class="flex flex-1 flex-col">
      <div class="flex items-center justify-between border-b border-border px-3 py-2">
        <div class="flex items-center gap-2">
          <span
            :class="[
              'h-2 w-2 rounded-full',
              mqttState.connected ? 'bg-green-500' : 'bg-muted-foreground'
            ]"
          />
          <span class="text-sm">{{ mqttState.connected ? 'Connected' : 'Disconnected' }}</span>
        </div>
        <UiButton v-if="mqttState.messages.length > 0" variant="ghost" size="sm" @click="store.clearMqttMessages()">
          <Icon name="lucide:trash-2" class="mr-1 h-3 w-3" />
          Clear
        </UiButton>
      </div>

      <div ref="messagesContainer" class="flex-1 overflow-auto p-3 space-y-2">
        <div
          v-for="msg in mqttState.messages"
          :key="msg.id"
          :class="[
            'rounded-lg p-3',
            msg.direction === 'sent' ? 'bg-primary/10 border border-primary/20' : 'bg-muted'
          ]"
        >
          <div class="flex items-center justify-between mb-1">
            <div class="flex items-center gap-2">
              <Icon
                :name="msg.direction === 'sent' ? 'lucide:arrow-up-right' : 'lucide:arrow-down-left'"
                :class="msg.direction === 'sent' ? 'text-primary' : 'text-muted-foreground'"
                class="h-3 w-3"
              />
              <span v-if="msg.topic" class="text-xs font-medium">{{ msg.topic }}</span>
              <span class="text-xs text-muted-foreground">QoS {{ msg.qos }}</span>
              <span v-if="msg.retained" class="text-xs text-muted-foreground">(retained)</span>
            </div>
            <span class="text-xs text-muted-foreground">{{ formatTime(msg.timestamp) }}</span>
          </div>
          <pre class="text-sm whitespace-pre-wrap break-all font-mono">{{ msg.payload }}</pre>
        </div>

        <div v-if="mqttState.messages.length === 0" class="flex items-center justify-center h-full text-muted-foreground">
          <div class="text-center">
            <Icon name="lucide:radio-tower" class="mx-auto h-12 w-12 opacity-50" />
            <p class="mt-2">No messages yet</p>
            <p class="text-sm opacity-70">Connect, subscribe and start messaging</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
