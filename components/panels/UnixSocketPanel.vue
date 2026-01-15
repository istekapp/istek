<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { UnixSocketRequest, HttpResponse } from '~/types'
import { formatBytes, formatDuration, getStatusColor, tryParseJson } from '~/lib/utils'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as UnixSocketRequest)
const response = computed(() => activeTab.value.response as HttpResponse | null)

const methods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD']
const requestTab = ref<'headers' | 'body'>('headers')

const sendRequest = async () => {
  if (!request.value.socketPath || !request.value.path) return
  
  store.setActiveLoading(true)
  store.setActiveResponse(null)

  try {
    const headers = request.value.headers
      .filter(h => h.enabled && h.key)
      .reduce((acc, h) => ({ ...acc, [h.key]: h.value }), {})
    
    const result = await invoke<HttpResponse>('send_unix_socket_request', {
      socketPath: request.value.socketPath,
      method: request.value.method,
      path: request.value.path,
      headers,
      body: request.value.body || null,
    })
    
    store.setActiveResponse(result)
    store.addToHistory(request.value, result)
  } catch (error: any) {
    store.setActiveResponse({
      status: 0,
      statusText: 'Error',
      headers: {},
      body: error.toString(),
      time: 0,
      size: 0,
    })
  } finally {
    store.setActiveLoading(false)
  }
}

const formattedBody = computed(() => {
  if (!response.value) return ''
  return tryParseJson(response.value.body)
})
</script>

<template>
  <div class="flex h-full">
    <!-- Left Panel - Request -->
    <div class="flex w-1/2 flex-col border-r border-border">
      <!-- Socket Path & Method -->
      <div class="border-b border-border p-3 space-y-2">
        <div class="flex items-center gap-2">
          <div class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1">
            <Icon name="lucide:plug" class="h-3 w-3 text-orange-500" />
            <span class="text-xs font-mono text-orange-500">UNIX</span>
          </div>
          <UiInput
            :model-value="request.socketPath"
            placeholder="/var/run/docker.sock"
            class="flex-1 font-mono"
            @update:model-value="store.updateActiveRequest({ socketPath: $event })"
          />
        </div>
        <div class="flex items-center gap-2">
          <select
            :value="request.method"
            class="h-10 rounded-md border border-input bg-background px-3 font-mono text-sm font-semibold"
            @change="store.updateActiveRequest({ method: ($event.target as HTMLSelectElement).value as any })"
          >
            <option v-for="m in methods" :key="m" :value="m">{{ m }}</option>
          </select>
          <UiInput
            :model-value="request.path"
            placeholder="/v1.43/containers/json"
            class="flex-1 font-mono"
            @update:model-value="store.updateActiveRequest({ path: $event })"
          />
          <UiButton
            :disabled="activeTab.isLoading || !request.socketPath || !request.path"
            @click="sendRequest"
            class="gap-2"
          >
            <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="h-4 w-4 animate-spin" />
            <Icon v-else name="lucide:play" class="h-4 w-4 fill-current" />
            Run
          </UiButton>
        </div>
      </div>

      <!-- Request Tabs -->
      <div class="flex border-b border-border">
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            requestTab === 'headers'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="requestTab = 'headers'"
        >
          Headers
        </button>
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            requestTab === 'body'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="requestTab = 'body'"
        >
          Body
        </button>
      </div>

      <!-- Request Content -->
      <div class="flex-1 overflow-auto p-3">
        <template v-if="requestTab === 'headers'">
          <div class="space-y-2">
            <div
              v-for="header in request.headers"
              :key="header.id"
              class="flex items-center gap-2"
            >
              <input
                type="checkbox"
                :checked="header.enabled"
                class="h-4 w-4 rounded border-input accent-primary"
                @change="store.toggleHeader(header.id)"
              />
              <UiHeaderKeyInput
                :model-value="header.key"
                placeholder="Header name"
                class="flex-1"
                @update:model-value="store.updateHeader(header.id, 'key', $event)"
              />
              <UiHeaderValueInput
                :model-value="header.value"
                :header-key="header.key"
                placeholder="Value"
                class="flex-1"
                @update:model-value="store.updateHeader(header.id, 'value', $event)"
              />
              <UiButton
                variant="ghost"
                size="icon"
                class="h-8 w-8"
                @click="store.removeHeader(header.id)"
              >
                <Icon name="lucide:trash-2" class="h-4 w-4" />
              </UiButton>
            </div>
            <UiButton variant="outline" size="sm" @click="store.addHeader()">
              <Icon name="lucide:plus" class="mr-2 h-4 w-4" />
              Add Header
            </UiButton>
          </div>
        </template>
        <template v-else>
          <ClientOnly>
            <CodeEditor
              :model-value="request.body"
              language="json"
              min-height="300px"
              @update:model-value="store.updateActiveRequest({ body: $event })"
            />
          </ClientOnly>
        </template>
      </div>
    </div>

    <!-- Right Panel - Response -->
    <div class="flex w-1/2 flex-col">
      <div class="flex items-center justify-between border-b border-border px-3 py-2">
        <span class="text-sm font-medium">Response</span>
        <div v-if="response" class="flex items-center gap-2 text-xs">
          <span :class="getStatusColor(response.status)">
            {{ response.status }} {{ response.statusText }}
          </span>
          <span class="text-muted-foreground">{{ formatDuration(response.time) }}</span>
          <span class="text-muted-foreground">{{ formatBytes(response.size) }}</span>
        </div>
      </div>

      <div v-if="activeTab.isLoading" class="flex flex-1 items-center justify-center">
        <Icon name="lucide:loader-2" class="h-8 w-8 animate-spin text-primary" />
      </div>

      <div v-else-if="!response" class="flex flex-1 items-center justify-center text-muted-foreground">
        <div class="text-center">
          <Icon name="lucide:plug" class="mx-auto h-12 w-12 opacity-50" />
          <p class="mt-2">Send a request to see the response</p>
          <p class="text-sm opacity-70 mt-1">Common sockets: Docker, containerd, snapd</p>
        </div>
      </div>

      <div v-else class="flex-1 overflow-hidden">
        <ClientOnly>
          <CodeEditor
            :model-value="formattedBody"
            language="json"
            :readonly="true"
            min-height="100%"
          />
        </ClientOnly>
      </div>
    </div>
  </div>
</template>
