<template>
  <v-app-bar color="primary" density="comfortable">
    <v-app-bar-title
      ><v-icon icon="mdi-mailbox-outline" class="mr-3"></v-icon>XSS Receiver</v-app-bar-title
    >
    <template #append>
      <v-btn to="/" variant="text" prepend-icon="mdi-web">HTTP 日志</v-btn>
      <v-btn to="/http-routes" variant="text" prepend-icon="mdi-routes">HTTP 路由</v-btn>
      <v-btn to="/dns-log" variant="text" prepend-icon="mdi-dns-outline">DNS 日志</v-btn>
      <v-btn to="/dns-routes" variant="text" prepend-icon="mdi-routes">DNS 路由</v-btn>
      <v-btn to="/files" variant="text" prepend-icon="mdi-folder-multiple">文件管理</v-btn>
      <v-btn
        v-if="userStore.isAdmin"
        to="/system-log"
        variant="text"
        prepend-icon="mdi-text-box-outline"
        >系统日志</v-btn
      >
      <v-btn to="/users" variant="text" prepend-icon="mdi-account-group">用户管理</v-btn>
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()
</script>
