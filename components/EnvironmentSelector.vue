<script setup lang="ts">
const variableStore = useVariableStore()
const { environments, activeEnvironmentId, activeEnvironment, resolvedVariables } = variableStore

const showDropdown = ref(false)
const dropdownRef = useClickOutside(() => {
  showDropdown.value = false
})

const variableCount = computed(() => resolvedVariables.value.size)
</script>

<template>
  <div ref="dropdownRef" class="relative">
    <button
      class="flex items-center gap-2 rounded-md border border-input bg-background px-3 py-2 text-sm font-medium hover:bg-accent transition-colors"
      @click="showDropdown = !showDropdown"
    >
      <span
        v-if="activeEnvironment"
        class="w-3 h-3 rounded-full"
        :style="{ backgroundColor: activeEnvironment.color }"
      />
      <span v-else class="w-3 h-3 rounded-full bg-muted" />
      <span>{{ activeEnvironment?.name || 'No Environment' }}</span>
      <span class="text-xs text-muted-foreground">({{ variableCount }})</span>
      <Icon name="lucide:chevron-down" class="h-4 w-4 text-muted-foreground" />
    </button>

    <div
      v-if="showDropdown"
      class="absolute right-0 top-full z-50 mt-1 w-56 rounded-md border border-border bg-popover p-1.5 shadow-lg"
    >
      <button
        v-for="env in environments"
        :key="env.id"
        :class="[
          'flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-sm transition-colors',
          activeEnvironmentId === env.id
            ? 'bg-accent'
            : 'hover:bg-accent'
        ]"
        @click="variableStore.setActiveEnvironment(env.id); showDropdown = false"
      >
        <span
          class="w-3 h-3 rounded-full shrink-0"
          :style="{ backgroundColor: env.color }"
        />
        <span class="flex-1 text-left">{{ env.name }}</span>
        <span class="text-xs text-muted-foreground">{{ env.variables.length }}</span>
        <Icon
          v-if="activeEnvironmentId === env.id"
          name="lucide:check"
          class="h-4 w-4 text-primary"
        />
      </button>

      <div class="border-t border-border mt-1 pt-1">
        <button
          class="flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
          @click="variableStore.openVariableManager('variables'); showDropdown = false"
        >
          <Icon name="lucide:settings-2" class="h-4 w-4" />
          <span>Manage Variables</span>
        </button>
      </div>
    </div>
  </div>
</template>
