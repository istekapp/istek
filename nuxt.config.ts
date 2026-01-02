export default defineNuxtConfig({
  compatibilityDate: '2024-12-01',
  devtools: { enabled: false },
  
  ssr: false,
  
  // Disable experimental features that cause EBADF on newer Node versions
  experimental: {
    componentIslands: false,
  },
  
  dev: {
    // Workaround for spawn EBADF error
  },
  
  modules: [
    '@nuxtjs/tailwindcss',
    '@nuxt/icon',
  ],

  app: {
    head: {
      title: 'istek - API Client',
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      ],
      link: [
        { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
        { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' },
        { rel: 'stylesheet', href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap' }
      ]
    }
  },

  tailwindcss: {
    cssPath: '~/assets/css/main.css',
  },

  vite: {
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      strictPort: true,
    },
    build: {
      chunkSizeWarningLimit: 1000,
      rollupOptions: {
        output: {
          manualChunks: {
            'codemirror': [
              'codemirror',
              '@codemirror/lang-json',
              '@codemirror/lang-xml',
              '@codemirror/lang-html',
              '@codemirror/autocomplete',
              '@codemirror/lint',
              '@codemirror/state',
              '@codemirror/view',
            ],
          },
        },
      },
    },
  },
})
