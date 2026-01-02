<script setup lang="ts">
import type { HttpMethod } from '~/types'
import { cn } from '~/lib/utils'

interface Props {
  method: HttpMethod
  size?: 'sm' | 'md'
}

const props = withDefaults(defineProps<Props>(), {
  size: 'md',
})

const methodClass = computed(() => `method-badge-${props.method.toLowerCase()}`)
</script>

<template>
  <span
    :class="cn(
      'inline-flex items-center justify-center rounded border font-mono font-semibold uppercase',
      methodClass,
      size === 'sm' ? 'h-6 min-w-[3rem] px-1.5 text-xs' : 'h-7 min-w-[3.5rem] px-2 text-sm'
    )"
  >
    {{ method }}
  </span>
</template>
