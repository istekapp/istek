<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  modelValue: string
  class?: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

provide('tabsValue', computed(() => props.modelValue))
provide('setTabsValue', (value: string) => emit('update:modelValue', value))
</script>

<template>
  <div :class="cn('', props.class)">
    <slot />
  </div>
</template>
