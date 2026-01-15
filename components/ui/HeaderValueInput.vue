<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  modelValue?: string
  headerKey?: string
  placeholder?: string
  disabled?: boolean
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  headerKey: '',
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

// Header value suggestions based on header key
const headerValueSuggestions: Record<string, string[]> = {
  'Content-Type': [
    'application/json',
    'application/xml',
    'application/x-www-form-urlencoded',
    'multipart/form-data',
    'text/plain',
    'text/html',
    'text/css',
    'text/javascript',
    'application/javascript',
    'application/pdf',
    'application/octet-stream',
    'image/png',
    'image/jpeg',
    'image/gif',
    'image/webp',
    'image/svg+xml',
    'audio/mpeg',
    'video/mp4',
    'application/zip',
    'application/gzip',
  ],
  'Accept': [
    'application/json',
    'application/xml',
    'text/plain',
    'text/html',
    '*/*',
    'application/json, text/plain, */*',
    'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'image/*',
    'audio/*',
    'video/*',
  ],
  'Accept-Encoding': [
    'gzip',
    'deflate',
    'br',
    'gzip, deflate',
    'gzip, deflate, br',
    'identity',
    '*',
  ],
  'Accept-Language': [
    'en-US',
    'en-GB',
    'en',
    'tr-TR',
    'tr',
    'de-DE',
    'de',
    'fr-FR',
    'fr',
    'es-ES',
    'es',
    'it-IT',
    'it',
    'pt-BR',
    'pt',
    'ja-JP',
    'ja',
    'zh-CN',
    'zh',
    'ko-KR',
    'ko',
    'en-US,en;q=0.9',
    '*',
  ],
  'Authorization': [
    'Bearer ',
    'Basic ',
    'Digest ',
    'OAuth ',
    'Token ',
    'Api-Key ',
  ],
  'Cache-Control': [
    'no-cache',
    'no-store',
    'max-age=0',
    'max-age=3600',
    'max-age=86400',
    'max-age=31536000',
    'must-revalidate',
    'no-cache, no-store, must-revalidate',
    'public',
    'private',
    'immutable',
  ],
  'Connection': [
    'keep-alive',
    'close',
    'upgrade',
  ],
  'Content-Encoding': [
    'gzip',
    'deflate',
    'br',
    'identity',
  ],
  'Content-Disposition': [
    'inline',
    'attachment',
    'attachment; filename="file.txt"',
    'form-data; name="field"',
    'form-data; name="file"; filename="document.pdf"',
  ],
  'X-Content-Type-Options': [
    'nosniff',
  ],
  'X-Frame-Options': [
    'DENY',
    'SAMEORIGIN',
    'ALLOW-FROM ',
  ],
  'X-XSS-Protection': [
    '0',
    '1',
    '1; mode=block',
  ],
  'Strict-Transport-Security': [
    'max-age=31536000',
    'max-age=31536000; includeSubDomains',
    'max-age=31536000; includeSubDomains; preload',
  ],
  'Access-Control-Allow-Origin': [
    '*',
    'null',
  ],
  'Access-Control-Allow-Methods': [
    'GET',
    'POST',
    'PUT',
    'DELETE',
    'PATCH',
    'OPTIONS',
    'GET, POST',
    'GET, POST, PUT, DELETE',
    'GET, POST, PUT, DELETE, PATCH, OPTIONS',
    '*',
  ],
  'Access-Control-Allow-Headers': [
    'Content-Type',
    'Authorization',
    'X-Requested-With',
    'Content-Type, Authorization',
    'Content-Type, Authorization, X-Requested-With',
    '*',
  ],
  'Access-Control-Allow-Credentials': [
    'true',
    'false',
  ],
  'Access-Control-Max-Age': [
    '86400',
    '3600',
    '600',
  ],
  'Pragma': [
    'no-cache',
  ],
  'Expires': [
    '0',
    '-1',
  ],
  'Transfer-Encoding': [
    'chunked',
    'compress',
    'deflate',
    'gzip',
    'identity',
  ],
  'User-Agent': [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
    'Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
    'curl/8.0.0',
    'Istek/1.0',
  ],
  'Origin': [
    'http://localhost:3000',
    'http://localhost:8080',
    'https://example.com',
  ],
  'Referer': [
    'http://localhost:3000/',
    'https://example.com/',
  ],
  'DNT': [
    '0',
    '1',
  ],
  'Upgrade': [
    'websocket',
    'h2c',
  ],
  'If-Match': [
    '*',
  ],
  'If-None-Match': [
    '*',
  ],
  'Range': [
    'bytes=0-',
    'bytes=0-1023',
    'bytes=0-499, 500-999',
  ],
  'Content-Security-Policy': [
    "default-src 'self'",
    "default-src 'self'; script-src 'self' 'unsafe-inline'",
    "default-src 'self'; img-src *; media-src media1.com media2.com; script-src userscripts.example.com",
  ],
}

const normalizedHeaderKey = computed(() => {
  // Normalize header key for matching (case-insensitive)
  const key = props.headerKey.toLowerCase()
  return Object.keys(headerValueSuggestions).find(k => k.toLowerCase() === key) || ''
})

const availableSuggestions = computed(() => {
  return headerValueSuggestions[normalizedHeaderKey.value] || []
})

const filteredSuggestions = computed(() => {
  const term = searchTerm.value.toLowerCase()
  const suggestions = availableSuggestions.value
  if (!term) return suggestions.slice(0, 15)
  return suggestions.filter(s => s.toLowerCase().includes(term)).slice(0, 15)
})

const handleInput = (e: Event) => {
  const target = e.target as HTMLInputElement
  const value = target.value
  searchTerm.value = value
  emit('update:modelValue', value)
  
  if (availableSuggestions.value.length > 0) {
    showAutocomplete.value = true
    selectedIndex.value = 0
  }
}

const handleFocus = () => {
  if (availableSuggestions.value.length > 0) {
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

const selectValue = (value: string) => {
  emit('update:modelValue', value)
  searchTerm.value = value
  showAutocomplete.value = false
  inputRef.value?.focus()
}

const handleKeyDown = (e: KeyboardEvent) => {
  if (!showAutocomplete.value || filteredSuggestions.value.length === 0) return
  
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value + 1) % filteredSuggestions.value.length
      break
    case 'ArrowUp':
      e.preventDefault()
      selectedIndex.value = (selectedIndex.value - 1 + filteredSuggestions.value.length) % filteredSuggestions.value.length
      break
    case 'Enter':
    case 'Tab':
      if (filteredSuggestions.value.length > 0) {
        e.preventDefault()
        selectValue(filteredSuggestions.value[selectedIndex.value])
      }
      break
    case 'Escape':
      showAutocomplete.value = false
      break
  }
}

// Watch for header key changes to reset state
watch(() => props.headerKey, () => {
  selectedIndex.value = 0
})
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
      v-if="showAutocomplete && filteredSuggestions.length > 0"
      class="absolute left-0 top-full z-50 mt-1 w-full rounded-md border border-border bg-popover shadow-lg overflow-hidden"
    >
      <div class="max-h-64 overflow-auto p-1">
        <button
          v-for="(suggestion, index) in filteredSuggestions"
          :key="suggestion"
          :class="[
            'flex w-full items-center gap-2 rounded px-3 py-2 text-sm transition-colors text-left',
            index === selectedIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'
          ]"
          @mousedown.prevent="selectValue(suggestion)"
          @mouseenter="selectedIndex = index"
        >
          <Icon name="lucide:text" class="h-4 w-4 text-muted-foreground shrink-0" />
          <span class="font-mono truncate">{{ suggestion }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
