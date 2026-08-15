import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 9999,
    // 开发时将 /api 请求代理到后端，避免跨域问题
    proxy: {
      '/api': {
        target: 'http://localhost:8765',
        changeOrigin: true,
      }
    }
  }
})
