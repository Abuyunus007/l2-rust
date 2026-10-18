//! Frame codec: `u16 LE` total length (including the 2 header bytes), then the body.
//!
//! Java: `commons/network/handler/ReadHandler.java` (`handleHeader`, `MAX_PACKET_SIZE`).

use std::io;

use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

/// Size of the length header.
pub const HEADER_SIZE: usize = 2;

/// Largest body: a `u16` total length minus the header (Java `MAX_PACKET_SIZE = 65533`).
pub const MAX_PAYLOAD: usize = u16::MAX as usize - HEADER_SIZE;

/// Splits a byte stream into frame bodies (still encrypted) and writes bodies with a header.
#[derive(Debug, Default, Clone, Copy)]
pub struct FrameCodec;

impl Decoder for FrameCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        loop {
            if src.len() < HEADER_SIZE {
                return Ok(None);
            }
            let total = usize::from(u16::from_le_bytes([src[0], src[1]]));
            if total <= HEADER_SIZE {
                // NOTE(java-quirk): Java's ReadHandler treats `dataSize <= 0` as "nothing to read"
                // and just reads the next header, so empty frames are skipped silently.
                src.advance(HEADER_SIZE);
                continue;
            }
            if src.len() < total {
                src.reserve(total - src.len());
                return Ok(None);
            }
            src.advance(HEADER_SIZE);
            return Ok(Some(src.split_to(total - HEADER_SIZE)));
        }
    }
}

impl Encoder<Vec<u8>> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, body: Vec<u8>, dst: &mut BytesMut) -> Result<(), io::Error> {
        if body.len() > MAX_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "frame body of {} bytes exceeds the maximum of {MAX_PAYLOAD}",
                    body.len()
                ),
            ));
        }
        dst.reserve(HEADER_SIZE + body.len());
        dst.put_u16_le((body.len() + HEADER_SIZE) as u16); // checked above, fits
        dst.extend_from_slice(&body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(input: &[u8]) -> (Vec<Vec<u8>>, usize) {
        let mut codec = FrameCodec;
        let mut buf = BytesMut::from(input);
        let mut frames = Vec::new();
        while let Some(frame) = codec.decode(&mut buf).unwrap() {
            frames.push(frame.to_vec());
        }
        (frames, buf.len())
    }

    #[test]
    fn partial_header_waits() {
        assert_eq!(decode_all(&[5]), (vec![], 1));
    }

    #[test]
    fn partial_body_waits() {
        assert_eq!(decode_all(&[5, 0, 1, 2]), (vec![], 4));
    }

    #[test]
    fn several_frames_in_one_read() {
        let (frames, rest) = decode_all(&[3, 0, 0xaa, 4, 0, 1, 2, 5]);
        assert_eq!(frames, vec![vec![0xaa], vec![1, 2]]);
        assert_eq!(rest, 1);
    }

    #[test]
    fn empty_headers_are_skipped_like_java() {
        let (frames, rest) = decode_all(&[0, 0, 2, 0, 1, 0, 3, 0, 7]);
        assert_eq!(frames, vec![vec![7]]);
        assert_eq!(rest, 0);
    }

    #[test]
    fn encode_writes_total_length() {
        let mut dst = BytesMut::new();
        FrameCodec.encode(vec![9, 8, 7], &mut dst).unwrap();
        assert_eq!(&dst[..], &[5, 0, 9, 8, 7]);
    }

    #[test]
    fn encode_rejects_oversized() {
        let mut dst = BytesMut::new();
        assert!(FrameCodec.encode(vec![0; MAX_PAYLOAD + 1], &mut dst).is_err());
        assert!(FrameCodec.encode(vec![0; MAX_PAYLOAD], &mut dst).is_ok());
        assert_eq!(&dst[..2], &[0xff, 0xff]);
    }
}
