use crate::chunk::{Action, MycosChunk, Trigger};
use crate::csr::{Effect, CSR};
use crate::layout::bit_to_word;

#[derive(Debug)]
pub struct Link {
    pub from_chunk: u32,
    pub from_out_idx: u32,
    pub trigger: Trigger,
    pub action: Action,
    pub to_chunk: u32,
    pub to_in_idx: u32,
    pub order_tag: u32,
}

#[derive(Debug)]
pub enum LinkError {
    UnexpectedEof,
    InvalidTrigger(u8),
    InvalidAction(u8),
    FromChunkOutOfRange(u32),
    ToChunkOutOfRange(u32),
    FromOutIndexOutOfRange { chunk: u32, index: u32 },
    ToInIndexOutOfRange { chunk: u32, index: u32 },
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::UnexpectedEof => write!(f, "unexpected eof"),
            LinkError::InvalidTrigger(v) => write!(f, "invalid trigger {v}"),
            LinkError::InvalidAction(v) => write!(f, "invalid action {v}"),
            LinkError::FromChunkOutOfRange(c) => write!(f, "from chunk {c} out of range"),
            LinkError::ToChunkOutOfRange(c) => write!(f, "to chunk {c} out of range"),
            LinkError::FromOutIndexOutOfRange { chunk, index } => {
                write!(f, "from chunk {chunk} output index {index} out of range")
            }
            LinkError::ToInIndexOutOfRange { chunk, index } => {
                write!(f, "to chunk {chunk} input index {index} out of range")
            }
        }
    }
}

impl std::error::Error for LinkError {}

pub fn parse_links(data: &[u8]) -> Result<Vec<Link>, LinkError> {
    const LINK_BYTES: usize = 24;
    if data.len() % LINK_BYTES != 0 {
        return Err(LinkError::UnexpectedEof);
    }
    let mut links = Vec::with_capacity(data.len() / LINK_BYTES);
    for chunk in data.chunks_exact(LINK_BYTES) {
        let from_chunk = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let from_out_idx = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
        let trigger =
            Trigger::try_from(chunk[8]).map_err(|_| LinkError::InvalidTrigger(chunk[8]))?;
        let action = Action::try_from(chunk[9]).map_err(|_| LinkError::InvalidAction(chunk[9]))?;
        // chunk[10..12] reserved
        let to_chunk = u32::from_le_bytes(chunk[12..16].try_into().unwrap());
        let to_in_idx = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        let order_tag = u32::from_le_bytes(chunk[20..24].try_into().unwrap());
        links.push(Link {
            from_chunk,
            from_out_idx,
            trigger,
            action,
            to_chunk,
            to_in_idx,
            order_tag,
        });
    }
    Ok(links)
}

pub fn validate_links(links: &[Link], chunks: &[MycosChunk]) -> Result<(), LinkError> {
    for link in links {
        let from_chunk = chunks
            .get(link.from_chunk as usize)
            .ok_or(LinkError::FromChunkOutOfRange(link.from_chunk))?;
        let to_chunk = chunks
            .get(link.to_chunk as usize)
            .ok_or(LinkError::ToChunkOutOfRange(link.to_chunk))?;
        if link.from_out_idx >= from_chunk.output_count {
            return Err(LinkError::FromOutIndexOutOfRange {
                chunk: link.from_chunk,
                index: link.from_out_idx,
            });
        }
        if link.to_in_idx >= to_chunk.input_count {
            return Err(LinkError::ToInIndexOutOfRange {
                chunk: link.to_chunk,
                index: link.to_in_idx,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkOffsets {
    pub input: u32,
    pub output: u32,
    pub internal: u32,
}

/// Compute base offsets for Inputs, Outputs, and Internals of each chunk.
///
/// Offsets are expressed in **bits** relative to the start of the global
/// device buffers for each section. They are used to map per-chunk bit indices
/// to global identifiers when linking multiple chunks together.
pub fn compute_base_offsets(chunks: &[MycosChunk]) -> Vec<ChunkOffsets> {
    let mut offs = Vec::with_capacity(chunks.len());
    let mut base_in = 0u32;
    let mut base_out = 0u32;
    let mut base_int = 0u32;
    for ch in chunks {
        offs.push(ChunkOffsets {
            input: base_in,
            output: base_out,
            internal: base_int,
        });
        base_in += ch.input_count;
        base_out += ch.output_count;
        base_int += ch.internal_count;
    }
    offs
}

/// Build a CSR adjacency for inter-chunk links using global bit ids.
///
/// Sources are chunk **outputs**; targets are **inputs** of other chunks.
/// The returned `CSR` can be processed exactly like intra-chunk connections
/// during expansion.
pub fn build_link_csr(links: &[Link], chunks: &[MycosChunk]) -> CSR {
    let offsets = compute_base_offsets(chunks);
    let out_total = chunks.iter().map(|c| c.output_count).sum::<u32>() as usize;

    let mut offs_on = vec![0u32; out_total + 1];
    let mut offs_off = vec![0u32; out_total + 1];
    let mut offs_tog = vec![0u32; out_total + 1];

    for link in links {
        let from = offsets[link.from_chunk as usize].output + link.from_out_idx;
        let from = from as usize;
        match link.trigger {
            Trigger::On => offs_on[from + 1] += 1,
            Trigger::Off => offs_off[from + 1] += 1,
            Trigger::Toggle => offs_tog[from + 1] += 1,
        }
    }

    for i in 0..out_total {
        offs_on[i + 1] += offs_on[i];
        offs_off[i + 1] += offs_off[i];
        offs_tog[i + 1] += offs_tog[i];
    }

    let base_off = offs_on[out_total];
    let base_tog = base_off + offs_off[out_total];
    for v in &mut offs_off {
        *v += base_off;
    }
    for v in &mut offs_tog {
        *v += base_tog;
    }

    let mut effects = vec![Effect::default(); links.len()];
    let mut next_on = offs_on[..out_total].to_vec();
    let mut next_off = offs_off[..out_total].to_vec();
    let mut next_tog = offs_tog[..out_total].to_vec();

    for link in links {
        let from = offsets[link.from_chunk as usize].output + link.from_out_idx;
        let to = offsets[link.to_chunk as usize].input + link.to_in_idx;
        let (to_word, mask) = bit_to_word(to);
        let effect = Effect {
            to_word,
            mask,
            action: link.action,
            order_tag: link.order_tag,
            to_is_internal: false,
            to_bit: to,
        };
        match link.trigger {
            Trigger::On => {
                let idx = next_on[from as usize] as usize;
                effects[idx] = effect;
                next_on[from as usize] += 1;
            }
            Trigger::Off => {
                let idx = next_off[from as usize] as usize;
                effects[idx] = effect;
                next_off[from as usize] += 1;
            }
            Trigger::Toggle => {
                let idx = next_tog[from as usize] as usize;
                effects[idx] = effect;
                next_tog[from as usize] += 1;
            }
        }
    }

    for i in 0..out_total {
        let start = offs_on[i] as usize;
        let end = offs_on[i + 1] as usize;
        effects[start..end].sort_by(|a, b| {
            a.to_word
                .cmp(&b.to_word)
                .then(a.order_tag.cmp(&b.order_tag))
        });

        let start = offs_off[i] as usize;
        let end = offs_off[i + 1] as usize;
        effects[start..end].sort_by(|a, b| {
            a.to_word
                .cmp(&b.to_word)
                .then(a.order_tag.cmp(&b.order_tag))
        });

        let start = offs_tog[i] as usize;
        let end = offs_tog[i + 1] as usize;
        effects[start..end].sort_by(|a, b| {
            a.to_word
                .cmp(&b.to_word)
                .then(a.order_tag.cmp(&b.order_tag))
        });
    }

    CSR {
        offs_on,
        offs_off,
        offs_tog,
        effects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{parse_chunk, validate_chunk};
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
    }

    const LINKS_BASIC: [u8; 24] = [
        0, 0, 0, 0, // from_chunk
        0, 0, 0, 0, // from_out_idx
        0, // trigger On
        0, // action Enable
        0, 0, // reserved
        1, 0, 0, 0, // to_chunk
        0, 0, 0, 0, // to_in_idx
        0, 0, 0, 0, // order_tag
    ];

    #[test]
    fn parse_and_validate_basic() {
        let links = parse_links(&LINKS_BASIC).unwrap();
        assert_eq!(links.len(), 1);
        let chunk_a_data = std::fs::read(fixtures().join("tiny_toggle.myc")).unwrap();
        let chunk_b_data = std::fs::read(fixtures().join("noop.myc")).unwrap();
        let chunk_a = parse_chunk(&chunk_a_data).unwrap();
        let chunk_b = parse_chunk(&chunk_b_data).unwrap();
        validate_chunk(&chunk_a).unwrap();
        validate_chunk(&chunk_b).unwrap();
        let chunks = vec![chunk_a, chunk_b];
        validate_links(&links, &chunks).unwrap();
    }

    #[test]
    fn invalid_to_in_index() {
        let mut data = LINKS_BASIC.to_vec();
        // set to_in_idx (offset 16..20) to 5, beyond noop.myc Ni=2
        data[16] = 5;
        data[17] = 0;
        data[18] = 0;
        data[19] = 0;
        let links = parse_links(&data).unwrap();
        let chunk_a_data = std::fs::read(fixtures().join("tiny_toggle.myc")).unwrap();
        let chunk_b_data = std::fs::read(fixtures().join("noop.myc")).unwrap();
        let chunk_a = parse_chunk(&chunk_a_data).unwrap();
        let chunk_b = parse_chunk(&chunk_b_data).unwrap();
        validate_chunk(&chunk_a).unwrap();
        validate_chunk(&chunk_b).unwrap();
        let chunks = vec![chunk_a, chunk_b];
        assert!(matches!(
            validate_links(&links, &chunks),
            Err(LinkError::ToInIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn compute_offsets_and_map() {
        let chunk_a_data = std::fs::read(fixtures().join("tiny_toggle.myc")).unwrap();
        let chunk_b_data = std::fs::read(fixtures().join("noop.myc")).unwrap();
        let chunk_a = parse_chunk(&chunk_a_data).unwrap();
        let chunk_b = parse_chunk(&chunk_b_data).unwrap();
        let chunks = vec![chunk_a, chunk_b];
        let offs = compute_base_offsets(&chunks);
        assert_eq!(offs.len(), 2);
        assert_eq!(
            offs[0],
            ChunkOffsets {
                input: 0,
                output: 0,
                internal: 0
            }
        );
        assert_eq!(
            offs[1],
            ChunkOffsets {
                input: 1,
                output: 1,
                internal: 1
            }
        );

        let links = parse_links(&LINKS_BASIC).unwrap();
        let csr = build_link_csr(&links, &chunks);
        assert_eq!(csr.effects.len(), 1);
        let effect = csr.effects[0];
        // to_chunk (1) input index 0 -> global input bit 1
        assert_eq!(effect.to_bit, 1);
        let (word, mask) = crate::layout::bit_to_word(1);
        assert_eq!(effect.to_word, word);
        assert_eq!(effect.mask, mask);
        assert!(!effect.to_is_internal);

        // from_chunk (0) output index 0 -> global output bit 0
        assert_eq!(csr.offs_on[0], 0);
        assert_eq!(csr.offs_on[1], 1);
    }
}
