<script setup lang="ts">
import { cn } from '~/lib/utils'

interface Props {
  value: string
  class?: string
}

const props = defineProps<Props>()

const currentValue = inject<Ref<string>>('tabsValue')
const isActive = computed(() => currentValue?.value === props.value)
</script>

<template>
  <div
    v-if="isActive"
    :class="cn(
      'mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
      props.class
    )"
  >
    <slot />
  </div>
</template>
