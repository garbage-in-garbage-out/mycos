use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Input = 0,
    Internal = 1,
    Output = 2,
}

impl TryFrom<u8> for Section {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Section::Input),
            1 => Ok(Section::Internal),
            2 => Ok(Section::Output),
            _ => Err(Error::InvalidSection(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    On = 0,
    Off = 1,
    Toggle = 2,
}

impl TryFrom<u8> for Trigger {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Trigger::On),
            1 => Ok(Trigger::Off),
            2 => Ok(Trigger::Toggle),
            _ => Err(Error::InvalidTrigger(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Enable = 0,
    Disable = 1,
    Toggle = 2,
}

impl TryFrom<u8> for Action {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Action::Enable),
            1 => Ok(Action::Disable),
            2 => Ok(Action::Toggle),
            _ => Err(Error::InvalidAction(value)),
        }
    }
}

#[derive(Debug)]
pub struct Connection {
    pub from_section: Section,
    pub to_section: Section,
    pub trigger: Trigger,
    pub action: Action,
    pub from_index: u32,
    pub to_index: u32,
    pub order_tag: u32,
}

#[derive(Debug)]
pub struct MycosChunk {
    pub input_bits: Vec<u8>,
    pub output_bits: Vec<u8>,
    pub internal_bits: Vec<u8>,
    pub input_count: u32,
    pub output_count: u32,
    pub internal_count: u32,
    pub connections: Vec<Connection>,
}

#[derive(Debug)]
pub enum Error {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnexpectedEof,
    InvalidSection(u8),
    InvalidTrigger(u8),
    InvalidAction(u8),
    InvalidConnectionEdge { from: Section, to: Section },
    FromIndexOutOfRange { section: Section, index: u32 },
    ToIndexOutOfRange { section: Section, index: u32 },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidMagic => write!(f, "invalid magic"),
            Error::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            Error::UnexpectedEof => write!(f, "unexpected eof"),
            Error::InvalidSection(v) => write!(f, "invalid section {v}"),
            Error::InvalidTrigger(v) => write!(f, "invalid trigger {v}"),
            Error::InvalidAction(v) => write!(f, "invalid action {v}"),
            Error::InvalidConnectionEdge { from, to } => {
                write!(f, "invalid connection edge {:?}->{:?}", from, to)
            }
            Error::FromIndexOutOfRange { section, index } => {
                write!(f, "from index {index} out of range for {:?}", section)
            }
            Error::ToIndexOutOfRange { section, index } => {
                write!(f, "to index {index} out of range for {:?}", section)
            }
        }
    }
}

impl std::error::Error for Error {}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, Error> {
    if *cursor + 2 > bytes.len() {
        return Err(Error::UnexpectedEof);
    }
    let v = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor += 2;
    Ok(v)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, Error> {
    if *cursor + 4 > bytes.len() {
        return Err(Error::UnexpectedEof);
    }
    let v = u32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(v)
}

pub fn parse_chunk(bytes: &[u8]) -> Result<MycosChunk, Error> {
    if bytes.len() < 32 {
        return Err(Error::UnexpectedEof);
    }
    if &bytes[0..8] != b"MYCOSCH0" {
        return Err(Error::InvalidMagic);
    }
    let mut cursor = 8;
    let version = read_u16(bytes, &mut cursor)?;
    if version != 1 {
        return Err(Error::UnsupportedVersion(version));
    }
    let _flags = read_u16(bytes, &mut cursor)?;
    let input_count = read_u32(bytes, &mut cursor)?;
    let output_count = read_u32(bytes, &mut cursor)?;
    let internal_count = read_u32(bytes, &mut cursor)?;
    let connection_count = read_u32(bytes, &mut cursor)? as usize;
    let _reserved = read_u32(bytes, &mut cursor)?;

    let input_bytes = ((input_count + 7) / 8) as usize;
    let output_bytes = ((output_count + 7) / 8) as usize;
    let internal_bytes = ((internal_count + 7) / 8) as usize;
    let bits_total = input_bytes + output_bytes + internal_bytes;
    if cursor + bits_total > bytes.len() {
        return Err(Error::UnexpectedEof);
    }
    let input_bits = bytes[cursor..cursor + input_bytes].to_vec();
    cursor += input_bytes;
    let output_bits = bytes[cursor..cursor + output_bytes].to_vec();
    cursor += output_bytes;
    let internal_bits = bytes[cursor..cursor + internal_bytes].to_vec();
    cursor += internal_bytes;
    let pad = (4 - (bits_total % 4)) % 4;
    if cursor + pad > bytes.len() {
        return Err(Error::UnexpectedEof);
    }
    cursor += pad;

    let mut connections = Vec::with_capacity(connection_count);
    for _ in 0..connection_count {
        if cursor + 16 > bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let from_section = Section::try_from(bytes[cursor])?;
        let to_section = Section::try_from(bytes[cursor + 1])?;
        let trigger = Trigger::try_from(bytes[cursor + 2])?;
        let action = Action::try_from(bytes[cursor + 3])?;
        let from_index = u32::from_le_bytes([
            bytes[cursor + 4],
            bytes[cursor + 5],
            bytes[cursor + 6],
            bytes[cursor + 7],
        ]);
        let to_index = u32::from_le_bytes([
            bytes[cursor + 8],
            bytes[cursor + 9],
            bytes[cursor + 10],
            bytes[cursor + 11],
        ]);
        let order_tag = u32::from_le_bytes([
            bytes[cursor + 12],
            bytes[cursor + 13],
            bytes[cursor + 14],
            bytes[cursor + 15],
        ]);
        connections.push(Connection {
            from_section,
            to_section,
            trigger,
            action,
            from_index,
            to_index,
            order_tag,
        });
        cursor += 16;
    }

    Ok(MycosChunk {
        input_bits,
        output_bits,
        internal_bits,
        input_count,
        output_count,
        internal_count,
        connections,
    })
}

pub fn validate_chunk(chunk: &MycosChunk) -> Result<(), Error> {
    for conn in &chunk.connections {
        match (conn.from_section, conn.to_section) {
            (Section::Input, Section::Internal)
            | (Section::Internal, Section::Internal)
            | (Section::Internal, Section::Output) => {}
            _ => {
                return Err(Error::InvalidConnectionEdge {
                    from: conn.from_section,
                    to: conn.to_section,
                })
            }
        }
        match conn.from_section {
            Section::Input => {
                if conn.from_index >= chunk.input_count {
                    return Err(Error::FromIndexOutOfRange {
                        section: conn.from_section,
                        index: conn.from_index,
                    });
                }
            }
            Section::Internal => {
                if conn.from_index >= chunk.internal_count {
                    return Err(Error::FromIndexOutOfRange {
                        section: conn.from_section,
                        index: conn.from_index,
                    });
                }
            }
            Section::Output => {
                return Err(Error::InvalidConnectionEdge {
                    from: conn.from_section,
                    to: conn.to_section,
                })
            }
        }
        match conn.to_section {
            Section::Internal => {
                if conn.to_index >= chunk.internal_count {
                    return Err(Error::ToIndexOutOfRange {
                        section: conn.to_section,
                        index: conn.to_index,
                    });
                }
            }
            Section::Output => {
                if conn.to_index >= chunk.output_count {
                    return Err(Error::ToIndexOutOfRange {
                        section: conn.to_section,
                        index: conn.to_index,
                    });
                }
            }
            Section::Input => {
                return Err(Error::InvalidConnectionEdge {
                    from: conn.from_section,
                    to: conn.to_section,
                })
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
    }

    #[test]
    fn parse_all_fixtures() {
        for entry in fs::read_dir(fixtures()).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) == Some("myc") {
                let data = fs::read(entry.path()).unwrap();
                let chunk = parse_chunk(&data).unwrap();
                validate_chunk(&chunk).unwrap();
            }
        }
    }

    #[test]
    fn invalid_magic() {
        let path = fixtures().join("tiny_toggle.myc");
        let mut data = fs::read(path).unwrap();
        data[0] = 0;
        let err = parse_chunk(&data).unwrap_err();
        assert!(matches!(err, Error::InvalidMagic));
    }

    #[test]
    fn invalid_connection_edge() {
        let path = fixtures().join("tiny_toggle.myc");
        let mut data = fs::read(path).unwrap();
        // first connection to_section byte at 37
        data[37] = 2; // Input -> Output (invalid)
        let chunk = parse_chunk(&data).unwrap();
        assert!(matches!(
            validate_chunk(&chunk),
            Err(Error::InvalidConnectionEdge { .. })
        ));
    }

    #[test]
    fn invalid_from_index() {
        let path = fixtures().join("tiny_toggle.myc");
        let mut data = fs::read(path).unwrap();
        // first connection from_index at 40..43
        data[40] = 5; // Ni = 1, so out of range
        let chunk = parse_chunk(&data).unwrap();
        assert!(matches!(
            validate_chunk(&chunk),
            Err(Error::FromIndexOutOfRange { .. })
        ));
    }
}
