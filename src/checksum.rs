pub fn fold16(seed: u16, data: &[u8]) -> u16 {
    let mut acc = u32::from(seed);
    for (idx, b) in data.iter().enumerate() {
        let lane = if idx & 1 == 0 {
            u32::from(*b)
        } else {
            u32::from(*b) << 8
        };
        acc = acc.wrapping_add(lane);
        acc = (acc & 0xffff) + (acc >> 16);
    }
    !(acc as u16)
}

pub fn frame_checksum(kind: u8, session: u16, seq: u16, payload: &[u8]) -> u16 {
    let mut seed = 0x4c54u16;
    seed = seed.wrapping_add(u16::from(kind));
    seed = seed.wrapping_add(session.rotate_left(3));
    seed = seed.wrapping_add(seq.rotate_right(2));
    fold16(seed, payload)
}

pub fn rolling_bias(data: &[u8]) -> u8 {
    let mut a = 0x63u8;
    let mut b = 0x91u8;
    for byte in data {
        a = a.wrapping_add(*byte).rotate_left(1);
        b ^= a.wrapping_mul(17);
    }
    a ^ b
}
