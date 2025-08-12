use bitvec::prelude::*;
use serde::{Deserialize, Serialize};

/// Top-level genome structure containing chunk genes and links between them.
#[derive(Serialize, Deserialize, Clone)]
pub struct Genome {
    pub chunks: Vec<ChunkGene>,
    pub links: Vec<LinkGene>,
    pub meta: GenomeMeta,
}

impl Genome {
    /// Create a new genome, validating and sorting its contents.
    pub fn new(
        mut chunks: Vec<ChunkGene>,
        mut links: Vec<LinkGene>,
        meta: GenomeMeta,
    ) -> Result<Self, ValidationError> {
        let genome = Self {
            chunks: chunks.clone(),
            links: links.clone(),
            meta,
        };
        // Validate before sorting to surface errors early.
        genome.validate_chunks_and_links(&chunks, &links)?;
        // Sort after successful validation.
        Genome::sort_internal(&mut chunks, &mut links);
        Ok(Self {
            chunks,
            links,
            meta: genome.meta,
        })
    }

    fn validate_chunks_and_links(
        &self,
        chunks: &[ChunkGene],
        links: &[LinkGene],
    ) -> Result<(), ValidationError> {
        for (i, chunk) in chunks.iter().enumerate() {
            chunk.validate().map_err(|e| e.in_chunk(i as u32))?;
        }
        for link in links {
            link.validate()?;
            if (link.from_chunk as usize) >= chunks.len() {
                return Err(ValidationError::InvalidLinkFromChunk(link.from_chunk));
            }
            if (link.to_chunk as usize) >= chunks.len() {
                return Err(ValidationError::InvalidLinkToChunk(link.to_chunk));
            }
            let from_chunk = &chunks[link.from_chunk as usize];
            if link.from_out_idx >= from_chunk.no {
                return Err(ValidationError::InvalidLinkFromIndex {
                    chunk: link.from_chunk,
                    index: link.from_out_idx,
                });
            }
            let to_chunk = &chunks[link.to_chunk as usize];
            if link.to_in_idx >= to_chunk.ni {
                return Err(ValidationError::InvalidLinkToIndex {
                    chunk: link.to_chunk,
                    index: link.to_in_idx,
                });
            }
        }
        Ok(())
    }

    fn sort_internal(chunks: &mut [ChunkGene], links: &mut [LinkGene]) {
        for chunk in chunks {
            chunk.sort();
        }
        links.sort_by(|a, b| {
            (a.from_chunk, a.from_out_idx, a.order_tag).cmp(&(
                b.from_chunk,
                b.from_out_idx,
                b.order_tag,
            ))
        });
    }

    /// Validate the genome after construction.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_chunks_and_links(&self.chunks, &self.links)
    }

    /// Sort connections and links according to canonical rules.
    pub fn sort(&mut self) {
        Genome::sort_internal(&mut self.chunks, &mut self.links);
    }
}

/// Metadata associated with a genome.
#[derive(Serialize, Deserialize, Clone)]
pub struct GenomeMeta {
    pub seed: u64,
    pub tag: String,
}

impl GenomeMeta {
    pub fn new(seed: u64, tag: String) -> Self {
        Self { seed, tag }
    }
}

/// Gene describing a single chunk in the genome.
#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkGene {
    pub ni: u32,
    pub no: u32,
    pub nn: u32,
    pub inputs_init: BitVec<u8, Lsb0>,
    pub outputs_init: BitVec<u8, Lsb0>,
    pub internals_init: BitVec<u8, Lsb0>,
    pub conns: Vec<ConnGene>,
}

impl ChunkGene {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ni: u32,
        no: u32,
        nn: u32,
        inputs_init: BitVec<u8, Lsb0>,
        outputs_init: BitVec<u8, Lsb0>,
        internals_init: BitVec<u8, Lsb0>,
        conns: Vec<ConnGene>,
    ) -> Self {
        Self {
            ni,
            no,
            nn,
            inputs_init,
            outputs_init,
            internals_init,
            conns,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.inputs_init.len() != self.ni as usize {
            return Err(ValidationError::InputsLenMismatch {
                expected: self.ni,
                actual: self.inputs_init.len(),
            });
        }
        if self.outputs_init.len() != self.no as usize {
            return Err(ValidationError::OutputsLenMismatch {
                expected: self.no,
                actual: self.outputs_init.len(),
            });
        }
        if self.internals_init.len() != self.nn as usize {
            return Err(ValidationError::InternalsLenMismatch {
                expected: self.nn,
                actual: self.internals_init.len(),
            });
        }
        for conn in &self.conns {
            conn.validate()?;
            match conn.from_section {
                0 => {
                    if conn.from_index >= self.ni {
                        return Err(ValidationError::FromIndexOutOfRange {
                            section: conn.from_section,
                            index: conn.from_index,
                        });
                    }
                }
                1 => {
                    if conn.from_index >= self.nn {
                        return Err(ValidationError::FromIndexOutOfRange {
                            section: conn.from_section,
                            index: conn.from_index,
                        });
                    }
                }
                _ => {
                    return Err(ValidationError::InvalidConnEdge {
                        from_section: conn.from_section,
                        to_section: conn.to_section,
                    })
                }
            }
            match conn.to_section {
                1 => {
                    if conn.to_index >= self.nn {
                        return Err(ValidationError::ToIndexOutOfRange {
                            section: conn.to_section,
                            index: conn.to_index,
                        });
                    }
                }
                2 => {
                    if conn.to_index >= self.no {
                        return Err(ValidationError::ToIndexOutOfRange {
                            section: conn.to_section,
                            index: conn.to_index,
                        });
                    }
                }
                _ => {
                    return Err(ValidationError::InvalidConnEdge {
                        from_section: conn.from_section,
                        to_section: conn.to_section,
                    })
                }
            }
        }
        Ok(())
    }

    pub fn sort(&mut self) {
        self.conns.sort_by(|a, b| {
            (a.from_section, a.from_index, a.order_tag).cmp(&(
                b.from_section,
                b.from_index,
                b.order_tag,
            ))
        });
    }
}

/// Gene describing a connection within a chunk.
#[derive(Serialize, Deserialize, Clone)]
pub struct ConnGene {
    pub from_section: u8,
    pub to_section: u8,
    pub trigger: u8,
    pub action: u8,
    pub from_index: u32,
    pub to_index: u32,
    pub order_tag: u32,
}

impl ConnGene {
    pub fn new(
        from_section: u8,
        to_section: u8,
        trigger: u8,
        action: u8,
        from_index: u32,
        to_index: u32,
        order_tag: u32,
    ) -> Result<Self, ValidationError> {
        let conn = Self {
            from_section,
            to_section,
            trigger,
            action,
            from_index,
            to_index,
            order_tag,
        };
        conn.validate()?;
        Ok(conn)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.trigger > 2 {
            return Err(ValidationError::InvalidTrigger(self.trigger));
        }
        if self.action > 2 {
            return Err(ValidationError::InvalidAction(self.action));
        }
        match (self.from_section, self.to_section) {
            (0, 1) | (1, 1 | 2) => Ok(()),
            _ => Err(ValidationError::InvalidConnEdge {
                from_section: self.from_section,
                to_section: self.to_section,
            }),
        }
    }
}

/// Gene describing a link between chunks.
#[derive(Serialize, Deserialize, Clone)]
pub struct LinkGene {
    pub from_chunk: u32,
    pub from_out_idx: u32,
    pub trigger: u8,
    pub action: u8,
    pub to_chunk: u32,
    pub to_in_idx: u32,
    pub order_tag: u32,
}

impl LinkGene {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from_chunk: u32,
        from_out_idx: u32,
        trigger: u8,
        action: u8,
        to_chunk: u32,
        to_in_idx: u32,
        order_tag: u32,
    ) -> Result<Self, ValidationError> {
        let link = Self {
            from_chunk,
            from_out_idx,
            trigger,
            action,
            to_chunk,
            to_in_idx,
            order_tag,
        };
        link.validate()?;
        Ok(link)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.trigger > 2 {
            return Err(ValidationError::InvalidTrigger(self.trigger));
        }
        if self.action > 2 {
            return Err(ValidationError::InvalidAction(self.action));
        }
        Ok(())
    }
}

/// Errors that can occur during validation of genome structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    InvalidConnEdge { from_section: u8, to_section: u8 },
    FromIndexOutOfRange { section: u8, index: u32 },
    ToIndexOutOfRange { section: u8, index: u32 },
    InputsLenMismatch { expected: u32, actual: usize },
    OutputsLenMismatch { expected: u32, actual: usize },
    InternalsLenMismatch { expected: u32, actual: usize },
    InvalidLinkFromChunk(u32),
    InvalidLinkToChunk(u32),
    InvalidLinkFromIndex { chunk: u32, index: u32 },
    InvalidLinkToIndex { chunk: u32, index: u32 },
    InvalidTrigger(u8),
    InvalidAction(u8),
}

impl ValidationError {
    fn in_chunk(self, _chunk: u32) -> Self {
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ValidationError::*;
        match self {
            InvalidConnEdge {
                from_section,
                to_section,
            } => {
                write!(
                    f,
                    "invalid connection edge {}->{}",
                    from_section, to_section
                )
            }
            FromIndexOutOfRange { section, index } => {
                write!(
                    f,
                    "from index {} out of range for section {}",
                    index, section
                )
            }
            ToIndexOutOfRange { section, index } => {
                write!(f, "to index {} out of range for section {}", index, section)
            }
            InputsLenMismatch { expected, actual } => {
                write!(f, "inputs_init length {} != {}", actual, expected)
            }
            OutputsLenMismatch { expected, actual } => {
                write!(f, "outputs_init length {} != {}", actual, expected)
            }
            InternalsLenMismatch { expected, actual } => {
                write!(f, "internals_init length {} != {}", actual, expected)
            }
            InvalidLinkFromChunk(c) => write!(f, "link from_chunk {} out of range", c),
            InvalidLinkToChunk(c) => write!(f, "link to_chunk {} out of range", c),
            InvalidLinkFromIndex { chunk, index } => {
                write!(
                    f,
                    "link from_out_idx {} out of range for chunk {}",
                    index, chunk
                )
            }
            InvalidLinkToIndex { chunk, index } => {
                write!(
                    f,
                    "link to_in_idx {} out of range for chunk {}",
                    index, chunk
                )
            }
            InvalidTrigger(t) => write!(f, "invalid trigger {}", t),
            InvalidAction(a) => write!(f, "invalid action {}", a),
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conn_gene_validation() {
        // valid Input -> Internal
        assert!(ConnGene::new(0, 1, 0, 0, 0, 0, 0).is_ok());
        // invalid Input -> Output
        assert!(matches!(
            ConnGene::new(0, 2, 0, 0, 0, 0, 0),
            Err(ValidationError::InvalidConnEdge { .. })
        ));
    }

    #[test]
    fn chunk_gene_validation() {
        let conn = ConnGene::new(0, 1, 0, 0, 0, 0, 0).unwrap();
        let chunk = ChunkGene::new(
            1,
            0,
            1,
            bitvec![u8, Lsb0; 0],
            BitVec::new(),
            bitvec![u8, Lsb0; 0],
            vec![conn],
        );
        assert!(chunk.validate().is_ok());

        let bad_conn = ConnGene::new(0, 1, 0, 0, 1, 0, 0).unwrap();
        let bad_chunk = ChunkGene::new(
            1,
            0,
            1,
            bitvec![u8, Lsb0; 0],
            BitVec::new(),
            bitvec![u8, Lsb0; 0],
            vec![bad_conn],
        );
        assert!(matches!(
            bad_chunk.validate(),
            Err(ValidationError::FromIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn genome_validate_and_sort() {
        let conn_a1 = ConnGene::new(1, 2, 0, 0, 0, 0, 1).unwrap();
        let conn_a0 = ConnGene::new(1, 2, 0, 0, 0, 0, 0).unwrap();
        let chunk_a = ChunkGene::new(
            0,
            1,
            1,
            BitVec::new(),
            bitvec![u8, Lsb0; 0],
            bitvec![u8, Lsb0; 0],
            vec![conn_a1.clone(), conn_a0.clone()],
        );

        let chunk_b = ChunkGene::new(
            1,
            0,
            0,
            bitvec![u8, Lsb0; 0],
            BitVec::new(),
            BitVec::new(),
            Vec::new(),
        );

        let link = LinkGene::new(0, 0, 0, 0, 1, 0, 1).unwrap();

        let genome = Genome::new(
            vec![chunk_a, chunk_b],
            vec![link],
            GenomeMeta::new(0, "tag".into()),
        )
        .unwrap();

        // connections sorted by order_tag
        assert_eq!(genome.chunks[0].conns[0].order_tag, 0);
        assert_eq!(genome.chunks[0].conns[1].order_tag, 1);

        // links sorted
        assert_eq!(genome.links[0].order_tag, 1);
        assert!(genome.validate().is_ok());
    }
}
