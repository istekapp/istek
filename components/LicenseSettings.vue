<script setup lang="ts">
const licenseStore = useLicenseStore()
const { isLicensed, isActivating, error } = licenseStore
const licenseKey = ref('')

onMounted(() => {
  if (isLicensed.value && licenseStore.state.value.key) {
    licenseKey.value = licenseStore.state.value.key
  }
})

const handleActivate = async () => {
  if (!licenseKey.value.trim()) return
  await licenseStore.activateLicense(licenseKey.value.trim())
}
</script>

<template>
  <div>
    <h3 class="text-lg font-semibold mb-1">License</h3>
    <p class="text-sm text-muted-foreground mb-6">Activate your commercial license</p>

    <!-- Status Card -->
    <div 
      :class="[
        'p-4 rounded-lg border mb-6',
        isLicensed 
          ? 'bg-green-500/10 border-green-500/20 text-green-700 dark:text-green-400' 
          : 'bg-muted/50 border-border text-muted-foreground'
      ]"
    >
      <div class="flex items-center gap-3">
        <Icon 
          :name="isLicensed ? 'lucide:check-circle' : 'lucide:key'" 
          class="h-6 w-6" 
        />
        <div>
          <h4 class="font-semibold text-base">
            {{ isLicensed ? 'License Active' : 'No License' }}
          </h4>
          <p class="text-sm opacity-90">
            {{ isLicensed ? 'Thank you for supporting Istek!' : 'Enter your license key to unlock commercial use.' }}
          </p>
        </div>
      </div>
    </div>

    <!-- Activation Form -->
    <div class="space-y-4">
      <label class="block text-sm font-medium">License Key</label>
      <div class="flex gap-2">
        <UiInput
          v-model="licenseKey"
          placeholder="ISTEK-XXXX-XXXX-XXXX"
          class="font-mono h-10 flex-1"
          :disabled="isLicensed || isActivating"
        />
        <UiButton 
          @click="handleActivate" 
          :disabled="isActivating || !licenseKey || isLicensed"
          class="min-w-[100px]"
        >
          <Icon v-if="isActivating" name="lucide:loader-2" class="mr-2 h-4 w-4 animate-spin" />
          {{ isActivating ? 'Activating...' : 'Activate' }}
        </UiButton>
      </div>
      <p v-if="error" class="text-sm text-destructive mt-1">{{ error }}</p>
    </div>

    <!-- Buy Link -->
    <div class="text-center pt-8">
      <p class="text-sm text-muted-foreground mb-2">Don't have a license?</p>
      <a 
        href="https://istek.app/pricing" 
        target="_blank"
        class="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground font-medium hover:bg-primary/90 transition-colors"
      >
        Purchase a License
        <Icon name="lucide:external-link" class="h-4 w-4" />
      </a>
    </div>
  </div>
</template>
