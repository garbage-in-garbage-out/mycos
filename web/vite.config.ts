import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { cpSync, existsSync, mkdirSync, statSync } from 'node:fs';

const rootDir = dirname(fileURLToPath(import.meta.url));
const enginePkgSrc = resolve(rootDir, '../engine/pkg');
const enginePkgDest = resolve(rootDir, 'engine/pkg');
const fixturesSrc = resolve(rootDir, '../fixtures');
const fixturesDest = resolve(rootDir, 'public/fixtures');

function copyAssets(): void {
  if (existsSync(enginePkgSrc)) {
    cpSync(enginePkgSrc, enginePkgDest, { recursive: true });
  }
  if (existsSync(fixturesSrc)) {
    mkdirSync(fixturesDest, { recursive: true });
    cpSync(fixturesSrc, fixturesDest, {
      recursive: true,
      filter: (src) => statSync(src).isDirectory() || src.endsWith('.myc'),
    });
  }
}

export default defineConfig({
  build: {
    target: 'esnext',
  },
  plugins: [
    {
      name: 'copy-assets',
      buildStart: copyAssets,
      configureServer() {
        copyAssets();
      },
    },
  ],
});
