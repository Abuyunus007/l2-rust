//! Client packets of the community board: `RequestShowBoard` (the board button) and
//! `RequestBBSwrite` (the `Write` buttons of the board pages).
//!
//! These only read the packet. The pages and the DB rows are the world's job
//! (`l2-world/src/community_board/`).

// Java: gameserver/network/clientpackets/RequestShowBoard.java
client_packet!(
    /// The player opens the community board (`REQUEST_SHOW_BOARD`). The world shows the page of
    /// `BBSDefault`.
    RequestShowBoard,
    plain RequestShowBoard {
        /// Java `readInt(); // Unused.`
        unused: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestBBSwrite.java
client_packet!(
    /// A `Write <url> <arg1> ... <arg5>` button of a board page (`REQUEST_BBS_WRITE`).
    RequestBBSwrite,
    plain RequestBbsWrite {
        /// `Topic`, `Post`, `Region` or `Notice`: picks the board handler.
        url: String,
        arg1: String,
        arg2: String,
        arg3: String,
        arg4: String,
        arg5: String,
    }
);

#[cfg(test)]
mod tests {
    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    #[test]
    fn show_board_is_one_unused_int() {
        let p = RequestShowBoard { unused: 7 };
        let bytes = p.to_bytes();
        assert_eq!(bytes, vec![0x5E, 7, 0, 0, 0]);
        assert_eq!(
            RequestShowBoard::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            p
        );
        // Java's `readInt()` fails on a short body.
        assert!(RequestShowBoard::decode(&mut PacketReader::new(&[])).is_err());
    }

    #[test]
    fn bbs_write_carries_six_strings() {
        let p = RequestBBSwrite {
            url: "Notice".into(),
            arg1: "Set".into(),
            arg2: "_".into(),
            arg3: "Hello clan".into(),
            arg4: "Content".into(),
            arg5: String::new(),
        };
        let bytes = p.to_bytes();
        assert_eq!(bytes[0], 0x24);
        assert_eq!(
            RequestBBSwrite::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            p
        );
        // Six UTF-16LE strings with a terminating zero each: the empty one is two bytes.
        let empty = RequestBBSwrite {
            url: String::new(),
            arg1: String::new(),
            arg2: String::new(),
            arg3: String::new(),
            arg4: String::new(),
            arg5: String::new(),
        };
        assert_eq!(empty.to_bytes().len(), 1 + 6 * 2);
    }
}
