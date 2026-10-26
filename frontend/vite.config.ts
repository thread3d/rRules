/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  // Phase 28 (Plan 28-02): Vitest component-test configuration.
  // jsdom env so @testing-library/react can render; globals:true so
  // `describe`/`it`/`expect` work without per-file imports; setupFiles
  // registers @testing-library/jest-dom matchers globally.
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
  },
  // tanstackRouter MUST come before react() so routeTree.gen.ts is generated
  // before the React plugin processes imports (RESEARCH § Pattern 1).
  // Tailwind 4 ships a Vite plugin that replaces the PostCSS setup (and prefixes with
  // Lightning CSS), so `postcss.config.js` and `autoprefixer` are gone.
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
  ],

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
        // One vendor chunk. An earlier hand-split (react / tanstack / zustand /
        // vendor) produced a production bundle where `vendor` read `useState`
        // off an undefined React — the app rendered a blank page in a browser
        // while every jsdom test passed, because the tests run source modules and
        // never load the built chunks. Bundle splitting is a caching
        // optimisation; a build that does not boot is not worth 200 KB.
        //
        // Vite 8 bundles with Rolldown and still honours this hook here (the build
        // emits one `vendor` chunk holding React), but the guarantee is the browser
        // gate: `make e2e` loads the real bundle and fails if any route renders
        // nothing, which is what caught the original blank page.
        manualChunks(id) {
          if (id.includes('node_modules')) {
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
