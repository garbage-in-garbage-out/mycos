use crate::chunk::{Action, MycosChunk, Section, Trigger};
use crate::layout::bit_to_word;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub to_word: u32,
    pub mask: u32,
    pub action: Action,
    pub order_tag: u32,
    pub to_is_internal: bool,
    pub to_bit: u32,
}

impl Default for Effect {
    fn default() -> Self {
        Self {
            to_word: 0,
            mask: 0,
            action: Action::Enable,
            order_tag: 0,
            to_is_internal: false,
            to_bit: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CSR {
    pub offs_on: Vec<u32>,
    pub offs_off: Vec<u32>,
    pub offs_tog: Vec<u32>,
    pub effects: Vec<Effect>,
}

pub fn build_csr(chunk: &MycosChunk) -> CSR {
    let src_total = (chunk.input_count + chunk.internal_count) as usize;
    let mut offs_on = vec![0u32; src_total + 1];
    let mut offs_off = vec![0u32; src_total + 1];
    let mut offs_tog = vec![0u32; src_total + 1];

    for conn in &chunk.connections {
        let from = match conn.from_section {
            Section::Input => conn.from_index as usize,
            Section::Internal => (chunk.input_count + conn.from_index) as usize,
            Section::Output => continue,
        };
        match conn.trigger {
            Trigger::On => offs_on[from + 1] += 1,
            Trigger::Off => offs_off[from + 1] += 1,
            Trigger::Toggle => offs_tog[from + 1] += 1,
        }
    }

    for i in 0..src_total {
        offs_on[i + 1] += offs_on[i];
        offs_off[i + 1] += offs_off[i];
        offs_tog[i + 1] += offs_tog[i];
    }

    let base_off = offs_on[src_total];
    let base_tog = base_off + offs_off[src_total];

    for v in &mut offs_off {
        *v += base_off;
    }
    for v in &mut offs_tog {
        *v += base_tog;
    }

    let mut effects = vec![Effect::default(); chunk.connections.len()];
    let mut next_on = offs_on[..src_total].to_vec();
    let mut next_off = offs_off[..src_total].to_vec();
    let mut next_tog = offs_tog[..src_total].to_vec();

    for conn in &chunk.connections {
        let from = match conn.from_section {
            Section::Input => conn.from_index as usize,
            Section::Internal => (chunk.input_count + conn.from_index) as usize,
            Section::Output => continue,
        };
        let (to_word, mask) = bit_to_word(conn.to_index);
        let effect = Effect {
            to_word,
            mask,
            action: conn.action,
            order_tag: conn.order_tag,
            to_is_internal: matches!(conn.to_section, Section::Internal),
            to_bit: conn.to_index,
        };
        match conn.trigger {
            Trigger::On => {
                let idx = next_on[from] as usize;
                effects[idx] = effect;
                next_on[from] += 1;
            }
            Trigger::Off => {
                let idx = next_off[from] as usize;
                effects[idx] = effect;
                next_off[from] += 1;
            }
            Trigger::Toggle => {
                let idx = next_tog[from] as usize;
                effects[idx] = effect;
                next_tog[from] += 1;
            }
        }
    }

    for i in 0..src_total {
        let start = offs_on[i] as usize;
        let end = offs_on[i + 1] as usize;
        effects[start..end].sort_by_key(|e| e.to_word);

        let start = offs_off[i] as usize;
        let end = offs_off[i + 1] as usize;
        effects[start..end].sort_by_key(|e| e.to_word);

        let start = offs_tog[i] as usize;
        let end = offs_tog[i + 1] as usize;
        effects[start..end].sort_by_key(|e| e.to_word);
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
    use crate::chunk::{parse_chunk, validate_chunk, Trigger};
    use std::fs;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
    }

    #[test]
    fn build_all_fixtures() {
        for entry in fs::read_dir(fixtures()).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) == Some("myc") {
                let data = fs::read(entry.path()).unwrap();
                let chunk = parse_chunk(&data).unwrap();
                validate_chunk(&chunk).unwrap();
                let csr = build_csr(&chunk);
                let src_total = (chunk.input_count + chunk.internal_count) as usize;
                assert_eq!(csr.offs_on.len(), src_total + 1);
                assert_eq!(csr.offs_off.len(), src_total + 1);
                assert_eq!(csr.offs_tog.len(), src_total + 1);
                assert_eq!(csr.effects.len(), chunk.connections.len());

                let count_on = chunk
                    .connections
                    .iter()
                    .filter(|c| matches!(c.trigger, Trigger::On))
                    .count() as u32;
                let count_off = chunk
                    .connections
                    .iter()
                    .filter(|c| matches!(c.trigger, Trigger::Off))
                    .count() as u32;
                let count_tog = chunk
                    .connections
                    .iter()
                    .filter(|c| matches!(c.trigger, Trigger::Toggle))
                    .count() as u32;
                assert_eq!(csr.offs_on[src_total], count_on);
                assert_eq!(csr.offs_off[src_total], count_on + count_off);
                assert_eq!(csr.offs_tog[src_total], count_on + count_off + count_tog);

                for i in 0..src_total {
                    assert!(csr.offs_on[i] <= csr.offs_on[i + 1]);
                    assert!(csr.offs_off[i] <= csr.offs_off[i + 1]);
                    assert!(csr.offs_tog[i] <= csr.offs_tog[i + 1]);

                    let slice = &csr.effects[csr.offs_on[i] as usize..csr.offs_on[i + 1] as usize];
                    assert!(slice.windows(2).all(|w| w[0].to_word <= w[1].to_word));
                    let slice =
                        &csr.effects[csr.offs_off[i] as usize..csr.offs_off[i + 1] as usize];
                    assert!(slice.windows(2).all(|w| w[0].to_word <= w[1].to_word));
                    let slice =
                        &csr.effects[csr.offs_tog[i] as usize..csr.offs_tog[i + 1] as usize];
                    assert!(slice.windows(2).all(|w| w[0].to_word <= w[1].to_word));
                }

                let total = csr.effects.len() as u32;
                assert_eq!(csr.offs_tog[src_total], total);
                for eff in &csr.effects {
                    let (w, m) = bit_to_word(eff.to_bit);
                    assert_eq!((w, m), (eff.to_word, eff.mask));
                    if eff.to_is_internal {
                        assert!(eff.to_bit < chunk.internal_count);
                    } else {
                        assert!(eff.to_bit < chunk.output_count);
                    }
                }
            }
        }
    }
}
