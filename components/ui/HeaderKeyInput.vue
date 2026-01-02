<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  modelValue?: string
  placeholder?: string
  disabled?: boolean
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  placeholder: '',
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const inputRef = ref<HTMLInputElement | null>(null)
const showAutocomplete = ref(false)
const selectedIndex = ref(0)
const searchTerm = ref('')

// Common HTTP headers
const commonHeaders = [
  // Authentication & Authorization
  'Authorization',
  'WWW-Authenticate',
  'Proxy-Authorization',
  'Proxy-Authenticate',
  
  // Content
  'Content-Type',
  'Content-Length',
  'Content-Encoding',
  'Content-Language',
  'Content-Location',
  'Content-Disposition',
  'Content-Range',
  'Content-MD5',
  
  // Caching
  'Cache-Control',
  'Expires',
  'Pragma',
  'ETag',
  'If-Match',
  'If-None-Match',
  'If-Modified-Since',
  'If-Unmodified-Since',
  'Last-Modified',
  'Age',
  'Vary',
  
  // Request Context
  'Accept',
  'Accept-Charset',
  'Accept-Encoding',
  'Accept-Language',
  'Accept-Ranges',
  'Host',
  'Referer',
  'User-Agent',
  'Origin',
  
  // Connection
  'Connection',
  'Keep-Alive',
  'Upgrade',
  'Transfer-Encoding',
  
  // CORS
  'Access-Control-Allow-Origin',
  'Access-Control-Allow-Methods',
  'Access-Control-Allow-Headers',
  'Access-Control-Allow-Credentials',
  'Access-Control-Expose-Headers',
  'Access-Control-Max-Age',
  'Access-Control-Request-Method',
  'Access-Control-Request-Headers',
  
  // Security
  'X-Frame-Options',
  'X-Content-Type-Options',
  'X-XSS-Protection',
  'Strict-Transport-Security',
  'Content-Security-Policy',
  
  // Cookies
  'Cookie',
  'Set-Cookie',
  
  // Custom/Common
  'X-API-Key',
  'X-Auth-Token',
  'X-Request-ID',
  'X-Correlation-ID',
  'X-Forwarded-For',
  'X-Forwarded-Host',
  'X-Forwarded-Proto',
  'X-Real-IP',
  
  // Rate Limiting
  'X-RateLimit-Limit',
  'X-RateLimit-Remaining',
  'X-RateLimit-Reset',
  'Retry-After',
  
  // Misc
  'Date',
  'Location',
  'Server',
  'Allow',
  'Range',
  'DNT',
  'Expect',
  'From',
  'TE',
  'Trailer',
  'Via',
  'Warning',
]

const filteredHeaders = computed(() => {
  const term = searchTerm.value.toLowerCase()
  if (!term) return commonHeaders.slice(0, 15)
  return commonHeaders.filter(h => h.toLowerCase().includes(term)).slice(0, 15)
})

const handleInput = (e: Event) => {
  const target = e.target as HTMLInputElement
  const value = target.value
  searchTerm.value = value
  emit('update:modelValue', value)
  
  if (value) {
    showAutocomplete.value = true
    selectedIndex.value = 0
  } else {
    showAutocomplete.value = false
  }
}

const handleFocus = () => {
  if (props.modelValue || filteredHeaders.value.length > 0) {
    searchTerm.value = props.modelValue
    showAutocomplete.value = true
    selectedIndex.value = 0
  }
}

const handleBlur = () => {
  // Delay to allow click on autocomplete item
  setTimeout(() => {
    showAutocomplete.value = false
  }, 150)
}

const selectHeader = (header: string) => {
  emit('update:modelValue', header)
  searchTerm.value = header
  showAutocomplete.value = false
  inputRef.value?.focus()
}

const handleKeyDown = (e: KeyboardEvent) => {
  if (!showAutocomplete.value) return
  
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value + 1) % filteredHeaders.value.length
      break
    case 'ArrowUp':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value - 1 + filteredHeaders.value.length) % filteredHeaders.value.length
      break
    case 'Enter':
    case 'Tab':
      if (filteredHeaders.value.length > 0) {
        e.preventDefault()
        selectHeader(filteredHeaders.value[selectedIndex.value])
      }
      break
    case 'Escape':
      showAutocomplete.value = false
      break
  }
}
</script>

<template>
  <div :class="cn('relative', props.class)">
    <input
      ref="inputRef"
      type="text"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :class="cn(
        'flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-base ring-offset-background file:border-0 file:bg-transparent file:text-base file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50'
      )"
      autocomplete="off"
      spellcheck="false"
      @input="handleInput"
      @focus="handleFocus"
      @blur="handleBlur"
      @keydown="handleKeyDown"
    />
    
    <!-- Autocomplete dropdown -->
    <div
      v-if="showAutocomplete && filteredHeaders.length > 0"
      class="absolute left-0 top-full z-50 mt-1 w-full rounded-md border border-border bg-popover shadow-lg overflow-hidden"
    >
      <div class="max-h-64 overflow-auto p-1">
        <button
          v-for="(header, index) in filteredHeaders"
          :key="header"
          :class="[
            'flex w-full items-center gap-2 rounded px-3 py-2 text-sm transition-colors text-left',
            index === selectedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'
          ]"
          @mousedown.prevent="selectHeader(header)"
          @mouseenter="selectedIndex = index"
        >
          <Icon name="lucide:heading" class="h-4 w-4 text-muted-foreground shrink-0" />
          <span class="font-mono">{{ header }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
