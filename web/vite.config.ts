import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const rootDir = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  build: {
    target: 'esnext',
  },
  server: {
    fs: {
      allow: [resolve(rootDir, '..')],
    },
  },
});
