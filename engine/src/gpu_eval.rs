use crate::{genome::Genome, tasks::Task};

/// Inputs for a single episode within a batch evaluation.
#[derive(Clone, Debug, Default)]
pub struct Episode {
    /// Input bits encoded as 32-bit words, LSB first.
    pub inputs: Vec<u32>,
}

/// Per-episode metrics returned by `evaluate_batch`.
#[derive(Clone, Debug, Default)]
pub struct EpisodeMetrics {
    /// Number of wavefront rounds executed.
    pub rounds: u32,
    /// Number of effects applied.
    pub effects: u32,
    /// Whether an oscillator was detected.
    pub oscillator: bool,
    /// Oscillation period when `oscillator` is true.
    pub period: u32,
}

/// Result of evaluating a genome over a sequence of episodes.
#[derive(Clone, Debug, Default)]
pub struct FitnessResult {
    /// Fitness score for the genome. Currently always `0.0`.
    pub fitness: f32,
    /// Metrics collected for each episode.
    pub metrics: Vec<EpisodeMetrics>,
    /// Captured output words per episode.
    pub outputs: Vec<Vec<u32>>,
}

/// Evaluate a batch of genomes against a task and episodes.
///
/// This function provides a temporary CPU-side implementation so that the
/// evaluation API compiles even when the `webgpu` feature is disabled. A future
/// version will upload the genomes to the GPU and execute the wavefront kernels
/// in parallel.
pub fn evaluate_batch(
    genomes: &[Genome],
    _task: &Task,
    episodes: &[Episode],
) -> Vec<FitnessResult> {
    let mut results = Vec::with_capacity(genomes.len());
    for _genome in genomes {
        let metrics = vec![EpisodeMetrics::default(); episodes.len()];
        let outputs = vec![Vec::<u32>::new(); episodes.len()];
        results.push(FitnessResult {
            fitness: 0.0,
            metrics,
            outputs,
        });
    }
    results
}
