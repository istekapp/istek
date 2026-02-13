<script setup lang="ts">
const licenseStore = useLicenseStore()
const variableStore = useVariableStore()
const { shouldShowEvaluationDialog } = licenseStore

function openLicenseSettings() {
  licenseStore.dismissEvaluationExpired()
  variableStore.openVariableManager('license')
}

function continueFree() {
  licenseStore.dismissEvaluationExpired()
}
</script>

<template>
  <!-- Evaluation Expired Dialog -->
  <Teleport to="body">
    <Transition
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="shouldShowEvaluationDialog"
        class="fixed inset-0 z-[300] flex items-center justify-center"
      >
        <!-- Backdrop -->
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" />

        <!-- Dialog -->
        <Transition
          enter-active-class="transition-all duration-300 ease-out delay-100"
          enter-from-class="opacity-0 scale-95 translate-y-4"
          enter-to-class="opacity-100 scale-100 translate-y-0"
          leave-active-class="transition-all duration-200 ease-in"
          leave-from-class="opacity-100 scale-100 translate-y-0"
          leave-to-class="opacity-0 scale-95 translate-y-4"
        >
          <div
            v-if="shouldShowEvaluationDialog"
            class="relative w-full max-w-md mx-4 bg-popover border border-border rounded-2xl shadow-2xl overflow-hidden"
          >
            <!-- Decorative top accent -->
            <div class="h-1 bg-gradient-to-r from-primary via-purple-500 to-primary" />

            <!-- Content -->
            <div class="p-8 text-center">
              <!-- Icon -->
              <div class="w-16 h-16 mx-auto mb-5 rounded-2xl bg-primary/10 border border-primary/20 flex items-center justify-center">
                <Icon name="lucide:clock" class="h-8 w-8 text-primary" />
              </div>

              <!-- Title -->
              <h2 class="text-xl font-bold mb-2">
                Your Trial Has Wrapped Up
              </h2>

              <!-- Description -->
              <p class="text-sm text-muted-foreground leading-relaxed mb-6">
                Thanks for taking Istek for a spin! The preview window has closed, but nothing changes
                — the app is still yours to use at no cost. If Istek is part of your professional
                workflow, picking up a license helps keep the project moving forward.
              </p>

              <!-- Buttons -->
              <div class="flex flex-col gap-3">
                <button
                  class="w-full px-4 py-2.5 text-sm font-medium bg-primary text-primary-foreground rounded-lg hover:bg-primary/90 transition-colors"
                  @click="openLicenseSettings"
                >
                  Activate a License
                </button>
                <button
                  class="w-full px-4 py-2.5 text-sm text-muted-foreground hover:text-foreground hover:bg-accent rounded-lg transition-colors"
                  @click="continueFree"
                >
                  Continue Free
                </button>
              </div>
            </div>

            <!-- Footer -->
            <div class="px-8 py-4 border-t border-border bg-muted/30">
              <p class="text-xs text-muted-foreground text-center leading-relaxed">
                Personal and non-commercial use is always free. Licenses apply to workplace and business use only.
                <a
                  href="https://istek.app/pricing"
                  target="_blank"
                  class="text-primary hover:underline"
                >See pricing</a>
              </p>
            </div>
          </div>
        </Transition>
      </div>
    </Transition>
  </Teleport>
</template>
