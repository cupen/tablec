import { defineConfig } from 'vite';

// Frontend build for the tablec webui. `pnpm build` emits `dist/`, which the
// Rust crate embeds at compile time (see src/handlers.rs). `pnpm dev` serves
// the app on :5173 and proxies /api to a locally running `tablec webui`
// server (default :8080) so frontend iteration needs no Rust rebuild.
export default defineConfig({
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    // Ship one self-contained bundle; the embedded assets are small enough
    // that splitting into many hashed chunks only adds include_dir lookups.
    rollupOptions: { output: { manualChunks: undefined } },
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
});
