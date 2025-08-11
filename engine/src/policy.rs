use crate::chunk::Action;
use serde::Serialize;

/// Policy applied when guards trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Policy {
    /// Revert to last stable state seen before the cycle.
    FreezeLastStable,
    /// Resolve competing effects with commutative precedence.
    ClampCommutative,
    /// Toggle bits once based on cycle parity.
    ParityQuench,
}

/// Result of executing with guards and policies applied.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub rounds: u32,
    pub effects_applied: u64,
    pub oscillator: bool,
    pub period: u32,
    pub policy: Option<Policy>,
    pub internals: Vec<u32>,
    pub outputs: Vec<u32>,
}

/// Ring buffer based cycle detector using 128-bit hashes of the internal state.
pub struct CycleDetector {
    ring: Vec<u128>,
    pos: usize,
}

impl CycleDetector {
    pub fn new(window: usize) -> Self {
        Self {
            ring: vec![0; window],
            pos: 0,
        }
    }

    /// Observe a new internal state. Returns `Some(period)` when a cycle is
    /// detected, otherwise `None`.
    pub fn observe(&mut self, state: &[u32]) -> Option<u32> {
        let h = hash_state(state);
        for i in 0..self.ring.len() {
            if self.ring[i] == h {
                let period = (self.ring.len() + self.pos - i) % self.ring.len();
                self.ring[self.pos] = h;
                self.pos = (self.pos + 1) % self.ring.len();
                return Some(period as u32);
            }
        }
        self.ring[self.pos] = h;
        self.pos = (self.pos + 1) % self.ring.len();
        None
    }
}

/// Simple 128-bit FNV-1a style hash matching the GPU implementation.
fn hash_state(words: &[u32]) -> u128 {
    let mut h0: u32 = 0x811c9dc5;
    let mut h1: u32 = 0x811c9dc5;
    let mut h2: u32 = 0x811c9dc5;
    let mut h3: u32 = 0x811c9dc5;
    for &w in words {
        h0 = (h0 ^ w).wrapping_mul(0x0100_0193);
        h1 = (h1 ^ (w >> 1)).wrapping_mul(0x0100_0193);
        h2 = (h2 ^ (w >> 2)).wrapping_mul(0x0100_0193);
        h3 = (h3 ^ (w >> 3)).wrapping_mul(0x0100_0193);
    }
    ((h0 as u128) << 96) | ((h1 as u128) << 64) | ((h2 as u128) << 32) | (h3 as u128)
}

/// Apply the `freeze_last_stable` policy by restoring `curr` to `stable`.
pub fn freeze_last_stable(curr: &mut [u32], stable: &[u32]) {
    for (c, s) in curr.iter_mut().zip(stable.iter()) {
        *c = *s;
    }
}

/// Resolve a set of `Action`s using commutative precedence.
pub fn clamp_commutative(actions: &[Action]) -> Option<Action> {
    let mut disable = false;
    let mut enable = false;
    let mut toggle_count = 0u32;
    for &a in actions {
        match a {
            Action::Disable => disable = true,
            Action::Enable => enable = true,
            Action::Toggle => toggle_count += 1,
        }
    }
    if disable {
        Some(Action::Disable)
    } else if enable {
        Some(Action::Enable)
    } else if toggle_count % 2 == 1 {
        Some(Action::Toggle)
    } else {
        None
    }
}

/// Apply the `parity_quench` policy which toggles bits based on cycle parity.
pub fn parity_quench(curr: &mut [u32], period: u32) {
    if period % 2 == 1 {
        for w in curr.iter_mut() {
            *w = !*w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn cycle_detection_and_freeze_snapshot() {
        let mut det = CycleDetector::new(8);
        let mut state = vec![1u32];
        let stable = vec![0u32];
        assert!(det.observe(&state).is_none());
        state[0] = 3;
        assert!(det.observe(&state).is_none());
        state[0] = 2;
        assert!(det.observe(&state).is_none());
        state[0] = 1;
        let period = det.observe(&state).unwrap();
        let mut final_state = state.clone();
        freeze_last_stable(&mut final_state, &stable);
        let res = json!({
            "period": period,
            "final_state": final_state,
        });
        assert_json_snapshot!("freeze_last_stable", res);
    }
}
