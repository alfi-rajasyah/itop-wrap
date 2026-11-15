/// <reference types="vitest/config" />
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: 'safari17',
    // Every asset is a file, never a data: URL, so the CSP's font-src stays 'self' (tauri.conf.json).
    assetsInlineLimit: 0,
    // kit.html (the component gallery) isn't shipped: the dev server still serves it at /kit.html.
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        menubar: fileURLToPath(new URL('./menubar.html', import.meta.url)),
      },
    },
  },
  test: { environment: 'jsdom', globals: true, setupFiles: ['./src/test-setup.ts'] },
})
