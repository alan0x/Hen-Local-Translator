import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { resolve } from 'node:path';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    strictPort: true,
    host: '127.0.0.1',
    port: 1420
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome105', 'safari13'],
    minify: !process.env.TAURI_DEBUG,
    sourcemap: Boolean(process.env.TAURI_DEBUG),
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        overlay: resolve(__dirname, 'overlay.html'),
        voiceLab: resolve(__dirname, 'voice-lab.html')
      }
    }
  }
});
