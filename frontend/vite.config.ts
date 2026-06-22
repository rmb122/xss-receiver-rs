import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'
import vuetify from 'vite-plugin-vuetify'
import { nodePolyfills } from 'vite-plugin-node-polyfills'

export default defineConfig({
  base: './',
  plugins: [
    vue(),
    vuetify({ autoImport: true }),
    vueDevTools(),
    // iconv-lite -> safer-buffer needs Node's Buffer in the browser
    nodePolyfills({ include: ['buffer'], globals: { Buffer: true } }),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          monaco: ['monaco-editor'],
        }
      }
    }
  },
  optimizeDeps: {
    include: ['monaco-editor'],
  },
  server: {
    proxy: {
      '/super_admin': {
        target: 'http://127.0.0.1:8000',
        changeOrigin: true
      }
    }
  }
})
