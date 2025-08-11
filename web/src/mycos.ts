// TypeScript wrapper around the WASM engine bindings.
//
// This module dynamically loads the generated wasm-bindgen package from the
// Rust `engine` crate and exposes a stable, ergonomic API for the browser
// shell. The wrapper simply forwards calls to the underlying `MycosHandle`
// methods while keeping the handle private to this module.

export interface Metrics {
  rounds: number;
  effects: number;
}

export interface MycosHandle {
  load_chunks(chunks: ArrayBuffer[]): void;
  load_links(links: ArrayBuffer): void;
  set_inputs(chunkId: number, words: Uint32Array): void;
  tick(maxRounds?: number): Metrics;
  get_outputs(chunkId: number, out: Uint32Array): void;
  set_policy(mode: string): void;
}

interface WasmModule {
  default: () => Promise<void>;
  init(canvas: HTMLCanvasElement | null): Promise<MycosHandle>;
}

let wasm: WasmModule | undefined;
let handle: MycosHandle | null = null;

async function ensureWasm(): Promise<WasmModule> {
  if (!wasm) {
    wasm = (await import('../../engine/pkg')) as unknown as WasmModule;
    await wasm.default();
  }
  return wasm;
}

/**
 * Initialize the engine and return a handle.
 */
export async function init(canvas?: HTMLCanvasElement): Promise<MycosHandle> {
  const mod = await ensureWasm();
  handle = await mod.init(canvas ?? null);
  return handle;
}

function ensureHandle(): MycosHandle {
  if (!handle) {
    throw new Error('Mycos engine not initialized');
  }
  return handle;
}

export function loadChunks(chunks: ArrayBuffer[]): void {
  ensureHandle().load_chunks(chunks);
}

export function loadLinks(links: ArrayBuffer): void {
  ensureHandle().load_links(links);
}

export function setInputs(chunkId: number, words: Uint32Array): void {
  ensureHandle().set_inputs(chunkId, words);
}

export function tick(maxRounds?: number): Metrics {
  return ensureHandle().tick(maxRounds);
}

export function getOutputs(chunkId: number, out: Uint32Array): void {
  ensureHandle().get_outputs(chunkId, out);
}

export function setPolicy(mode: 'freeze' | 'clamp' | 'parity'): void {
  ensureHandle().set_policy(mode);
}

