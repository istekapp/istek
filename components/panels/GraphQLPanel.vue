<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { buildClientSchema, getIntrospectionQuery, type IntrospectionQuery } from 'graphql'
import type { GraphQLRequest, GraphQLResponse } from '~/types'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as GraphQLRequest)
const response = computed(() => activeTab.value.response as GraphQLResponse | null)

const activeQueryTab = ref<'query' | 'variables'>('query')
const graphqlSchema = ref<any>(null)
const isLoadingSchema = ref(false)
const schemaError = ref<string | null>(null)
const queryEditorRef = ref<any>(null)

// Fetch GraphQL schema for autocomplete
const fetchSchema = async () => {
  if (!request.value.url) return
  
  isLoadingSchema.value = true
  schemaError.value = null
  
  try {
    const headers = request.value.headers
      .filter(h => h.enabled && h.key)
      .reduce((acc, h) => ({ ...acc, [h.key]: h.value }), {})
    
    const result = await invoke<GraphQLResponse>('send_graphql_request', {
      url: request.value.url,
      headers,
      query: getIntrospectionQuery(),
      variables: null,
      operationName: null,
    })
    
    if (result.data) {
      const schema = buildClientSchema(result.data as unknown as IntrospectionQuery)
      graphqlSchema.value = schema
      // Update editor schema for autocomplete
      nextTick(() => {
        if (queryEditorRef.value) {
          queryEditorRef.value.updateGraphQLSchema(schema)
        }
      })
    } else if (result.errors) {
      schemaError.value = result.errors[0]?.message || 'Failed to fetch schema'
    }
  } catch (error: any) {
    schemaError.value = error.toString()
    console.error('Failed to fetch GraphQL schema:', error)
  } finally {
    isLoadingSchema.value = false
  }
}

// Auto-fetch schema when URL changes
watch(() => request.value.url, (newUrl) => {
  if (newUrl && newUrl.trim()) {
    // Debounce schema fetch
    const timeout = setTimeout(() => {
      fetchSchema()
    }, 500)
    return () => clearTimeout(timeout)
  }
}, { immediate: true })

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

        <!-- Schema status indicator -->
        <div class="flex items-center gap-1">
          <button
            v-if="isLoadingSchema"
            class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1 text-xs text-muted-foreground"
            disabled
          >
            <Icon name="lucide:loader-2" class="h-3 w-3 animate-spin" />
            <span>Loading schema...</span>
          </button>
          <button
            v-else-if="graphqlSchema"
            class="flex items-center gap-1 rounded-md border border-green-500/30 bg-green-500/10 px-2 py-1 text-xs text-green-500 hover:bg-green-500/20"
            title="Schema loaded - autocomplete enabled. Click to refresh."
            @click="fetchSchema"
          >
            <Icon name="lucide:check-circle" class="h-3 w-3" />
            <span>Schema</span>
          </button>
          <button
            v-else-if="schemaError"
            class="flex items-center gap-1 rounded-md border border-destructive/30 bg-destructive/10 px-2 py-1 text-xs text-destructive hover:bg-destructive/20"
            :title="schemaError + ' - Click to retry'"
            @click="fetchSchema"
          >
            <Icon name="lucide:alert-circle" class="h-3 w-3" />
            <span>Schema</span>
          </button>
          <button
            v-else-if="request.url"
            class="flex items-center gap-1 rounded-md border border-input bg-muted px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
            title="Click to fetch schema for autocomplete"
            @click="fetchSchema"
          >
            <Icon name="lucide:download" class="h-3 w-3" />
            <span>Fetch Schema</span>
          </button>
        </div>

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
              ref="queryEditorRef"
              :model-value="request.query"
              language="graphql"
              :graphql-schema="graphqlSchema"
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
