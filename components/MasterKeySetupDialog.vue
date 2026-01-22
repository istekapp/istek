<script setup lang="ts">
const encryption = useWorkspaceEncryption()

const step = ref<'choose' | 'generate' | 'import' | 'confirm'>('choose')
const importKey = ref('')
const importError = ref('')
const copied = ref(false)
const confirmed = ref(false)
const loading = ref(false)

// Start generation flow
const startGenerate = async () => {
  console.log('[MasterKeySetup] startGenerate called')
  loading.value = true
  try {
    await encryption.generateMasterKey()
    console.log('[MasterKeySetup] Key generated, moving to generate step')
    step.value = 'generate'
  } catch (e) {
    console.error('[MasterKeySetup] Failed to generate key:', e)
  } finally {
    loading.value = false
  }
}

// Copy master key to clipboard
const copyKey = async () => {
  if (encryption.masterKeyForDisplay.value) {
    await navigator.clipboard.writeText(encryption.masterKeyForDisplay.value)
    copied.value = true
    setTimeout(() => { copied.value = false }, 2000)
  }
}

// Confirm and store the key
const confirmAndStore = async () => {
  console.log('[MasterKeySetup] confirmAndStore called', { 
    confirmed: confirmed.value, 
    hasKey: !!encryption.masterKeyForDisplay.value 
  })
  
  if (!confirmed.value || !encryption.masterKeyForDisplay.value) {
    console.log('[MasterKeySetup] Early return - confirmed or key missing')
    return
  }
  
  loading.value = true
  try {
    console.log('[MasterKeySetup] Storing master key...')
    await encryption.storeMasterKey(encryption.masterKeyForDisplay.value)
    console.log('[MasterKeySetup] Master key stored successfully')
    close()
  } catch (e) {
    console.error('[MasterKeySetup] Failed to store key:', e)
  } finally {
    loading.value = false
  }
}

// Import existing key
const submitImport = async () => {
  if (!importKey.value.trim()) {
    importError.value = 'Please enter a master key'
    return
  }
  
  loading.value = true
  importError.value = ''
  try {
    console.log('[MasterKeySetup] Importing master key...')
    await encryption.importMasterKey(importKey.value.trim())
    console.log('[MasterKeySetup] Master key imported successfully')
    close()
  } catch (e: any) {
    importError.value = e.message || 'Invalid master key'
  } finally {
    loading.value = false
  }
}

// Close and reset
const close = () => {
  encryption.cancelSetup()
  step.value = 'choose'
  importKey.value = ''
  importError.value = ''
  confirmed.value = false
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="encryption.showMasterKeySetup.value"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      @click.self="close"
    >
      <div class="w-[500px] rounded-lg border border-border bg-card shadow-2xl">
        <!-- Step: Choose -->
        <template v-if="step === 'choose'">
          <!-- Header -->
          <div class="border-b border-border px-6 py-4">
            <h2 class="text-lg font-semibold flex items-center gap-2">
              <Icon name="lucide:shield" class="h-5 w-5 text-primary" />
              Enable Workspace Encryption
            </h2>
            <p class="text-sm text-muted-foreground mt-1">
              Sensitive values are encrypted with AES-256-GCM and can be safely committed to git.
              You'll need a master key to encrypt and decrypt these values.
            </p>
          </div>

          <!-- Content -->
          <div class="p-6 space-y-4">
            <button
              class="w-full p-4 rounded-lg border border-border hover:border-primary/50 hover:bg-accent/50 transition-colors text-left"
              :disabled="loading"
              @click="startGenerate"
            >
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
                  <Icon v-if="loading" name="lucide:loader-2" class="h-5 w-5 text-primary animate-spin" />
                  <Icon v-else name="lucide:key" class="h-5 w-5 text-primary" />
                </div>
                <div>
                  <div class="font-medium">Generate New Master Key</div>
                  <div class="text-sm text-muted-foreground mt-1">
                    Create a new master key for this workspace. You'll need to save it securely and share it with team members.
                  </div>
                </div>
              </div>
            </button>

            <button
              class="w-full p-4 rounded-lg border border-border hover:border-primary/50 hover:bg-accent/50 transition-colors text-left"
              @click="step = 'import'"
            >
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-lg bg-amber-500/10 flex items-center justify-center shrink-0">
                  <Icon name="lucide:download" class="h-5 w-5 text-amber-500" />
                </div>
                <div>
                  <div class="font-medium">Import Existing Key</div>
                  <div class="text-sm text-muted-foreground mt-1">
                    Enter a master key shared by a team member to decrypt existing sensitive values.
                  </div>
                </div>
              </div>
            </button>
          </div>

          <!-- Footer -->
          <div class="border-t border-border px-6 py-4 flex justify-end">
            <UiButton variant="outline" @click="close">Cancel</UiButton>
          </div>
        </template>

        <!-- Step: Generate - Show the key -->
        <template v-else-if="step === 'generate'">
          <!-- Header -->
          <div class="border-b border-border px-6 py-4">
            <h2 class="text-lg font-semibold flex items-center gap-2">
              <Icon name="lucide:key" class="h-5 w-5 text-primary" />
              Your Master Key
            </h2>
            <p class="text-sm text-muted-foreground mt-1">
              <span class="text-destructive font-medium">This is the only time you'll see this key!</span>
              Save it in a secure location like a password manager.
            </p>
          </div>

          <!-- Content -->
          <div class="p-6 space-y-4">
            <!-- Warning banner -->
            <div class="p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-500 text-sm flex items-start gap-2">
              <Icon name="lucide:alert-triangle" class="h-4 w-4 mt-0.5 shrink-0" />
              <div>
                <strong>Important:</strong> If you lose this key, you won't be able to decrypt your sensitive values.
                Share this key securely with team members who need access.
              </div>
            </div>

            <!-- The key -->
            <div class="relative">
              <div class="p-4 rounded-lg bg-muted font-mono text-sm break-all select-all border border-border">
                {{ encryption.masterKeyForDisplay.value }}
              </div>
              <button
                class="absolute top-2 right-2 p-2 rounded-md hover:bg-background transition-colors"
                @click="copyKey"
              >
                <Icon 
                  :name="copied ? 'lucide:check' : 'lucide:copy'" 
                  :class="['h-4 w-4', copied ? 'text-green-500' : 'text-muted-foreground']" 
                />
              </button>
            </div>

            <!-- Confirmation checkbox -->
            <label class="flex items-start gap-3 cursor-pointer">
              <input
                v-model="confirmed"
                type="checkbox"
                class="mt-1 h-4 w-4 rounded border-border text-primary focus:ring-primary"
              />
              <span class="text-sm">
                I have saved this master key in a secure location and understand that I cannot recover it if lost.
              </span>
            </label>
          </div>

          <!-- Footer -->
          <div class="border-t border-border px-6 py-4 flex justify-end gap-3">
            <UiButton variant="outline" @click="step = 'choose'">Back</UiButton>
            <UiButton 
              :disabled="!confirmed || loading" 
              @click="confirmAndStore"
            >
              <Icon v-if="loading" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
              Enable Encryption
            </UiButton>
          </div>
        </template>

        <!-- Step: Import existing key -->
        <template v-else-if="step === 'import'">
          <!-- Header -->
          <div class="border-b border-border px-6 py-4">
            <h2 class="text-lg font-semibold flex items-center gap-2">
              <Icon name="lucide:download" class="h-5 w-5 text-amber-500" />
              Import Master Key
            </h2>
            <p class="text-sm text-muted-foreground mt-1">
              Enter the master key shared by a team member to access encrypted sensitive values.
            </p>
          </div>

          <!-- Content -->
          <div class="p-6 space-y-4">
            <div class="space-y-2">
              <label class="text-sm font-medium">Master Key</label>
              <textarea
                v-model="importKey"
                rows="3"
                class="w-full p-3 rounded-lg border border-border bg-background font-mono text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary"
                placeholder="Paste your master key here..."
              />
              <p v-if="importError" class="text-sm text-destructive">{{ importError }}</p>
            </div>

            <div class="p-3 rounded-lg bg-muted/50 text-sm text-muted-foreground">
              <Icon name="lucide:info" class="h-4 w-4 inline mr-1" />
              The master key is stored securely in your system keychain and never leaves your device.
            </div>
          </div>

          <!-- Footer -->
          <div class="border-t border-border px-6 py-4 flex justify-end gap-3">
            <UiButton variant="outline" @click="step = 'choose'">Back</UiButton>
            <UiButton 
              :disabled="!importKey.trim() || loading" 
              @click="submitImport"
            >
              <Icon v-if="loading" name="lucide:loader-2" class="h-4 w-4 mr-2 animate-spin" />
              Import Key
            </UiButton>
          </div>
        </template>
      </div>
    </div>
  </Teleport>
</template>
