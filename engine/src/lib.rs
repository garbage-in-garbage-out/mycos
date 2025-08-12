pub mod chunk;
pub mod cpu_ref;
pub mod crossover;
pub mod csr;
pub mod embed;
pub mod genome;
pub mod gpu_eval;
pub mod layout;
pub mod link;
pub mod mutations;
pub mod policy;
pub mod scc;

#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
pub mod api;
#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
pub mod gpu;
pub use chunk::{
    parse_chunk, validate_chunk, Action, Connection, Error, MycosChunk, Section, Trigger,
};
pub use crossover::crossover;
pub use csr::{build_csr, Effect, CSR};
pub use embed::{execute_gated_alias, execute_gated_copy, parse_embeds, Embed, EmbedError, IoMode};
pub use genome::{ChunkGene, ConnGene, Genome, GenomeMeta, LinkGene, ValidationError};
pub use gpu_eval::{evaluate_batch, Episode, EpisodeMetrics, FitnessResult, Task};
pub use layout::{
    bit_to_word, clr_bit, connection_table_offset, section_offsets, set_bit, xor_bit, HEADER_BYTES,
};
pub use link::{
    build_link_csr, compute_base_offsets, parse_links, validate_links, ChunkOffsets, Link,
    LinkError,
};
pub use mutations::mutate;
pub use policy::{
    clamp_commutative, freeze_last_stable, parity_quench, CycleDetector, ExecutionResult, Policy,
};
pub use scc::{build_internal_graph, scc_ids_and_topo_levels};

#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
pub use gpu::device::init_device;
