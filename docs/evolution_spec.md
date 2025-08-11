# Scope

Defines representations, constraints, algorithms, pipelines, file formats, and APIs for evolving Mycos networks (chunks + links) to solve tasks. Targets GPU-first evaluation (WebGPU) with deterministic semantics.

# Terminology

* **Chunk**: Mycos processing unit with Inputs (I), Internals (N), Outputs (O) and Connections (C).
* **Link**: Inter-chunk edge Output(A) → Input(B).
* **Genome**: Encodes one network (set of chunks, connections, links, initial states, metadata).
* **Phenotype**: Compiled Mycos binaries (.myc) + link table for execution.
* **Episode**: One evaluation run with specified inputs/initial conditions and tick budget.
* **Fitness**: Scalar or vector objective computed from outputs and metrics.

# Determinism Requirements

1. All randomized operations derive from a **per-genome seed** (64-bit) and **per-generation seed** (64-bit).
2. Connection and link effects resolve by **(to\_bit\_global, order\_tag)** with **last-writer-wins**.
3. Sorting and reductions use stable, fixed-radix passes (fixed endianness, pass order).
4. All parameters and seeds recorded in artifacts; results must be bit-reproducible on same adapter/driver.

# Genome Specification

## Schema (logical, language-agnostic)

```
Genome {
  meta: {
    seed: u64,         // RNG seed used for mutations & crossover decisions
    tag: string        // optional label
  }
  chunks: [ChunkGene]  // length >=1
  links:  [LinkGene]   // may be empty
}

ChunkGene {
  ni: u32             // input bits
  no: u32             // output bits
  nn: u32             // internal bits
  inputs_init:   bitset(ni)    // default 0 unless task specifies otherwise
  outputs_init:  bitset(no)    // default 0
  internals_init:bitset(nn)    // small random seed allowed
  conns: [ConnGene]            // allowed edge types only
}

ConnGene {                      // 1:1 with Mycos connection record
  from_section: u8  // 0=Input, 1=Internal
  to_section:   u8  // 1=Internal, 2=Output
  trigger:      u8  // 0=On, 1=Off, 2=Toggle
  action:       u8  // 0=Enable, 1=Disable, 2=Toggle
  from_index:   u32 // bit index within from_section
  to_index:     u32 // bit index within to_section
  order_tag:    u32 // strictly increasing per (from_section, from_index)
}

LinkGene {
  from_chunk:   u32
  from_out_idx: u32
  trigger:      u8  // 0=On, 1=Off, 2=Toggle
  action:       u8  // 0=Enable, 1=Disable, 2=Toggle
  to_chunk:     u32
  to_in_idx:    u32
  order_tag:    u32 // strictly increasing per (from_chunk, from_out_idx)
}
```

## Invariants

* Valid edge types only: Input→Internal, Internal→Internal, Internal→Output; Links: Output→Input.
* Indices must be in range for their sections.
* `order_tag` monotone per source; ties forbidden for same source→target; if duplicates occur, keep highest tag.
* `conns` sorted by `(from_section, from_index, order_tag)`; `links` by `(from_chunk, from_out_idx, order_tag)`.

# Phenotype Build (Genome → Executable)

## Compiler Responsibilities

1. Emit each `ChunkGene` as **MycosChunk v1** binary (header, bit sections, connection table).
2. Emit all `LinkGene` records into `links.bin` (packed little-endian), one struct per link:

```
u32 from_chunk, u32 from_out_idx, u8 trigger, u8 action, u16 reserved=0,
u32 to_chunk,   u32 to_in_idx,    u32 order_tag
```

3. Validate invariants; reject on violation.
4. Optional pruning: remove connections unreachable from Inputs (fast forward reachability); must be deterministic.

# Population & Speciation

## Population

* Size `P` ∈ \[256, 2048] (tunable). Each genome has unique `meta.seed`.

## Optional Speciation

* Distance `D(g1,g2) = wC·Δ#chunks + wK·Δ#conns_norm + wL·Δ#links + wTA·L1(trigger/action hist)`
* Adaptive threshold keeps target species count S ∈ \[5,15].
* Elitism: carry top `E` individuals per species (E ∈ {1,2}).

# Mutation Operators

## Application

* For each genome, attempt independent operations with probabilities below.
* After each operation: re-sort, re-validate; retry up to 3 attempts if invalid, otherwise skip.
* All randomness from `meta.seed` advanced by a counter (Xoshiro/PCG recommended).

## Operators (default probabilities)

* **Add connection** (p=0.20):
  Sample valid `(from_section, to_section)`; sample indices uniformly; random `trigger/action`; set `order_tag = prev_max + 1` for that source.
* **Remove connection** (p=0.15): delete uniformly at random.
* **Rewire target** (p=0.15): keep `from*`, resample `to_index` in same `to_section`.
* **Flip trigger** (p=0.05): On→Off→Toggle→On cycle.
* **Flip action** (p=0.05): Enable→Disable→Toggle→Enable cycle.
* **Bump order** (p=0.05): `order_tag += U{1..5}` for chosen connection; maintain monotonicity by re-spreading tags if needed.
* **Add internal bits** (p=0.05): `nn += k` (k∈\[1,8]); extend `internals_init` with zeros; reindex targets ≥ insertion point unchanged (append-only to avoid reindex churn).
* **Remove sparse internal bits** (p=0.03): remove up to k indices with no incident edges; drop conns pointing to removed bits; compact bitset.
* **Add link** (p=0.10): random valid Output(A)→Input(B); new `order_tag = prev_max + 1` for that output.
* **Remove link** (p=0.07): delete uniformly.
* **Init state tweak** (p=0.05): flip random internal init bit.
* **Gate insert (optional)** (p=0.02): add child chunk and parent gate mapping (alias I/O); see nesting spec; must compile to valid phenotype if nesting is enabled.

## Bounds (hard caps)

* `max_chunks`, `max_conns_per_chunk`, `max_links`, `max_nn_per_chunk`.
* If operation exceeds caps, rollback operation.

# Crossover

## Alignment Rules

* **Connections**: align by `(from_section, from_index, to_section, to_index)`.

  * If both parents have it: choose action/trigger from either with p=0.5; `order_tag = max(tagA, tagB)` or sample between.
  * If only one has it: include with p=0.5.
* **Links**: align by `(from_chunk, from_out_idx, to_chunk, to_in_idx)` with same rule.
* **Chunks**: baseline is parent A’s chunk list; with p=0.5 replace chunk i by parent B’s if exists.
* Enforce bounds and invariants; re-sort after merge.

# Tasks, Episodes, Fitness

## Task Schema

```
Task {
  name: string
  io: {
    inputs:  [(chunk_id:u32, bit_idx:u32)] // controlled by task
    outputs: [(chunk_id:u32, bit_idx:u32)] // observed for scoring
  }
  episodes: [EpisodeSpec]  // length E >= 1
  tick_budget: u32         // max micro-steps per episode
  scoring: ScoringSpec
}

EpisodeSpec {
  input_schedule: [InputVector] // per tick vectors or single vector (length 1)
  overrides: { internals_init?: [(chunk_id, bit_idx, value)] } // optional
}

InputVector { bits: bitset|word-array for all task-mapped inputs }
```

## ScoringSpec (normative examples)

* **Combinational mapping**: one tick; Hamming similarity of outputs vs targets.
  `score = 1 - H(outputs ⊕ targets)/M`, M = #observed outputs.
* **Sequential mapping**: T ticks; sum per-tick matches or distance to target sequence.
* **Latency-aware**: reward decays by steps until first match; `score = exp(-α · ticks_to_success)`.
* **Multi-objective**: vector `(score, -#connections, -sum(nn), -osc_penalty)`; Pareto sort or lexicographic.

## Episode Execution (per individual)

1. Initialize `Prev` state from `*_init` (apply overrides).
2. For each tick:

   * Write Inputs from `input_schedule[t]` into `Curr`.
   * Run GPU wavefront loop with round cap/guards until frontier empty or cap.
   * Read Outputs (only task-mapped bits) and metrics (rounds, effects\_applied, oscillator, period).
3. Accumulate fitness per `ScoringSpec`.

# Evaluation Pipeline (GPU Batched)

## Packing

* Concatenate all individuals’ sections into global device buffers; maintain per-individual base offsets for Inputs/Internals/Outputs and CSR/effects.
* Ensure **disjoint keyspaces** for resolver:
  `key.to_bit = to_bit_global | (individual_id << INDIV_SHIFT)` so proposals never cross individuals.

## Kernels (shared with Mycos engine)

* K1 detect edges → frontiers
* K2 expand (count + emit) → proposals `(to_bit_key, order_tag, action)`
* K3 resolve (radix sort by `(to_bit_key, order_tag)`, segmented take-last) → winners
* K4 commit winners → Curr
* K5 next frontier from Internals (and swap Prev\_internal)
* K6 finalize episode tick; also update effect counters and hash ring for cycle detection

## Guards & Oscillation

* `max_rounds` (default 1024), `max_effects` (default 5e6).
* Cycle detection: ring buffer of last `R=8` 128-bit hashes of Internals; if hash repeats → cycle of period ≤ R.
* Policies on trigger (configurable):

  * `freeze_last_stable`, or
  * `clamp_commutative` (Disable > Enable > Toggle parity), or
  * `parity_quench`.
* Metrics exported per episode: `{rounds, effectsApplied, oscillator:boolean, period:u32, policy_applied}`.

# Selection

## Options

* **Tournament** size `k ∈ [3,7]` (per species if speciation enabled).
* **Elitism**: carry top `E` per species.
* **Diversity pressure**: optional novelty metric (see below).

# Novelty Search (Optional)

* Behavior descriptor (BD): last K outputs (concatenated) or sequence hash.
* Novelty = average distance to k-nearest in archive.
* Combined objective: `fitness' = α·fitness + (1-α)·normalized_novelty`, `α∈[0,1]`.

# Regularization

## Hard Caps

* Reject genomes exceeding `max_chunks`, `max_conns_per_chunk`, `max_links`, `max_nn_per_chunk`.

## Soft Penalties

* Fitness penalties added per episode or per individual:

  * `λ_conn · total_connections`
  * `λ_bits · Σ nn`
  * `λ_links · total_links`
  * `λ_osc · rounds_if_oscillating + λ_cycle · 1{period>0}`
  * Optional minimum-activity constraint: penalize `effectsApplied == 0` for tasks expecting activity.

# Reproducibility & Checkpointing

## Artifacts

* **Genome JSON** with `meta.seed`, all genes sorted.
* **Phenotype binaries** (optional cache): `.myc` per chunk + `links.bin`.
* **Evaluation log**: per episode metrics and fitness.
* **Evolution state**: generation number, population seeds, RNG states, hyperparameters, engine version, GPU adapter info.

## Resume

* Reload population + RNG states → continue; results must match if rerun on same stack.

# APIs

## Engine/WASM (host interface)

```
init(canvas?: HTMLCanvasElement): Promise<Handle>

loadGenome(genomeJson: ArrayBuffer): GenomeHandle        // or load many
evaluate(genomes: GenomeHandle[], task: Task, episodes: EpisodeSpec[], opts:{tickBudget:u32})
  -> FitnessResult[]

type FitnessResult = {
  fitness: number | number[],               // scalar or vector
  metrics: Array<{ rounds:u32, effectsApplied:u32, oscillator:bool, period:u32, policy:u8 }>
  outputs?: Uint32Array[]                   // optional capture per episode
}
```

## Compiler

```
compile(genome: Genome) -> { chunks: Vec<Vec<u8>>, links: Vec<u8> }
```

## Evolution Loop (host)

* `run_evolution(config: EvoConfig) -> Checkpoint`
* `mutate(genome, seed)`; `crossover(a,b,seed)`.

# File Formats

## Genome JSON (canonical)

* UTF-8, canonical key order, arrays sorted per invariants.
* Bitsets encoded as base64 for compactness or as hex strings; must specify endianness (LSB-first within words).

## Fixtures

* Provide reference tasks and episodes with expected outputs for small nets:

  * `T00_wire_echo`, `T01_xor2`, `T02_sr_latch`, `T03_pulse_counter`, `T04_cross_chunk`.

# Hyperparameters (defaults)

```
pop_size           = 512
gens               = 200
episodes_per_ind   = 16
tick_budget        = 256
mutation_rate      = 0.9 (independent ops per genome allowed)
crossover_rate     = 0.6
speciation_on      = true
species_target     = 10
tournament_k       = 5
elitism_per_sp     = 2

λ_conn             = 1e-4
λ_bits             = 1e-4
λ_links            = 1e-4
λ_osc              = 1e-3
λ_cycle            = 5e-4

max_rounds         = 1024
max_effects        = 5_000_000
cycle_hash_window  = 8
```

# Security & Safety

* Cap GPU memory per batch; validate all external genome inputs before compilation.
* Deny malformed binaries; never execute chunks with out-of-range indices.
* Timeouts per evaluation batch to avoid browser hangs.

# Conformance Tests (normative)

1. **Determinism**: same seeds → identical fitness and metrics across two runs.
2. **Resolve Semantics**: crafted conflicts resolving by highest `order_tag`.
3. **Oscillation Detection**: known 2-cycle fixture sets period flag; policies terminate within limits.
4. **Mutation Validity**: after N random mutations, genome remains valid and compilable.
5. **Crossover Validity**: children compile; constraints observed.
6. **End-to-End**: `T01_xor2` solvable to ≥0.95 score within 100 generations at defaults.

# Non-Normative Guidance

* Favor append-only growth of `nn` to avoid heavy reindexing; prune with fitness penalties.
* Batch many individuals per evaluation to amortize sort; ensure disjoint resolver keyspaces.
