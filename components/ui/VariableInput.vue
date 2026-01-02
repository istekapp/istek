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
  { name: 'hash.md5', snippet: 'hash.md5("value")', description: 'MD5 hash' },
  { name: 'hash.sha1', snippet: 'hash.sha1("value")', description: 'SHA1 hash' },
  { name: 'hash.sha256', snippet: 'hash.sha256("value")', description: 'SHA256 hash' },
  { name: 'hash.sha512', snippet: 'hash.sha512("value")', description: 'SHA512 hash' },
  // HMAC functions
  { name: 'hmac.sha256', snippet: 'hmac.sha256("value", "key")', description: 'HMAC-SHA256' },
  { name: 'hmac.sha512', snippet: 'hmac.sha512("value", "key")', description: 'HMAC-SHA512' },
  // Encoding functions
  { name: 'base64.encode', snippet: 'base64.encode("value")', description: 'Base64 encode' },
  { name: 'base64.decode', snippet: 'base64.decode("encoded")', description: 'Base64 decode' },
  { name: 'url.encode', snippet: 'url.encode("value")', description: 'URL encode' },
  { name: 'url.decode', snippet: 'url.decode("encoded")', description: 'URL decode' },
  // Encryption (keychain)
  { name: 'encrypt', snippet: 'encrypt("key_name")', description: 'Get value from keychain' },
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

// Parse text into segments for highlighting
const segments = computed(() => {
  const text = props.modelValue || ''
  const result: Array<{ type: 'text' | 'variable-valid' | 'variable-invalid' | 'variable-open'; content: string }> = []
  
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
      const isValid = variableStore.resolvedVariables.value.has(varName)
      result.push({ 
        type: isValid ? 'variable-valid' : 'variable-invalid', 
        content: match[0] 
      })
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

// Handle keyboard navigation
const handleKeyDown = (e: KeyboardEvent) => {
  if (!showAutocomplete.value) return
  
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value + 1) % autocompleteItems.value.length
      break
    case 'ArrowUp':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value - 1 + autocompleteItems.value.length) % autocompleteItems.value.length
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

// Tooltip state
const showTooltip = ref(false)
const previewValue = computed(() => {
  if (!hasVariables.value) return props.modelValue
  return variableStore.interpolate(props.modelValue)
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
      <div class="font-mono text-base whitespace-pre" style="letter-spacing: 0.08em">
        <span
          v-for="(segment, i) in segments"
          :key="i"
          :class="{
            'text-foreground': segment.type === 'text',
            'text-blue-400 font-semibold': segment.type === 'variable-valid',
            'text-red-400 font-semibold': segment.type === 'variable-invalid',
            'text-yellow-400 font-semibold': segment.type === 'variable-open',
          }"
          :style="segment.type !== 'text' ? {
            background: segment.type === 'variable-valid' ? 'rgba(59, 130, 246, 0.3)' : 
                        segment.type === 'variable-invalid' ? 'rgba(239, 68, 68, 0.3)' : 
                        'rgba(234, 179, 8, 0.3)',
            borderRadius: '2px',
            boxShadow: segment.type === 'variable-valid' ? 'inset 0 0 0 1px rgba(59, 130, 246, 0.4)' :
                       segment.type === 'variable-invalid' ? 'inset 0 0 0 1px rgba(239, 68, 68, 0.4)' :
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
        caretColor: hasUnresolved ? 'hsl(var(--destructive))' : hasVariables ? 'hsl(var(--primary))' : undefined,
        letterSpacing: '0.08em'
      }"
      autocomplete="off"
      spellcheck="false"
      @input="handleInput"
      @keydown="handleKeyDown"
      @click="handleClick"
      @paste="handlePaste"
      @focus="checkForAutocomplete"
      @mouseenter="showTooltip = true"
      @mouseleave="showTooltip = false"
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
      <div class="max-h-64 overflow-auto p-1">
        <button
          v-for="(item, index) in autocompleteItems"
          :key="item.type === 'variable' ? item.name : item.snippet"
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
    >
      <div class="space-y-2">
        <div class="flex items-center gap-2 text-xs text-muted-foreground">
          <Icon name="lucide:eye" class="h-3 w-3" />
          <span>Resolved URL</span>
        </div>
        <div class="font-mono text-sm break-all">
          {{ previewValue }}
        </div>
        <div v-if="hasUnresolved" class="flex items-start gap-2 text-xs text-destructive mt-2 pt-2 border-t border-border">
          <Icon name="lucide:alert-triangle" class="h-3 w-3 mt-0.5" />
          <div>
            <span>Unresolved: </span>
            <span class="font-mono">{{ unresolvedVars.join(', ') }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
