// ============ Workspace ============
export interface Workspace {
  id: string
  name: string
  syncPath?: string  // If set, enables filesystem sync + git
  isDefault: boolean
  createdAt: number
}

// Protocol Types
export type ProtocolType = 'http' | 'websocket' | 'graphql' | 'grpc' | 'mqtt' | 'unix-socket' | 'mcp' | 'sse'

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'

export interface KeyValue {
  id: string
  key: string
  value: string
  enabled: boolean
  required?: boolean
  description?: string
}

// Form data field for multipart/form-data requests
export interface FormDataField {
  id: string
  key: string
  value: string
  type: 'text' | 'file'
  enabled: boolean
  // For file type
  fileName?: string
  fileSize?: number
  mimeType?: string
}

// ============ HTTP ============
export interface HttpRequest {
  id: string
  name: string
  protocol: 'http'
  method: HttpMethod
  url: string
  headers: KeyValue[]
  params: KeyValue[]
  body: string
  bodyType: 'none' | 'json' | 'xml' | 'html' | 'raw' | 'form-data' | 'x-www-form-urlencoded'
  /** Form data for form-data body type (supports files) */
  formData?: FormDataField[]
  /** URL encoded data for x-www-form-urlencoded body type */
  urlEncodedData?: KeyValue[]
  /** Response schema from OpenAPI/Swagger for mock generation */
  responseSchema?: Record<string, any>
  /** Authentication configuration */
  auth?: AuthConfig
  /** Parent folder ID for inheritance */
  folderId?: string
  /** Pre-request script (JavaScript) */
  preRequestScript?: string
  /** Post-request script (JavaScript) */
  postRequestScript?: string
}

export interface HttpResponse {
  status: number
  statusText: string
  headers: Record<string, string>
  body: string
  time: number
  size: number
}

// ============ WebSocket ============
export interface WebSocketRequest {
  id: string
  name: string
  protocol: 'websocket'
  url: string
  headers: KeyValue[]
  message: string
  messageType: 'text' | 'binary'
}

export interface WebSocketMessage {
  id: string
  direction: 'sent' | 'received'
  data: string
  timestamp: number
  type: 'text' | 'binary' | 'ping' | 'pong' | 'open' | 'close' | 'error'
}

export interface WebSocketState {
  connected: boolean
  messages: WebSocketMessage[]
  connectionId: string | null
}

// ============ GraphQL ============
export interface GraphQLRequest {
  id: string
  name: string
  protocol: 'graphql'
  url: string
  headers: KeyValue[]
  query: string
  variables: string
  operationName: string
}

export interface GraphQLResponse {
  data: any
  errors?: Array<{ message: string; locations?: any[]; path?: string[] }>
  time: number
}

// ============ gRPC ============
export interface GrpcRequest {
  id: string
  name: string
  protocol: 'grpc'
  url: string
  service: string
  method: string
  protoFile: string
  message: string
  metadata: KeyValue[]
}

export interface GrpcResponse {
  data: any
  metadata: Record<string, string>
  status: number
  statusMessage: string
  time: number
}

// ============ MQTT ============
export interface MqttRequest {
  id: string
  name: string
  protocol: 'mqtt'
  broker: string
  port: number
  clientId: string
  username: string
  password: string
  topic: string
  message: string
  qos: 0 | 1 | 2
  retain: boolean
  useTls: boolean
}

export interface MqttMessage {
  id: string
  topic: string
  payload: string
  qos: number
  retained: boolean
  timestamp: number
  direction: 'sent' | 'received'
}

export interface MqttState {
  connected: boolean
  messages: MqttMessage[]
  subscribedTopics: string[]
  connectionId: string | null
}

// ============ Unix Socket ============
export interface UnixSocketRequest {
  id: string
  name: string
  protocol: 'unix-socket'
  socketPath: string
  method: HttpMethod
  path: string
  headers: KeyValue[]
  body: string
}

// ============ SSE (Server-Sent Events) ============
export interface SseRequest {
  id: string
  name: string
  protocol: 'sse'
  url: string
  headers: KeyValue[]
  withCredentials: boolean
}

export interface SseEvent {
  id: string
  eventId?: string
  eventType: string
  data: string
  timestamp: number
  retry?: number
}

export interface SseState {
  connected: boolean
  events: SseEvent[]
  connectionId: string | null
  lastEventId?: string
}

// ============ MCP (Model Context Protocol) ============
export type McpTransportType = 'stdio' | 'sse'

export interface McpRequest {
  id: string
  name: string
  protocol: 'mcp'
  transportType: McpTransportType
  // For stdio transport
  command?: string
  args?: string[]
  env?: Record<string, string>
  // For SSE transport
  serverUrl?: string
  // Selected tool to call
  selectedTool?: string
  toolInput: string // JSON input for the tool
}

export interface McpTool {
  name: string
  description?: string
  inputSchema: {
    type: string
    properties?: Record<string, any>
    required?: string[]
  }
}

export interface McpResource {
  uri: string
  name: string
  description?: string
  mimeType?: string
}

export interface McpPrompt {
  name: string
  description?: string
  arguments?: Array<{
    name: string
    description?: string
    required?: boolean
  }>
}

export interface McpServerInfo {
  name: string
  version: string
  protocolVersion: string
}

export interface McpResponse {
  serverInfo?: McpServerInfo
  tools: McpTool[]
  resources: McpResource[]
  prompts: McpPrompt[]
  // Tool call result
  result?: any
  error?: string
  time: number
}

export interface McpState {
  connected: boolean
  serverInfo?: McpServerInfo
  tools: McpTool[]
  resources: McpResource[]
  prompts: McpPrompt[]
  connectionId: string | null
}

// MCP Config Discovery
export interface DiscoveredMcp {
  name: string
  command: string
  args: string[]
  env: Record<string, string>
  source: string
}

export interface McpDiscoveryResult {
  source: string
  configPath?: string
  servers: DiscoveredMcp[]
  error?: string
}

// ============ Mock Server ============
export interface MockEndpoint {
  id: string
  method: string
  path: string
  responseStatus: number
  responseHeaders: Record<string, string>
  responseBody: string
  delayMs?: number
}

export interface MockServerConfig {
  id: string
  name: string
  port: number
  endpoints: MockEndpoint[]
}

export interface MockServerInfo {
  id: string
  name: string
  port: number
  endpointCount: number
  running: boolean
}

export interface MockRequestLog {
  id: string
  serverId: string
  timestamp: number
  method: string
  path: string
  query?: string
  headers: Record<string, string>
  body?: string
  matchedEndpoint?: string
  responseStatus: number
  responseTimeMs: number
}

// ============ Union Types ============
export type RequestType = 
  | HttpRequest 
  | WebSocketRequest 
  | GraphQLRequest 
  | GrpcRequest 
  | MqttRequest 
  | UnixSocketRequest
  | McpRequest
  | SseRequest

export type ResponseType = HttpResponse | GraphQLResponse | GrpcResponse | McpResponse

// ============ Auth Configuration ============
export type AuthType = 'none' | 'inherit' | 'basic' | 'bearer' | 'api-key' | 'oauth2'

export interface OAuth2Config {
  grantType: 'authorization_code' | 'client_credentials' | 'password' | 'implicit'
  authUrl?: string
  tokenUrl?: string
  clientId?: string
  clientSecret?: string
  scope?: string
  // For password grant
  username?: string
  password?: string
  // Token storage
  accessToken?: string
  refreshToken?: string
  expiresAt?: number
}

export interface AuthConfig {
  type: AuthType
  enabled: boolean
  enabledWhen?: string // Conditional expression: "{{ENV}} == 'production'"
  // Basic Auth
  username?: string
  password?: string
  // Bearer Token
  token?: string
  prefix?: string // Default: "Bearer"
  // API Key
  apiKeyName?: string
  apiKeyValue?: string
  apiKeyIn?: 'header' | 'query'
  // OAuth2
  oauth2?: OAuth2Config
}

// ============ Folder/Collection Settings ============
export interface FolderSettings {
  auth?: AuthConfig
  headers?: KeyValue[]
  variables?: Variable[]
  baseUrl?: string
  queryParams?: KeyValue[]
}

// ============ Collections & History ============
export interface CollectionFolder {
  id: string
  name: string
  parentId?: string  // For nested folders
  requests: RequestType[]
  folders?: CollectionFolder[]  // Nested folders
  settings?: FolderSettings
}

export interface Collection {
  id: string
  name: string
  requests: RequestType[]
  folders?: CollectionFolder[]
  settings?: FolderSettings  // Collection-level (workspace) settings
  createdAt: number
}

export interface HistoryItem {
  id: string
  request: RequestType
  response: ResponseType | null
  timestamp: number
}

// ============ Tab ============
export type TabType = 'request' | 'test' | 'mock'

export interface RequestTab {
  id: string
  type: 'request'
  protocol: ProtocolType
  request: RequestType
  response: ResponseType | null
  isLoading: boolean
  isDirty: boolean
  // Source tracking - where this request came from
  sourceCollectionId?: string
  sourceRequestId?: string
  // Protocol specific states
  wsState?: WebSocketState
  mqttState?: MqttState
  mcpState?: McpState
  sseState?: SseState
}

export interface TestTab {
  id: string
  type: 'test'
  name: string
  collectionId?: string
  collectionName: string
  // Test state managed by TestPanel component
}

export interface MockTab {
  id: string
  type: 'mock'
  name: string
  collectionId?: string
  collectionName: string
}

// Playground types
export interface PlaygroundStatus {
  running: boolean
  httpUrl: string | null
  wsUrl: string | null
  graphqlUrl: string | null
  mqttUrl: string | null
  grpcUrl: string | null
  unixSocket: string | null
  openapiUrl: string | null
  sseUrl: string | null
}

export type Tab = RequestTab | TestTab | MockTab

// Legacy compatibility - Tab used to be just RequestTab
export interface LegacyTab {
  id: string
  protocol: ProtocolType
  request: RequestType
  response: ResponseType | null
  isLoading: boolean
  isDirty: boolean
  wsState?: WebSocketState
  mqttState?: MqttState
  mcpState?: McpState
  sseState?: SseState
}

// ============ Test Runner ============

// Assertion Types for configurable test assertions
export type AssertionType = 
  | 'status'           // status == 200
  | 'status_range'     // status in 200-299
  | 'jsonpath'         // $.data.id == "123"
  | 'contains'         // body contains "success"
  | 'response_time'    // responseTime < 500ms
  | 'header'           // header exists or equals value

export interface Assertion {
  id: string
  type: AssertionType
  enabled: boolean
  // For status
  expectedStatus?: number
  // For status_range
  minStatus?: number
  maxStatus?: number
  // For jsonpath
  jsonPath?: string
  operator?: 'equals' | 'not_equals' | 'contains' | 'exists' | 'not_exists'
  expectedValue?: string
  // For contains
  searchString?: string
  // For response_time
  maxTimeMs?: number
  // For header
  headerName?: string
  headerValue?: string
}

// Variable Extraction for response chaining
export interface VariableExtraction {
  id: string
  variableName: string
  jsonPath: string
  enabled: boolean
}

export interface TestRequest {
  id: string
  name: string
  method: string
  url: string
  headers: KeyValue[]
  params: KeyValue[]
  body?: string
  bodyType: string
  // Configurable assertions for this request
  assertions?: Assertion[]
  // Variables to extract from response
  extractVariables?: VariableExtraction[]
}

export type TestStatus = 'pending' | 'running' | 'passed' | 'failed' | 'error'

export interface AssertionResult {
  name: string
  passed: boolean
  expected: string
  actual: string
}

// Extracted variable result
export interface ExtractedVariable {
  variableName: string
  jsonPath: string
  value: string
  success: boolean
  error?: string
}

export interface TestResult {
  requestId: string
  requestName: string
  method: string
  url: string
  status: TestStatus
  responseStatus?: number
  responseTime?: number
  responseSize?: number
  responseBody?: string
  error?: string
  assertions: AssertionResult[]
  extractedVariables?: ExtractedVariable[]
}

export interface TestRunConfig {
  id: string
  name: string
  requests: TestRequest[]
  stopOnFailure: boolean
  delayBetweenRequests: number
}

export interface TestRunSummary {
  runId: string
  name: string
  total: number
  passed: number
  failed: number
  errors: number
  totalTime: number
  results: TestResult[]
}

export interface TestProgressEvent {
  runId: string
  current: number
  total: number
  result: TestResult
}

// Test Run History for storing past runs
export interface TestRunHistory {
  id: string
  runId: string
  collectionId?: string
  collectionName: string
  timestamp: number
  summary: TestRunSummary
}

// ============ Cloud Secret Providers ============
export type CloudSecretProviderType = 'aws' | 'gcp' | 'azure'

export interface CloudSecretProviderConfig {
  id: string
  name: string
  providerType: CloudSecretProviderType
  enabled: boolean
  // AWS
  awsRegion?: string
  awsAccessKeyId?: string
  awsSecretAccessKey?: string
  awsSecretName?: string
  // GCP
  gcpProjectId?: string
  gcpCredentialsJson?: string
  gcpSecretName?: string
  // Azure
  azureVaultUrl?: string
  azureTenantId?: string
  azureClientId?: string
  azureClientSecret?: string
  azureSecretName?: string
  createdAt: number
}

export interface SecretValue {
  key: string
  value: string
}

export interface FetchSecretsResult {
  success: boolean
  secrets: SecretValue[]
  error?: string
}

// ============ Variables & Environments ============
export type SecretProviderType = 'manual' | 'vault' | '1password' | 'bitwarden' | 'aws' | 'gcp' | 'azure'

export interface SecretProviderConfig {
  id: string
  name: string
  type: SecretProviderType
  enabled: boolean
  config: VaultConfig | OnePasswordConfig | BitwardenConfig | AwsConfig | GcpConfig | AzureConfig | null
  createdAt: number
}

// Connection-only configs (no secret path - that's specified per-variable)
export interface VaultConfig {
  address: string
  token: string
  namespace?: string
  mountPath: string  // e.g., "secret" or "kv"
}

export interface OnePasswordConfig {
  serviceAccountToken: string
  vaultId: string
}

export interface BitwardenConfig {
  serverUrl: string
  apiKey: string
  organizationId?: string
}

export interface AwsConfig {
  region: string
  accessKeyId: string
  secretAccessKey: string
}

export interface GcpConfig {
  projectId: string
  credentialsJson: string
}

export interface AzureConfig {
  vaultUrl: string
  tenantId: string
  clientId: string
  clientSecret: string
}

export interface Variable {
  id: string
  key: string
  value: string
  description?: string
  isSecret: boolean
  // If value comes from a secret provider
  secretProvider?: {
    providerId: string
    // Path to the secret (format depends on provider):
    // - Vault: "data/myapp/config" -> key "password"
    // - AWS: "myapp/production" -> key "db_password"  
    // - 1Password: item reference
    // - etc.
    secretPath: string
    secretKey: string  // The key within the secret (for JSON secrets)
  }
  enabled: boolean
}

export interface Environment {
  id: string
  name: string
  color: string
  variables: Variable[]
  isDefault?: boolean
  shareable?: boolean  // If true, this environment will be synced to Git
  createdAt: number
}

export interface VariableScope {
  // Global variables apply to all environments
  global: Variable[]
  // Environment-specific variables (can override global)
  environments: Environment[]
  // Active environment ID
  activeEnvironmentId: string | null
}

// ============ Git Sync ============

export interface SyncConfig {
  enabled: boolean
  syncPath: string
  syncCollections: boolean
  syncEnvironments: boolean
  syncGlobalVariables: boolean
}

export interface SyncChange {
  changeType: 'added' | 'modified' | 'deleted'
  resourceType: 'collection' | 'environment' | 'global_variable'
  resourceId: string
  resourceName: string
  source: 'local' | 'external'
}

export interface SyncStatus {
  isInitialized: boolean
  syncPath: string
  localChanges: SyncChange[]
  externalChanges: SyncChange[]
  lastSync: number | null
}

export interface GitFileChange {
  path: string
  status: 'new' | 'modified' | 'deleted' | 'renamed'
}

export interface GitStatus {
  isRepo: boolean
  branch: string | null
  hasRemote: boolean
  remoteUrl: string | null
  uncommittedChanges: GitFileChange[]
  ahead: number
  behind: number
}

export interface GitCommit {
  id: string
  message: string
  author: string
  timestamp: number
}
