import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    outDir: 'src-tauri/overlay',
    emptyOutDir: true,
    target: 'es2021',
    minify: 'esbuild',
    reportCompressedSize: false,
    cssCodeSplit: false,
    lib: {
      entry: 'src/overlay-entry.ts',
      name: 'SemiosOverlay',
      formats: ['iife'],
      fileName: () => 'inject.js',
    },
  },
});
