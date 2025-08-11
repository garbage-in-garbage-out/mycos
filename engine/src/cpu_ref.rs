use crate::chunk::{Action, MycosChunk, Section, Trigger};
use crate::layout::{bit_to_word, clr_bit, set_bit, xor_bit};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    section: Section,
    index: u32,
    edge: Edge,
}

fn get_bit(words: &[u32], idx: u32) -> bool {
    let (w, m) = bit_to_word(idx);
    (words[w as usize] & m) != 0
}

fn set_bit_action(words: &mut [u32], idx: u32, action: Action) {
    let (w, m) = bit_to_word(idx);
    match action {
        Action::Enable => set_bit(&mut words[w as usize], m),
        Action::Disable => clr_bit(&mut words[w as usize], m),
        Action::Toggle => xor_bit(&mut words[w as usize], m),
    }
}

fn bytes_to_words(bytes: &[u8], bit_count: u32) -> Vec<u32> {
    let word_count = bit_count.div_ceil(32) as usize;
    let mut out = vec![0u32; word_count];
    for bit in 0..bit_count {
        let b = bytes[(bit / 8) as usize];
        if (b >> (bit % 8)) & 1 != 0 {
            let (w, m) = bit_to_word(bit);
            set_bit(&mut out[w as usize], m);
        }
    }
    out
}

fn words_to_bytes(words: &[u32], bit_count: u32) -> Vec<u8> {
    let byte_count = (bit_count as usize).div_ceil(8);
    let mut out = vec![0u8; byte_count];
    for bit in 0..bit_count {
        let (w, m) = bit_to_word(bit);
        if (words[w as usize] & m) != 0 {
            out[(bit / 8) as usize] |= 1 << (bit % 8);
        }
    }
    out
}

/// Execute the given chunk on the CPU until quiescence.
/// Returns final Input, Output, Internal bit vectors (as bytes).
pub fn execute(chunk: &MycosChunk) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let input = bytes_to_words(&chunk.input_bits, chunk.input_count);
    let mut output = bytes_to_words(&chunk.output_bits, chunk.output_count);
    let mut internal = bytes_to_words(&chunk.internal_bits, chunk.internal_count);

    let mut q = VecDeque::new();
    // seed queue with initial set bits (On + Toggle)
    for i in 0..chunk.input_count {
        if get_bit(&input, i) {
            q.push_back(Event {
                section: Section::Input,
                index: i,
                edge: Edge::On,
            });
            q.push_back(Event {
                section: Section::Input,
                index: i,
                edge: Edge::Toggle,
            });
        }
    }
    for i in 0..chunk.internal_count {
        if get_bit(&internal, i) {
            q.push_back(Event {
                section: Section::Internal,
                index: i,
                edge: Edge::On,
            });
            q.push_back(Event {
                section: Section::Internal,
                index: i,
                edge: Edge::Toggle,
            });
        }
    }

    const MAX_EFFECTS: usize = 5_000_000;
    let mut effects_applied = 0usize;

    while let Some(ev) = q.pop_front() {
        if effects_applied >= MAX_EFFECTS {
            break;
        }
        // gather proposals
        let mut proposals: Vec<((Section, u32), (Action, u32))> = Vec::new();
        for conn in &chunk.connections {
            if conn.from_section != ev.section || conn.from_index != ev.index {
                continue;
            }
            let trigger_match = matches!(
                (ev.edge, conn.trigger),
                (Edge::On, Trigger::On)
                    | (Edge::Off, Trigger::Off)
                    | (Edge::Toggle, Trigger::Toggle)
            );
            if !trigger_match {
                continue;
            }
            let key = (conn.to_section, conn.to_index);
            if let Some((_, (act, tag))) = proposals.iter_mut().find(|(k, _)| *k == key) {
                if conn.order_tag >= *tag {
                    *act = conn.action;
                    *tag = conn.order_tag;
                }
            } else {
                proposals.push((key, (conn.action, conn.order_tag)));
            }
        }

        for ((to_section, to_index), (action, _tag)) in proposals {
            let words = match to_section {
                Section::Internal => &mut internal,
                Section::Output => &mut output,
                Section::Input => continue, // invalid target
            };
            let before = get_bit(words, to_index);
            set_bit_action(words, to_index, action);
            let after = get_bit(words, to_index);
            effects_applied += 1;
            if before != after && matches!(to_section, Section::Internal) {
                let edge = if after { Edge::On } else { Edge::Off };
                q.push_back(Event {
                    section: Section::Internal,
                    index: to_index,
                    edge,
                });
                q.push_back(Event {
                    section: Section::Internal,
                    index: to_index,
                    edge: Edge::Toggle,
                });
            }
        }
    }

    (
        words_to_bytes(&input, chunk.input_count),
        words_to_bytes(&output, chunk.output_count),
        words_to_bytes(&internal, chunk.internal_count),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::parse_chunk;
    use std::fs;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
    }

    #[test]
    fn tiny_toggle_propagates() {
        let data = fs::read(fixtures().join("tiny_toggle.myc")).unwrap();
        let mut chunk = parse_chunk(&data).unwrap();
        // simulate input bit going high
        if !chunk.input_bits.is_empty() {
            chunk.input_bits[0] = 1;
        }
        let (_i, o, n) = execute(&chunk);
        assert_eq!(n[0], 1);
        assert_eq!(o[0], 1);
    }
}
