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

pub const HEADER_BYTES: usize = 32;

/// Compute byte offsets of each bit section (Inputs, Outputs, Internals)
/// according to the binary layout specification.
///
/// Offsets are from the start of the chunk binary. Inputs begin immediately
/// after the 32-byte header; subsequent sections follow sequentially using
/// byte counts rounded up to the next byte.
pub fn section_offsets(ni: u32, no: u32, nn: u32) -> (usize, usize, usize) {
    let input_bytes = (ni as usize).div_ceil(8);
    let output_bytes = (no as usize).div_ceil(8);
    let _internal_bytes = (nn as usize).div_ceil(8); // for symmetry, may be useful to callers

    let input = HEADER_BYTES;
    let output = input + input_bytes;
    let internal = output + output_bytes;
    (input, output, internal)
}

/// Starting offset of the connection table in bytes.
///
/// This follows immediately after the bit sections plus padding to the next
/// 4-byte boundary as required by the spec.
pub fn connection_table_offset(ni: u32, no: u32, nn: u32) -> usize {
    let input_bytes = (ni as usize).div_ceil(8);
    let output_bytes = (no as usize).div_ceil(8);
    let internal_bytes = (nn as usize).div_ceil(8);
    let bits_total = input_bytes + output_bytes + internal_bytes;
    let pad = (4 - (bits_total % 4)) % 4;
    HEADER_BYTES + bits_total + pad
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

    #[test]
    fn section_offset_calculation() {
        // Ni=1, No=1, Nn=1 -> each consumes 1 byte
        let (i, o, n) = section_offsets(1, 1, 1);
        assert_eq!((i, o, n), (32, 33, 34));

        // Ni=9 -> 2 bytes, No=17 -> 3 bytes, Nn=0
        let (i2, o2, n2) = section_offsets(9, 17, 0);
        assert_eq!((i2, o2, n2), (32, 34, 37));

        // Ni=0, No=0, Nn=0 -> all start after header
        let (i3, o3, n3) = section_offsets(0, 0, 0);
        assert_eq!((i3, o3, n3), (32, 32, 32));

        // Connection table offset for Ni=1, No=1, Nn=1
        let conn_off = connection_table_offset(1, 1, 1);
        // Total bits bytes = 3 -> pad = 1 -> 32 + 3 + 1 = 36
        assert_eq!(conn_off, 36);
    }
}
