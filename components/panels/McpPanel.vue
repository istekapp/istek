<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { McpRequest, McpTool, McpResource, McpPrompt, McpServerInfo, DiscoveredMcp, McpDiscoveryResult } from '~/types'
import { generateId } from '~/lib/utils'

const store = useAppStore()
const { activeTab } = store

const request = computed(() => activeTab.value.request as McpRequest)
const mcpState = computed(() => activeTab.value.mcpState!)

// Connection mode: 'discover' or 'manual'
const connectionMode = ref<'discover' | 'manual'>('discover')

// Discovery state
const discoveryResults = ref<McpDiscoveryResult[]>([])
const isDiscovering = ref(false)
const selectedSource = ref<string | null>(null)

// Manual input
const manualCommand = ref('')
const manualArgs = ref('')
const manualEnv = ref('')

// Tool execution state
const activeSection = ref<'tools' | 'resources' | 'prompts'>('tools')
const selectedTool = ref<McpTool | null>(null)
const selectedResource = ref<McpResource | null>(null)
const selectedPrompt = ref<McpPrompt | null>(null)
const toolInput = ref('{}')
const promptArgs = ref<Record<string, string>>({})
const resultOutput = ref<string>('')
const isExecuting = ref(false)

// App icons and colors
const appConfig: Record<string, { icon: string; color: string; bgColor: string }> = {
  'Claude Desktop': { icon: 'simple-icons:anthropic', color: 'text-orange-400', bgColor: 'bg-orange-500/10' },
  'VS Code': { icon: 'simple-icons:visualstudiocode', color: 'text-blue-400', bgColor: 'bg-blue-500/10' },
  'Cursor': { icon: 'lucide:pointer', color: 'text-purple-400', bgColor: 'bg-purple-500/10' },
  'Windsurf': { icon: 'lucide:wind', color: 'text-cyan-400', bgColor: 'bg-cyan-500/10' },
  'OpenCode': { icon: 'lucide:terminal', color: 'text-green-400', bgColor: 'bg-green-500/10' },
}

const getAppConfig = (source: string) => {
  return appConfig[source] || { icon: 'lucide:cpu', color: 'text-gray-400', bgColor: 'bg-gray-500/10' }
}

// Discover MCP configs on mount
onMounted(async () => {
  await discoverConfigs()
})

const discoverConfigs = async () => {
  isDiscovering.value = true
  try {
    const results = await invoke<McpDiscoveryResult[]>('mcp_discover_configs')
    discoveryResults.value = results
    
    // Auto-select first source with servers
    const firstWithServers = results.find(r => r.servers.length > 0)
    if (firstWithServers) {
      selectedSource.value = firstWithServers.source
    }
  } catch (error) {
    console.error('Failed to discover MCP configs:', error)
  } finally {
    isDiscovering.value = false
  }
}

const selectedSourceResult = computed(() => {
  return discoveryResults.value.find(r => r.source === selectedSource.value)
})

const allServers = computed(() => {
  return discoveryResults.value.flatMap(r => r.servers)
})

const connectToServer = async (server: DiscoveredMcp) => {
  store.updateActiveRequest({
    command: server.command,
    args: server.args,
    env: server.env,
  })
  
  await connect()
}

const connectManual = async () => {
  const args = manualArgs.value.trim() ? manualArgs.value.trim().split(/\s+/) : []
  const env: Record<string, string> = {}
  
  if (manualEnv.value.trim()) {
    manualEnv.value.split('\n').forEach(line => {
      const idx = line.indexOf('=')
      if (idx > 0) {
        env[line.substring(0, idx).trim()] = line.substring(idx + 1).trim()
      }
    })
  }
  
  store.updateActiveRequest({
    command: manualCommand.value,
    args,
    env,
  })
  
  await connect()
}

const connect = async () => {
  if (!request.value.command) return
  
  store.setActiveLoading(true)
  resultOutput.value = ''
  
  try {
    const result = await invoke<{
      success: boolean
      connectionId: string | null
      serverInfo: McpServerInfo | null
      tools: McpTool[]
      resources: McpResource[]
      prompts: McpPrompt[]
      error: string | null
    }>('mcp_connect', {
      command: request.value.command,
      args: request.value.args || [],
      env: request.value.env || {},
    })
    
    if (result.success && result.connectionId) {
      store.updateMcpState({
        connected: true,
        connectionId: result.connectionId,
        serverInfo: result.serverInfo || undefined,
        tools: result.tools,
        resources: result.resources,
        prompts: result.prompts,
      })
      
      if (result.tools.length > 0) {
        selectedTool.value = result.tools[0]
        generateToolInputTemplate(result.tools[0])
      }
    } else {
      resultOutput.value = `Connection failed: ${result.error || 'Unknown error'}`
    }
  } catch (error: any) {
    resultOutput.value = `Error: ${error.toString()}`
  } finally {
    store.setActiveLoading(false)
  }
}

const disconnect = async () => {
  if (mcpState.value.connectionId) {
    await invoke('mcp_disconnect', { connectionId: mcpState.value.connectionId })
    store.updateMcpState({
      connected: false,
      connectionId: null,
      tools: [],
      resources: [],
      prompts: [],
    })
    selectedTool.value = null
    selectedResource.value = null
    selectedPrompt.value = null
    resultOutput.value = ''
  }
}

const generateToolInputTemplate = (tool: McpTool) => {
  const schema = tool.inputSchema
  if (schema && schema.properties) {
    const template: Record<string, any> = {}
    for (const [key, value] of Object.entries(schema.properties as Record<string, any>)) {
      if (value.type === 'string') template[key] = ''
      else if (value.type === 'number' || value.type === 'integer') template[key] = 0
      else if (value.type === 'boolean') template[key] = false
      else if (value.type === 'array') template[key] = []
      else if (value.type === 'object') template[key] = {}
      else template[key] = null
    }
    toolInput.value = JSON.stringify(template, null, 2)
  } else {
    toolInput.value = '{}'
  }
}

const selectTool = (tool: McpTool) => {
  selectedTool.value = tool
  generateToolInputTemplate(tool)
}

const selectResource = (resource: McpResource) => {
  selectedResource.value = resource
}

const selectPrompt = (prompt: McpPrompt) => {
  selectedPrompt.value = prompt
  promptArgs.value = {}
  if (prompt.arguments) {
    prompt.arguments.forEach(arg => {
      promptArgs.value[arg.name] = ''
    })
  }
}

const callTool = async () => {
  if (!selectedTool.value || !mcpState.value.connectionId) return
  
  isExecuting.value = true
  resultOutput.value = ''
  
  try {
    let args: any
    try {
      args = JSON.parse(toolInput.value)
    } catch {
      resultOutput.value = 'Invalid JSON input'
      return
    }
    
    const result = await invoke<{
      success: boolean
      result: any
      error: string | null
      time: number
    }>('mcp_call_tool', {
      connectionId: mcpState.value.connectionId,
      toolName: selectedTool.value.name,
      arguments: args,
    })
    
    if (result.success) {
      resultOutput.value = JSON.stringify(result.result, null, 2)
    } else {
      resultOutput.value = `Error: ${result.error}`
    }
  } catch (error: any) {
    resultOutput.value = `Error: ${error.toString()}`
  } finally {
    isExecuting.value = false
  }
}

const readResource = async () => {
  if (!selectedResource.value || !mcpState.value.connectionId) return
  
  isExecuting.value = true
  resultOutput.value = ''
  
  try {
    const result = await invoke<{
      success: boolean
      result: any
      error: string | null
      time: number
    }>('mcp_read_resource', {
      connectionId: mcpState.value.connectionId,
      uri: selectedResource.value.uri,
    })
    
    if (result.success) {
      resultOutput.value = JSON.stringify(result.result, null, 2)
    } else {
      resultOutput.value = `Error: ${result.error}`
    }
  } catch (error: any) {
    resultOutput.value = `Error: ${error.toString()}`
  } finally {
    isExecuting.value = false
  }
}

const getPrompt = async () => {
  if (!selectedPrompt.value || !mcpState.value.connectionId) return
  
  isExecuting.value = true
  resultOutput.value = ''
  
  try {
    const result = await invoke<{
      success: boolean
      result: any
      error: string | null
      time: number
    }>('mcp_get_prompt', {
      connectionId: mcpState.value.connectionId,
      promptName: selectedPrompt.value.name,
      arguments: promptArgs.value,
    })
    
    if (result.success) {
      resultOutput.value = JSON.stringify(result.result, null, 2)
    } else {
      resultOutput.value = `Error: ${result.error}`
    }
  } catch (error: any) {
    resultOutput.value = `Error: ${error.toString()}`
  } finally {
    isExecuting.value = false
  }
}

onUnmounted(() => {
  if (mcpState.value.connectionId) {
    invoke('mcp_disconnect', { connectionId: mcpState.value.connectionId })
  }
})
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- Not Connected State -->
    <template v-if="!mcpState.connected">
      <!-- Header with mode toggle -->
      <div class="border-b border-border p-4">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold">MCP Client</h2>
          <div class="flex items-center gap-2 bg-muted rounded-lg p-1">
            <button
              :class="[
                'px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
                connectionMode === 'discover' ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'
              ]"
              @click="connectionMode = 'discover'"
            >
              <Icon name="lucide:search" class="h-4 w-4 mr-1.5 inline" />
              Discover
            </button>
            <button
              :class="[
                'px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
                connectionMode === 'manual' ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'
              ]"
              @click="connectionMode = 'manual'"
            >
              <Icon name="lucide:terminal" class="h-4 w-4 mr-1.5 inline" />
              Manual
            </button>
          </div>
        </div>
        
        <!-- Loading state -->
        <div v-if="isDiscovering" class="flex items-center justify-center py-8">
          <Icon name="lucide:loader-2" class="h-6 w-6 animate-spin text-muted-foreground" />
          <span class="ml-2 text-muted-foreground">Discovering MCP servers...</span>
        </div>
      </div>
      
      <!-- Discover Mode -->
      <template v-if="connectionMode === 'discover' && !isDiscovering">
        <div class="flex-1 flex min-h-0">
          <!-- App Sources Sidebar -->
          <div class="w-56 border-r border-border flex flex-col">
            <div class="p-3 border-b border-border">
              <div class="flex items-center justify-between">
                <span class="text-sm font-medium text-muted-foreground">Sources</span>
                <button
                  class="p-1 rounded hover:bg-accent"
                  title="Refresh"
                  @click="discoverConfigs"
                >
                  <Icon name="lucide:refresh-cw" class="h-4 w-4 text-muted-foreground" />
                </button>
              </div>
            </div>
            
            <UiScrollArea class="flex-1">
              <div class="p-2 space-y-1">
                <!-- All Servers option -->
                <button
                  :class="[
                    'w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors',
                    selectedSource === null
                      ? 'bg-primary/10 text-primary'
                      : 'hover:bg-accent'
                  ]"
                  @click="selectedSource = null"
                >
                  <div class="h-8 w-8 rounded-lg bg-muted flex items-center justify-center">
                    <Icon name="lucide:layers" class="h-4 w-4" />
                  </div>
                  <div class="flex-1 text-left">
                    <div class="text-sm font-medium">All Servers</div>
                    <div class="text-xs text-muted-foreground">{{ allServers.length }} servers</div>
                  </div>
                </button>
                
                <!-- Individual sources -->
                <button
                  v-for="result in discoveryResults"
                  :key="result.source"
                  :class="[
                    'w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors',
                    selectedSource === result.source
                      ? 'bg-primary/10 text-primary'
                      : 'hover:bg-accent'
                  ]"
                  @click="selectedSource = result.source"
                >
                  <div :class="['h-8 w-8 rounded-lg flex items-center justify-center', getAppConfig(result.source).bgColor]">
                    <Icon :name="getAppConfig(result.source).icon" :class="['h-4 w-4', getAppConfig(result.source).color]" />
                  </div>
                  <div class="flex-1 text-left">
                    <div class="text-sm font-medium">{{ result.source }}</div>
                    <div class="text-xs text-muted-foreground">
                      {{ result.servers.length }} server{{ result.servers.length !== 1 ? 's' : '' }}
                    </div>
                  </div>
                </button>
              </div>
            </UiScrollArea>
          </div>
          
          <!-- Server List -->
          <div class="flex-1 flex flex-col">
            <div class="p-3 border-b border-border">
              <span class="text-sm font-medium">
                {{ selectedSource ? selectedSource : 'All' }} MCP Servers
              </span>
            </div>
            
            <UiScrollArea class="flex-1">
              <div class="p-3 space-y-2">
                <!-- No servers found -->
                <div v-if="(selectedSource ? selectedSourceResult?.servers : allServers)?.length === 0" class="text-center py-12">
                  <Icon name="lucide:inbox" class="h-12 w-12 mx-auto text-muted-foreground/50" />
                  <p class="mt-4 text-muted-foreground">No MCP servers found</p>
                  <p class="text-sm text-muted-foreground/70 mt-1">
                    Configure MCP servers in Claude Desktop, VS Code, or Cursor
                  </p>
                </div>
                
                <!-- Server cards -->
                <div
                  v-for="server in (selectedSource ? selectedSourceResult?.servers : allServers)"
                  :key="`${server.source}-${server.name}`"
                  class="border border-border rounded-lg p-4 hover:border-primary/50 transition-colors"
                >
                  <div class="flex items-start justify-between gap-4">
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2">
                        <h3 class="font-medium truncate">{{ server.name }}</h3>
                        <span
                          v-if="!selectedSource"
                          :class="['text-xs px-1.5 py-0.5 rounded', getAppConfig(server.source).bgColor, getAppConfig(server.source).color]"
                        >
                          {{ server.source }}
                        </span>
                      </div>
                      <p class="text-sm text-muted-foreground font-mono mt-1 truncate">
                        {{ server.command }} {{ server.args.join(' ') }}
                      </p>
                      <div v-if="Object.keys(server.env).length > 0" class="mt-2 flex flex-wrap gap-1">
                        <span
                          v-for="(value, key) in server.env"
                          :key="key"
                          class="text-xs bg-muted px-1.5 py-0.5 rounded font-mono"
                        >
                          {{ key }}
                        </span>
                      </div>
                    </div>
                    <UiButton
                      size="sm"
                      @click="connectToServer(server)"
                      :disabled="activeTab.isLoading"
                    >
                      <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="h-4 w-4 animate-spin mr-1.5" />
                      <Icon v-else name="lucide:plug" class="h-4 w-4 mr-1.5" />
                      Connect
                    </UiButton>
                  </div>
                </div>
              </div>
            </UiScrollArea>
          </div>
        </div>
      </template>
      
      <!-- Manual Mode -->
      <template v-if="connectionMode === 'manual' && !isDiscovering">
        <div class="flex-1 p-4 space-y-4">
          <div>
            <label class="text-sm font-medium mb-1.5 block">Command</label>
            <UiInput
              v-model="manualCommand"
              placeholder="npx -y @modelcontextprotocol/server-filesystem"
              class="font-mono"
            />
            <p class="text-xs text-muted-foreground mt-1">The command to start the MCP server</p>
          </div>
          
          <div>
            <label class="text-sm font-medium mb-1.5 block">Arguments</label>
            <UiInput
              v-model="manualArgs"
              placeholder="/path/to/directory"
              class="font-mono"
            />
            <p class="text-xs text-muted-foreground mt-1">Space-separated arguments</p>
          </div>
          
          <div>
            <label class="text-sm font-medium mb-1.5 block">Environment Variables</label>
            <textarea
              v-model="manualEnv"
              placeholder="API_KEY=your_key&#10;ANOTHER_VAR=value"
              class="w-full h-24 px-3 py-2 bg-background border border-input rounded-md font-mono text-sm resize-none focus:outline-none focus:ring-2 focus:ring-ring"
            />
            <p class="text-xs text-muted-foreground mt-1">One per line, KEY=VALUE format</p>
          </div>
          
          <UiButton
            @click="connectManual"
            :disabled="!manualCommand.trim() || activeTab.isLoading"
            class="w-full"
          >
            <Icon v-if="activeTab.isLoading" name="lucide:loader-2" class="h-4 w-4 animate-spin mr-2" />
            <Icon v-else name="lucide:plug" class="h-4 w-4 mr-2" />
            Connect
          </UiButton>
          
          <!-- Example servers -->
          <div class="pt-4 border-t border-border">
            <p class="text-sm font-medium mb-2">Example Servers</p>
            <div class="space-y-2">
              <button
                class="w-full text-left p-3 rounded-lg border border-border hover:border-primary/50 transition-colors"
                @click="manualCommand = 'npx'; manualArgs = '-y @modelcontextprotocol/server-filesystem /tmp'"
              >
                <div class="font-medium text-sm">Filesystem</div>
                <div class="text-xs text-muted-foreground font-mono">npx -y @modelcontextprotocol/server-filesystem</div>
              </button>
              <button
                class="w-full text-left p-3 rounded-lg border border-border hover:border-primary/50 transition-colors"
                @click="manualCommand = 'npx'; manualArgs = '-y @modelcontextprotocol/server-github'"
              >
                <div class="font-medium text-sm">GitHub</div>
                <div class="text-xs text-muted-foreground font-mono">npx -y @modelcontextprotocol/server-github</div>
              </button>
              <button
                class="w-full text-left p-3 rounded-lg border border-border hover:border-primary/50 transition-colors"
                @click="manualCommand = 'npx'; manualArgs = '-y @modelcontextprotocol/server-everything'"
              >
                <div class="font-medium text-sm">Everything (Demo)</div>
                <div class="text-xs text-muted-foreground font-mono">npx -y @modelcontextprotocol/server-everything</div>
              </button>
            </div>
          </div>
        </div>
      </template>
      
      <!-- Error output -->
      <div v-if="resultOutput && !mcpState.connected" class="border-t border-border p-4">
        <div class="bg-destructive/10 text-destructive rounded-lg p-3 text-sm font-mono">
          {{ resultOutput }}
        </div>
      </div>
    </template>
    
    <!-- Connected State -->
    <template v-else>
      <!-- Connection Header -->
      <div class="border-b border-border p-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="h-10 w-10 rounded-lg bg-green-500/10 flex items-center justify-center">
              <Icon name="lucide:check-circle" class="h-5 w-5 text-green-500" />
            </div>
            <div>
              <div class="font-medium">
                {{ mcpState.serverInfo?.name || 'MCP Server' }}
              </div>
              <div class="text-sm text-muted-foreground">
                {{ mcpState.tools.length }} tools, {{ mcpState.resources.length }} resources, {{ mcpState.prompts.length }} prompts
              </div>
            </div>
          </div>
          <UiButton variant="destructive" size="sm" @click="disconnect">
            <Icon name="lucide:plug-off" class="h-4 w-4 mr-1.5" />
            Disconnect
          </UiButton>
        </div>
      </div>
      
      <!-- Main Content -->
      <div class="flex-1 flex min-h-0">
        <!-- Left: Categories & Items -->
        <div class="w-72 border-r border-border flex flex-col">
          <div class="flex border-b border-border">
            <button
              v-for="section in ['tools', 'resources', 'prompts'] as const"
              :key="section"
              :class="[
                'flex-1 px-3 py-2.5 text-sm font-medium capitalize transition-colors',
                activeSection === section
                  ? 'border-b-2 border-primary text-foreground'
                  : 'text-muted-foreground hover:text-foreground'
              ]"
              @click="activeSection = section"
            >
              {{ section }}
              <span class="ml-1 text-xs opacity-60">
                ({{ section === 'tools' ? mcpState.tools.length : section === 'resources' ? mcpState.resources.length : mcpState.prompts.length }})
              </span>
            </button>
          </div>
          
          <UiScrollArea class="flex-1">
            <!-- Tools -->
            <div v-if="activeSection === 'tools'" class="p-2 space-y-1">
              <button
                v-for="tool in mcpState.tools"
                :key="tool.name"
                :class="[
                  'w-full text-left px-3 py-2.5 rounded-lg transition-colors',
                  selectedTool?.name === tool.name
                    ? 'bg-primary/10 text-primary'
                    : 'hover:bg-accent'
                ]"
                @click="selectTool(tool)"
              >
                <div class="font-medium text-sm">{{ tool.name }}</div>
                <div v-if="tool.description" class="text-xs text-muted-foreground line-clamp-2 mt-0.5">
                  {{ tool.description }}
                </div>
              </button>
            </div>
            
            <!-- Resources -->
            <div v-if="activeSection === 'resources'" class="p-2 space-y-1">
              <button
                v-for="resource in mcpState.resources"
                :key="resource.uri"
                :class="[
                  'w-full text-left px-3 py-2.5 rounded-lg transition-colors',
                  selectedResource?.uri === resource.uri
                    ? 'bg-primary/10 text-primary'
                    : 'hover:bg-accent'
                ]"
                @click="selectResource(resource)"
              >
                <div class="font-medium text-sm">{{ resource.name }}</div>
                <div class="text-xs text-muted-foreground truncate">{{ resource.uri }}</div>
              </button>
              <div v-if="mcpState.resources.length === 0" class="text-center py-8 text-sm text-muted-foreground">
                No resources
              </div>
            </div>
            
            <!-- Prompts -->
            <div v-if="activeSection === 'prompts'" class="p-2 space-y-1">
              <button
                v-for="prompt in mcpState.prompts"
                :key="prompt.name"
                :class="[
                  'w-full text-left px-3 py-2.5 rounded-lg transition-colors',
                  selectedPrompt?.name === prompt.name
                    ? 'bg-primary/10 text-primary'
                    : 'hover:bg-accent'
                ]"
                @click="selectPrompt(prompt)"
              >
                <div class="font-medium text-sm">{{ prompt.name }}</div>
                <div v-if="prompt.description" class="text-xs text-muted-foreground line-clamp-2 mt-0.5">
                  {{ prompt.description }}
                </div>
              </button>
              <div v-if="mcpState.prompts.length === 0" class="text-center py-8 text-sm text-muted-foreground">
                No prompts
              </div>
            </div>
          </UiScrollArea>
        </div>
        
        <!-- Right: Input & Output -->
        <div class="flex-1 flex flex-col min-h-0">
          <!-- Tool Input -->
          <div v-if="activeSection === 'tools' && selectedTool" class="border-b border-border p-4 space-y-3">
            <div class="flex items-center justify-between">
              <div>
                <h3 class="font-medium">{{ selectedTool.name }}</h3>
                <p v-if="selectedTool.description" class="text-sm text-muted-foreground">
                  {{ selectedTool.description }}
                </p>
              </div>
              <UiButton @click="callTool" :disabled="isExecuting">
                <Icon v-if="isExecuting" name="lucide:loader-2" class="h-4 w-4 animate-spin mr-1.5" />
                <Icon v-else name="lucide:play" class="h-4 w-4 mr-1.5" />
                Execute
              </UiButton>
            </div>
            
            <div>
              <label class="text-sm text-muted-foreground mb-1 block">Input (JSON)</label>
              <ClientOnly>
                <CodeEditor
                  v-model="toolInput"
                  language="json"
                  min-height="100px"
                />
              </ClientOnly>
            </div>
          </div>
          
          <!-- Resource Read -->
          <div v-if="activeSection === 'resources' && selectedResource" class="border-b border-border p-4">
            <div class="flex items-center justify-between">
              <div>
                <h3 class="font-medium">{{ selectedResource.name }}</h3>
                <p class="text-sm text-muted-foreground font-mono">{{ selectedResource.uri }}</p>
              </div>
              <UiButton @click="readResource" :disabled="isExecuting">
                <Icon v-if="isExecuting" name="lucide:loader-2" class="h-4 w-4 animate-spin mr-1.5" />
                <Icon v-else name="lucide:file-text" class="h-4 w-4 mr-1.5" />
                Read
              </UiButton>
            </div>
          </div>
          
          <!-- Prompt Get -->
          <div v-if="activeSection === 'prompts' && selectedPrompt" class="border-b border-border p-4 space-y-3">
            <div class="flex items-center justify-between">
              <div>
                <h3 class="font-medium">{{ selectedPrompt.name }}</h3>
                <p v-if="selectedPrompt.description" class="text-sm text-muted-foreground">
                  {{ selectedPrompt.description }}
                </p>
              </div>
              <UiButton @click="getPrompt" :disabled="isExecuting">
                <Icon v-if="isExecuting" name="lucide:loader-2" class="h-4 w-4 animate-spin mr-1.5" />
                <Icon v-else name="lucide:message-square" class="h-4 w-4 mr-1.5" />
                Get Prompt
              </UiButton>
            </div>
            
            <div v-if="selectedPrompt.arguments?.length" class="space-y-2">
              <div v-for="arg in selectedPrompt.arguments" :key="arg.name">
                <label class="text-sm text-muted-foreground mb-1 block">
                  {{ arg.name }}
                  <span v-if="arg.required" class="text-destructive">*</span>
                </label>
                <UiInput v-model="promptArgs[arg.name]" :placeholder="arg.description || arg.name" />
              </div>
            </div>
          </div>
          
          <!-- Output -->
          <div class="flex-1 flex flex-col min-h-0">
            <div class="px-4 py-2 border-b border-border text-sm font-medium text-muted-foreground">
              Result
            </div>
            <div class="flex-1 min-h-0">
              <ClientOnly>
                <CodeEditor
                  :model-value="resultOutput"
                  language="json"
                  :readonly="true"
                  min-height="100%"
                  class="h-full"
                />
              </ClientOnly>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
