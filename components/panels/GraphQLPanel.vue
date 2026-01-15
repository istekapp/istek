<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { GraphQLRequest, GraphQLResponse } from '~/types'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as GraphQLRequest)
const response = computed(() => activeTab.value.response as GraphQLResponse | null)

const activeQueryTab = ref<'query' | 'variables'>('query')

const emit = defineEmits<{
  send: []
}>()

const sendRequest = async () => {
  if (!request.value.url) return
  
  store.setActiveLoading(true)
  store.setActiveResponse(null)

  try {
    const headers = request.value.headers
      .filter(h => h.enabled && h.key)
      .reduce((acc, h) => ({ ...acc, [h.key]: h.value }), {})
    
    const result = await invoke<GraphQLResponse>('send_graphql_request', {
      url: request.value.url,
      headers,
      query: request.value.query,
      variables: request.value.variables || null,
      operationName: request.value.operationName || null,
    })
    
    store.setActiveResponse(result)
    store.addToHistory(request.value, result)
  } catch (error: any) {
    store.setActiveResponse({
      data: null,
      errors: [{ message: error.toString() }],
      time: 0,
    })
  } finally {
    store.setActiveLoading(false)
  }
}

const formatResponse = computed(() => {
  if (!response.value) return ''
  return JSON.stringify(response.value.data || response.value.errors, null, 2)
})
</script>

<template>
  <div class="flex h-full">
    <!-- Left Panel - Query -->
    <div class="flex w-1/2 flex-col border-r border-border">
      <!-- URL Bar -->
      <div class="flex items-center gap-2 border-b border-border p-3">
        <div class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1">
          <span class="text-xs font-mono text-pink-500">GQL</span>
        </div>
        
        <UiInput
          :model-value="request.url"
          placeholder="https://api.example.com/graphql"
          class="flex-1 font-mono"
          @update:model-value="store.updateActiveRequest({ url: $event })"
        />

        <UiButton
          :disabled="activeTab.isLoading || !request.url"
          @click="sendRequest"
        >
          <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
          <Icon v-else name="lucide:play" class="mr-2 h-4 w-4" />
          Run
        </UiButton>
      </div>

      <!-- Query Tabs -->
      <div class="flex border-b border-border">
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            activeQueryTab === 'query'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeQueryTab = 'query'"
        >
          Query
        </button>
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            activeQueryTab === 'variables'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeQueryTab = 'variables'"
        >
          Variables
        </button>
        <button
          :class="[
            'px-4 py-2 text-sm font-medium transition-colors',
            activeQueryTab === 'headers'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          ]"
          @click="activeQueryTab = 'headers' as any"
        >
          Headers
        </button>
      </div>

      <!-- Query Editor -->
      <div class="flex-1 overflow-hidden">
        <ClientOnly>
          <template v-if="activeQueryTab === 'query'">
            <CodeEditor
              :model-value="request.query"
              language="text"
              min-height="100%"
              @update:model-value="store.updateActiveRequest({ query: $event })"
            />
          </template>
          <template v-else-if="activeQueryTab === 'variables'">
            <CodeEditor
              :model-value="request.variables"
              language="json"
              min-height="100%"
              @update:model-value="store.updateActiveRequest({ variables: $event })"
            />
          </template>
          <template v-else>
            <div class="p-3 space-y-2">
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
        </ClientOnly>
      </div>
    </div>

    <!-- Right Panel - Response -->
    <div class="flex w-1/2 flex-col">
      <div class="flex items-center justify-between border-b border-border px-3 py-2">
        <span class="text-sm font-medium">Response</span>
        <div v-if="response" class="flex items-center gap-2 text-xs text-muted-foreground">
          <span>{{ response.time }}ms</span>
          <span
            v-if="response.errors"
            class="rounded bg-destructive/20 px-1.5 py-0.5 text-destructive"
          >
            {{ response.errors.length }} error(s)
          </span>
        </div>
      </div>

      <div v-if="activeTab.isLoading" class="flex flex-1 items-center justify-center">
        <Icon name="lucide:loader-2" class="h-8 w-8 animate-spin text-primary" />
      </div>

      <div v-else-if="!response" class="flex flex-1 items-center justify-center text-muted-foreground">
        <div class="text-center">
          <Icon name="lucide:hexagon" class="mx-auto h-12 w-12 opacity-50" />
          <p class="mt-2">Run a query to see results</p>
        </div>
      </div>

      <div v-else class="flex-1 overflow-hidden">
        <ClientOnly>
          <CodeEditor
            :model-value="formatResponse"
            language="json"
            :readonly="true"
            min-height="100%"
          />
        </ClientOnly>
      </div>
    </div>
  </div>
</template>
