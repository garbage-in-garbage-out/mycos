use crate::tasks::{EpisodeSpec, Task};

/// Scoring strategies supported by the engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScoringSpec {
    /// Measure Hamming similarity of outputs versus expected targets.
    /// The score is `1.0 - H(outputs XOR targets) / M`, where `M` is the
    /// number of observed output bits.
    Hamming,
}

/// Compute a fitness score for a task given the captured outputs for each
/// episode. `outputs` must have the same shape as `task.episodes`: a vector of
/// episodes, each containing per-tick output words.
pub fn score(task: &Task, outputs: &[Vec<Vec<u32>>]) -> f32 {
    assert_eq!(task.episodes.len(), outputs.len());
    match task.scoring {
        ScoringSpec::Hamming => {
            let mut total_score = 0.0f32;
            for (spec, actual) in task.episodes.iter().zip(outputs.iter()) {
                total_score += hamming_episode(spec, actual, task.io.outputs.len());
            }
            total_score / task.episodes.len() as f32
        }
    }
}

fn hamming_episode(spec: &EpisodeSpec, actual: &[Vec<u32>], output_bits: usize) -> f32 {
    assert_eq!(spec.expected.len(), actual.len());
    let mut total_bits = 0u32;
    let mut diff_bits = 0u32;
    for (expected_tick, actual_tick) in spec.expected.iter().zip(actual.iter()) {
        assert_eq!(expected_tick.len(), actual_tick.len());
        for (e, a) in expected_tick.iter().zip(actual_tick.iter()) {
            diff_bits += (e ^ a).count_ones();
        }
        total_bits += output_bits as u32;
    }
    if total_bits == 0 {
        1.0
    } else {
        1.0 - diff_bits as f32 / total_bits as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{
        t00_wire_echo, t01_xor_2, t02_sr_latch, t03_pulse_counter, t04_cross_chunk_relay,
    };

    fn perfect_outputs(task: &Task) -> Vec<Vec<Vec<u32>>> {
        task.episodes.iter().map(|e| e.expected.clone()).collect()
    }

    fn flipped_outputs(task: &Task) -> Vec<Vec<Vec<u32>>> {
        let mut outs = perfect_outputs(task);
        if let Some(first_tick) = outs.get_mut(0).and_then(|ep| ep.get_mut(0)) {
            if let Some(word) = first_tick.get_mut(0) {
                *word ^= 1; // flip least significant bit
            }
        }
        outs
    }

    #[test]
    fn score_wire_echo() {
        let task = t00_wire_echo();
        let good = perfect_outputs(&task);
        let bad = flipped_outputs(&task);
        assert_eq!(score(&task, &good), 1.0);
        assert!(score(&task, &bad) < 1.0);
    }

    #[test]
    fn score_xor2() {
        let task = t01_xor_2();
        let good = perfect_outputs(&task);
        let bad = flipped_outputs(&task);
        assert_eq!(score(&task, &good), 1.0);
        assert!(score(&task, &bad) < 1.0);
    }

    #[test]
    fn score_sr_latch() {
        let task = t02_sr_latch();
        let good = perfect_outputs(&task);
        let bad = flipped_outputs(&task);
        assert_eq!(score(&task, &good), 1.0);
        assert!(score(&task, &bad) < 1.0);
    }

    #[test]
    fn score_pulse_counter() {
        let task = t03_pulse_counter();
        let good = perfect_outputs(&task);
        let bad = flipped_outputs(&task);
        assert_eq!(score(&task, &good), 1.0);
        assert!(score(&task, &bad) < 1.0);
    }

    #[test]
    fn score_cross_chunk_relay() {
        let task = t04_cross_chunk_relay();
        let good = perfect_outputs(&task);
        let bad = flipped_outputs(&task);
        assert_eq!(score(&task, &good), 1.0);
        assert!(score(&task, &bad) < 1.0);
    }
}
