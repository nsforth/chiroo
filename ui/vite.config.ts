// vite.static.config.ts
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueRouter from 'vue-router/vite'
import vueLayouts from 'vite-plugin-vue-layouts'
import ui from '@nuxt/ui/vite'

export default defineConfig({
  plugins: [
    vueRouter({ dts: 'src/route-map.d.ts' }),
    vueLayouts(),
    vue(),
    ui({ prose: true, ui: { colors: { primary: 'blue', neutral: 'zinc' } } })
  ],
  build: {
    outDir: 'dist/static'
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        secure: false,
      }
    }
  }
})