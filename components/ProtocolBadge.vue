<script setup lang="ts">
import type { RequestType, HttpMethod } from '~/types'

const props = defineProps<{
  request: RequestType
  size?: 'sm' | 'md'
}>()

const size = props.size || 'md'

// Get display info based on protocol
const displayInfo = computed(() => {
  const request = props.request
  
  switch (request.protocol) {
    case 'http':
      return {
        icon: 'lucide:globe',
        label: request.method,
        color: getMethodColor(request.method),
        url: request.url
      }
    case 'websocket':
      return {
        icon: 'lucide:plug',
        label: 'WS',
        color: 'bg-protocol-ws/25 text-protocol-ws',
        url: request.url
      }
    case 'graphql':
      return {
        icon: 'lucide:hexagon',
        label: 'GQL',
        color: 'bg-protocol-graphql/25 text-protocol-graphql',
        url: request.url
      }
    case 'grpc':
      return {
        icon: 'lucide:cpu',
        label: 'gRPC',
        color: 'bg-protocol-grpc/25 text-protocol-grpc',
        url: request.url
      }
    case 'mqtt':
      return {
        icon: 'lucide:radio',
        label: 'MQTT',
        color: 'bg-protocol-mqtt/25 text-protocol-mqtt',
        url: `${request.broker}:${request.port}`
      }
    case 'unix-socket':
      return {
        icon: 'lucide:terminal',
        label: request.method,
        color: getMethodColor(request.method),
        url: `${request.socketPath}${request.path}`
      }
    case 'mcp':
      return {
        icon: 'lucide:cpu',
        label: 'MCP',
        color: 'bg-protocol-mcp/25 text-protocol-mcp',
        url: request.command || ''
      }
    default:
      return {
        icon: 'lucide:circle',
        label: '?',
        color: 'bg-muted text-muted-foreground',
        url: ''
      }
  }
})

function getMethodColor(method: HttpMethod): string {
  const colors: Record<HttpMethod, string> = {
    GET: 'bg-method-get/25 text-method-get',
    POST: 'bg-method-post/25 text-method-post',
    PUT: 'bg-method-put/25 text-method-put',
    PATCH: 'bg-method-patch/25 text-method-patch',
    DELETE: 'bg-method-delete/25 text-method-delete',
    HEAD: 'bg-method-head/25 text-method-head',
    OPTIONS: 'bg-method-options/25 text-method-options',
  }
  return colors[method] || colors.GET
}
</script>

<template>
  <span
    :class="[
      'inline-flex items-center justify-center rounded font-mono font-semibold uppercase',
      displayInfo.color,
      size === 'sm' ? 'h-6 min-w-[3rem] px-1.5 text-xs' : 'h-7 min-w-[3.5rem] px-2 text-sm'
    ]"
  >
    {{ displayInfo.label }}
  </span>
</template>
