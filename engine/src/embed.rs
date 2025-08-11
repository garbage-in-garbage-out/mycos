use crate::chunk::MycosChunk;
use crate::cpu_ref;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoMode {
    Alias = 0,
    Copy = 1,
}

#[derive(Debug)]
pub struct Embed {
    pub parent_chunk: u32,
    pub child_chunk: u32,
    pub gate_bit: u32,
    pub io_mode: IoMode,
    pub map_in: Vec<(u32, u32)>,  // (parent_bit, child_in_bit)
    pub map_out: Vec<(u32, u32)>, // (child_out_bit, parent_bit)
    pub gate_prev: bool,          // last observed gate state
}

#[derive(Debug)]
pub enum EmbedError {
    UnexpectedEof,
    InvalidIoMode(u8),
}

fn read_u32(data: &[u8], cursor: &mut usize) -> Result<u32, EmbedError> {
    if *cursor + 4 > data.len() {
        return Err(EmbedError::UnexpectedEof);
    }
    let v = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().unwrap());
    *cursor += 4;
    Ok(v)
}

/// Parse one or more embed records from the given byte slice.
pub fn parse_embeds(data: &[u8]) -> Result<Vec<Embed>, EmbedError> {
    let mut cursor = 0usize;
    let mut embeds = Vec::new();
    while cursor < data.len() {
        if data.len() - cursor < 20 {
            return Err(EmbedError::UnexpectedEof);
        }
        let parent_chunk = read_u32(data, &mut cursor)?;
        let child_chunk = read_u32(data, &mut cursor)?;
        let gate_bit = read_u32(data, &mut cursor)?;
        let io_mode = match data.get(cursor) {
            Some(0) => IoMode::Alias,
            Some(1) => IoMode::Copy,
            Some(&v) => return Err(EmbedError::InvalidIoMode(v)),
            None => return Err(EmbedError::UnexpectedEof),
        };
        cursor += 1;
        // skip reserved[3]
        if cursor + 3 > data.len() {
            return Err(EmbedError::UnexpectedEof);
        }
        cursor += 3;
        let map_in_count = read_u32(data, &mut cursor)? as usize;
        if cursor + map_in_count * 8 > data.len() {
            return Err(EmbedError::UnexpectedEof);
        }
        let mut map_in = Vec::with_capacity(map_in_count);
        for _ in 0..map_in_count {
            let parent_bit = read_u32(data, &mut cursor)?;
            let child_in_bit = read_u32(data, &mut cursor)?;
            map_in.push((parent_bit, child_in_bit));
        }
        let map_out_count = read_u32(data, &mut cursor)? as usize;
        if cursor + map_out_count * 8 > data.len() {
            return Err(EmbedError::UnexpectedEof);
        }
        let mut map_out = Vec::with_capacity(map_out_count);
        for _ in 0..map_out_count {
            let child_out_bit = read_u32(data, &mut cursor)?;
            let parent_bit = read_u32(data, &mut cursor)?;
            map_out.push((child_out_bit, parent_bit));
        }
        embeds.push(Embed {
            parent_chunk,
            child_chunk,
            gate_bit,
            io_mode,
            map_in,
            map_out,
            gate_prev: false,
        });
    }
    Ok(embeds)
}

fn get_bit(bytes: &[u8], idx: u32) -> bool {
    let byte = bytes[(idx / 8) as usize];
    ((byte >> (idx % 8)) & 1) != 0
}

fn set_bit_val(bytes: &mut [u8], idx: u32, val: bool) {
    let (byte_idx, bit_idx) = ((idx / 8) as usize, idx % 8);
    if val {
        bytes[byte_idx] |= 1 << bit_idx;
    } else {
        bytes[byte_idx] &= !(1 << bit_idx);
    }
}

/// Execute the child chunk if the parent's gate bit is set.
/// Child inputs/outputs are aliased to parent bits per `map_in`/`map_out`.
/// Parent connections are not evaluated here; caller should run parent logic first if needed.
pub fn execute_gated_alias(parent: &mut MycosChunk, child: &MycosChunk, embed: &Embed) {
    if !get_bit(&parent.internal_bits, embed.gate_bit) {
        return;
    }
    let mut child_clone = child.clone();
    // alias inputs from parent (internal/output) bits
    for (p_bit, c_bit) in &embed.map_in {
        let val = get_bit(&parent.internal_bits, *p_bit);
        set_bit_val(&mut child_clone.input_bits, *c_bit, val);
    }
    let (_ci, child_out, _cn) = cpu_ref::execute(&child_clone);
    for (c_bit, p_bit) in &embed.map_out {
        let val = get_bit(&child_out, *c_bit);
        set_bit_val(&mut parent.output_bits, *p_bit, val);
    }
}

/// Execute the child chunk in copy-in/copy-out mode.
/// Inputs are copied from parent when the gate bit transitions from 0→1.
/// After running the child to quiescence, outputs are copied back to the parent.
pub fn execute_gated_copy(parent: &mut MycosChunk, child: &mut MycosChunk, embed: &mut Embed) {
    let gate_now = get_bit(&parent.internal_bits, embed.gate_bit);
    if gate_now && !embed.gate_prev {
        for (p_bit, c_bit) in &embed.map_in {
            let val = get_bit(&parent.internal_bits, *p_bit);
            set_bit_val(&mut child.input_bits, *c_bit, val);
        }
    }
    if gate_now {
        let (ci, co, cn) = cpu_ref::execute(child);
        child.input_bits = ci;
        child.output_bits = co.clone();
        child.internal_bits = cn;
        for (c_bit, p_bit) in &embed.map_out {
            let val = get_bit(&co, *c_bit);
            set_bit_val(&mut parent.output_bits, *p_bit, val);
        }
    }
    embed.gate_prev = gate_now;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{parse_chunk, MycosChunk};

    #[test]
    fn parse_basic_embed() {
        // parent_chunk=0, child_chunk=1, gate_bit=0, io_mode=0
        // map_in: (1 -> 0)
        // map_out: (0 -> 0)
        let data: Vec<u8> = vec![
            0, 0, 0, 0, // parent_chunk
            1, 0, 0, 0, // child_chunk
            0, 0, 0, 0, // gate_bit
            0, // io_mode alias
            0, 0, 0, // reserved
            1, 0, 0, 0, // map_in_count
            1, 0, 0, 0, // parent_bit
            0, 0, 0, 0, // child_in_bit
            1, 0, 0, 0, // map_out_count
            0, 0, 0, 0, // child_out_bit
            0, 0, 0, 0, // parent_bit
        ];
        let embeds = parse_embeds(&data).unwrap();
        assert_eq!(embeds.len(), 1);
        let e = &embeds[0];
        assert_eq!(e.parent_chunk, 0);
        assert_eq!(e.child_chunk, 1);
        assert_eq!(e.gate_bit, 0);
        assert_eq!(e.io_mode, IoMode::Alias);
        assert!(!e.gate_prev);
        assert_eq!(e.map_in, vec![(1, 0)]);
        assert_eq!(e.map_out, vec![(0, 0)]);
    }

    #[test]
    fn gate_controls_child_alias() {
        // Parent chunk: Ni=0, No=1, Nn=2 (gate + mapped input)
        let parent = MycosChunk {
            input_bits: vec![],
            output_bits: vec![0],
            internal_bits: vec![0],
            input_count: 0,
            output_count: 1,
            internal_count: 2,
            connections: vec![],
            name: None,
            note: None,
            build_hash: None,
        };
        // Child chunk from fixture
        let data = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("fixtures")
                .join("gated_child.myc"),
        )
        .unwrap();
        let child = parse_chunk(&data).unwrap();
        let embed = Embed {
            parent_chunk: 0,
            child_chunk: 1,
            gate_bit: 0,
            io_mode: IoMode::Alias,
            map_in: vec![(1, 0)],  // parent internal[1] -> child input[0]
            map_out: vec![(0, 0)], // child output[0] -> parent output[0]
            gate_prev: false,
        };

        // Case 1: gate=0 -> child inactive
        let mut parent_state = parent.clone();
        // set child input source to 1 but gate stays 0
        parent_state.internal_bits[0] |= 1 << 1;
        execute_gated_alias(&mut parent_state, &child, &embed);
        assert_eq!(parent_state.output_bits[0], 0);

        // Case 2: gate=1 -> child active
        let mut parent_state = parent.clone();
        parent_state.internal_bits[0] |= 1 << 0; // gate on
        parent_state.internal_bits[0] |= 1 << 1; // input high
        execute_gated_alias(&mut parent_state, &child, &embed);
        assert_eq!(parent_state.output_bits[0], 1);
    }

    #[test]
    fn copy_mode_gate_edges() {
        // Parent chunk: Ni=0, No=1, Nn=2 (gate + mapped input)
        let parent = MycosChunk {
            input_bits: vec![],
            output_bits: vec![0],
            internal_bits: vec![0],
            input_count: 0,
            output_count: 1,
            internal_count: 2,
            connections: vec![],
            name: None,
            note: None,
            build_hash: None,
        };
        let data = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("fixtures")
                .join("gated_child.myc"),
        )
        .unwrap();
        let mut child = parse_chunk(&data).unwrap();
        let mut embed = Embed {
            parent_chunk: 0,
            child_chunk: 1,
            gate_bit: 0,
            io_mode: IoMode::Copy,
            map_in: vec![(1, 0)],
            map_out: vec![(0, 0)],
            gate_prev: false,
        };

        // gate closed - no activity
        let mut parent_state = parent.clone();
        parent_state.internal_bits[0] |= 1 << 1; // potential input
        execute_gated_copy(&mut parent_state, &mut child, &mut embed);
        assert_eq!(parent_state.output_bits[0], 0);
        assert!(!embed.gate_prev);

        // open gate, copy input, run child
        parent_state.internal_bits[0] |= 1 << 0; // gate on
        execute_gated_copy(&mut parent_state, &mut child, &mut embed);
        assert_eq!(parent_state.output_bits[0], 1);
        assert!(embed.gate_prev);

        // change parent input while gate open -> child output unchanged
        parent_state.internal_bits[0] &= !(1 << 1); // parent input off
        execute_gated_copy(&mut parent_state, &mut child, &mut embed);
        assert_eq!(parent_state.output_bits[0], 1);

        // close gate
        parent_state.internal_bits[0] &= !(1 << 0); // gate off
        execute_gated_copy(&mut parent_state, &mut child, &mut embed);
        assert!(!embed.gate_prev);
    }
}
