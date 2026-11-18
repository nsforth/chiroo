import './assets/css/main.css'

import { createApp } from 'vue'
import type { RouteRecordRaw } from 'vue-router'
import { createRouter, createWebHistory } from 'vue-router'
import { routes, handleHotUpdate } from 'vue-router/auto-routes'
import { setupLayouts } from 'virtual:generated-layouts'
import ui from '@nuxt/ui/vue-plugin'

import { addCollection } from '@iconify/vue'
import { icons as lucideIcons } from '@iconify-json/lucide'

import App from './App.vue'
import { i18n } from './plugins/i18n'
import { head, runBoot } from './boot'

// Bundle the lucide icon set locally so icons render without internet access
addCollection(lucideIcons)

function mountMainApp() {
  const router = createRouter({
    routes: setupLayouts(routes as RouteRecordRaw[]),
    history: createWebHistory()
  })

  const app = createApp(App)

  app.use(head)
  app.use(router)
  app.use(ui)
  app.use(i18n)

  app.mount('#app')

  // This will update routes at runtime without reloading the page
  if (import.meta.hot) {
    handleHotUpdate(router)
  }
}

async function boot() {
  await runBoot()
  mountMainApp()
}

void boot()
