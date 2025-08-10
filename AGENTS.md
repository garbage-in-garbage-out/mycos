# AGENTS.md — Mycos Project Guidelines

This document contains **all information AI agents need** to work effectively on Mycos.  
It is **binding**: follow these specifications, conventions, and best practices exactly.

---

## 1. Project Overview

**Mycos** is an evolving mesh intelligence engine built from **network chunks**.  
A chunk contains:
- A **bitset state**: Inputs, Outputs, Internals.
- A set of **connections** between bits with **trigger** (On/Off/Toggle) and **action** (Enable/Disable/Toggle) semantics.
- Only three connection types are valid: Input→Internal, Internal→Internal, Internal→Output.

Multiple chunks link together into a **supergraph**:
- Default wiring: Output(A) → Input(B).
- Optional: Gated nesting (parent chunk activates a child).

Execution is **deterministic** and **GPU-first**, using a **global synchronous wavefront**:
- WebGPU compute shaders (WGSL) handle per-tick micro-steps.
- Last-writer-wins ordering based on `order_tag`.

Oscillations are allowed for evolved logic but must be detected and quenched if infinite.

---

## 2. Implementation Stack

- **Engine Language:** Rust → compiled to WASM for web use.
- **GPU API:** [`wgpu`](https://github.com/gfx-rs/wgpu) (Rust) targeting WebGPU.
- **Shaders:** WGSL (K1–K6 stages).
- **UI Shell:** TypeScript, using WebGPU bindings to WASM exports.

Agents should:
- Implement binary parsing, CSR building, and SCC condensation in Rust.
- Keep large arrays **device-resident** to avoid costly CPU–GPU transfers.
- Use **zero-copy** and **bit-packed** structures for binary parsing.

---

## 3. Binary Specification (v1)

**Format:**
- Little-endian, LSB-first bits.
- `u32` word alignment for GPU use.

**Header (offsets in bytes):**
```
0x00  8  Magic = "MYCOSCH0"
0x08  2  Version = 0x0001
0x0A  2  Flags
0x0C  4  InputBits     (Ni)
0x10  4  OutputBits    (No)
0x14  4  InternalBits  (Nn)
0x18  4  ConnectionCount (Nc)
0x1C  4  Reserved
```

**Bit Sections:**
- Inputs: ceil(Ni/8) bytes
- Outputs: ceil(No/8) bytes
- Internals: ceil(Nn/8) bytes

**Connection Table (Nc × 16 bytes):**
```
u8  from\_section  (0=Input, 1=Internal)
u8  to\_section    (1=Internal, 2=Output)
u8  trigger       (0=On, 1=Off, 2=Toggle)
u8  action        (0=Enable, 1=Disable, 2=Toggle)
u32 from\_index
u32 to\_index
u32 order\_tag
```

**Trailer:** Optional TLV, 4-byte aligned.

---

## 4. GPU Processing Pipeline

Each tick:

1. **Inject inputs** from host → Curr.
2. **K1 Detect edges**:  
   - `flips = Curr ^ Prev`  
   - `rises = flips & Curr`  
   - `falls = flips & Prev`  
   Build `On`, `Off`, `Toggle` frontiers.
3. **K2 Expand (two-pass)**:  
   - CSR-by-trigger expansion into proposals `(to_bit_global, order_tag, action)`.
4. **K3 Resolve**:  
   - Radix sort by `(to_bit, order_tag)`; segmented take-last to ensure last-writer-wins.
5. **K4 Commit**: Apply actions to Curr (OR/ANDN/XOR).
6. **K5 Build next frontier**: Diff Curr vs Prev (internals only).
7. Repeat until frontier empty or guard triggers.
8. **K6 Finalize**: Prev = Curr for all sections.

---

## 5. Performance Best Practices

Agents must:
- **Precompute CSR** adjacency per trigger type.
- **Sort effects** within each CSR range by `to_word` for coalesced writes.
- **Preallocate buffers** for proposals/winners sized from historical peaks.
- Use **two-pass expansion** to avoid atomics in proposal writes.
- Keep **GPU buffers persistent**; never reallocate per tick.
- Batch **multiple chunks** per tick to amortize sort cost.

---

## 6. Oscillation Handling

Guards:
- `max_rounds = 1024`
- `max_effects = 5,000,000`
- Cycle detection via ring buffer of 128-bit hashes of internal state (default window R=8).

Policies:
- `freeze_last_stable`: revert to last stable state.
- `clamp_commutative`: resolve with commutative precedence (Disable > Enable > Toggle parity).
- `parity_quench`: toggle bits once based on cycle parity.

Agents must:
- Detect cycles **on GPU** to avoid CPU sync costs.
- Apply policy immediately upon guard trigger.

---

## 7. Chunk Wiring

### Flat Links (default)
- Only Output→Input allowed.
- Same trigger/action/order semantics as intra-chunk connections.
- Global `order_tag` space across all connections and links.

### Gated Nesting (optional)
- Gate bit in parent internal space enables child.
- I/O modes:
  - **Alias**: mapped bits are physically the same.
  - **Copy-in/out**: copy inputs/outputs at gate edges.

Agents implementing gates must:
- Check `gate_state` before expanding gated connections.
- Minimize branch divergence by grouping gated sources.

---

## 8. Development Rules for Agents

- **Code Style:** Follow Rust 2021 idioms; TS code must be typed and lint-clean.
- **Determinism:** All GPU kernels must yield identical results across runs for same input.
- **Testing:** Maintain CPU reference executor (in Rust) for small cases to verify GPU output.
- **Versioning:** If binary format changes, bump `Version` in header and keep reader backward-compat.
- **Error Handling:** Fail-fast on invalid binaries or illegal connections.

---

## 9. Deliverables for Agents

When producing code or artifacts, agents must:
1. Preserve **full binary spec compliance**.
2. Maintain deterministic results and last-writer-wins semantics.
3. Provide **unit tests** for:
   - Parsing & validation
   - CSR build
   - GPU kernels (on small fixtures)
   - Oscillation detection
4. Update **README.md** and **docs/spec.md** if any interface or format changes.
5. Avoid premature optimization before correctness is proven.

---

## 10. Reference Defaults

- Word size: 32 bits
- Bit order: LSB-first
- Max rounds: 1024
- Max effects: 5e6
- Cycle hash window: 8
- Resolve key: `(to_bit, order_tag)` with last-writer-wins
