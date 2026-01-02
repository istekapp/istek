<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { PlaygroundStatus } from '~/types'

const isLoading = ref(false)
const status = ref<PlaygroundStatus | null>(null)
const isExpanded = ref(false)
const error = ref<string | null>(null)

// Load status on mount
onMounted(async () => {
  await loadStatus()
})

const loadStatus = async () => {
  try {
    status.value = await invoke<PlaygroundStatus>('playground_status')
  } catch (e: any) {
    console.error('Failed to load playground status:', e)
  }
}

const togglePlayground = async () => {
  isLoading.value = true
  error.value = null

  try {
    if (status.value?.running) {
      await invoke('playground_stop')
    } else {
      await invoke<PlaygroundStatus>('playground_start')
    }
    await loadStatus()
  } catch (e: any) {
    error.value = e.toString()
    console.error('Playground error:', e)
  } finally {
    isLoading.value = false
  }
}

const copyToClipboard = async (text: string) => {
  try {
    await navigator.clipboard.writeText(text)
  } catch (e) {
    console.error('Failed to copy:', e)
  }
}

const endpoints = computed(() => {
  if (!status.value?.running) return []
  
  return [
    { name: 'HTTP API', url: status.value.httpUrl, icon: 'lucide:globe', color: 'text-blue-500' },
    { name: 'WebSocket', url: status.value.wsUrl, icon: 'lucide:radio', color: 'text-green-500' },
    { name: 'GraphQL', url: status.value.graphqlUrl, icon: 'lucide:hexagon', color: 'text-pink-500' },
    { name: 'SSE', url: status.value.sseUrl, icon: 'lucide:activity', color: 'text-orange-400' },
    { name: 'MQTT', url: status.value.mqttUrl, icon: 'lucide:radio-tower', color: 'text-purple-500' },
    { name: 'gRPC', url: status.value.grpcUrl, icon: 'lucide:cpu', color: 'text-amber-500' },
    { name: 'Unix Socket', url: status.value.unixSocket, icon: 'lucide:plug', color: 'text-gray-500' },
    { name: 'OpenAPI Spec', url: status.value.openapiUrl, icon: 'lucide:file-json', color: 'text-cyan-500' },
  ].filter(e => e.url)
})
</script>

<template>
  <div class="relative">
    <!-- Main Toggle Button -->
    <button
      class="flex items-center gap-2 px-3 py-2 rounded-lg transition-colors w-full"
      :class="[
        status?.running 
          ? 'bg-green-500/10 hover:bg-green-500/20 text-green-500' 
          : 'bg-muted hover:bg-accent text-muted-foreground hover:text-foreground'
      ]"
      @click="isExpanded = !isExpanded"
    >
      <!-- Status indicator -->
      <div 
        class="h-2 w-2 rounded-full"
        :class="status?.running ? 'bg-green-500 animate-pulse' : 'bg-gray-400'"
      />
      
      <span class="text-sm font-medium flex-1 text-left">Playground</span>
      
      <Icon 
        :name="isExpanded ? 'lucide:chevron-up' : 'lucide:chevron-down'" 
        class="h-4 w-4" 
      />
    </button>

    <!-- Expanded Panel -->
    <div 
      v-if="isExpanded" 
      class="mt-2 rounded-lg border border-border bg-background p-3 space-y-3"
    >
      <!-- Start/Stop Button -->
      <UiButton
        :variant="status?.running ? 'destructive' : 'default'"
        size="sm"
        class="w-full"
        :disabled="isLoading"
        @click="togglePlayground"
      >
        <Icon 
          v-if="isLoading" 
          name="lucide:loader-2" 
          class="h-4 w-4 mr-2 animate-spin" 
        />
        <Icon 
          v-else 
          :name="status?.running ? 'lucide:square' : 'lucide:play'" 
          class="h-4 w-4 mr-2" 
        />
        {{ status?.running ? 'Stop Playground' : 'Start Playground' }}
      </UiButton>

      <!-- Error message -->
      <div v-if="error" class="text-xs text-destructive p-2 bg-destructive/10 rounded">
        {{ error }}
      </div>

      <!-- Endpoints list when running -->
      <div v-if="status?.running && endpoints.length > 0" class="space-y-2">
        <div class="text-xs text-muted-foreground font-medium uppercase tracking-wider">
          Endpoints
        </div>
        
        <div class="space-y-1.5">
          <div
            v-for="endpoint in endpoints"
            :key="endpoint.name"
            class="group flex items-center gap-2 p-2 rounded-md bg-muted/50 hover:bg-muted transition-colors"
          >
            <Icon :name="endpoint.icon" :class="['h-4 w-4', endpoint.color]" />
            <div class="flex-1 min-w-0">
              <div class="text-xs font-medium">{{ endpoint.name }}</div>
              <div class="text-xs text-muted-foreground truncate font-mono">
                {{ endpoint.url }}
              </div>
            </div>
            <button
              class="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-accent transition-all"
              title="Copy URL"
              @click.stop="copyToClipboard(endpoint.url!)"
            >
              <Icon name="lucide:copy" class="h-3 w-3 text-muted-foreground" />
            </button>
          </div>
        </div>
      </div>

      <!-- Info when stopped -->
      <div v-else-if="!status?.running" class="text-xs text-muted-foreground">
        Start the playground to get demo endpoints for testing HTTP, WebSocket, GraphQL, MQTT, and gRPC.
      </div>
    </div>
  </div>
</template>
