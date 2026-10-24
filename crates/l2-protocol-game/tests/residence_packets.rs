//! M7 residence packets (M7-RES-01) against the byte layout worked out by hand from the Java
//! `writeImpl` code, plus a round trip. The expected bytes are built without the crate's
//! `PacketWriter`, so a mistake in the encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::AgitDecoInfo;

// Java: AgitDecoInfo.writeImpl: AGIT_DECO_INFO (0xFD), the hall id, then twelve bytes: fireplace,
// carpet, statue (= carpet), chandelier, mirror, crystal (0), curtain, magic curtain, flag twice,
// front platform, item create.
#[test]
fn agit_deco_info_layout_and_round_trip() {
    let packet = AgitDecoInfo {
        hall_id: 36,
        fireplace: 2,
        carpet_statue: 1,
        chandelier: 2,
        mirror: 1,
        crystal: 0,
        curtain: 2,
        magic_curtain: 1,
        flag: 2,
        front_platform: 1,
        item_create: 1,
    };
    let mut expected = vec![0xFD];
    expected.extend(36_i32.to_le_bytes());
    expected.extend([2, 1, 1, 2, 1, 0, 2, 1, 2, 2, 1, 1]);
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected);
    assert_eq!(bytes.len(), 1 + 4 + 12);
    let decoded = AgitDecoInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(decoded, packet);

    // A hall without functions: twelve zeros.
    let empty = AgitDecoInfo {
        hall_id: 21,
        ..AgitDecoInfo::default()
    };
    let mut expected = vec![0xFD];
    expected.extend(21_i32.to_le_bytes());
    expected.extend([0; 12]);
    assert_eq!(empty.to_bytes(), expected);
}
