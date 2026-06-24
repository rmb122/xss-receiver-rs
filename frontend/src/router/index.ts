import { createRouter, createWebHashHistory } from 'vue-router'
import LoginView from '@/views/LoginView.vue'
import HttpLogView from '@/views/HttpLogView.vue'
import HttpRouteView from '@/views/HttpRouteView.vue'
import DnsLogView from '@/views/DnsLogView.vue'
import DnsRouteView from '@/views/DnsRouteView.vue'
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
      path: '/http-routes',
      name: 'httpRoutes',
      component: HttpRouteView,
    },
    {
      path: '/dns-log',
      name: 'dnsLog',
      component: DnsLogView,
    },
    {
      path: '/dns-routes',
      name: 'dnsRoutes',
      component: DnsRouteView,
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
