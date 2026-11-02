//! The Java custom features of `config/Custom/*.ini` (M9, D-57): one file per feature, each off by
//! default as in Java and switched by the keys of its `.ini`.
// Java: gameserver/config/custom/*.java, dist/game/data/scripts/custom/**
//
// # The buffer cards (M9-C03)
// - `community_board.rs`: the custom home board (`CommunityBoard.ini`).
// - `scheme_buffer.rs`: the scheme buffer NPC (`SchemeBuffer.ini`).
// - `sell_buffs.rs`: the buff shop (`SellBuffs.ini`).

pub mod community_board;
pub mod scheme_buffer;
pub mod sell_buffs;

/// The state of the M9-C03 features, owned by the world (D-10).
#[derive(Debug, Default)]
pub struct CustomBuffers {
    pub(crate) board: community_board::CustomBoardSettings,
    pub(crate) scheme: scheme_buffer::SchemeBufferState,
    pub(crate) sell: sell_buffs::SellBuffsState,
}
