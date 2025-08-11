use crate::chunk::{Action, MycosChunk, Trigger};

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
}
