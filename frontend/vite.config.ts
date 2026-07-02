import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      '@': '/src',
    },
  },

  server: {
    port: 5173,
    proxy: {
      // Dev: forward relative '/v1' requests to the backend (D-22-04). The
      // backend serves the /v1 scope directly (no /api prefix), so no rewrite
      // is needed. The browser sees same-origin requests to /v1/*.
      '/v1': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },

  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (id.includes('react') || id.includes('react-dom')) {
              return 'react-vendor'
            }
            if (id.includes('@tanstack')) {
              return 'tanstack-vendor'
            }
            if (id.includes('zustand')) {
              return 'zustand-vendor'
            }
            return 'vendor'
          }
        },
      },
    },
    chunkSizeWarningLimit: 600,
  },

  optimizeDeps: {
    include: [
      '@tanstack/react-query',
      '@tanstack/react-router',
      'zustand',
      'lucide-react',
      'react',
      'react-dom'
    ],
  },
})
