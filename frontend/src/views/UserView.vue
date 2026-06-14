<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-account-group</v-icon>
        用户管理
        <v-spacer />
        <v-btn
          v-if="userStore.isAdmin"
          color="primary"
          prepend-icon="mdi-plus"
          @click="openCreateDialog"
        >
          新建用户
        </v-btn>
      </v-card-title>
      <v-progress-linear v-if="loading" indeterminate color="primary" />
      <v-data-table
        :headers="headers"
        :items="users"
        :loading="loading"
        density="comfortable"
        no-data-text="暂无用户数据"
        items-per-page="-1"
      >
        <template #item.create_time="{ item }">
          {{ formatTime(item.create_time) }}
        </template>

        <template #item.actions="{ item }">
          <v-btn
            v-if="userStore.isAdmin || item.id === userStore.user?.id"
            icon
            size="small"
            variant="text"
            color="primary"
            @click="openEditDialog(item)"
          >
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn
            v-if="userStore.isAdmin && item.id !== 1"
            icon
            size="small"
            variant="text"
            color="error"
            @click="handleDelete(item)"
          >
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>

        <template #bottom />
      </v-data-table>
    </v-card>

    <!-- 创建/编辑对话框 -->
    <v-dialog v-model="dialogVisible" max-width="500">
      <v-card>
        <v-card-title>{{ isEditing ? '编辑用户' : '新建用户' }}</v-card-title>
        <v-card-text>
          <v-text-field
            v-model="form.username"
            label="用户名"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-text-field
            v-model="form.password"
            :label="isEditing ? '新密码（留空则不修改）' : '密码'"
            type="password"
            variant="outlined"
            density="compact"
          />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="dialogVisible = false">取消</v-btn>
          <v-btn color="primary" variant="flat" :loading="saving" @click="handleSave">
            {{ isEditing ? '保存' : '创建' }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <ConfirmDialog ref="confirmDialog" />
  </v-container>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getAllUsers, createUser, updateUser, deleteUser } from '@/api/user'
import { useUserStore } from '@/stores/user'
import { showSuccessToast } from '@/utils/toast'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import type { UserResponse } from '@/types/user'
import type { DataTableHeader } from 'vuetify'
import { formatTime } from '@/utils/format'

const userStore = useUserStore()

const headers: DataTableHeader[] = [
  { title: 'ID', key: 'id', width: '80px', align: 'center' },
  { title: '用户名', key: 'username' },
  { title: '创建时间', key: 'create_time', width: '200px', align: 'center' },
  { title: '操作', key: 'actions', width: '120px', align: 'center', sortable: false },
]

const users = ref<UserResponse[]>([])
const loading = ref(false)
const saving = ref(false)
const dialogVisible = ref(false)
const isEditing = ref(false)
const editingId = ref<number | null>(null)
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()

const form = ref({
  username: '',
  password: '',
})

function resetForm() {
  form.value = { username: '', password: '' }
}

async function fetchUsers() {
  loading.value = true
  try {
    if (userStore.isAdmin) {
      users.value = await getAllUsers()
    } else if (userStore.user) {
      users.value = [
        {
          id: userStore.user.id,
          username: userStore.user.username,
          create_time: '',
        },
      ]
    }
  } finally {
    loading.value = false
  }
}

function openCreateDialog() {
  isEditing.value = false
  editingId.value = null
  resetForm()
  dialogVisible.value = true
}

function openEditDialog(user: UserResponse) {
  isEditing.value = true
  editingId.value = user.id
  form.value = {
    username: user.username,
    password: '',
  }
  dialogVisible.value = true
}

async function handleSave() {
  saving.value = true
  try {
    if (isEditing.value && editingId.value !== null) {
      const payload: { user_id: number; username?: string; password?: string } = {
        user_id: editingId.value,
      }
      if (form.value.username) payload.username = form.value.username
      if (form.value.password) payload.password = form.value.password
      await updateUser(payload)
      showSuccessToast('用户更新成功')
      // 如果修改了自己的信息，刷新 store
      if (editingId.value === userStore.user?.id) {
        await userStore.loadUser()
      }
    } else {
      await createUser({ username: form.value.username, password: form.value.password })
      showSuccessToast('用户创建成功')
    }
    dialogVisible.value = false
    fetchUsers()
  } finally {
    saving.value = false
  }
}

async function handleDelete(user: UserResponse) {
  const confirmed = await confirmDialog.value!.open(
    '确认删除',
    `确定要删除用户 "${user.username}" 吗？`,
  )
  if (!confirmed) return

  await deleteUser({ user_id: user.id })
  showSuccessToast('用户删除成功')
  fetchUsers()
}

onMounted(() => {
  fetchUsers()
})
</script>
