// WGSL compute kernels for Mycos execution pipeline.
// All kernels operate on u32 word arrays with LSB-first bit order.
// Each entry point is deterministic.

const WORD_BITS : u32 = 32u;

struct Counts {
    input_bits: u32;
    internal_bits: u32;
    output_bits: u32;
    frontier_cap: u32;
    proposal_cap: u32;
    hash_window: u32;
    _pad0: u32;
    _pad1: u32;
};
@group(0) @binding(0) var<uniform> counts: Counts;

struct Words {
    data: array<u32>;
};

@group(0) @binding(1) var<storage, read_write> prev_inputs: Words;
@group(0) @binding(2) var<storage, read_write> curr_inputs: Words;
@group(0) @binding(3) var<storage, read_write> prev_internals: Words;
@group(0) @binding(4) var<storage, read_write> curr_internals: Words;
@group(0) @binding(5) var<storage, read_write> prev_outputs: Words;
@group(0) @binding(6) var<storage, read_write> curr_outputs: Words;

// Frontier lists and counts
@group(0) @binding(7) var<storage, read_write> frontier_on: Words;
@group(0) @binding(8) var<storage, read_write> frontier_off: Words;
@group(0) @binding(9) var<storage, read_write> frontier_toggle: Words;

struct FrontierCounts {
    on: u32;
    off: u32;
    toggle: u32;
    _pad: u32;
};
@group(0) @binding(10) var<storage, read_write> frontier_counts: FrontierCounts;

// CSR adjacency
struct Effect {
    to_bit: u32;
    order_tag: u32;
    action: u32; // 0=Enable,1=Disable,2=Toggle
    _pad: u32;
};

@group(0) @binding(11) var<storage, read> csr_offs_on: Words;
@group(0) @binding(12) var<storage, read> csr_offs_off: Words;
@group(0) @binding(13) var<storage, read> csr_offs_toggle: Words;
@group(0) @binding(14) var<storage, read> csr_effects_on: array<Effect>;
@group(0) @binding(15) var<storage, read> csr_effects_off: array<Effect>;
@group(0) @binding(16) var<storage, read> csr_effects_toggle: array<Effect>;

// Proposals buffer
@group(0) @binding(17) var<storage, read_write> proposals: array<Effect>;
struct U32Buf { value: u32; };
@group(0) @binding(18) var<storage, read_write> proposal_count: U32Buf;

// Winners buffer
struct Winner {
    to_bit: u32;
    action: u32;
    _pad0: u32;
    _pad1: u32;
};
@group(0) @binding(19) var<storage, read_write> winners: array<Winner>;
@group(0) @binding(20) var<storage, read_write> winners_count: U32Buf;

// Metrics
struct Metrics {
    effects_applied: u32;
    _pad0: u32;
    _pad1: u32;
    _pad2: u32;
};
@group(0) @binding(21) var<storage, read_write> metrics: Metrics;

// Cycle hash ring buffer
@group(0) @binding(22) var<storage, read_write> hash_ring: Words; // length = hash_window * 4
struct HashState {
    pos: u32;
    detected: u32;
    period: u32;
    _pad: u32;
};
@group(0) @binding(23) var<storage, read_write> hash_state: HashState;

fn word_index(bit: u32) -> u32 {
    return bit / WORD_BITS;
}

fn bit_mask(bit: u32) -> u32 {
    return 1u << (bit % WORD_BITS);
}

fn rotl32(x: u32, r: u32) -> u32 {
    return (x << r) | (x >> (32u - r));
}

fn murmur_mix(h: u32, k: u32) -> u32 {
    var kk = k * 0xcc9e2d51u;
    kk = rotl32(kk, 15u);
    kk = kk * 0x1b873593u;
    var hh = h ^ kk;
    hh = rotl32(hh, 13u);
    return hh * 5u + 0xe6546b64u;
}

fn murmur_fmix(mut h: u32) -> u32 {
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);
    return h;
}

// ---------------------------------------------------------------
// K1_detect_edges: Compute bit transitions and build initial frontiers.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k1_detect_edges(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    frontier_counts.on = 0u;
    frontier_counts.off = 0u;
    frontier_counts.toggle = 0u;

    var global_bit: u32 = 0u;

    let input_words = (counts.input_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var w = 0u; w < input_words; w = w + 1u) {
        let cur = curr_inputs.data[w];
        let prev = prev_inputs.data[w];
        let flips = cur ^ prev;
        var mask = 1u;
        for (var b = 0u; b < WORD_BITS && global_bit < counts.input_bits; b = b + 1u) {
            if ((flips & mask) != 0u) {
                if ((cur & mask) != 0u) {
                    frontier_on.data[frontier_counts.on] = global_bit;
                    frontier_counts.on = frontier_counts.on + 1u;
                }
                if ((prev & mask) != 0u) {
                    frontier_off.data[frontier_counts.off] = global_bit;
                    frontier_counts.off = frontier_counts.off + 1u;
                }
                frontier_toggle.data[frontier_counts.toggle] = global_bit;
                frontier_counts.toggle = frontier_counts.toggle + 1u;
            }
            global_bit = global_bit + 1u;
            mask = mask << 1u;
        }
    }

    let internal_offset = counts.input_bits;
    let internal_words = (counts.internal_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var w = 0u; w < internal_words; w = w + 1u) {
        let cur = curr_internals.data[w];
        let prev = prev_internals.data[w];
        let flips = cur ^ prev;
        var mask = 1u;
        for (var b = 0u; b < WORD_BITS && (b + w * WORD_BITS) < counts.internal_bits; b = b + 1u) {
            if ((flips & mask) != 0u) {
                let idx = internal_offset + w * WORD_BITS + b;
                if ((cur & mask) != 0u) {
                    frontier_on.data[frontier_counts.on] = idx;
                    frontier_counts.on = frontier_counts.on + 1u;
                }
                if ((prev & mask) != 0u) {
                    frontier_off.data[frontier_counts.off] = idx;
                    frontier_counts.off = frontier_counts.off + 1u;
                }
                frontier_toggle.data[frontier_counts.toggle] = idx;
                frontier_counts.toggle = frontier_counts.toggle + 1u;
            }
            mask = mask << 1u;
        }
    }

    let output_offset = counts.input_bits + counts.internal_bits;
    let output_words = (counts.output_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var w = 0u; w < output_words; w = w + 1u) {
        let cur = curr_outputs.data[w];
        let prev = prev_outputs.data[w];
        let flips = cur ^ prev;
        var mask = 1u;
        for (var b = 0u; b < WORD_BITS && (b + w * WORD_BITS) < counts.output_bits; b = b + 1u) {
            if ((flips & mask) != 0u) {
                let idx = output_offset + w * WORD_BITS + b;
                if ((cur & mask) != 0u) {
                    frontier_on.data[frontier_counts.on] = idx;
                    frontier_counts.on = frontier_counts.on + 1u;
                }
                if ((prev & mask) != 0u) {
                    frontier_off.data[frontier_counts.off] = idx;
                    frontier_counts.off = frontier_counts.off + 1u;
                }
                frontier_toggle.data[frontier_counts.toggle] = idx;
                frontier_counts.toggle = frontier_counts.toggle + 1u;
            }
            mask = mask << 1u;
        }
    }
}

// ---------------------------------------------------------------
// K2_expand_count: First pass of CSR expansion, counting proposals.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k2_expand_count(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    var total: u32 = 0u;

    var i: u32 = 0u;
    while (i < frontier_counts.on) {
        let bit = frontier_on.data[i];
        let start = csr_offs_on.data[bit];
        let end = csr_offs_on.data[bit + 1u];
        total = total + (end - start);
        i = i + 1u;
    }

    i = 0u;
    while (i < frontier_counts.off) {
        let bit = frontier_off.data[i];
        let start = csr_offs_off.data[bit];
        let end = csr_offs_off.data[bit + 1u];
        total = total + (end - start);
        i = i + 1u;
    }

    i = 0u;
    while (i < frontier_counts.toggle) {
        let bit = frontier_toggle.data[i];
        let start = csr_offs_toggle.data[bit];
        let end = csr_offs_toggle.data[bit + 1u];
        total = total + (end - start);
        i = i + 1u;
    }

    proposal_count.value = total;
}

// ---------------------------------------------------------------
// K2_expand_emit: Second pass CSR expansion emitting proposals.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k2_expand_emit(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    var idx: u32 = 0u;

    var i: u32 = 0u;
    while (i < frontier_counts.on) {
        let bit = frontier_on.data[i];
        let start = csr_offs_on.data[bit];
        let end = csr_offs_on.data[bit + 1u];
        var j = start;
        while (j < end) {
            proposals[idx] = csr_effects_on[j];
            idx = idx + 1u;
            j = j + 1u;
        }
        i = i + 1u;
    }

    i = 0u;
    while (i < frontier_counts.off) {
        let bit = frontier_off.data[i];
        let start = csr_offs_off.data[bit];
        let end = csr_offs_off.data[bit + 1u];
        var j = start;
        while (j < end) {
            proposals[idx] = csr_effects_off[j];
            idx = idx + 1u;
            j = j + 1u;
        }
        i = i + 1u;
    }

    i = 0u;
    while (i < frontier_counts.toggle) {
        let bit = frontier_toggle.data[i];
        let start = csr_offs_toggle.data[bit];
        let end = csr_offs_toggle.data[bit + 1u];
        var j = start;
        while (j < end) {
            proposals[idx] = csr_effects_toggle[j];
            idx = idx + 1u;
            j = j + 1u;
        }
        i = i + 1u;
    }

    proposal_count.value = idx;
}

// ---------------------------------------------------------------
// K3_resolve: Sort proposals and select winners per to_bit.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k3_resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    let n = proposal_count.value;

    // Insertion sort by (to_bit, order_tag)
    var i: u32 = 1u;
    while (i < n) {
        var key = proposals[i];
        var j: i32 = i32(i);
        loop {
            if (j <= 0) { break; }
            let prev = proposals[u32(j - 1)];
            if (prev.to_bit > key.to_bit || (prev.to_bit == key.to_bit && prev.order_tag > key.order_tag)) {
                proposals[u32(j)] = prev;
                j = j - 1;
            } else {
                break;
            }
        }
        proposals[u32(j)] = key;
        i = i + 1u;
    }

    // Take last proposal for each to_bit
    var wcount: u32 = 0u;
    var k: u32 = 0u;
    while (k < n) {
        var current = proposals[k];
        while (k + 1u < n && proposals[k + 1u].to_bit == current.to_bit) {
            k = k + 1u;
            current = proposals[k];
        }
        winners[wcount].to_bit = current.to_bit;
        winners[wcount].action = current.action;
        wcount = wcount + 1u;
        k = k + 1u;
    }
    winners_count.value = wcount;
}

// ---------------------------------------------------------------
// K4_commit: Apply winning proposals to current state.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k4_commit(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    let internal_offset = counts.input_bits;
    let output_offset = counts.input_bits + counts.internal_bits;
    let n = winners_count.value;

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let w = winners[i];
        let bit = w.to_bit;
        let action = w.action;
        if (bit >= internal_offset && bit < output_offset) {
            let local = bit - internal_offset;
            let word = word_index(local);
            let mask = bit_mask(local);
            var val = curr_internals.data[word];
            if (action == 0u) {
                val = val | mask;
            } else if (action == 1u) {
                val = val & (~mask);
            } else {
                val = val ^ mask;
            }
            curr_internals.data[word] = val;
        } else if (bit >= output_offset) {
            let local = bit - output_offset;
            let word = word_index(local);
            let mask = bit_mask(local);
            var val = curr_outputs.data[word];
            if (action == 0u) {
                val = val | mask;
            } else if (action == 1u) {
                val = val & (~mask);
            } else {
                val = val ^ mask;
            }
            curr_outputs.data[word] = val;
        }
        metrics.effects_applied = metrics.effects_applied + 1u;
    }
}

// ---------------------------------------------------------------
// K5_next_frontier: Diff internals to build next frontier.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn k5_next_frontier(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    frontier_counts.on = 0u;
    frontier_counts.off = 0u;
    frontier_counts.toggle = 0u;

    let internal_words = (counts.internal_bits + WORD_BITS - 1u) / WORD_BITS;
    let offset = counts.input_bits;
    for (var w = 0u; w < internal_words; w = w + 1u) {
        let cur = curr_internals.data[w];
        let prev = prev_internals.data[w];
        let flips = cur ^ prev;
        var mask = 1u;
        for (var b = 0u; b < WORD_BITS && (w * WORD_BITS + b) < counts.internal_bits; b = b + 1u) {
            if ((flips & mask) != 0u) {
                let idx = offset + w * WORD_BITS + b;
                if ((cur & mask) != 0u) {
                    frontier_on.data[frontier_counts.on] = idx;
                    frontier_counts.on = frontier_counts.on + 1u;
                }
                if ((prev & mask) != 0u) {
                    frontier_off.data[frontier_counts.off] = idx;
                    frontier_counts.off = frontier_counts.off + 1u;
                }
                frontier_toggle.data[frontier_counts.toggle] = idx;
                frontier_counts.toggle = frontier_counts.toggle + 1u;
            }
            mask = mask << 1u;
        }
        prev_internals.data[w] = cur;
    }
}

// ---------------------------------------------------------------
// Kfinal_finalize: Commit prev = curr and write per-tick metrics.
// ---------------------------------------------------------------
@compute @workgroup_size(64)
fn kfinal_finalize(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u || id.z != 0u) {
        return;
    }

    var h0: u32 = 0u;
    var h1: u32 = 0u;
    var h2: u32 = 0u;
    var h3: u32 = 0u;
    let words = (counts.internal_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var i = 0u; i < words; i = i + 1u) {
        let w = curr_internals.data[i];
        h0 = murmur_mix(h0, w);
        h1 = murmur_mix(h1, rotl32(w, 8u));
        h2 = murmur_mix(h2, rotl32(w, 16u));
        h3 = murmur_mix(h3, rotl32(w, 24u));
    }
    let len = words * 4u;
    h0 = murmur_fmix(h0 ^ len);
    h1 = murmur_fmix(h1 ^ len);
    h2 = murmur_fmix(h2 ^ len);
    h3 = murmur_fmix(h3 ^ len);

    var repeat: u32 = 0u;
    var period: u32 = 0u;
    let window = counts.hash_window;
    for (var i = 0u; i < window; i = i + 1u) {
        let base = i * 4u;
        if (hash_ring.data[base] == h0 && hash_ring.data[base + 1u] == h1 && hash_ring.data[base + 2u] == h2 && hash_ring.data[base + 3u] == h3) {
            repeat = 1u;
            period = (window + hash_state.pos - i) % window;
        }
    }
    let pos = hash_state.pos;
    let base = pos * 4u;
    hash_ring.data[base] = h0;
    hash_ring.data[base + 1u] = h1;
    hash_ring.data[base + 2u] = h2;
    hash_ring.data[base + 3u] = h3;
    hash_state.pos = (pos + 1u) % window;
    hash_state.detected = repeat;
    hash_state.period = period;

    let input_words = (counts.input_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var i = 0u; i < input_words; i = i + 1u) {
        prev_inputs.data[i] = curr_inputs.data[i];
    }

    let internal_words = (counts.internal_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var i = 0u; i < internal_words; i = i + 1u) {
        prev_internals.data[i] = curr_internals.data[i];
    }

    let output_words = (counts.output_bits + WORD_BITS - 1u) / WORD_BITS;
    for (var i = 0u; i < output_words; i = i + 1u) {
        prev_outputs.data[i] = curr_outputs.data[i];
    }
}

