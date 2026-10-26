//! Client packets of the command channel: invite, answer, oust and the party member window.
//!
//! Plain readers; the rules (who may invite, the Strategy Guide, the Clan Imperium skill ...) are
//! the world's job (`l2-world/src/party/command_channel.rs`). Java has no client packet for
//! leaving or deleting a channel: those are the user commands 96 and 93 (`BypassUserCmd`), and
//! the channel info window is user command 97.

client_packet!(
    // Java: gameserver/network/clientpackets/RequestExAskJoinMPCC.java
    /// The party leader invites another party into its command channel
    /// (`REQUEST_EX_ASK_JOIN_MPCC`, `0xD0` 0x0006). `name` is any member of the other party.
    RequestExAskJoinMpcc,
    ex RequestExAskJoinMpcc {
        name: String,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestExAcceptJoinMPCC.java
    /// The invited party leader answers (`REQUEST_EX_ACCEPT_JOIN_MPCC`, `0xD0` 0x0007): `1`
    /// accepts, anything else declines.
    RequestExAcceptJoinMpcc,
    ex RequestExAcceptJoinMpcc {
        response: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestExOustFromMPCC.java
    /// The channel leader dismisses the party of `name` (`REQUEST_EX_OUST_FROM_MPCC`, `0xD0`
    /// 0x0008).
    RequestExOustFromMpcc,
    ex RequestExOustFromMpcc {
        name: String,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestExMPCCShowPartyMembersInfo.java
    /// A click on a party in the channel window (`REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO`,
    /// `0xD0` 0x002D). `party_leader_id` is the object id of the party's leader.
    RequestExMpccShowPartyMembersInfo,
    ex RequestExMpccShowPartyMembersInfo {
        party_leader_id: i32,
    }
);
