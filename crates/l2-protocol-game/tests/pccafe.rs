//! `ExPCCafePointInfo` (M6-FU-11): the byte layout worked out by hand from the Java `writeImpl`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::ExPcCafePointInfo;

fn expected(points: i32, add: i32, period: u8, remain: i32, kind: u8, time: i32) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(0x32u16.to_le_bytes());
    out.extend(points.to_le_bytes());
    out.extend(add.to_le_bytes());
    out.push(period);
    out.extend(remain.to_le_bytes());
    out.push(kind);
    out.extend(time.to_le_bytes());
    out
}

fn round_trip(packet: ExPcCafePointInfo, bytes: &[u8]) {
    assert_eq!(packet.to_bytes(), bytes);
    let decoded = ExPcCafePointInfo::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
    assert_eq!(decoded, packet);
}

#[test]
fn gaining_points_shows_yellow_with_time_times_three() {
    // Java `new ExPCCafePointInfo(points, 10, 1)`: type 0, period 1, remain 42, time 1 * 3.
    round_trip(
        ExPcCafePointInfo::new(1_200, 10, 1),
        &expected(1_200, 10, 1, 42, 0, 3),
    );
}

#[test]
fn spending_points_is_point_type_three() {
    // A multisell cost: `new ExPCCafePointInfo(points, -amount, 0)`.
    round_trip(
        ExPcCafePointInfo::new(500, -700, 0),
        &expected(500, -700, 1, 42, 3, 0),
    );
}

#[test]
fn the_empty_packet_is_all_zero() {
    // Java `new ExPCCafePointInfo()`.
    round_trip(ExPcCafePointInfo::default(), &expected(0, 0, 0, 0, 0, 0));
}
