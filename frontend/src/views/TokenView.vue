<template>
  <AuthCard>
    <template #actions>
      <v-btn color="primary" prepend-icon="mdi-plus" :disabled="loading" @click="openCreateDialog">
        签发 token
      </v-btn>
    </template>
    <v-data-table
      :headers="headers"
      :items="tokens"
      :loading="loading"
      density="comfortable"
      no-data-text="暂无 token"
      :items-per-page="-1"
    >
      <template #item.create_time="{ item }">{{ formatTime(item.create_time) }}</template>
      <template #item.expires_at="{ item }">
        {{ item.expires_at ? formatTime(item.expires_at) : '永久有效' }}
      </template>
      <template #item.allowed_ip="{ item }">{{ item.allowed_ip ?? '不限' }}</template>
      <template #item.status="{ item }">
        <v-chip :color="isExpired(item) ? 'default' : 'success'" size="small" variant="tonal">
          {{ isExpired(item) ? '已过期' : '有效' }}
        </v-chip>
      </template>
      <template #item.actions="{ item }">
        <v-btn
          icon="mdi-key-remove"
          variant="text"
          color="error"
          size="small"
          aria-label="撤销 token"
          title="撤销 token"
          :loading="revokingId === item.id"
          @click="handleRevoke(item)"
        />
      </template>
      <template #bottom />
    </v-data-table>
  </AuthCard>

  <v-dialog v-model="dialogVisible" :persistent="saving" max-width="560">
    <v-card>
      <v-card-title>{{ issuedToken ? 'token 已签发' : '签发 token' }}</v-card-title>
      <template v-if="issuedToken">
        <v-card-text>
          <v-alert type="info" variant="tonal" class="mb-4">
            token 仅显示一次, 请在关闭前复制保存.
          </v-alert>
          <v-textarea :model-value="issuedToken" label="Token" readonly auto-grow rows="2" />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="dialogVisible = false">关闭</v-btn>
          <v-btn color="primary" variant="flat" prepend-icon="mdi-content-copy" @click="copyToken">
            复制 token
          </v-btn>
        </v-card-actions>
      </template>
      <v-form v-else ref="createForm" @submit.prevent="handleCreate">
        <v-card-text>
          <v-text-field
            v-model="name"
            label="名称"
            placeholder="例如: AI 助手"
            :rules="[(value: string) => !!value.trim() || '请输入名称']"
            variant="outlined"
            density="compact"
            hide-details
          />
          <v-switch v-model="permanent" label="永久有效" color="primary" hide-details />
          <v-text-field
            v-if="!permanent"
            v-model="expiresAt"
            type="datetime-local"
            label="到期时间 (本地时区)"
            :rules="[validateExpiry]"
            variant="outlined"
            density="compact"
            class="mb-4"
            hide-details
          />
          <v-text-field
            v-model="allowedIp"
            label="允许的客户端 IP (可选)"
            hint="留空表示不限制, 支持单个 IPv4 或 IPv6 地址."
            persistent-hint
            variant="outlined"
            density="compact"
          />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" :disabled="saving" @click="dialogVisible = false">取消</v-btn>
          <v-btn color="primary" variant="flat" type="submit" :loading="saving">签发</v-btn>
        </v-card-actions>
      </v-form>
    </v-card>
  </v-dialog>

  <ConfirmDialog ref="confirmDialog" />
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useNow } from '@vueuse/core'
import type { DataTableHeader } from 'vuetify'
import { VForm } from 'vuetify/components'
import { createToken, deleteToken, getTokens } from '@/api/token'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import AuthCard from '@/components/AuthCard.vue'
import type { TokenInfo } from '@/types/token'
import { formatTime } from '@/utils/format'
import { showErrorToast, showSuccessToast } from '@/utils/toast'

const headers: DataTableHeader[] = [
  { title: '名称', key: 'name' },
  { title: '所属用户', key: 'username' },
  { title: '创建时间', key: 'create_time' },
  { title: '到期时间', key: 'expires_at' },
  { title: '客户端 IP', key: 'allowed_ip' },
  { title: '状态', key: 'status', sortable: false },
  { title: '操作', key: 'actions', align: 'center', sortable: false },
]

const tokens = ref<TokenInfo[]>([])
const loading = ref(false)
const saving = ref(false)
const revokingId = ref<number | null>(null)
const dialogVisible = ref(false)
const issuedToken = ref('')
const name = ref('')
const permanent = ref(true)
const expiresAt = ref('')
const allowedIp = ref('')
const createForm = ref<InstanceType<typeof VForm>>()
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()
const now = useNow({ interval: 1000 })

function isExpired(token: TokenInfo): boolean {
  return token.expires_at !== null && new Date(token.expires_at).getTime() <= now.value.getTime()
}

function validateExpiry(value: string): true | string {
  const time = new Date(value).getTime()
  if (!Number.isFinite(time)) return '请选择有效的到期时间'
  return time > Date.now() || '到期时间必须晚于当前时间'
}

function openCreateDialog() {
  name.value = ''
  permanent.value = true
  expiresAt.value = ''
  allowedIp.value = ''
  dialogVisible.value = true
}

async function handleCreate() {
  if (saving.value) return
  saving.value = true
  try {
    if (!(await createForm.value?.validate())?.valid) return
    const { token, ...info } = await createToken({
      name: name.value.trim(),
      expires_at: permanent.value ? null : new Date(expiresAt.value).toISOString(),
      allowed_ip: allowedIp.value.trim() || null,
    })
    issuedToken.value = token
    tokens.value.unshift(info)
  } finally {
    saving.value = false
  }
}

async function copyToken() {
  try {
    await navigator.clipboard.writeText(issuedToken.value)
    showSuccessToast('token 已复制')
  } catch {
    showErrorToast('无法访问剪贴板, 请手动复制 token')
  }
}

async function handleRevoke(token: TokenInfo) {
  const confirmed = await confirmDialog.value!.open(
    '撤销 token',
    `确定要撤销 "${token.name}" 吗? 使用此 token 的后续请求将无法通过认证.`,
  )
  if (!confirmed) return
  revokingId.value = token.id
  try {
    await deleteToken(token.id)
    tokens.value = tokens.value.filter((item) => item.id !== token.id)
    showSuccessToast('token 已撤销')
  } finally {
    revokingId.value = null
  }
}

watch(dialogVisible, (visible) => {
  if (!visible) issuedToken.value = ''
})

onMounted(async () => {
  loading.value = true
  try {
    tokens.value = await getTokens()
  } finally {
    loading.value = false
  }
})
</script>
