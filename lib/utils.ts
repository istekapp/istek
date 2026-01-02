import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'
import type { Collection, CollectionFolder, FolderSettings, AuthConfig, KeyValue, Variable } from '~/types'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function generateId(): string {
  return crypto.randomUUID()
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

export function getStatusColor(status: number): string {
  if (status >= 200 && status < 300) return 'text-green-500'
  if (status >= 300 && status < 400) return 'text-yellow-500'
  if (status >= 400 && status < 500) return 'text-orange-500'
  if (status >= 500) return 'text-red-500'
  return 'text-muted-foreground'
}

export function tryParseJson(str: string): string {
  try {
    return JSON.stringify(JSON.parse(str), null, 2)
  } catch {
    return str
  }
}

// ============ Auth Inheritance Resolution ============

/**
 * Find a folder by ID within a collection (including nested folders)
 */
export function findFolderById(collection: Collection, folderId: string): CollectionFolder | undefined {
  if (!collection.folders) return undefined
  
  for (const folder of collection.folders) {
    if (folder.id === folderId) return folder
    
    // Check nested folders
    if (folder.folders) {
      const found = findFolderInNested(folder.folders, folderId)
      if (found) return found
    }
  }
  return undefined
}

function findFolderInNested(folders: CollectionFolder[], folderId: string): CollectionFolder | undefined {
  for (const folder of folders) {
    if (folder.id === folderId) return folder
    if (folder.folders) {
      const found = findFolderInNested(folder.folders, folderId)
      if (found) return found
    }
  }
  return undefined
}

/**
 * Get the parent folder of a given folder
 */
export function getParentFolder(collection: Collection, folder: CollectionFolder): CollectionFolder | undefined {
  if (!folder.parentId) return undefined
  return findFolderById(collection, folder.parentId)
}

/**
 * Build the inheritance chain from request level up to collection level
 * Returns array from most specific (folder) to least specific (collection)
 */
export function buildInheritanceChain(
  collection: Collection,
  folderId?: string
): FolderSettings[] {
  const chain: FolderSettings[] = []
  
  // Start from the folder if provided
  if (folderId) {
    let currentFolder = findFolderById(collection, folderId)
    
    while (currentFolder) {
      if (currentFolder.settings) {
        chain.push(currentFolder.settings)
      }
      
      // Move to parent folder
      if (currentFolder.parentId) {
        currentFolder = findFolderById(collection, currentFolder.parentId)
      } else {
        break
      }
    }
  }
  
  // Add collection-level settings at the end (least specific)
  if (collection.settings) {
    chain.push(collection.settings)
  }
  
  return chain
}

/**
 * Resolve auth configuration by walking up the inheritance chain
 * If auth type is 'inherit', continue to parent; otherwise use the current config
 */
export function resolveAuth(
  collection: Collection,
  folderId?: string,
  requestAuth?: AuthConfig
): AuthConfig | undefined {
  // If request has explicit auth (not 'inherit'), use it
  if (requestAuth && requestAuth.type !== 'inherit') {
    return requestAuth
  }
  
  const chain = buildInheritanceChain(collection, folderId)
  
  for (const settings of chain) {
    if (settings.auth) {
      // If this level says 'inherit', continue to parent
      if (settings.auth.type === 'inherit') {
        continue
      }
      // Otherwise, use this auth config
      return settings.auth
    }
  }
  
  return undefined
}

/**
 * Resolve headers by merging from collection down to folder level
 * Child headers override parent headers with the same key
 */
export function resolveHeaders(
  collection: Collection,
  folderId?: string,
  requestHeaders?: KeyValue[]
): KeyValue[] {
  const chain = buildInheritanceChain(collection, folderId)
  
  // Start with collection-level headers (reverse the chain so collection is first)
  const reversedChain = [...chain].reverse()
  
  // Use a map to track headers by key (for override logic)
  const headersMap = new Map<string, KeyValue>()
  
  // Apply headers from least specific to most specific
  for (const settings of reversedChain) {
    if (settings.headers) {
      for (const header of settings.headers) {
        if (header.enabled) {
          headersMap.set(header.key.toLowerCase(), header)
        }
      }
    }
  }
  
  // Apply request-level headers (most specific)
  if (requestHeaders) {
    for (const header of requestHeaders) {
      if (header.enabled) {
        headersMap.set(header.key.toLowerCase(), header)
      }
    }
  }
  
  return Array.from(headersMap.values())
}

/**
 * Resolve variables by merging from collection down to folder level
 * Child variables override parent variables with the same key
 */
export function resolveVariables(
  collection: Collection,
  folderId?: string
): Variable[] {
  const chain = buildInheritanceChain(collection, folderId)
  
  // Start with collection-level (reverse the chain)
  const reversedChain = [...chain].reverse()
  
  const variablesMap = new Map<string, Variable>()
  
  for (const settings of reversedChain) {
    if (settings.variables) {
      for (const variable of settings.variables) {
        if (variable.enabled) {
          variablesMap.set(variable.key, variable)
        }
      }
    }
  }
  
  return Array.from(variablesMap.values())
}

/**
 * Resolve base URL by finding the first non-empty baseUrl in the chain
 */
export function resolveBaseUrl(
  collection: Collection,
  folderId?: string
): string | undefined {
  const chain = buildInheritanceChain(collection, folderId)
  
  for (const settings of chain) {
    if (settings.baseUrl && settings.baseUrl.trim()) {
      return settings.baseUrl
    }
  }
  
  return undefined
}

/**
 * Apply auth config to headers (convert auth to actual headers/params)
 */
export function applyAuthToHeaders(
  auth: AuthConfig,
  headers: KeyValue[],
  variableResolver: (value: string) => string
): KeyValue[] {
  const result = [...headers]
  
  if (!auth.enabled) return result
  
  // Check conditional enablement
  if (auth.enabledWhen) {
    const condition = variableResolver(auth.enabledWhen)
    // Simple evaluation - just check if it's truthy after variable resolution
    // A more sophisticated implementation would parse the expression
    if (!condition || condition === 'false' || condition === '0') {
      return result
    }
  }
  
  switch (auth.type) {
    case 'basic': {
      const username = variableResolver(auth.username || '')
      const password = variableResolver(auth.password || '')
      const encoded = btoa(`${username}:${password}`)
      result.push({
        id: generateId(),
        key: 'Authorization',
        value: `Basic ${encoded}`,
        enabled: true
      })
      break
    }
    
    case 'bearer': {
      const token = variableResolver(auth.token || '')
      const prefix = auth.prefix || 'Bearer'
      result.push({
        id: generateId(),
        key: 'Authorization',
        value: `${prefix} ${token}`,
        enabled: true
      })
      break
    }
    
    case 'api-key': {
      const keyName = variableResolver(auth.apiKeyName || '')
      const keyValue = variableResolver(auth.apiKeyValue || '')
      
      if (auth.apiKeyIn === 'header') {
        result.push({
          id: generateId(),
          key: keyName,
          value: keyValue,
          enabled: true
        })
      }
      // Note: query params would be handled separately
      break
    }
  }
  
  return result
}

/**
 * Get API key query params if auth is api-key with apiKeyIn='query'
 */
export function getAuthQueryParams(
  auth: AuthConfig | undefined,
  variableResolver: (value: string) => string
): KeyValue[] {
  if (!auth || !auth.enabled || auth.type !== 'api-key' || auth.apiKeyIn !== 'query') {
    return []
  }
  
  const keyName = variableResolver(auth.apiKeyName || '')
  const keyValue = variableResolver(auth.apiKeyValue || '')
  
  return [{
    id: generateId(),
    key: keyName,
    value: keyValue,
    enabled: true
  }]
}
