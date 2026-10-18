#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Checks of the generated `SystemMessageId` / `NpcStringId` tables (codegen target `messages`).
//! Expected values come from the Java sources (`SystemMessageId.java`, `NpcStringId.java`).

use l2_protocol_game::generated::npc_string_id::NpcStringId;
use l2_protocol_game::generated::system_message_id::SystemMessageId;
use std::collections::HashSet;

#[test]
fn counts_match_java() {
    assert_eq!(SystemMessageId::ALL.len(), 3236);
    assert_eq!(NpcStringId::ALL.len(), 5915);
}

#[test]
fn ids_round_trip_and_are_unique() {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for m in SystemMessageId::ALL {
        assert_eq!(SystemMessageId::from_id(m.id()), Some(m));
        assert!(ids.insert(m.id()), "duplicate id {}", m.id());
        assert!(names.insert(m.name()), "duplicate name {}", m.name());
    }
    ids.clear();
    names.clear();
    for n in NpcStringId::ALL {
        assert_eq!(NpcStringId::from_id(n.id()), Some(n));
        assert!(ids.insert(n.id()), "duplicate id {}", n.id());
        assert!(names.insert(n.name()), "duplicate name {}", n.name());
    }
}

#[test]
fn unknown_ids_are_none() {
    assert_eq!(SystemMessageId::from_id(-1), None);
    assert_eq!(SystemMessageId::from_id(6508), None);
    assert_eq!(NpcStringId::from_id(0), None); // NpcStringId starts at 1
    assert_eq!(NpcStringId::from_id(10_004_442), None);
}

#[test]
fn system_message_spot_checks() {
    let m = SystemMessageId::from_id(0).unwrap();
    assert_eq!(m, SystemMessageId::YouHaveBeenDisconnectedFromTheServer);
    assert_eq!(m.name(), "YOU_HAVE_BEEN_DISCONNECTED_FROM_THE_SERVER");
    assert_eq!(m.message(), "You have been disconnected from the server.");
    assert_eq!(m.param_count(), 0);

    let m = SystemMessageId::from_id(46).unwrap();
    assert_eq!(m.name(), "YOU_USE_S1");
    assert_eq!(m.message(), "You use $s1.");
    assert_eq!(m.param_count(), 1);

    // The Java declaration order is by id, so the first and last entries are the id extremes.
    assert_eq!(SystemMessageId::ALL[0].id(), 0);
    let last = SystemMessageId::ALL[SystemMessageId::ALL.len() - 1];
    assert_eq!(last.id(), 6507);
    assert_eq!(last.message(), "You feel refreshed. Everything appears clear.");

    // Non-ASCII text is kept as is.
    assert_eq!(
        SystemMessageId::from_id(94).unwrap().message(),
        "2004-2011 \u{a9} NC Interactive, Inc. All Rights Reserved."
    );
    // Duplicate Java names get a numeric suffix in the Java source already.
    assert_eq!(SystemMessageId::from_id(2).unwrap().name(), "S1_DOES_NOT_EXIST");
    assert_eq!(SystemMessageId::from_id(6).unwrap().name(), "S1_DOES_NOT_EXIST_2");
    // Empty texts exist in the Java table.
    assert_eq!(SystemMessageId::from_id(3245).unwrap().message(), "");
}

#[test]
fn npc_string_spot_checks() {
    let n = NpcStringId::from_id(1).unwrap();
    assert_eq!(n.name(), "HELLO_I_AM_S1_YOU_ARE_S2_RIGHT_HEHEHE");
    assert_eq!(n.message(), "Hello! I am $s1. You are $s2, right? Hehehe");
    assert_eq!(n.param_count(), 2);

    let n = NpcStringId::from_id(4151).unwrap();
    assert_eq!(n.name(), "DELIVERY_DUTY_COMPLETE_N_GO_FIND_THE_NEWBIE_GUIDE");
    // Java source has `\\n`: a literal backslash and `n`, not a newline.
    assert_eq!(
        n.message(),
        "Delivery duty complete. \\n Go find the Newbie Guide."
    );

    let last = NpcStringId::from_id(10_004_441).unwrap();
    assert_eq!(
        last.name(),
        "IT_TELEPORTS_THE_GUARD_MEMBERS_OF_THE_ELMORE_IMPERIAL_CASTLE_TO_THE_INSIDE_OF_THE_CASTLE"
    );
    assert_eq!(NpcStringId::ALL[NpcStringId::ALL.len() - 1], last);
    assert_eq!(NpcStringId::from_id(1_800_701).unwrap().name(), "EMPTY_4");
}
