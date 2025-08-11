# Mycos

**Mycos** is an evolving mesh intelligence engine built from interconnected **network chunks**.  
Each chunk contains bits of state and a set of directed connections with trigger/action semantics.  
Chunks can be linked into larger systems, with deterministic processing on the GPU via WebGPU.

The name comes from *mykos* — Greek for “fungus” — evoking the mycelial network: a living, adaptive mesh.

---

## Documentation

- [API reference](docs/api.md)
- [WGSL kernels](docs/kernels.md)
- [Technical specification](docs/spec.md)

---

## Features

- **Formal binary chunk format** with Inputs, Outputs, Internals, and a fixed-size connection table.
- **Deterministic GPU execution** (WGSL kernels, WebGPU) using a global wavefront + last-writer-wins resolve.
- **Flexible wiring**:
  - **Flat** Output→Input links between chunks (default)
  - **Optional gated nesting** for modular subgraphs
- **Loop-safe** — allows useful cycles but detects and quenches infinite oscillations.
- **Evolvable** — supports genetic/evolutionary methods to grow new connections, loops, and modules.

---

## Architecture

### 1. Network Chunk
A **chunk** is a complete, isolated unit:
- **Data Block**: contiguous bits split into:
  - **Inputs** — external data entry points
  - **Outputs** — action/data results
  - **Internals** — arbitrary state
- **Connections**:
  - Allowed: Input→Internal, Internal→Internal, Internal→Output
  - **Trigger**: On / Off / Toggle
  - **Action**: Enable / Disable / Toggle
  - **OrderTag**: resolves conflicts deterministically

### 2. Binary Layout (v1)
- Little-endian, LSB-first bits
- Header: magic, version, bit counts, connection count
- Bit sections: packed Inputs, Outputs, Internals
- Connection table: fixed 16-byte records
- Optional TLV trailer for metadata

### 3. GPU Wavefront Pipeline
Per tick:
1. **Inject inputs** from host
2. **Detect edges** (On/Off/Toggle frontiers)
3. **Expand** via CSR adjacency (two-pass)
4. **Resolve** by `(to_bit, order_tag)` — last-writer wins
5. **Commit** actions to current state
6. **Build next frontier** from changed Internals
7. Repeat until frontier empty or guard triggered
8. **Finalize tick** — commit Curr → Prev

### 4. Oscillation Handling
Guards:
- `max_rounds` (default: 1024)
- `max_effects` (default: 5,000,000)
- Cycle detection via 128-bit hash (window R=8)

Policies:
- **freeze_last_stable**
- **clamp_commutative**
- **parity_quench**

### 5. Chunk Wiring
- **Flat Links**: Output(A) → Input(B)
- **Gated Nesting** (optional):
  - Gate bit in parent activates child
  - I/O modes: Alias or Copy-in/out

---

## Implementation Plan

- **Language**: Rust (engine) → WASM for browser; TypeScript for UI shell.
- **GPU API**: [wgpu](https://github.com/gfx-rs/wgpu) (Rust) → WGSL compute shaders.
- **Binary parser**: zero-copy structs with validation.
- **Precompute**: CSR adjacency, effect packing, SCC condensation.
- **WGSL Kernels**:
  - `K1_detect_edges`
  - `K2_expand_count` / `K2_expand_emit`
  - `K3_resolve`
  - `K4_commit`
  - `K5_next_frontier`
  - `Kc_cycle_hash`
  - `Kfinal_finalize`

---

## WASM API

```ts
init(canvas?: HTMLCanvasElement): Promise<MycosHandle>
loadChunks(chunks: ArrayBuffer[]): void
loadLinks(links: ArrayBuffer): void
setInputs(chunkId: number, words: Uint32Array): void
tick(maxRounds?: number): Metrics
getOutputs(chunkId: number, out: Uint32Array): void
setPolicy(mode: "freeze" | "clamp" | "parity"): void
```

`Metrics`:

```ts
{
  rounds: number
  effectsApplied: number
  proposals: number
  winners: number
  oscillator: boolean
  period: number
  policy: string
}
```

---

## Development

### Prerequisites

* Rust nightly (for `wasm32-unknown-unknown` target)
* Node.js + npm
* WebGPU-enabled browser (latest Chrome, Edge, Safari TP)

### Build

```bash
# Build engine to WASM
cd engine
cargo build --target wasm32-unknown-unknown --release

# Generate bindings
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/engine.wasm
cp -r pkg ../web/engine/pkg

# Start web UI
cd ../web
npm install
npm run dev
```

---

## Defaults

* `max_rounds`: 1024
* `max_effects`: 5,000,000
* `cycle_hash_size`: 8
* Word size: 32 bits, LSB-first
* Resolve: `(to_bit, order_tag)` last-writer-wins
