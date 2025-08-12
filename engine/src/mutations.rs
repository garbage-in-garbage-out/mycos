use crate::genome::{ChunkGene, ConnGene, Genome, LinkGene};
use rand::{Rng, RngCore};

// Probabilities per genome per generation
const P_ADD_CONN: f64 = 0.20;
const P_REMOVE_CONN: f64 = 0.15;
const P_REWIRE: f64 = 0.15;
const P_FLIP_TRIGGER: f64 = 0.05;
const P_FLIP_ACTION: f64 = 0.05;
const P_BUMP_ORDER: f64 = 0.05;
const P_ADD_BIT: f64 = 0.05;
const P_REMOVE_BIT: f64 = 0.03;
const P_ADD_LINK: f64 = 0.10;
const P_REMOVE_LINK: f64 = 0.07;
const P_INIT_TWEAK: f64 = 0.05;
const P_GATE_INSERT: f64 = 0.02; // optional

/// Apply mutation operators with their probabilities.
/// Each mutation retries up to three times if validation fails.
pub fn mutate(genome: &mut Genome, rng: &mut dyn RngCore) {
    if rng.gen::<f64>() < P_ADD_CONN {
        apply_with_retry(genome, rng, add_connection);
    }
    if rng.gen::<f64>() < P_REMOVE_CONN {
        apply_with_retry(genome, rng, remove_connection);
    }
    if rng.gen::<f64>() < P_REWIRE {
        apply_with_retry(genome, rng, rewire_target);
    }
    if rng.gen::<f64>() < P_FLIP_TRIGGER {
        apply_with_retry(genome, rng, flip_trigger);
    }
    if rng.gen::<f64>() < P_FLIP_ACTION {
        apply_with_retry(genome, rng, flip_action);
    }
    if rng.gen::<f64>() < P_BUMP_ORDER {
        apply_with_retry(genome, rng, bump_order_tag);
    }
    if rng.gen::<f64>() < P_ADD_BIT {
        apply_with_retry(genome, rng, add_internal_bit);
    }
    if rng.gen::<f64>() < P_REMOVE_BIT {
        apply_with_retry(genome, rng, remove_internal_bit);
    }
    if rng.gen::<f64>() < P_ADD_LINK {
        apply_with_retry(genome, rng, add_link);
    }
    if rng.gen::<f64>() < P_REMOVE_LINK {
        apply_with_retry(genome, rng, remove_link);
    }
    if rng.gen::<f64>() < P_INIT_TWEAK {
        apply_with_retry(genome, rng, init_state_tweak);
    }
    if rng.gen::<f64>() < P_GATE_INSERT {
        apply_with_retry(genome, rng, gate_insert);
    }
}

fn apply_with_retry(
    genome: &mut Genome,
    rng: &mut dyn RngCore,
    mutator: fn(&mut Genome, &mut dyn RngCore),
) {
    let original = genome.clone();
    for _ in 0..3 {
        mutator(genome, rng);
        genome.sort();
        if genome.validate().is_ok() {
            return;
        }
        *genome = original.clone();
    }
    *genome = original;
}

fn add_connection(genome: &mut Genome, rng: &mut dyn RngCore) {
    if genome.chunks.is_empty() {
        return;
    }
    let chunk_idx = rng.next_u32() as usize % genome.chunks.len();
    let chunk = &mut genome.chunks[chunk_idx];
    if chunk.nn == 0 && chunk.no == 0 {
        return;
    }
    let edge = rng.next_u32() % 3;
    let (from_section, to_section) = match edge {
        0 => (0, 1),
        1 => (1, 1),
        _ => (1, 2),
    };
    let from_index = match from_section {
        0 => rng.next_u32() % chunk.ni.max(1),
        1 => rng.next_u32() % chunk.nn.max(1),
        _ => 0,
    };
    let to_index = match to_section {
        1 => rng.next_u32() % chunk.nn.max(1),
        2 => rng.next_u32() % chunk.no.max(1),
        _ => 0,
    };
    let trigger = (rng.next_u32() % 3) as u8;
    let action = (rng.next_u32() % 3) as u8;
    let max_tag = chunk
        .conns
        .iter()
        .filter(|c| c.from_section == from_section && c.from_index == from_index)
        .map(|c| c.order_tag)
        .max()
        .unwrap_or(0);
    let order_tag = if chunk
        .conns
        .iter()
        .any(|c| c.from_section == from_section && c.from_index == from_index)
    {
        max_tag + 1
    } else {
        0
    };
    chunk.conns.push(ConnGene {
        from_section,
        to_section,
        trigger,
        action,
        from_index,
        to_index,
        order_tag,
    });
    fix_conn_order_tags(chunk);
}

fn remove_connection(genome: &mut Genome, rng: &mut dyn RngCore) {
    let indices: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.conns.is_empty())
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return;
    }
    let cidx = indices[rng.next_u32() as usize % indices.len()];
    let chunk = &mut genome.chunks[cidx];
    let conn_idx = rng.next_u32() as usize % chunk.conns.len();
    chunk.conns.remove(conn_idx);
    fix_conn_order_tags(chunk);
}

fn rewire_target(genome: &mut Genome, rng: &mut dyn RngCore) {
    let indices: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.conns.is_empty())
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return;
    }
    let cidx = indices[rng.next_u32() as usize % indices.len()];
    let chunk = &mut genome.chunks[cidx];
    let conn_idx = rng.next_u32() as usize % chunk.conns.len();
    let conn = &mut chunk.conns[conn_idx];
    let range = match conn.to_section {
        1 => chunk.nn,
        2 => chunk.no,
        _ => 0,
    };
    if range == 0 {
        return;
    }
    let mut new_idx = rng.next_u32() % range;
    if range > 1 {
        for _ in 0..5 {
            if new_idx != conn.to_index {
                break;
            }
            new_idx = rng.next_u32() % range;
        }
    }
    conn.to_index = new_idx;
}

fn flip_trigger(genome: &mut Genome, rng: &mut dyn RngCore) {
    let indices: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.conns.is_empty())
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return;
    }
    let cidx = indices[rng.next_u32() as usize % indices.len()];
    let chunk = &mut genome.chunks[cidx];
    let conn_idx = rng.next_u32() as usize % chunk.conns.len();
    chunk.conns[conn_idx].trigger = (chunk.conns[conn_idx].trigger + 1) % 3;
}

fn flip_action(genome: &mut Genome, rng: &mut dyn RngCore) {
    let indices: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.conns.is_empty())
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return;
    }
    let cidx = indices[rng.next_u32() as usize % indices.len()];
    let chunk = &mut genome.chunks[cidx];
    let conn_idx = rng.next_u32() as usize % chunk.conns.len();
    chunk.conns[conn_idx].action = (chunk.conns[conn_idx].action + 1) % 3;
}

fn bump_order_tag(genome: &mut Genome, rng: &mut dyn RngCore) {
    let indices: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.conns.is_empty())
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return;
    }
    let cidx = indices[rng.next_u32() as usize % indices.len()];
    let chunk = &mut genome.chunks[cidx];
    let conn_idx = rng.next_u32() as usize % chunk.conns.len();
    let bump = rng.next_u32() % 5 + 1;
    chunk.conns[conn_idx].order_tag += bump;
    fix_conn_order_tags(chunk);
}

fn add_internal_bit(genome: &mut Genome, rng: &mut dyn RngCore) {
    if genome.chunks.is_empty() {
        return;
    }
    let idx = rng.next_u32() as usize % genome.chunks.len();
    let chunk = &mut genome.chunks[idx];
    let add = (rng.next_u32() % 8 + 1) as usize;
    chunk.nn += add as u32;
    chunk.internals_init.resize(chunk.nn as usize, false);
}

fn remove_internal_bit(genome: &mut Genome, rng: &mut dyn RngCore) {
    let candidates: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| c.nn > 0)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let cidx = candidates[rng.next_u32() as usize % candidates.len()];
    let chunk = &mut genome.chunks[cidx];
    let unused: Vec<u32> = (0..chunk.nn)
        .filter(|i| {
            !chunk.conns.iter().any(|c| {
                (c.from_section == 1 && c.from_index == *i)
                    || (c.to_section == 1 && c.to_index == *i)
            })
        })
        .collect();
    if unused.is_empty() {
        return;
    }
    let remove = unused[rng.next_u32() as usize % unused.len()] as usize;
    chunk.internals_init.remove(remove);
    chunk.nn -= 1;
    for conn in &mut chunk.conns {
        if conn.from_section == 1 {
            if conn.from_index == remove as u32 {
                conn.from_index = u32::MAX;
            } else if conn.from_index > remove as u32 {
                conn.from_index -= 1;
            }
        }
        if conn.to_section == 1 {
            if conn.to_index == remove as u32 {
                conn.to_index = u32::MAX;
            } else if conn.to_index > remove as u32 {
                conn.to_index -= 1;
            }
        }
    }
    chunk
        .conns
        .retain(|c| c.from_index != u32::MAX && c.to_index != u32::MAX);
    fix_conn_order_tags(chunk);
}

fn add_link(genome: &mut Genome, rng: &mut dyn RngCore) {
    if genome.chunks.is_empty() {
        return;
    }
    let from_chunk_idx = rng.next_u32() as usize % genome.chunks.len();
    let to_chunk_idx = rng.next_u32() as usize % genome.chunks.len();
    let from_chunk = &genome.chunks[from_chunk_idx];
    let to_chunk = &genome.chunks[to_chunk_idx];
    if from_chunk.no == 0 || to_chunk.ni == 0 {
        return;
    }
    let from_out_idx = rng.next_u32() % from_chunk.no;
    let to_in_idx = rng.next_u32() % to_chunk.ni;
    let trigger = (rng.next_u32() % 3) as u8;
    let action = (rng.next_u32() % 3) as u8;
    let max_tag = genome
        .links
        .iter()
        .filter(|l| l.from_chunk == from_chunk_idx as u32 && l.from_out_idx == from_out_idx)
        .map(|l| l.order_tag)
        .max()
        .unwrap_or(0);
    let order_tag = if genome
        .links
        .iter()
        .any(|l| l.from_chunk == from_chunk_idx as u32 && l.from_out_idx == from_out_idx)
    {
        max_tag + 1
    } else {
        0
    };
    genome.links.push(LinkGene {
        from_chunk: from_chunk_idx as u32,
        from_out_idx,
        trigger,
        action,
        to_chunk: to_chunk_idx as u32,
        to_in_idx,
        order_tag,
    });
    fix_link_order_tags(genome);
}

fn remove_link(genome: &mut Genome, rng: &mut dyn RngCore) {
    if genome.links.is_empty() {
        return;
    }
    let idx = rng.next_u32() as usize % genome.links.len();
    genome.links.remove(idx);
    fix_link_order_tags(genome);
}

fn init_state_tweak(genome: &mut Genome, rng: &mut dyn RngCore) {
    let candidates: Vec<usize> = genome
        .chunks
        .iter()
        .enumerate()
        .filter(|(_, c)| c.nn > 0)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let cidx = candidates[rng.next_u32() as usize % candidates.len()];
    let chunk = &mut genome.chunks[cidx];
    let bit = rng.next_u32() as usize % chunk.nn as usize;
    let current = chunk.internals_init[bit];
    chunk.internals_init.set(bit, !current);
}

fn gate_insert(_genome: &mut Genome, _rng: &mut dyn RngCore) {
    // Optional gate insertion not implemented.
}

fn fix_conn_order_tags(chunk: &mut ChunkGene) {
    chunk.conns.sort_by(|a, b| {
        (a.from_section, a.from_index, a.order_tag).cmp(&(
            b.from_section,
            b.from_index,
            b.order_tag,
        ))
    });
    let mut last_source: Option<(u8, u32)> = None;
    let mut last_tag = 0u32;
    for conn in &mut chunk.conns {
        let source = (conn.from_section, conn.from_index);
        if Some(source) != last_source {
            last_source = Some(source);
            last_tag = conn.order_tag;
        } else if conn.order_tag <= last_tag {
            last_tag += 1;
            conn.order_tag = last_tag;
        } else {
            last_tag = conn.order_tag;
        }
    }
}

fn fix_link_order_tags(genome: &mut Genome) {
    genome.links.sort_by(|a, b| {
        (a.from_chunk, a.from_out_idx, a.order_tag).cmp(&(
            b.from_chunk,
            b.from_out_idx,
            b.order_tag,
        ))
    });
    let mut last_source: Option<(u32, u32)> = None;
    let mut last_tag = 0u32;
    for link in &mut genome.links {
        let source = (link.from_chunk, link.from_out_idx);
        if Some(source) != last_source {
            last_source = Some(source);
            last_tag = link.order_tag;
        } else if link.order_tag <= last_tag {
            last_tag += 1;
            link.order_tag = last_tag;
        } else {
            last_tag = link.order_tag;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::GenomeMeta;
    use bitvec::prelude::*;
    use rand::rngs::mock::StepRng;

    struct SeqRng {
        values: Vec<u32>,
        idx: usize,
    }

    impl RngCore for SeqRng {
        fn next_u32(&mut self) -> u32 {
            let v = *self.values.get(self.idx).unwrap_or(&0);
            self.idx += 1;
            v
        }

        fn next_u64(&mut self) -> u64 {
            self.next_u32() as u64
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(4) {
                let bytes = self.next_u32().to_le_bytes();
                let len = chunk.len();
                chunk.copy_from_slice(&bytes[..len]);
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    fn simple_genome() -> Genome {
        let chunk = ChunkGene::new(
            1,
            1,
            1,
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            vec![],
        );
        Genome::new(vec![chunk], vec![], GenomeMeta::new(0, "t".into())).unwrap()
    }

    #[test]
    fn test_add_connection() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 1);
        add_connection(&mut genome, &mut rng);
        genome.sort();
        genome.validate().unwrap();
        assert_eq!(genome.chunks[0].conns.len(), 1);
    }

    #[test]
    fn test_remove_connection() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 0);
        add_connection(&mut genome, &mut rng);
        remove_connection(&mut genome, &mut rng);
        genome.sort();
        genome.validate().unwrap();
        assert_eq!(genome.chunks[0].conns.len(), 0);
    }

    #[test]
    fn test_rewire_target() {
        let chunk = ChunkGene::new(
            0,
            0,
            2,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0; 0, 0],
            vec![ConnGene::new(1, 1, 0, 0, 0, 0, 0).unwrap()],
        );
        let mut genome = Genome::new(vec![chunk], vec![], GenomeMeta::new(0, "t".into())).unwrap();
        let mut rng = StepRng::new(1, 0);
        rewire_target(&mut genome, &mut rng);
        genome.sort();
        genome.validate().unwrap();
        assert_eq!(genome.chunks[0].conns[0].to_index, 1);
    }

    #[test]
    fn test_flip_trigger() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 0);
        add_connection(&mut genome, &mut rng);
        flip_trigger(&mut genome, &mut rng);
        assert_eq!(genome.chunks[0].conns[0].trigger, 1);
    }

    #[test]
    fn test_flip_action() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 0);
        add_connection(&mut genome, &mut rng);
        flip_action(&mut genome, &mut rng);
        assert_eq!(genome.chunks[0].conns[0].action, 1);
    }

    #[test]
    fn test_bump_order_tag() {
        let chunk = ChunkGene::new(
            0,
            1,
            1,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            vec![
                ConnGene::new(1, 2, 0, 0, 0, 0, 0).unwrap(),
                ConnGene::new(1, 2, 0, 0, 0, 0, 1).unwrap(),
            ],
        );
        let mut genome = Genome::new(vec![chunk], vec![], GenomeMeta::new(0, "t".into())).unwrap();
        let old = genome.chunks[0].conns[0].order_tag;
        let mut rng = StepRng::new(0, 0);
        bump_order_tag(&mut genome, &mut rng);
        assert!(genome.chunks[0].conns[0].order_tag > old);
        genome.sort();
        genome.validate().unwrap();
    }

    #[test]
    fn test_add_internal_bit() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 0);
        add_internal_bit(&mut genome, &mut rng);
        assert_eq!(genome.chunks[0].nn, 2);
        assert_eq!(genome.chunks[0].internals_init.len(), 2);
    }

    #[test]
    fn test_remove_internal_bit() {
        let chunk = ChunkGene::new(
            0,
            0,
            2,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0; 0, 0],
            vec![ConnGene::new(1, 1, 0, 0, 0, 0, 0).unwrap()],
        );
        let mut genome = Genome::new(vec![chunk], vec![], GenomeMeta::new(0, "t".into())).unwrap();
        let mut rng = StepRng::new(0, 0);
        remove_internal_bit(&mut genome, &mut rng);
        assert_eq!(genome.chunks[0].nn, 1);
        assert_eq!(genome.chunks[0].internals_init.len(), 1);
    }

    #[test]
    fn test_add_link() {
        let chunk_a = ChunkGene::new(
            0,
            1,
            0,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0;],
            vec![],
        );
        let chunk_b = ChunkGene::new(
            1,
            0,
            0,
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            vec![],
        );
        let mut genome = Genome::new(
            vec![chunk_a, chunk_b],
            vec![],
            GenomeMeta::new(0, "t".into()),
        )
        .unwrap();
        let mut rng = SeqRng {
            values: vec![0, 1, 0, 0, 0, 0],
            idx: 0,
        };
        add_link(&mut genome, &mut rng);
        genome.sort();
        genome.validate().unwrap();
        assert_eq!(genome.links.len(), 1);
    }

    #[test]
    fn test_remove_link() {
        let chunk_a = ChunkGene::new(
            0,
            1,
            0,
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0;],
            vec![],
        );
        let chunk_b = ChunkGene::new(
            1,
            0,
            0,
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0;],
            bitvec![u8, Lsb0;],
            vec![],
        );
        let link = LinkGene::new(0, 0, 0, 0, 1, 0, 0).unwrap();
        let mut genome = Genome::new(
            vec![chunk_a, chunk_b],
            vec![link],
            GenomeMeta::new(0, "t".into()),
        )
        .unwrap();
        let mut rng = StepRng::new(0, 0);
        remove_link(&mut genome, &mut rng);
        assert_eq!(genome.links.len(), 0);
    }

    #[test]
    fn test_init_state_tweak() {
        let mut genome = simple_genome();
        let mut rng = StepRng::new(0, 0);
        init_state_tweak(&mut genome, &mut rng);
        assert!(genome.chunks[0].internals_init[0]);
    }
}
