use std::collections::BTreeMap;

use bitvec::prelude::*;
use rand::RngCore;

use crate::genome::{ChunkGene, ConnGene, Genome, GenomeMeta, LinkGene};

type ConnKey = (u8, u32, u8, u32);
type LinkKey = (u32, u32, u32, u32);
type ConnPair<'a> = (Option<&'a ConnGene>, Option<&'a ConnGene>);
type LinkPair<'a> = (Option<&'a LinkGene>, Option<&'a LinkGene>);

const MAX_CHUNKS: usize = 64;
const MAX_CONNS_PER_CHUNK: usize = 256;
const MAX_LINKS: usize = 256;
const MAX_NN_PER_CHUNK: u32 = 256;

pub fn crossover(a: &Genome, b: &Genome, rng: &mut dyn RngCore) -> Genome {
    let mut chunks: Vec<ChunkGene> = Vec::new();
    let max_chunk_len = a.chunks.len().max(b.chunks.len()).min(MAX_CHUNKS);
    for i in 0..max_chunk_len {
        match (a.chunks.get(i), b.chunks.get(i)) {
            (Some(ca), Some(cb)) => {
                chunks.push(crossover_chunk(ca, cb, rng));
            }
            (Some(ca), None) => {
                chunks.push(ca.clone());
            }
            (None, Some(cb)) => {
                chunks.push(cb.clone());
            }
            (None, None) => break,
        }
    }

    let mut links = crossover_links(&a.links, &b.links, &chunks, rng);
    fix_link_order_tags(&mut links);
    if links.len() > MAX_LINKS {
        links.truncate(MAX_LINKS);
        fix_link_order_tags(&mut links);
    }

    Genome::new(
        chunks,
        links,
        GenomeMeta::new(a.meta.seed, a.meta.tag.clone()),
    )
    .expect("crossover produced invalid genome")
}

fn crossover_chunk(a: &ChunkGene, b: &ChunkGene, rng: &mut dyn RngCore) -> ChunkGene {
    let ni = a.ni.max(b.ni);
    let no = a.no.max(b.no);
    let nn = a.nn.max(b.nn).min(MAX_NN_PER_CHUNK);

    let mut inputs_init = bitvec![u8, Lsb0; 0; ni as usize];
    for i in 0..ni as usize {
        let bit_a = i < a.inputs_init.len() && a.inputs_init[i];
        let bit_b = i < b.inputs_init.len() && b.inputs_init[i];
        inputs_init.set(
            i,
            if rng.next_u32() & 1 == 0 {
                bit_a
            } else {
                bit_b
            },
        );
    }
    let mut outputs_init = bitvec![u8, Lsb0; 0; no as usize];
    for i in 0..no as usize {
        let bit_a = i < a.outputs_init.len() && a.outputs_init[i];
        let bit_b = i < b.outputs_init.len() && b.outputs_init[i];
        outputs_init.set(
            i,
            if rng.next_u32() & 1 == 0 {
                bit_a
            } else {
                bit_b
            },
        );
    }
    let mut internals_init = bitvec![u8, Lsb0; 0; nn as usize];
    for i in 0..nn as usize {
        let bit_a = i < a.internals_init.len() && a.internals_init[i];
        let bit_b = i < b.internals_init.len() && b.internals_init[i];
        internals_init.set(
            i,
            if rng.next_u32() & 1 == 0 {
                bit_a
            } else {
                bit_b
            },
        );
    }

    let mut map: BTreeMap<ConnKey, ConnPair> = BTreeMap::new();
    for c in &a.conns {
        map.entry((c.from_section, c.from_index, c.to_section, c.to_index))
            .or_insert((None, None))
            .0 = Some(c);
    }
    for c in &b.conns {
        map.entry((c.from_section, c.from_index, c.to_section, c.to_index))
            .or_insert((None, None))
            .1 = Some(c);
    }

    let mut conns = Vec::new();
    for ((fs, fi, ts, ti), (ca, cb)) in map {
        let trigger = match (ca, cb) {
            (Some(ac), Some(bc)) => {
                if rng.next_u32() & 1 == 0 {
                    ac.trigger
                } else {
                    bc.trigger
                }
            }
            (Some(ac), None) => ac.trigger,
            (None, Some(bc)) => bc.trigger,
            _ => unreachable!(),
        };
        let action = match (ca, cb) {
            (Some(ac), Some(bc)) => {
                if rng.next_u32() & 1 == 0 {
                    ac.action
                } else {
                    bc.action
                }
            }
            (Some(ac), None) => ac.action,
            (None, Some(bc)) => bc.action,
            _ => unreachable!(),
        };
        let order_tag = match (ca, cb) {
            (Some(ac), Some(bc)) => {
                if rng.next_u32() & 1 == 0 {
                    ac.order_tag.max(bc.order_tag)
                } else if rng.next_u32() & 1 == 0 {
                    ac.order_tag
                } else {
                    bc.order_tag
                }
            }
            (Some(ac), None) => ac.order_tag,
            (None, Some(bc)) => bc.order_tag,
            _ => 0,
        };
        conns.push(ConnGene {
            from_section: fs,
            to_section: ts,
            trigger,
            action,
            from_index: fi,
            to_index: ti,
            order_tag,
        });
    }

    conns.retain(|c| {
        let from_ok = match c.from_section {
            0 => c.from_index < ni,
            1 => c.from_index < nn,
            _ => false,
        };
        let to_ok = match c.to_section {
            1 => c.to_index < nn,
            2 => c.to_index < no,
            _ => false,
        };
        from_ok && to_ok
    });

    fix_conn_order_tags(&mut conns);
    if conns.len() > MAX_CONNS_PER_CHUNK {
        conns.truncate(MAX_CONNS_PER_CHUNK);
        fix_conn_order_tags(&mut conns);
    }

    ChunkGene {
        ni,
        no,
        nn,
        inputs_init,
        outputs_init,
        internals_init,
        conns,
    }
}

fn crossover_links(
    a_links: &[LinkGene],
    b_links: &[LinkGene],
    chunks: &[ChunkGene],
    rng: &mut dyn RngCore,
) -> Vec<LinkGene> {
    let mut map: BTreeMap<LinkKey, LinkPair> = BTreeMap::new();
    for l in a_links {
        map.entry((l.from_chunk, l.from_out_idx, l.to_chunk, l.to_in_idx))
            .or_insert((None, None))
            .0 = Some(l);
    }
    for l in b_links {
        map.entry((l.from_chunk, l.from_out_idx, l.to_chunk, l.to_in_idx))
            .or_insert((None, None))
            .1 = Some(l);
    }

    let mut links = Vec::new();
    for ((fc, fo, tc, ti), (la, lb)) in map {
        if fc as usize >= chunks.len() || tc as usize >= chunks.len() {
            continue;
        }
        let from_chunk = &chunks[fc as usize];
        let to_chunk = &chunks[tc as usize];
        if fo >= from_chunk.no || ti >= to_chunk.ni {
            continue;
        }
        let trigger = match (la, lb) {
            (Some(la), Some(lb)) => {
                if rng.next_u32() & 1 == 0 {
                    la.trigger
                } else {
                    lb.trigger
                }
            }
            (Some(la), None) => la.trigger,
            (None, Some(lb)) => lb.trigger,
            _ => unreachable!(),
        };
        let action = match (la, lb) {
            (Some(la), Some(lb)) => {
                if rng.next_u32() & 1 == 0 {
                    la.action
                } else {
                    lb.action
                }
            }
            (Some(la), None) => la.action,
            (None, Some(lb)) => lb.action,
            _ => unreachable!(),
        };
        let order_tag = match (la, lb) {
            (Some(la), Some(lb)) => {
                if rng.next_u32() & 1 == 0 {
                    la.order_tag.max(lb.order_tag)
                } else if rng.next_u32() & 1 == 0 {
                    la.order_tag
                } else {
                    lb.order_tag
                }
            }
            (Some(la), None) => la.order_tag,
            (None, Some(lb)) => lb.order_tag,
            _ => 0,
        };
        links.push(LinkGene {
            from_chunk: fc,
            from_out_idx: fo,
            trigger,
            action,
            to_chunk: tc,
            to_in_idx: ti,
            order_tag,
        });
    }

    links
}

fn fix_conn_order_tags(conns: &mut [ConnGene]) {
    conns.sort_by(|a, b| {
        (a.from_section, a.from_index, a.order_tag).cmp(&(
            b.from_section,
            b.from_index,
            b.order_tag,
        ))
    });
    let mut last_source: Option<(u8, u32)> = None;
    let mut last_tag = 0u32;
    for c in conns.iter_mut() {
        let source = (c.from_section, c.from_index);
        if Some(source) != last_source {
            last_source = Some(source);
            last_tag = c.order_tag;
        } else if c.order_tag <= last_tag {
            last_tag += 1;
            c.order_tag = last_tag;
        } else {
            last_tag = c.order_tag;
        }
    }
}

fn fix_link_order_tags(links: &mut [LinkGene]) {
    links.sort_by(|a, b| {
        (a.from_chunk, a.from_out_idx, a.order_tag).cmp(&(
            b.from_chunk,
            b.from_out_idx,
            b.order_tag,
        ))
    });
    let mut last_source: Option<(u32, u32)> = None;
    let mut last_tag = 0u32;
    for l in links.iter_mut() {
        let source = (l.from_chunk, l.from_out_idx);
        if Some(source) != last_source {
            last_source = Some(source);
            last_tag = l.order_tag;
        } else if l.order_tag <= last_tag {
            last_tag += 1;
            l.order_tag = last_tag;
        } else {
            last_tag = l.order_tag;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{ConnGene, GenomeMeta, LinkGene};

    struct SeqRng {
        vals: Vec<u32>,
        idx: usize,
    }

    impl RngCore for SeqRng {
        fn next_u32(&mut self) -> u32 {
            let v = *self.vals.get(self.idx).unwrap_or(&0);
            self.idx += 1;
            v
        }
        fn next_u64(&mut self) -> u64 {
            self.next_u32() as u64
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(4) {
                let n = self.next_u32().to_le_bytes();
                for (b, o) in chunk.iter_mut().zip(n.iter()) {
                    *b = *o;
                }
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    #[test]
    fn crossover_aligns_connections_and_links() {
        let conn_a = ConnGene::new(1, 2, 0, 0, 0, 0, 1).unwrap();
        let conn_b = ConnGene::new(1, 2, 1, 1, 0, 0, 5).unwrap();
        let link_a = LinkGene::new(0, 0, 0, 0, 1, 0, 1).unwrap();
        let link_b = LinkGene::new(0, 0, 1, 1, 1, 0, 5).unwrap();
        let chunk0_a = ChunkGene::new(
            1,
            1,
            1,
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            vec![conn_a.clone()],
        );
        let chunk0_b = ChunkGene::new(
            1,
            1,
            1,
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            vec![conn_b.clone()],
        );
        let chunk1_a = ChunkGene::new(
            1,
            0,
            0,
            bitvec![u8, Lsb0; 0],
            BitVec::new(),
            BitVec::new(),
            Vec::new(),
        );
        let chunk1_b = chunk1_a.clone();
        let a = Genome::new(
            vec![chunk0_a, chunk1_a],
            vec![link_a.clone()],
            GenomeMeta::new(0, "a".into()),
        )
        .unwrap();
        let b = Genome::new(
            vec![chunk0_b, chunk1_b],
            vec![link_b.clone()],
            GenomeMeta::new(1, "b".into()),
        )
        .unwrap();
        let mut rng = SeqRng {
            vals: vec![0; 64],
            idx: 0,
        };
        let child = crossover(&a, &b, &mut rng);
        assert_eq!(child.chunks.len(), 2);
        assert_eq!(child.chunks[0].conns.len(), 1);
        let c = &child.chunks[0].conns[0];
        assert_eq!(c.trigger, conn_a.trigger);
        assert_eq!(c.action, conn_a.action);
        assert_eq!(c.order_tag, conn_a.order_tag.max(conn_b.order_tag));
        assert_eq!(child.links.len(), 1);
        let l = &child.links[0];
        assert_eq!(l.trigger, link_a.trigger);
        assert_eq!(l.action, link_a.action);
        assert_eq!(l.order_tag, link_a.order_tag.max(link_b.order_tag));
        assert!(child.validate().is_ok());
    }
}
