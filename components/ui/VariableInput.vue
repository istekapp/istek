<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  modelValue?: string
  type?: string
  placeholder?: string
  disabled?: boolean
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  type: 'text',
  placeholder: '',
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'paste-curl': [curl: string]
}>()

const variableStore = useVariableStore()
const inputRef = ref<HTMLInputElement | null>(null)
const containerRef = ref<HTMLDivElement | null>(null)
const highlightRef = ref<HTMLDivElement | null>(null)
const autocompleteListRef = ref<HTMLDivElement | null>(null)

// Autocomplete state
const showAutocomplete = ref(false)
const autocompleteItems = ref<AutocompleteItem[]>([])
const selectedIndex = ref(0)
const cursorPosition = ref(0)
const autocompletePosition = ref({ left: 0 })

// Check for variables in the value
const hasVariables = computed(() => variableStore.hasVariables(props.modelValue))
const unresolvedVars = computed(() => variableStore.getUnresolvedVariables(props.modelValue))
const hasUnresolved = computed(() => unresolvedVars.value.length > 0)

// Template functions available for autocomplete
const templateFunctions = [
  // Hash functions
  { name: 'hash.md5', snippet: "hash.md5('value')", description: 'MD5 hash' },
  { name: 'hash.sha1', snippet: "hash.sha1('value')", description: 'SHA1 hash' },
  { name: 'hash.sha256', snippet: "hash.sha256('value')", description: 'SHA256 hash' },
  { name: 'hash.sha512', snippet: "hash.sha512('value')", description: 'SHA512 hash' },
  // HMAC functions
  { name: 'hmac.sha256', snippet: "hmac.sha256('value', 'key')", description: 'HMAC-SHA256' },
  { name: 'hmac.sha512', snippet: "hmac.sha512('value', 'key')", description: 'HMAC-SHA512' },
  // Encoding functions
  { name: 'base64.encode', snippet: "base64.encode('value')", description: 'Base64 encode' },
  { name: 'base64.decode', snippet: "base64.decode('encoded')", description: 'Base64 decode' },
  { name: 'url.encode', snippet: "url.encode('value')", description: 'URL encode' },
  { name: 'url.decode', snippet: "url.decode('encoded')", description: 'URL decode' },
  // Utility functions
  { name: 'uuid', snippet: 'uuid()', description: 'Generate UUID v4' },
  { name: 'timestamp', snippet: 'timestamp()', description: 'Unix timestamp (seconds)' },
  { name: 'timestamp.ms', snippet: 'timestamp.ms()', description: 'Unix timestamp (milliseconds)' },
  // Random functions
  { name: 'random.int', snippet: 'random.int(1, 100)', description: 'Random integer' },
  { name: 'random.float', snippet: 'random.float(0, 1)', description: 'Random float' },
  { name: 'random.string', snippet: 'random.string(16)', description: 'Random string' },
  { name: 'random.hex', snippet: 'random.hex(16)', description: 'Random hex string' },
]

// Get all available variable names
const allVariableNames = computed(() => {
  return Array.from(variableStore.resolvedVariables.value.keys())
})

// Autocomplete item type
type AutocompleteItem = { type: 'variable', name: string } | { type: 'function', name: string, snippet: string, description: string }

// Template function names that are recognized
const knownTemplateFunctions = [
  'hash.md5', 'hash.sha1', 'hash.sha256', 'hash.sha512',
  'hmac.sha256', 'hmac.sha512',
  'base64.encode', 'base64.decode', 'encode.base64', 'decode.base64',
  'url.encode', 'url.decode', 'encode.url', 'decode.url',
  'sensitive',
  'uuid',
  'timestamp', 'timestamp.ms',
  'random.int', 'random.float', 'random.string', 'random.hex',
]

// Check if a name is a template function call (e.g., "$hash.md5(...)" or "hash.md5(...)")
const isTemplateFunction = (name: string): boolean => {
  // Remove leading $ if present
  const cleanName = name.startsWith('$') ? name.slice(1) : name
  // Extract function name (before the parenthesis)
  const fnMatch = cleanName.match(/^([\w.]+)\s*\(/)
  if (fnMatch) {
    return knownTemplateFunctions.includes(fnMatch[1])
  }
  return false
}

// Parse text into segments for highlighting
const segments = computed(() => {
  const text = props.modelValue || ''
  const result: Array<{ 
    type: 'text' | 'variable-valid' | 'variable-invalid' | 'variable-open' | 'function'
    content: string
    fullContent?: string // Original content for functions (before condensing)
    functionName?: string // For tooltip display
    functionArgs?: string // For tooltip display
  }> = []
  
  let lastIndex = 0
  // Match complete variables {{...}} and incomplete {{...
  const regex = /\{\{([^}]*)\}\}|\{\{([^}]*)/g
  let match
  
  while ((match = regex.exec(text)) !== null) {
    // Add text before match
    if (match.index > lastIndex) {
      result.push({ type: 'text', content: text.slice(lastIndex, match.index) })
    }
    
    if (match[1] !== undefined) {
      // Complete variable {{...}}
      const varName = match[1].trim()
      
      // Check if it's a template function
      if (isTemplateFunction(varName)) {
        // Extract function name and args for tooltip
        const cleanName = varName.startsWith('$') ? varName.slice(1) : varName
        const fnMatch = cleanName.match(/^([\w.]+)\s*\((.*)\)$/)
        const functionName = fnMatch ? fnMatch[1] : cleanName
        const functionArgs = fnMatch ? fnMatch[2] : ''
        
        // Create condensed display: {{$functionName(...)}}
        const displayContent = functionArgs 
          ? `{{$${functionName}(...)}}` 
          : match[0]
        
        result.push({ 
          type: 'function', 
          content: displayContent,
          fullContent: match[0], // Store original for reference
          functionName,
          functionArgs,
        })
      } else {
        // It's a variable - check if resolved
        const isValid = variableStore.resolvedVariables.value.has(varName)
        result.push({ 
          type: isValid ? 'variable-valid' : 'variable-invalid', 
          content: match[0] 
        })
      }
    } else if (match[2] !== undefined) {
      // Incomplete variable {{...
      result.push({ type: 'variable-open', content: match[0] })
    }
    
    lastIndex = match.index + match[0].length
  }
  
  // Add remaining text
  if (lastIndex < text.length) {
    result.push({ type: 'text', content: text.slice(lastIndex) })
  }
  
  return result
})

// Sync scroll between input and highlight layer
const syncScroll = () => {
  if (inputRef.value && highlightRef.value) {
    highlightRef.value.scrollLeft = inputRef.value.scrollLeft
  }
}

// Check if cursor is inside {{ for autocomplete
const checkForAutocomplete = () => {
  if (!inputRef.value) return
  
  const text = props.modelValue || ''
  const cursor = inputRef.value.selectionStart || 0
  cursorPosition.value = cursor
  
  // Find if we're inside an incomplete {{ 
  const textBeforeCursor = text.slice(0, cursor)
  const lastOpenBrace = textBeforeCursor.lastIndexOf('{{')
  const lastCloseBrace = textBeforeCursor.lastIndexOf('}}')
  
  if (lastOpenBrace > lastCloseBrace) {
    // We're inside {{ - show autocomplete
    const searchTerm = textBeforeCursor.slice(lastOpenBrace + 2).toLowerCase()
    
    // Filter variables
    const filteredVars: AutocompleteItem[] = allVariableNames.value
      .filter(name => name.toLowerCase().includes(searchTerm))
      .map(name => ({ type: 'variable' as const, name }))
    
    // Filter template functions (if search term starts with a function-like pattern)
    const filteredFns: AutocompleteItem[] = templateFunctions
      .filter(fn => fn.name.toLowerCase().includes(searchTerm) || fn.description.toLowerCase().includes(searchTerm))
      .map(fn => ({ type: 'function' as const, ...fn }))
    
    // Combine results: variables first, then functions
    const combined = [...filteredVars, ...filteredFns]
    
    if (combined.length > 0) {
      autocompleteItems.value = combined
      selectedIndex.value = 0
      showAutocomplete.value = true
      
      // Calculate position
      calculateAutocompletePosition(lastOpenBrace)
    } else {
      showAutocomplete.value = false
    }
  } else {
    showAutocomplete.value = false
  }
}

// Calculate autocomplete dropdown position
const calculateAutocompletePosition = (bracePosition: number) => {
  if (!inputRef.value || !containerRef.value) return
  
  // Create a temporary span to measure text width
  const span = document.createElement('span')
  span.style.font = window.getComputedStyle(inputRef.value).font
  span.style.visibility = 'hidden'
  span.style.position = 'absolute'
  span.style.whiteSpace = 'pre'
  span.textContent = props.modelValue?.slice(0, bracePosition) || ''
  document.body.appendChild(span)
  
  const textWidth = span.offsetWidth
  document.body.removeChild(span)
  
  // Account for padding and scroll
  const padding = 12 // px-3 = 12px
  const scrollLeft = inputRef.value.scrollLeft
  autocompletePosition.value = { left: Math.max(0, textWidth + padding - scrollLeft) }
}

// Insert selected item (variable or function)
const insertAutocompleteItem = (item: AutocompleteItem) => {
  if (!inputRef.value) return
  
  const text = props.modelValue || ''
  const cursor = cursorPosition.value
  const textBeforeCursor = text.slice(0, cursor)
  const lastOpenBrace = textBeforeCursor.lastIndexOf('{{')
  
  let insertText: string
  let cursorOffset: number
  
  if (item.type === 'variable') {
    // Insert variable: {{VAR_NAME}}
    insertText = `{{${item.name}}}`
    cursorOffset = insertText.length
  } else {
    // Insert function: {{$fn.name(...)}}
    insertText = `{{$${item.snippet}}}`
    cursorOffset = insertText.length
  }
  
  // Replace from {{ to cursor with the complete item
  const newText = text.slice(0, lastOpenBrace) + insertText + text.slice(cursor)
  emit('update:modelValue', newText)
  
  showAutocomplete.value = false
  
  // Set cursor after the inserted item
  nextTick(() => {
    if (inputRef.value) {
      const newPosition = lastOpenBrace + cursorOffset
      inputRef.value.setSelectionRange(newPosition, newPosition)
      inputRef.value.focus()
    }
  })
}

// Legacy function for backward compatibility
const insertVariable = (varName: string) => {
  insertAutocompleteItem({ type: 'variable', name: varName })
}

// Scroll selected item into view
const scrollSelectedIntoView = () => {
  nextTick(() => {
    if (!autocompleteListRef.value) return
    const selectedElement = autocompleteListRef.value.querySelector(`[data-index="${selectedIndex.value}"]`)
    if (selectedElement) {
      selectedElement.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }
  })
}

// Handle keyboard navigation
const handleKeyDown = (e: KeyboardEvent) => {
  if (!showAutocomplete.value) return
  
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value + 1) % autocompleteItems.value.length
      scrollSelectedIntoView()
      break
    case 'ArrowUp':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value - 1 + autocompleteItems.value.length) % autocompleteItems.value.length
      scrollSelectedIntoView()
      break
    case 'Enter':
    case 'Tab':
      if (autocompleteItems.value.length > 0) {
        e.preventDefault()
        insertAutocompleteItem(autocompleteItems.value[selectedIndex.value])
      }
      break
    case 'Escape':
      showAutocomplete.value = false
      break
  }
}

// Handle input changes
const handleInput = (e: Event) => {
  const target = e.target as HTMLInputElement
  emit('update:modelValue', target.value)
  nextTick(() => {
    checkForAutocomplete()
    syncScroll()
  })
}

// Handle click to update cursor position
const handleClick = () => {
  nextTick(() => checkForAutocomplete())
}

// Handle paste - detect curl commands
const handlePaste = (e: ClipboardEvent) => {
  const text = e.clipboardData?.getData('text')?.trim()
  if (text && text.toLowerCase().startsWith('curl ')) {
    e.preventDefault()
    emit('paste-curl', text)
  }
}

// Close autocomplete on outside click
const autocompleteRef = useClickOutside(() => {
  showAutocomplete.value = false
})

// Tooltip state - with delay to allow hovering over tooltip
const showTooltip = ref(false)
const tooltipHideTimeout = ref<ReturnType<typeof setTimeout> | null>(null)
const isHoveringTooltip = ref(false)

const handleTooltipMouseEnter = () => {
  if (tooltipHideTimeout.value) {
    clearTimeout(tooltipHideTimeout.value)
    tooltipHideTimeout.value = null
  }
  showTooltip.value = true
}

const handleTooltipMouseLeave = () => {
  // Delay hiding to allow moving to tooltip
  tooltipHideTimeout.value = setTimeout(() => {
    if (!isHoveringTooltip.value) {
      showTooltip.value = false
    }
  }, 150)
}

const handleTooltipEnter = () => {
  isHoveringTooltip.value = true
  if (tooltipHideTimeout.value) {
    clearTimeout(tooltipHideTimeout.value)
    tooltipHideTimeout.value = null
  }
}

const handleTooltipLeave = () => {
  isHoveringTooltip.value = false
  showTooltip.value = false
}

// Add missing variable
const addMissingVariable = async (varName: string) => {
  // Add the variable to global variables with empty value
  await variableStore.addGlobalVariable({
    key: varName,
    value: '',
    enabled: true
  })
  
  // Open Variables tab and focus on the new variable
  variableStore.openVariableManager('variables', varName)
  
  showTooltip.value = false
}

// Custom interpolation for preview that masks sensitive values
const previewValue = computed(() => {
  if (!hasVariables.value) return props.modelValue
  
  // Replace variables with their values, masking sensitive ones
  let result = props.modelValue || ''
  
  result = result.replace(/\{\{([^}(]+)\}\}/g, (match, varName) => {
    const trimmedName = varName.trim()
    const variable = variableStore.resolvedVariables.value.get(trimmedName)
    if (!variable) return match
    
    // Mask sensitive values in preview
    if (variable.isSecret) {
      return '••••••••'
    }
    return variable.value
  })
  
  return result
})

// Check if value contains functions
const hasFunctions = computed(() => {
  return segments.value.some(s => s.type === 'function')
})

// Get function details for tooltip display
const functionDetails = computed(() => {
  return segments.value
    .filter(s => s.type === 'function')
    .map(s => ({
      name: s.functionName || 'unknown',
      args: s.functionArgs || '',
      full: s.fullContent || s.content,
    }))
})
</script>

<template>
  <div ref="autocompleteRef" :class="cn('relative group', props.class)">
    <!-- Highlighted display layer (behind input) -->
    <div
      ref="highlightRef"
      class="absolute inset-0 pointer-events-none flex items-center px-3 overflow-hidden rounded-md"
      aria-hidden="true"
    >
      <div class="font-mono text-base whitespace-pre">
        <span
          v-for="(segment, i) in segments"
          :key="i"
          :class="{
            'text-foreground': segment.type === 'text',
            'text-blue-400': segment.type === 'variable-valid',
            'text-red-400': segment.type === 'variable-invalid',
            'text-yellow-400': segment.type === 'variable-open',
            'text-purple-400': segment.type === 'function',
          }"
          :style="segment.type !== 'text' ? {
            background: segment.type === 'variable-valid' ? 'rgba(59, 130, 246, 0.3)' : 
                        segment.type === 'variable-invalid' ? 'rgba(239, 68, 68, 0.3)' : 
                        segment.type === 'function' ? 'rgba(168, 85, 247, 0.3)' :
                        'rgba(234, 179, 8, 0.3)',
            borderRadius: '2px',
            boxShadow: segment.type === 'variable-valid' ? 'inset 0 0 0 1px rgba(59, 130, 246, 0.4)' :
                       segment.type === 'variable-invalid' ? 'inset 0 0 0 1px rgba(239, 68, 68, 0.4)' :
                       segment.type === 'function' ? 'inset 0 0 0 1px rgba(168, 85, 247, 0.4)' :
                       'inset 0 0 0 1px rgba(234, 179, 8, 0.4)'
          } : undefined"
        >{{ segment.content }}</span>
      </div>
    </div>

    <!-- Actual input (transparent text, visible caret) -->
    <input
      ref="inputRef"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :class="cn(
        'relative flex h-10 w-full rounded-md border bg-transparent px-3 py-2 font-mono text-base ring-offset-background file:border-0 file:bg-transparent file:text-base file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 text-transparent caret-foreground selection:bg-primary/30',
        hasVariables && !hasUnresolved && 'border-primary/50',
        hasUnresolved && 'border-destructive/50',
        !hasVariables && 'border-input'
      )"
      :style="{ 
        caretColor: hasUnresolved ? 'hsl(var(--destructive))' : hasVariables ? 'hsl(var(--primary))' : undefined
      }"
      autocomplete="off"
      spellcheck="false"
      @input="handleInput"
      @keydown="handleKeyDown"
      @click="handleClick"
      @paste="handlePaste"
      @focus="checkForAutocomplete"
      @mouseenter="handleTooltipMouseEnter"
      @mouseleave="handleTooltipMouseLeave"
      @scroll="syncScroll"
    />
    
    <!-- Variable indicator -->
    <div
      v-if="hasVariables && !showAutocomplete"
      class="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1 pointer-events-none"
    >
      <Icon
        v-if="hasUnresolved"
        name="lucide:alert-triangle"
        class="h-4 w-4 text-destructive"
      />
      <Icon
        v-else
        name="lucide:check-circle"
        class="h-4 w-4 text-primary"
      />
    </div>

    <!-- Autocomplete dropdown -->
    <div
      v-if="showAutocomplete && autocompleteItems.length > 0"
      class="absolute top-full z-50 mt-1 w-80 rounded-md border border-border bg-popover shadow-lg overflow-hidden"
      :style="{ left: autocompletePosition.left + 'px' }"
    >
      <div class="p-1.5 border-b border-border bg-muted/50">
        <span class="text-xs text-muted-foreground">Variables & Functions</span>
      </div>
      <div ref="autocompleteListRef" class="max-h-64 overflow-auto p-1">
        <button
          v-for="(item, index) in autocompleteItems"
          :key="item.type === 'variable' ? item.name : item.snippet"
          :data-index="index"
          :class="[
            'flex w-full items-center gap-2 rounded px-3 py-2 text-sm transition-colors',
            index === selectedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'
          ]"
          @click="insertAutocompleteItem(item)"
          @mouseenter="selectedIndex = index"
        >
          <!-- Variable item -->
          <template v-if="item.type === 'variable'">
            <Icon name="lucide:variable" class="h-4 w-4 text-primary shrink-0" />
            <span class="flex-1 text-left font-mono">{{ item.name }}</span>
            <span class="text-xs text-muted-foreground truncate max-w-[100px]">
              {{ variableStore.resolvedVariables.value.get(item.name)?.isSecret ? '••••••' : variableStore.resolvedVariables.value.get(item.name)?.value }}
            </span>
          </template>
          <!-- Function item -->
          <template v-else>
            <Icon name="lucide:function-square" class="h-4 w-4 text-method-post shrink-0" />
            <div class="flex-1 text-left">
              <div class="font-mono text-sm">{{ item.name }}</div>
              <div class="text-xs text-muted-foreground">{{ item.description }}</div>
            </div>
          </template>
        </button>
      </div>
      <div class="p-1.5 border-t border-border bg-muted/50 text-xs text-muted-foreground flex items-center gap-2">
        <span class="flex items-center gap-1"><kbd class="px-1 rounded bg-muted">↑↓</kbd> navigate</span>
        <span class="flex items-center gap-1"><kbd class="px-1 rounded bg-muted">Tab</kbd> select</span>
        <span class="flex items-center gap-1"><kbd class="px-1 rounded bg-muted">Esc</kbd> close</span>
      </div>
    </div>

    <!-- Variable preview tooltip -->
    <div
      v-if="showTooltip && hasVariables && !showAutocomplete"
      class="absolute left-0 top-full z-40 mt-1 max-w-md rounded-md border border-border bg-popover p-3 shadow-lg"
      @mouseenter="handleTooltipEnter"
      @mouseleave="handleTooltipLeave"
    >
      <div class="space-y-2">
        <div class="flex items-center gap-2 text-xs text-muted-foreground">
          <Icon name="lucide:eye" class="h-3 w-3" />
          <span>Resolved URL</span>
        </div>
        <div class="font-mono text-sm break-all">
          {{ previewValue }}
        </div>
        
        <!-- Function details -->
        <div v-if="hasFunctions" class="mt-2 pt-2 border-t border-border space-y-1.5">
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <Icon name="lucide:function-square" class="h-3 w-3" />
            <span>Functions</span>
          </div>
          <div 
            v-for="fn in functionDetails" 
            :key="fn.full"
            class="flex items-center gap-2 text-xs"
          >
            <span class="text-purple-400 font-mono">${{ fn.name }}</span>
            <template v-if="fn.name === 'sensitive'">
              <span class="text-muted-foreground font-mono">(********)</span>
              <Icon name="lucide:lock" class="h-3 w-3 text-muted-foreground" />
            </template>
            <span v-else-if="fn.args" class="text-muted-foreground font-mono truncate max-w-[200px]">({{ fn.args }})</span>
          </div>
        </div>
        
        <!-- Unresolved variables with Add button -->
        <div v-if="hasUnresolved" class="mt-2 pt-2 border-t border-border space-y-2">
          <div class="flex items-center gap-2 text-xs text-destructive">
            <Icon name="lucide:alert-triangle" class="h-3 w-3" />
            <span>Unresolved Variables</span>
          </div>
          <div 
            v-for="varName in unresolvedVars" 
            :key="varName"
            class="flex items-center justify-between gap-2 py-1"
          >
            <span class="font-mono text-sm text-destructive">{{ varName }}</span>
            <button
              class="flex items-center gap-1.5 px-2 py-1 text-xs font-medium rounded-md bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
              @click="addMissingVariable(varName)"
            >
              <Icon name="lucide:plus" class="h-3 w-3" />
              Add Variable
            </button>
          </div>
        </div>
      </div>
    </div>
    
  </div>
</template>
