import { createRouter, createWebHashHistory } from 'vue-router'
import LoginView from '@/views/LoginView.vue'
import HttpLogView from '@/views/HttpLogView.vue'
import RouteView from '@/views/RouteView.vue'
import FileView from '@/views/FileView.vue'
import SystemLogView from '@/views/SystemLogView.vue'
import UserView from '@/views/UserView.vue'

const router = createRouter({
  history: createWebHashHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: LoginView,
    },
    {
      path: '/',
      name: 'httpLog',
      component: HttpLogView,
    },
    {
      path: '/routes',
      name: 'routes',
      component: RouteView,
    },
    {
      path: '/files',
      name: 'files',
      component: FileView,
    },
    {
      path: '/system-log',
      name: 'systemLog',
      component: SystemLogView,
    },
    {
      path: '/users',
      name: 'users',
      component: UserView,
    },
  ],
})

export default router
