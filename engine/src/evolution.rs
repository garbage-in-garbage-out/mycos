use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
    checkpoint::{save, Checkpoint},
    crossover, evaluate_batch,
    gpu_eval::Episode,
    mutate, Genome, Task,
};

/// Configuration for the evolution loop.
///
/// The structure intentionally exposes only a subset of the parameters from the
/// design document so the loop can be exercised in tests without needing the
/// full runtime. Additional fields can be added as the engine matures.
#[derive(Clone)]
pub struct EvoConfig {
    /// Task describing episodes and scoring.
    pub task: Task,
    /// Genome used as a template for initial population.
    pub base_genome: Genome,
    /// Number of individuals per generation.
    pub pop_size: usize,
    /// Number of generations to run.
    pub generations: u32,
    /// Write a checkpoint every `checkpoint_interval` generations.
    pub checkpoint_interval: u32,
    /// File path for checkpoints. The file is overwritten each time.
    pub checkpoint_path: std::path::PathBuf,
    /// Optional speciation threshold; if `None` all individuals share one
    /// species.
    pub speciation_threshold: Option<f32>,
    /// Tournament size used during selection.
    pub tournament_size: usize,
    /// Number of elite individuals preserved per species.
    pub elitism: usize,
    /// Probability of applying crossover when generating offspring.
    pub crossover_rate: f32,
    /// Probability of applying mutation to an offspring genome.
    pub mutation_rate: f32,
    /// Seed for the top-level RNG driving evolution.
    pub seed: u64,
}

#[derive(Clone)]
struct Individual {
    genome: Genome,
    fitness: f32,
    species: usize,
}

/// Run the evolutionary loop returning the final [`Checkpoint`].
///
/// The implementation is intentionally minimal but wires together evaluation,
/// tournament selection, crossover, mutation, and basic checkpointing. It is
/// sufficient for exercising other components of the engine and can be extended
/// in future iterations.
pub fn run_evolution(config: EvoConfig) -> Checkpoint {
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);

    // --- Population initialisation ----------------------------------------------------------
    let mut population: Vec<Individual> = (0..config.pop_size)
        .map(|_| {
            let mut g = config.base_genome.clone();
            let seed = rng.gen();
            g.meta.seed = seed;
            // Apply a mutation so the population is not uniform.
            let mut grng = ChaCha8Rng::seed_from_u64(seed);
            mutate(&mut g, &mut grng);
            Individual {
                genome: g,
                fitness: 0.0,
                species: 0,
            }
        })
        .collect();

    // Episodes derived from the task. The current `evaluate_batch` stub ignores
    // these values, but creating them here matches the final API.
    let episodes: Vec<Episode> = config
        .task
        .episodes
        .iter()
        .map(|_| Episode::default())
        .collect();

    for gen in 0..config.generations {
        // --- Evaluation ---------------------------------------------------------------------
        let genomes: Vec<Genome> = population.iter().map(|i| i.genome.clone()).collect();
        let results = evaluate_batch(&genomes, &config.task, &episodes);
        for (ind, res) in population.iter_mut().zip(results.into_iter()) {
            ind.fitness = res.fitness;
        }

        // --- Speciation ---------------------------------------------------------------------
        if let Some(thresh) = config.speciation_threshold {
            let mut reps: Vec<Genome> = Vec::new();
            for ind in &mut population {
                let mut assigned = false;
                for (sid, rep) in reps.iter().enumerate() {
                    if genome_distance(&ind.genome, rep) <= thresh {
                        ind.species = sid;
                        assigned = true;
                        break;
                    }
                }
                if !assigned {
                    ind.species = reps.len();
                    reps.push(ind.genome.clone());
                }
            }
        } else {
            for ind in &mut population {
                ind.species = 0;
            }
        }

        // --- Selection & Reproduction -------------------------------------------------------
        let mut species_map: HashMap<usize, Vec<Individual>> = HashMap::new();
        for ind in population.into_iter() {
            species_map.entry(ind.species).or_default().push(ind);
        }

        let mut next_population: Vec<Individual> = Vec::with_capacity(config.pop_size);
        for (species_id, mut members) in species_map.into_iter() {
            // Sort descending by fitness so elites are first.
            members.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
            let elite_count = config.elitism.min(members.len());
            for e in members.iter().take(elite_count) {
                next_population.push(e.clone());
            }

            let offspring = members.len().saturating_sub(elite_count);
            for _ in 0..offspring {
                let p1 = tournament_index(&members, config.tournament_size, &mut rng);
                let mut child = members[p1].genome.clone();
                if rng.gen::<f32>() < config.crossover_rate && members.len() > 1 {
                    let p2 = tournament_index(&members, config.tournament_size, &mut rng);
                    child = crossover(&members[p1].genome, &members[p2].genome, &mut rng);
                }
                if rng.gen::<f32>() < config.mutation_rate {
                    let seed = rng.gen();
                    child.meta.seed = seed;
                    let mut grng = ChaCha8Rng::seed_from_u64(seed);
                    mutate(&mut child, &mut grng);
                }
                next_population.push(Individual {
                    genome: child,
                    fitness: 0.0,
                    species: species_id,
                });
            }
        }
        population = next_population;

        // --- Checkpointing ------------------------------------------------------------------
        if config.checkpoint_interval > 0 && (gen + 1) % config.checkpoint_interval == 0 {
            let cp = Checkpoint {
                generation: gen + 1,
                genomes: population.iter().map(|i| i.genome.clone()).collect(),
                fitness: population.iter().map(|i| i.fitness).collect(),
                rng: rng.clone(),
            };
            let _ = save(&config.checkpoint_path, &cp);
        }
    }

    Checkpoint {
        generation: config.generations,
        genomes: population.iter().map(|i| i.genome.clone()).collect(),
        fitness: population.iter().map(|i| i.fitness).collect(),
        rng,
    }
}

fn tournament_index(members: &[Individual], k: usize, rng: &mut ChaCha8Rng) -> usize {
    let mut best_idx = rng.gen_range(0..members.len());
    let mut best_fit = members[best_idx].fitness;
    for _ in 1..k {
        let idx = rng.gen_range(0..members.len());
        if members[idx].fitness > best_fit {
            best_fit = members[idx].fitness;
            best_idx = idx;
        }
    }
    best_idx
}

fn genome_distance(a: &Genome, b: &Genome) -> f32 {
    let dc = (a.chunks.len() as i32 - b.chunks.len() as i32).abs() as f32;
    let conns_a: usize = a.chunks.iter().map(|c| c.conns.len()).sum();
    let conns_b: usize = b.chunks.iter().map(|c| c.conns.len()).sum();
    let dconns = (conns_a as i32 - conns_b as i32).abs() as f32;
    dc + dconns
}
