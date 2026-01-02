export const useClickOutside = (callback: () => void) => {
  const elementRef = ref<HTMLElement | null>(null)

  const handleClickOutside = (event: MouseEvent) => {
    if (elementRef.value && !elementRef.value.contains(event.target as Node)) {
      callback()
    }
  }

  onMounted(() => {
    document.addEventListener('click', handleClickOutside, true)
  })

  onUnmounted(() => {
    document.removeEventListener('click', handleClickOutside, true)
  })

  return elementRef
}
