pub fn bit_to_word(bit_idx: u32) -> (u32, u32) {
    let word_idx = bit_idx >> 5; // divide by 32
    let mask = 1u32 << (bit_idx & 31); // LSB-first within word
    (word_idx, mask)
}

pub fn set_bit(word: &mut u32, mask: u32) {
    *word |= mask;
}

pub fn clr_bit(word: &mut u32, mask: u32) {
    *word &= !mask;
}

pub fn xor_bit(word: &mut u32, mask: u32) {
    *word ^= mask;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_to_word_edges() {
        assert_eq!(bit_to_word(0), (0, 1));
        assert_eq!(bit_to_word(31), (0, 1u32 << 31));
        assert_eq!(bit_to_word(32), (1, 1));
        assert_eq!(bit_to_word(63), (1, 1u32 << 31));
    }

    #[test]
    fn bit_ops_edges() {
        let mut words = [0u32; 2];
        let (w0, m0) = bit_to_word(0);
        set_bit(&mut words[w0 as usize], m0);
        assert_eq!(words[0], m0);

        let (w31, m31) = bit_to_word(31);
        set_bit(&mut words[w31 as usize], m31);
        assert_eq!(words[0], m0 | m31);

        let (w32, m32) = bit_to_word(32);
        set_bit(&mut words[w32 as usize], m32);
        assert_eq!(words[1], m32);

        xor_bit(&mut words[w0 as usize], m0);
        assert_eq!(words[0], m31);

        clr_bit(&mut words[w31 as usize], m31);
        assert_eq!(words[0], 0);
    }
}
