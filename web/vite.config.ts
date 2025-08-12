import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { cpSync, existsSync, mkdirSync, statSync } from 'node:fs';
import { execSync } from 'node:child_process';

const rootDir = dirname(fileURLToPath(import.meta.url));
const engineSrcDir = resolve(rootDir, '../engine');
const enginePkgSrc = resolve(engineSrcDir, 'pkg');
const enginePkgDest = resolve(rootDir, 'engine/pkg');
const fixturesSrc = resolve(rootDir, '../fixtures');
const fixturesDest = resolve(rootDir, 'public/fixtures');

function buildEngine(): void {
  if (!existsSync(enginePkgSrc)) {
    const cargoHome =
      process.env.CARGO_HOME || resolve(process.env.HOME || '', '.cargo');
    const wasmPackPath = resolve(
      cargoHome,
      'bin',
      process.platform === 'win32' ? 'wasm-pack.exe' : 'wasm-pack'
    );
    const wasmPackCmd = existsSync(wasmPackPath) ? wasmPackPath : 'wasm-pack';
    execSync(`${wasmPackCmd} build --target web --dev`, {
      cwd: engineSrcDir,
      stdio: 'inherit',
    });
  }
}

function copyAssets(): void {
  buildEngine();
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
