<script setup lang="ts">
import type { ProtocolType } from '~/types'

const store = useAppStore()
const { activeTab } = store

const protocols: { value: ProtocolType; label: string; icon: string; color: string }[] = [
  { value: 'http', label: 'HTTP', icon: 'lucide:globe', color: 'text-protocol-http' },
  { value: 'websocket', label: 'WebSocket', icon: 'lucide:radio', color: 'text-protocol-ws' },
  { value: 'sse', label: 'SSE', icon: 'lucide:activity', color: 'text-protocol-sse' },
  { value: 'graphql', label: 'GraphQL', icon: 'lucide:hexagon', color: 'text-protocol-graphql' },
  { value: 'grpc', label: 'gRPC', icon: 'lucide:cpu', color: 'text-protocol-grpc' },
  { value: 'mqtt', label: 'MQTT', icon: 'lucide:radio-tower', color: 'text-protocol-mqtt' },
  { value: 'unix-socket', label: 'Unix Socket', icon: 'lucide:plug', color: 'text-protocol-unix' },
  { value: 'mcp', label: 'MCP', icon: 'lucide:bot', color: 'text-protocol-mcp' },
]

const showDropdown = ref(false)
const dropdownRef = useClickOutside(() => {
  showDropdown.value = false
})

const currentProtocol = computed(() => 
  protocols.find(p => p.value === activeTab.value.protocol) || protocols[0]
)

const selectProtocol = (protocol: ProtocolType) => {
  store.changeProtocol(protocol)
  showDropdown.value = false
}
</script>

<template>
  <div ref="dropdownRef" class="relative">
    <button
      class="flex items-center gap-2 rounded-md border border-input bg-background px-4 py-2.5 text-base font-medium hover:bg-accent transition-colors"
      @click="showDropdown = !showDropdown"
    >
      <Icon :name="currentProtocol.icon" :class="['h-5 w-5', currentProtocol.color]" />
      <span>{{ currentProtocol.label }}</span>
      <Icon name="lucide:chevron-down" class="h-5 w-5 text-muted-foreground" />
    </button>
    
    <div
      v-if="showDropdown"
      class="absolute left-0 top-full z-50 mt-1 w-52 rounded-md border border-border bg-popover p-1.5 shadow-lg"
    >
      <button
        v-for="protocol in protocols"
        :key="protocol.value"
        :class="[
          'flex w-full items-center gap-3 rounded-md px-4 py-2.5 text-base transition-colors',
          protocol.value === activeTab.protocol 
            ? 'bg-accent text-accent-foreground' 
            : 'hover:bg-accent hover:text-accent-foreground'
        ]"
        @click="selectProtocol(protocol.value)"
      >
        <Icon :name="protocol.icon" :class="['h-5 w-5', protocol.color]" />
        <span>{{ protocol.label }}</span>
      </button>
    </div>
  </div>
</template>
