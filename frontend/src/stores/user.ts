import { ref } from 'vue'
import { defineStore } from 'pinia'
import { getCurrentUser } from '@/api/user'
import type { LoggedUser } from '@/types/user'

export const useUserStore = defineStore('user', () => {
  const user = ref<LoggedUser | null>(null)
  const isAdmin = ref(false)
  const isLoaded = ref(false)
  let initialLoadPromise: Promise<void> | null = null

  async function loadUser() {
    try {
      const response = await getCurrentUser()
      user.value = response.data.payload
      isAdmin.value = user.value?.id === 1
      isLoaded.value = true
    } catch {
      user.value = null
      isAdmin.value = false
      isLoaded.value = true
    }
  }

  function getInitialLoadPromise() {
    if (!initialLoadPromise) {
      initialLoadPromise = loadUser()
    }
    return initialLoadPromise
  }

  function clearUser() {
    user.value = null
    isAdmin.value = false
    isLoaded.value = false
    initialLoadPromise = null
  }

  return { user, isAdmin, isLoaded, loadUser, getInitialLoadPromise, clearUser }
})
