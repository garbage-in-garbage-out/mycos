# Mycos API Reference

This document describes the public interface exposed by the Mycos engine.
It covers both the WebAssembly bindings and the Rust crate exports.

## WebAssembly Interface

### `init(canvas?: HtmlCanvasElement): Promise<MycosHandle>`
Initialise WebGPU and create an engine handle.

| Parameter | Type | Description |
|-----------|------|-------------|
| `canvas`  | `HtmlCanvasElement?` | Optional canvas element; currently unused. |

Returns a `MycosHandle` with internal WebGPU `Device` and `Queue`.

### `MycosHandle` Methods

#### `loadChunks(chunks: Array)`
Load chunk binaries into the engine.

| Parameter | Type | Description |
|-----------|------|-------------|
| `chunks`  | `Array` of `ArrayBuffer` | Sequence of chunk binaries to parse and upload. |

#### `loadLinks(links: ArrayBuffer)`
Load a link graph describing inter‑chunk connections.

| Parameter | Type | Description |
|-----------|------|-------------|
| `links`   | `ArrayBuffer` | Link graph binary. |

#### `setInputs(chunkId: number, words: Uint32Array)`
Set input words for a specific chunk.

| Parameter | Type | Description |
|-----------|------|-------------|
| `chunkId` | `number` | Identifier of the target chunk. |
| `words`   | `Uint32Array` | View into WebAssembly memory containing input words. |

#### `tick(maxRounds?: number): Metrics`
Execute the engine for up to `maxRounds` wavefront rounds.

| Parameter | Type | Description |
|-----------|------|-------------|
| `maxRounds` | `number?` | Optional upper bound on rounds. |

Returns a `Metrics` struct.

#### `getOutputs(chunkId: number, out: Uint32Array)`
Read output words for a chunk.

| Parameter | Type | Description |
|-----------|------|-------------|
| `chunkId` | `number` | Identifier of the chunk. |
| `out`     | `Uint32Array` | Destination array for output words. |

#### `setPolicy(mode: string)`
Select the oscillation handling policy.

| Parameter | Type | Description |
|-----------|------|-------------|
| `mode`    | `string` | One of `"freeze"`, `"clamp"`, or `"parity"`. |

### `Metrics`
Execution metrics returned from `tick`.

| Field    | Type | Description |
|----------|------|-------------|
| `rounds` | `u32` | Wavefront rounds executed in the last tick. |
| `effects` | `u32` | Effects applied in the last tick. |

## Rust Crate Exports

The `engine` crate re-exports several utilities for binary parsing,
CSR construction, linking, and policy handling.

| Item | Description |
|------|-------------|
| `parse_chunk` / `validate_chunk` | Parse and validate chunk binaries. |
| `build_csr` | Build CSR adjacency from a chunk. |
| `execute_gated_alias` / `execute_gated_copy` | Execute embedded chunks with alias or copy I/O modes. |
| `parse_embeds` | Parse embedded chunk descriptors. |
| `bit_to_word`, `set_bit`, `clr_bit`, `xor_bit` | Bit-level helpers for packed sections. |
| `connection_table_offset`, `section_offsets`, `HEADER_BYTES` | Layout constants and utilities. |
| `build_link_csr`, `compute_base_offsets` | Construct link CSR and base offsets. |
| `parse_links` / `validate_links` | Parse and validate link graphs. |
| `clamp_commutative`, `freeze_last_stable`, `parity_quench` | Oscillation policies. |
| `CycleDetector`, `ExecutionResult`, `Policy` | Types supporting policy application. |
| `build_internal_graph`, `scc_ids_and_topo_levels` | Strongly connected component utilities. |
| `init_device` | Initialise a WebGPU device (WASM only). |

