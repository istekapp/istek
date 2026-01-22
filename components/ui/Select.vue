<script setup lang="ts">
import { cn } from '~/lib/utils'
import {
  SelectContent,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from 'radix-vue'

interface SelectOption {
  value: string
  label: string
  icon?: string
}

interface Props {
  modelValue?: string
  options: SelectOption[]
  placeholder?: string
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Select...',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const handleValueChange = (value: string) => {
  emit('update:modelValue', value)
}

// Get the selected option to show icon in trigger
const selectedOption = computed(() => {
  return props.options.find(opt => opt.value === props.modelValue)
})
</script>

<template>
  <SelectRoot :model-value="modelValue" @update:model-value="handleValueChange">
    <SelectTrigger
      :class="cn(
        'flex h-9 w-full items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50',
        props.class
      )"
    >
      <span class="flex items-center gap-2 truncate">
        <Icon v-if="selectedOption?.icon" :name="selectedOption.icon" class="h-4 w-4 shrink-0" />
        <SelectValue :placeholder="placeholder" />
      </span>
      <Icon name="lucide:chevron-down" class="h-4 w-4 opacity-50 shrink-0" />
    </SelectTrigger>

    <SelectPortal>
      <SelectContent
        class="relative z-50 min-w-[8rem] overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95"
        :side-offset="4"
        position="popper"
      >
        <SelectViewport class="p-1">
          <SelectItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            class="relative flex w-full cursor-pointer select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
          >
            <SelectItemIndicator class="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
              <Icon name="lucide:check" class="h-4 w-4" />
            </SelectItemIndicator>
            <span class="flex items-center gap-2">
              <Icon v-if="option.icon" :name="option.icon" class="h-4 w-4 shrink-0" />
              <SelectItemText>{{ option.label }}</SelectItemText>
            </span>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>
