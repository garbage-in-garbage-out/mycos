# Mycos WGSL Kernels

This document enumerates the compute shader entry points used by the Mycos
GPU pipeline and describes the buffers they operate on.

## Buffer Layouts

### `Counts`
| Field | Type | Description |
|-------|------|-------------|
| `input_bits` | `u32` | Number of input bits. |
| `internal_bits` | `u32` | Number of internal bits. |
| `output_bits` | `u32` | Number of output bits. |
| `frontier_cap` | `u32` | Capacity of each frontier list. |
| `proposal_cap` | `u32` | Capacity of the proposals buffer. |
| `hash_window` | `u32` | Cycle detection ring size. |
| `_pad0`, `_pad1` | `u32` | Reserved. |

### `FrontierCounts`
| Field | Type | Description |
|-------|------|-------------|
| `on` | `u32` | Count of rising bits. |
| `off` | `u32` | Count of falling bits. |
| `toggle` | `u32` | Count of toggled bits. |
| `_pad` | `u32` | Reserved. |

### `Effect`
| Field | Type | Description |
|-------|------|-------------|
| `to_bit` | `u32` | Destination bit index. |
| `order_tag` | `u32` | Conflict resolution tag. |
| `action` | `u32` | 0=Enable, 1=Disable, 2=Toggle. |
| `_pad` | `u32` | Reserved. |

### `Winner`
| Field | Type | Description |
|-------|------|-------------|
| `to_bit` | `u32` | Bit to modify. |
| `action` | `u32` | Winning action. |
| `_pad0`, `_pad1` | `u32` | Reserved. |

### `Metrics`
| Field | Type | Description |
|-------|------|-------------|
| `effects_applied` | `u32` | Total effects applied in the tick. |
| `_pad0`, `_pad1`, `_pad2` | `u32` | Reserved. |

### `HashState`
| Field | Type | Description |
|-------|------|-------------|
| `pos` | `u32` | Ring buffer position. |
| `detected` | `u32` | Non-zero if a cycle was detected. |
| `period` | `u32` | Cycle period when detected. |
| `_pad` | `u32` | Reserved. |

## Bindings
All kernels use bind group 0. The table lists binding indices and buffer roles.

| Binding | Name | Type | Access | Description |
|---------|------|------|--------|-------------|
| 0 | `counts` | uniform `Counts` | read | Global bit counts and capacities. |
| 1 | `prev_inputs` | storage `Words` | read_write | Previous input words. |
| 2 | `curr_inputs` | storage `Words` | read_write | Current input words. |
| 3 | `prev_internals` | storage `Words` | read_write | Previous internal words. |
| 4 | `curr_internals` | storage `Words` | read_write | Current internal words. |
| 5 | `prev_outputs` | storage `Words` | read_write | Previous output words. |
| 6 | `curr_outputs` | storage `Words` | read_write | Current output words. |
| 7 | `frontier_on` | storage `Words` | read_write | Rising-bit frontier list. |
| 8 | `frontier_off` | storage `Words` | read_write | Falling-bit frontier list. |
| 9 | `frontier_toggle` | storage `Words` | read_write | Toggled-bit frontier list. |
| 10 | `frontier_counts` | storage `FrontierCounts` | read_write | Frontier lengths. |
| 11 | `csr_offs_on` | storage `Words` | read | CSR offsets for On trigger. |
| 12 | `csr_offs_off` | storage `Words` | read | CSR offsets for Off trigger. |
| 13 | `csr_offs_toggle` | storage `Words` | read | CSR offsets for Toggle trigger. |
| 14 | `csr_effects_on` | storage `array<Effect>` | read | Effects for On trigger. |
| 15 | `csr_effects_off` | storage `array<Effect>` | read | Effects for Off trigger. |
| 16 | `csr_effects_toggle` | storage `array<Effect>` | read | Effects for Toggle trigger. |
| 17 | `proposals` | storage `array<Effect>` | read_write | Proposal buffer. |
| 18 | `proposal_count` | storage `u32` | read_write | Number of proposals emitted. |
| 19 | `winners` | storage `array<Winner>` | read_write | Winning effects. |
| 20 | `winners_count` | storage `u32` | read_write | Count of winners. |
| 21 | `metrics` | storage `Metrics` | read_write | Per-tick metrics. |
| 22 | `hash_ring` | storage `Words` | read_write | Cycle detection hash ring. |
| 23 | `hash_state` | storage `HashState` | read_write | Cycle detection state. |

## Entry Points

### `k1_detect_edges`
Computes transitions between previous and current bit states and populates
initial frontiers (`frontier_on`, `frontier_off`, `frontier_toggle`).

### `k2_expand_count`
First pass of CSR expansion. Traverses frontiers to count the total number
of effects and writes the result to `proposal_count`.

### `k2_expand_emit`
Second CSR pass that emits `Effect` proposals into the `proposals` buffer.

### `k3_resolve`
Sorts proposals by `(to_bit, order_tag)` and writes the last action per bit
into the `winners` buffer.

### `k4_commit`
Applies winning actions to `curr_internals` and `curr_outputs` and updates
`metrics.effects_applied`.

### `k5_next_frontier`
Differs `curr_internals` against `prev_internals` to build the next
iteration's frontiers.

### `kc_cycle_hash`
Computes a rolling hash of internal state, storing it in `hash_ring` and
reporting cycles via `hash_state`.

### `kfinal_finalize`
Copies current words into previous sections to complete the tick.

