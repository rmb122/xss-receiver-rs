<template>
  <v-card>
    <v-card-title class="d-flex align-center">
      <v-icon icon="mdi-shield-account-outline" class="mr-2" />
      认证管理
      <v-spacer />
      <slot name="actions" />
    </v-card-title>
    <v-tabs color="primary">
      <v-tab to="/auth/users" prepend-icon="mdi-account-group">用户管理</v-tab>
      <v-tab to="/auth/tokens" prepend-icon="mdi-key-outline">Token 管理</v-tab>
    </v-tabs>
    <slot />
  </v-card>
</template>
