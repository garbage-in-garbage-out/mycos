use crate::scoring::ScoringSpec;

/// Mapping of task-controlled inputs and observed outputs.
#[derive(Clone, Debug)]
pub struct Io {
    pub chunk_id: u32,
    pub bit_idx: u32,
}

#[derive(Clone, Debug)]
pub struct IoMap {
    pub inputs: Vec<Io>,
    pub outputs: Vec<Io>,
}

/// Specification of a single episode: initial state and stimuli per tick with
/// expected outputs used for scoring.
#[derive(Clone, Debug)]
pub struct EpisodeSpec {
    /// Input bit vectors per tick.
    pub stimulus: Vec<Vec<u32>>,
    /// Expected output bit vectors per tick.
    pub expected: Vec<Vec<u32>>,
}

/// Complete task description.
#[derive(Clone, Debug)]
pub struct Task {
    pub name: &'static str,
    pub io: IoMap,
    pub episodes: Vec<EpisodeSpec>,
    pub tick_budget: u32,
    pub scoring: ScoringSpec,
}

/// T-00 Wire-Echo: output mirrors input on the same tick.
pub fn t00_wire_echo() -> Task {
    Task {
        name: "T-00 Wire-Echo",
        io: IoMap {
            inputs: vec![Io {
                chunk_id: 0,
                bit_idx: 0,
            }],
            outputs: vec![Io {
                chunk_id: 0,
                bit_idx: 0,
            }],
        },
        episodes: vec![
            EpisodeSpec {
                stimulus: vec![vec![1]],
                expected: vec![vec![1]],
            },
            EpisodeSpec {
                stimulus: vec![vec![0]],
                expected: vec![vec![0]],
            },
        ],
        tick_budget: 1,
        scoring: ScoringSpec::Hamming,
    }
}

/// T-01 XOR-2: outputs XOR of two inputs.
pub fn t01_xor_2() -> Task {
    Task {
        name: "T-01 XOR-2",
        io: IoMap {
            inputs: vec![
                Io {
                    chunk_id: 0,
                    bit_idx: 0,
                },
                Io {
                    chunk_id: 0,
                    bit_idx: 1,
                },
            ],
            outputs: vec![Io {
                chunk_id: 0,
                bit_idx: 2,
            }],
        },
        episodes: vec![
            EpisodeSpec {
                stimulus: vec![vec![0b00]],
                expected: vec![vec![0]],
            },
            EpisodeSpec {
                stimulus: vec![vec![0b01]],
                expected: vec![vec![1]],
            },
            EpisodeSpec {
                stimulus: vec![vec![0b10]],
                expected: vec![vec![1]],
            },
            EpisodeSpec {
                stimulus: vec![vec![0b11]],
                expected: vec![vec![0]],
            },
        ],
        tick_budget: 1,
        scoring: ScoringSpec::Hamming,
    }
}

/// T-02 SR-Latch: implements a basic set-reset latch.
pub fn t02_sr_latch() -> Task {
    Task {
        name: "T-02 SR-Latch",
        io: IoMap {
            inputs: vec![
                Io {
                    chunk_id: 0,
                    bit_idx: 0,
                }, // S
                Io {
                    chunk_id: 0,
                    bit_idx: 1,
                }, // R
            ],
            outputs: vec![Io {
                chunk_id: 0,
                bit_idx: 2,
            }], // Q
        },
        episodes: vec![
            // Set then hold
            EpisodeSpec {
                stimulus: vec![vec![0b01], vec![0b00]],
                expected: vec![vec![1], vec![1]],
            },
            // Reset then hold
            EpisodeSpec {
                stimulus: vec![vec![0b10], vec![0b00]],
                expected: vec![vec![0], vec![0]],
            },
        ],
        tick_budget: 2,
        scoring: ScoringSpec::Hamming,
    }
}

/// T-03 Pulse-Counter: counts incoming pulses modulo 4 using two output bits.
pub fn t03_pulse_counter() -> Task {
    Task {
        name: "T-03 Pulse-Counter",
        io: IoMap {
            inputs: vec![Io {
                chunk_id: 0,
                bit_idx: 0,
            }],
            outputs: vec![
                Io {
                    chunk_id: 0,
                    bit_idx: 1,
                },
                Io {
                    chunk_id: 0,
                    bit_idx: 2,
                },
            ],
        },
        episodes: vec![EpisodeSpec {
            stimulus: vec![vec![1], vec![1], vec![1]],
            expected: vec![vec![1], vec![2], vec![3]],
        }],
        tick_budget: 3,
        scoring: ScoringSpec::Hamming,
    }
}

/// T-04 Cross-Chunk Relay: relays an input from chunk 0 to an output on chunk 1 with one tick delay.
pub fn t04_cross_chunk_relay() -> Task {
    Task {
        name: "T-04 Cross-Chunk Relay",
        io: IoMap {
            inputs: vec![Io {
                chunk_id: 0,
                bit_idx: 0,
            }],
            outputs: vec![Io {
                chunk_id: 1,
                bit_idx: 0,
            }],
        },
        episodes: vec![EpisodeSpec {
            stimulus: vec![vec![1], vec![0]],
            expected: vec![vec![0], vec![1]],
        }],
        tick_budget: 2,
        scoring: ScoringSpec::Hamming,
    }
}
