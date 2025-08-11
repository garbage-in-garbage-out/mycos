import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { cpSync, existsSync } from 'node:fs';

const rootDir = dirname(fileURLToPath(import.meta.url));
const enginePkgSrc = resolve(rootDir, '../engine/pkg');
const enginePkgDest = resolve(rootDir, 'engine/pkg');

function copyEnginePkg(): void {
  if (existsSync(enginePkgSrc)) {
    cpSync(enginePkgSrc, enginePkgDest, { recursive: true });
  }
}

export default defineConfig({
  build: {
    target: 'esnext',
  },
  server: {
    fs: {
      allow: [resolve(rootDir, '..')],
    },
  },
  plugins: [
    {
      name: 'copy-engine-pkg',
      buildStart: copyEnginePkg,
      configureServer() {
        copyEnginePkg();
      },
    },
  ],
});
