<template>
  <v-app>
    <AppNavbar v-if="showNavbar" />
    <MessageToast />
    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AppNavbar from '@/components/AppNavbar.vue'
import MessageToast from '@/components/MessageToast.vue'
import { useUserStore } from '@/stores/user'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()

const showNavbar = computed(() => route.path !== '/login')

watch(
  () => route.path,
  async () => {
    if (route.path === '/login') return
    try {
      await userStore.getInitialLoadPromise()
      if (!userStore.user) {
        router.push('/login')
      }
    } catch {
      router.push('/login')
    }
  },
  { immediate: true },
)
</script>
