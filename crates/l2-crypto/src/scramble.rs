//! Scrambled RSA modulus for the login `Init` packet.
//!
//! Java: loginserver/controller/ScrambledKeyPair.java (`scrambleModulus`)
//!
//! The client expects the 128-byte modulus of the 1024-bit RSA key in a shuffled form. Java first
//! strips the leading `0x00` that `BigInteger.toByteArray` adds when the top bit is set (see
//! [`RsaKeyPair::scrambled_modulus`](crate::RsaKeyPair::scrambled_modulus)), then applies four
//! steps. [`scramble_modulus`] takes the already stripped 128 bytes.

const HALF: usize = 0x40;

/// Step 1: swap bytes `0x00..0x04` with `0x4d..0x51`.
fn swap_ends(m: &mut [u8; 128]) {
    for i in 0..4 {
        m.swap(i, 0x4d + i);
    }
}

/// Step 2 (and its own inverse): first half ^= second half.
fn xor_first_half(m: &mut [u8; 128]) {
    for i in 0..HALF {
        m[i] ^= m[HALF + i];
    }
}

/// Step 3 (and its own inverse): bytes `0x0d..0x11` ^= bytes `0x34..0x38`.
fn xor_inner(m: &mut [u8; 128]) {
    for i in 0..4 {
        m[0x0d + i] ^= m[0x34 + i];
    }
}

/// Step 4 (and its own inverse): second half ^= first half.
fn xor_second_half(m: &mut [u8; 128]) {
    for i in 0..HALF {
        m[HALF + i] ^= m[i];
    }
}

/// Scrambles a 128-byte big-endian RSA modulus exactly like `ScrambledKeyPair.scrambleModulus`.
pub fn scramble_modulus(modulus_128: &[u8; 128]) -> [u8; 128] {
    let mut m = *modulus_128;
    swap_ends(&mut m);
    xor_first_half(&mut m);
    xor_inner(&mut m);
    xor_second_half(&mut m);
    m
}

/// Inverse of [`scramble_modulus`]: recovers the real modulus from the bytes a server sent in
/// its `Init` packet (needed by the proxy). Every step is an involution that leaves its own
/// inputs untouched, so undoing means applying the steps in reverse order.
pub fn unscramble_modulus(scrambled_128: &[u8; 128]) -> [u8; 128] {
    let mut m = *scrambled_128;
    xor_second_half(&mut m);
    xor_inner(&mut m);
    xor_first_half(&mut m);
    swap_ends(&mut m);
    m
}
