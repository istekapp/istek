<script setup lang="ts">
import type { HttpRequest, WebSocketRequest, GraphQLRequest, MqttRequest, UnixSocketRequest, Tab, RequestTab, TestTab, MockTab } from '~/types'

const store = useAppStore()
const { tabs, activeTabId, activeTab } = store

const isRequestTab = (tab: Tab): tab is RequestTab => {
  return tab.type === 'request' || !('type' in tab)
}

const isTestTab = (tab: Tab): tab is TestTab => {
  return tab.type === 'test'
}

const isMockTab = (tab: Tab): tab is MockTab => {
  return tab.type === 'mock'
}

const getProtocolIcon = (tab: Tab) => {
  if (isTestTab(tab)) {
    return 'lucide:play-circle'
  }
  if (isMockTab(tab)) {
    return 'lucide:server'
  }
  
  const protocol = (tab as RequestTab).protocol
  switch (protocol) {
    case 'http': return 'lucide:globe'
    case 'websocket': return 'lucide:radio'
    case 'graphql': return 'lucide:hexagon'
    case 'mqtt': return 'lucide:radio-tower'
    case 'unix-socket': return 'lucide:plug'
    case 'mcp': return 'lucide:cpu'
    default: return 'lucide:globe'
  }
}

const getProtocolColor = (tab: Tab) => {
  if (isTestTab(tab)) {
    return 'text-green-500'
  }
  if (isMockTab(tab)) {
    return 'text-orange-500'
  }
  
  const protocol = (tab as RequestTab).protocol
  switch (protocol) {
    case 'http': return 'text-protocol-http'
    case 'websocket': return 'text-protocol-ws'
    case 'graphql': return 'text-protocol-graphql'
    case 'mqtt': return 'text-protocol-mqtt'
    case 'unix-socket': return 'text-protocol-unix'
    case 'mcp': return 'text-purple-500'
    default: return 'text-protocol-http'
  }
}

const getTabLabel = (tab: Tab) => {
  if (isTestTab(tab)) {
    return tab.name || `Test: ${tab.collectionName}`
  }
  if (isMockTab(tab)) {
    return tab.name || `Mock: ${tab.collectionName}`
  }
  
  const reqTab = tab as RequestTab
  const request = reqTab.request
  if (request.name && request.name !== 'New Request' && !request.name.startsWith('New ')) {
    return request.name
  }
  
  switch (reqTab.protocol) {
    case 'http':
      return (request as HttpRequest).url || 'New Request'
    case 'websocket':
      return (request as WebSocketRequest).url || 'New WebSocket'
    case 'graphql':
      return (request as GraphQLRequest).url || 'New GraphQL'
    case 'mqtt':
      return (request as MqttRequest).broker || 'New MQTT'
    case 'unix-socket':
      return (request as UnixSocketRequest).socketPath || 'New Unix Socket'
    case 'mcp':
      return request.name || 'New MCP'
    default:
      return 'New Request'
  }
}

const handleAddTab = () => {
  // Only add request tabs from the + button, use current protocol
  const current = activeTab.value
  if (isRequestTab(current)) {
    store.addTab(current.protocol)
  } else {
    store.addTab('http')
  }
}
</script>

<template>
  <div class="flex flex-1 h-12 items-center bg-muted/30 overflow-hidden min-w-0">
    <!-- Scrollable tabs area -->
    <div class="flex-1 overflow-x-auto scrollbar-thin scrollbar-thumb-border scrollbar-track-transparent min-w-0">
      <div class="flex items-center">
        <div
          v-for="tab in tabs"
          :key="tab.id"
          :class="[
            'group flex h-12 items-center gap-2 border-r border-border px-4 text-base transition-colors cursor-pointer flex-shrink-0',
            tab.id === activeTabId
              ? 'bg-background text-foreground'
              : 'text-muted-foreground hover:bg-background/50 hover:text-foreground'
          ]"
          @click="store.setActiveTab(tab.id)"
        >
          <!-- Protocol/Test Icon -->
          <Icon 
            :name="getProtocolIcon(tab)" 
            :class="['h-4 w-4', getProtocolColor(tab)]" 
          />
          
          <!-- HTTP Method Badge (only for HTTP request tabs) -->
          <MethodBadge 
            v-if="isRequestTab(tab) && tab.protocol === 'http'" 
            :method="(tab.request as HttpRequest).method" 
            size="sm" 
          />
          
          <!-- Tab Label -->
          <span class="max-w-44 truncate">
            {{ getTabLabel(tab) }}
          </span>
          
          <!-- Connection indicator for realtime protocols -->
          <template v-if="isRequestTab(tab)">
            <span 
              v-if="(tab.protocol === 'websocket' && tab.wsState?.connected) || 
                    (tab.protocol === 'mqtt' && tab.mqttState?.connected)"
              class="h-2.5 w-2.5 rounded-full bg-method-get animate-pulse"
            />
            
            <!-- Dirty indicator -->
            <span v-else-if="tab.isDirty" class="h-2.5 w-2.5 rounded-full bg-primary" />
          </template>
          
          <!-- Close button -->
          <button
            class="ml-1 rounded p-1 opacity-0 hover:bg-muted group-hover:opacity-100"
            @click.stop="store.closeTab(tab.id)"
          >
            <Icon name="lucide:x" class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
    
    <!-- Add Tab Button - Fixed position, always visible -->
    <button
      class="flex h-12 w-10 items-center justify-center text-muted-foreground hover:text-foreground hover:bg-background/50 transition-colors flex-shrink-0 border-l border-border"
      @click="handleAddTab"
    >
      <Icon name="lucide:plus" class="h-4 w-4" />
    </button>
  </div>
</template>
