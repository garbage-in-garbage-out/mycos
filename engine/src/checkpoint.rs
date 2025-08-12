use std::fs;
use std::path::Path;

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::Genome;

/// Evolution checkpoint allowing training to resume deterministically.
#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Generation number at which the checkpoint was taken.
    pub generation: u32,
    /// Genomes comprising the population.
    pub genomes: Vec<Genome>,
    /// Fitness score for each genome.
    pub fitness: Vec<f32>,
    /// RNG state for the evolution loop.
    pub rng: ChaCha8Rng,
}

/// Save a checkpoint to the given path as JSON.
pub fn save(path: &Path, cp: &Checkpoint) -> std::io::Result<()> {
    let json = serde_json::to_string(cp)?;
    fs::write(path, json)
}

/// Load a checkpoint from the given path.
pub fn load(path: &Path) -> std::io::Result<Checkpoint> {
    let json = fs::read_to_string(path)?;
    let cp: Checkpoint = serde_json::from_str(&json)?;
    Ok(cp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitvec::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::fs;

    #[test]
    fn save_and_load_roundtrip() {
        let chunk = crate::ChunkGene::new(
            0,
            0,
            0,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            vec![],
        );
        let genome =
            crate::Genome::new(vec![chunk], vec![], crate::GenomeMeta::new(7, "".into())).unwrap();
        let rng = ChaCha8Rng::seed_from_u64(42);
        let cp = Checkpoint {
            generation: 3,
            genomes: vec![genome],
            fitness: vec![1.23],
            rng: rng.clone(),
        };
        let path = std::env::temp_dir().join("mycos_checkpoint_test.json");
        save(&path, &cp).unwrap();
        let loaded = load(&path).unwrap();
        fs::remove_file(path).ok();

        assert_eq!(loaded.generation, cp.generation);
        assert_eq!(loaded.genomes.len(), cp.genomes.len());
        assert_eq!(loaded.fitness, cp.fitness);
        let mut r1 = cp.rng.clone();
        let mut r2 = loaded.rng.clone();
        let v1: u64 = r1.gen();
        let v2: u64 = r2.gen();
        assert_eq!(v1, v2);
    }
}
