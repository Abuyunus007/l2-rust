//! Scenario: a party. The leader invites a member (loot by turn), the member accepts, the leader
//! casts a party buff, kills a rat (both share the exp and SP), picks up the loot (the rule hands
//! it out) and the member leaves, which ends a party of two.

use l2_protocol_game::server::{
    AbnormalStatusUpdate, AskJoinParty, DropItem, GetItem, JoinParty, NpcInfo, PartyDistributionType,
    PartySmallWindowAdd, PartySmallWindowAll, PartySmallWindowDelete, PartySpelled, UserInfo,
};

use super::*;

/// What the scenario saw, for the checks after it.
pub struct Played {
    left_a: Left,
    left_b: Left,
    /// The adena each member got from the rat's pile.
    adena_shares: (i64, i64),
    /// The object id of the rat tail the game made (it goes to the leader).
    tail_object_id: i32,
}

/// `Party.BONUS_EXP_SP[1]`: a party of two earns 10 % more.
const PARTY_BONUS_OF_TWO: f64 = 1.10;

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));

    // ---- both enter; the leader sees the rat ---------------------------------------------------
    let entered_a = h.enter(Who::A).await;
    let rat = all::<NpcInfo>(&entered_a, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m4_data::RAT_ID)
        .expect("the rat is in view");
    let entered_b = h.enter(Who::B).await;
    assert!(count_named(&entered_b, "CHAR_INFO") >= 1, "B sees A");
    h.collect_until(Who::A, &|body| is_named(body, "CHAR_INFO")).await;
    let attack_before = all::<UserInfo>(&entered_b, "USER_INFO").last().unwrap().p_atk;
    // The MP the leader starts with: the stat engine makes the maximum more than the 30 of the
    // template, and a created character starts with the maximum (the bare one with 30).
    let start_mp = all::<UserInfo>(&entered_a, "USER_INFO").remove(0).current_mp;

    // ---- the invitation: loot by turn ----------------------------------------------------------
    h.act(
        Who::A,
        act::join_party(NAME_B, PartyDistributionType::ByTurn.id()),
    )
    .await;
    let invited = h.collect_quiet(Who::A).await;
    assert_eq!(
        message_of(&invited, SystemMessageId::C1HasBeenInvitedToTheParty).params,
        [text(NAME_B)]
    );
    let asked = h.collect_quiet(Who::B).await;
    let ask: AskJoinParty = all(&asked, "ASK_JOIN_PARTY").remove(0);
    assert_eq!(
        (ask.requestor_name.as_str(), ask.distribution_type),
        (NAME_A, PartyDistributionType::ByTurn)
    );

    // ---- the answer: B joins A's new party -------------------------------------------------------
    h.act(Who::B, act::answer_join_party(1)).await;
    let joined_a = h.collect_quiet(Who::A).await;
    let joined_b = h.collect_quiet(Who::B).await;
    // Java `Party.addPartyMember`: the new member gets the whole window and "You have joined
    // <leader>'s party"; the party hears "<new member> has joined" and gets the window entry.
    assert_eq!(all::<JoinParty>(&joined_a, "JOIN_PARTY")[0].response, 1);
    assert_eq!(
        message_of(&joined_a, SystemMessageId::C1HasJoinedTheParty).params,
        [text(NAME_B)]
    );
    let add: PartySmallWindowAdd = all(&joined_a, "PARTY_SMALL_WINDOW_ADD").remove(0);
    assert_eq!((add.leader_object_id, add.member.object_id), (a, b));
    assert_eq!(add.distribution_type, PartyDistributionType::ByTurn);
    let window: PartySmallWindowAll = all(&joined_b, "PARTY_SMALL_WINDOW_ALL").remove(0);
    assert_eq!(window.leader_object_id, a);
    assert_eq!(
        window
            .members
            .iter()
            .map(|m| m.vitals.object_id)
            .collect::<Vec<_>>(),
        [a],
        "the window lists the others"
    );
    assert_eq!(
        message_of(&joined_b, SystemMessageId::YouHaveJoinedS1SParty).params,
        [text(NAME_A)]
    );

    // ---- the party buff: B gets it too, and both see it in the party window ----------------------
    h.act(Who::A, act::use_skill(m6_data::CHANT_OF_BATTLE)).await;
    let buffed_a = h
        .collect_until(Who::A, &|body| is_named(body, "ABNORMAL_STATUS_UPDATE"))
        .await;
    assert!(
        message_ids(&buffed_a).contains(&SystemMessageId::UseS1),
        "{:?}",
        message_ids(&buffed_a)
    );
    let buffed_b = h.collect_quiet(Who::B).await;
    for (who, batch) in [(Who::A, &buffed_a), (Who::B, &buffed_b)] {
        let icons: AbnormalStatusUpdate = all(batch, "ABNORMAL_STATUS_UPDATE").remove(0);
        assert_eq!(icons.effects.len(), 1, "{who:?}: one icon: {icons:?}");
        assert_eq!(icons.effects[0].display_id, m6_data::CHANT_OF_BATTLE);
        assert!(
            (m6_data::CHANT_SECONDS - 10..=m6_data::CHANT_SECONDS).contains(&icons.effects[0].time),
            "{who:?}: {}",
            icons.effects[0].time
        );
    }
    // The party sees the other's icons in `PartySpelled`.
    for (batch, other) in [(&buffed_a, b), (&buffed_b, a)] {
        let spelled: Vec<PartySpelled> = all(batch, "PARTY_SPELLED");
        assert!(
            spelled.iter().any(|s| s.creature_id == other
                && s.effects.iter().any(|e| e.display_id == m6_data::CHANT_OF_BATTLE)),
            "{spelled:?}"
        );
    }
    let attack_after = all::<UserInfo>(&buffed_b, "USER_INFO")
        .last()
        .expect("B's UserInfo")
        .p_atk;
    assert!(attack_after > attack_before, "{attack_after} > {attack_before}");
    // The caster paid 4 + 10 MP (the regeneration of the cast gives a little back).
    let mp = last_status(&buffed_a, a, CUR_MP).expect("A's MP changed");
    let cost = m6_data::CHANT_MP + m6_data::CHANT_MP_INITIAL;
    assert!(
        (start_mp - cost..=start_mp - cost + 4).contains(&mp),
        "{mp} of {start_mp}"
    );

    // ---- the hunt: A kills the rat; both get a share of exp and SP -------------------------------------
    h.act(Who::A, act::click(rat.object_id)).await;
    h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    h.act(Who::A, act::click(rat.object_id)).await;
    let fight = h.collect_until(Who::A, &|body| is_named(body, "DIE")).await;
    let after_fight_b = h.collect_quiet(Who::B).await;
    // Java `Party.distributeXpAndSp`: 100 exp * 1.10 for two members, split by the squares of the
    // levels (1:1) is 55; the SP, 12 * 1.10 / 2 = 6.6, is shown cut (`(long) addToSp`) and added
    // rounded (`Math.round`, see `PlayerStat.addExpAndSp`).
    let exp = (m4_data::RAT_EXP as f64 * PARTY_BONUS_OF_TWO / 2.0).round() as i64;
    let sp_shown = (m4_data::RAT_SP as f64 * PARTY_BONUS_OF_TWO / 2.0) as i64;
    assert_eq!((exp, sp_shown), (55, 6));
    for (who, batch) in [(Who::A, &fight), (Who::B, &after_fight_b)] {
        assert_eq!(
            message_of(batch, SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp).params,
            [
                SystemMessageParam::LongNumber(exp),
                SystemMessageParam::LongNumber(sp_shown)
            ],
            "{who:?}"
        );
        assert!(
            !message_ids(batch).contains(&SystemMessageId::YourLevelHasIncreased),
            "55 exp are 13 short of level 2"
        );
    }

    // ---- the loot: the rule is "by turn", so the soulshots (the first pickup) go to the member
    // after the leader and the rat tail to the leader; the adena is shared ------------------------------------
    let drops: Vec<DropItem> = all(&fight, "DROP_ITEM");
    assert_eq!(drops.len(), 3, "{:?}", names(&fight));
    let drop_of = |item_id: i32| {
        drops
            .iter()
            .find(|d| d.display_id == item_id)
            .expect("a drop")
            .object_id
    };
    let mut picked = Vec::new();
    for item_id in [SOULSHOT_ID, m4_data::RAT_TAIL_ID, ADENA_ID] {
        h.act(Who::A, act::click(drop_of(item_id))).await;
        let got = h.collect_until(Who::A, &|body| is_named(body, "GET_ITEM")).await;
        let get: GetItem = all(&got, "GET_ITEM").remove(0);
        assert_eq!((get.player_id, get.object_id), (a, drop_of(item_id)));
        picked.push(got);
    }
    let loot_b = h.collect_quiet(Who::B).await;
    // B: the soulshots (Java: `C1 obtained` to the others, `You have obtained` to the looter).
    assert_eq!(
        message_of(&loot_b, SystemMessageId::YouHaveObtainedS2S1).params,
        [
            SystemMessageParam::ItemName(SOULSHOT_ID),
            SystemMessageParam::LongNumber(m4_data::RAT_SOULSHOTS)
        ]
    );
    // A: heard who got the soulshots, got the tail itself.
    assert_eq!(
        message_of(&picked[0], SystemMessageId::C1HasObtainedS3S2).params,
        [
            text(NAME_B),
            SystemMessageParam::ItemName(SOULSHOT_ID),
            SystemMessageParam::LongNumber(m4_data::RAT_SOULSHOTS)
        ]
    );
    assert_eq!(
        message_of(&picked[1], SystemMessageId::YouHaveObtainedS1).params,
        [SystemMessageParam::ItemName(m4_data::RAT_TAIL_ID)]
    );
    assert_eq!(
        message_of(&loot_b, SystemMessageId::C1HasObtainedS2).params,
        [text(NAME_A), SystemMessageParam::ItemName(m4_data::RAT_TAIL_ID)]
    );
    // The pile of adena: `Party.distributeAdena` gives each member in range 25 / 2 = 12 and one of
    // them (picked by the world's RNG) the odd one.
    let share = |batch: &[Vec<u8>]| match message_of(batch, SystemMessageId::YouHaveEarnedS1Adena).params[..]
    {
        [SystemMessageParam::LongNumber(n)] => n,
        ref other => panic!("{other:?}"),
    };
    let adena_shares = (share(&picked[2]), share(&loot_b));
    assert_eq!(adena_shares.0 + adena_shares.1, m4_data::RAT_ADENA);
    assert!(
        adena_shares.0.min(adena_shares.1) == m4_data::RAT_ADENA / 2
            && adena_shares.0.max(adena_shares.1) == m4_data::RAT_ADENA / 2 + 1,
        "{adena_shares:?}"
    );

    // ---- B leaves: with two members the party is over ----------------------------------------------------
    h.act(Who::B, act::withdraw_party()).await;
    let over_a = h.collect_quiet(Who::A).await;
    let over_b = h.collect_quiet(Who::B).await;
    // Java `disbandParty`: "The party has dispersed." to both, then `removePartyMember` for each
    // member in turn. The leader goes first: it gets the empty window, B hears A leave; the list
    // is empty when B's turn comes, so B gets no `DeleteAll`.
    for batch in [&over_a, &over_b] {
        assert_eq!(message_ids(batch), [SystemMessageId::ThePartyHasDispersed]);
    }
    // (Regeneration updates may be mixed in; they are not what is checked.)
    let party_packets = |batch: &[Vec<u8>]| -> Vec<String> {
        names(batch)
            .into_iter()
            .filter(|n| {
                n == "SYSTEM_MESSAGE"
                    || n == "PARTY_SMALL_WINDOW_DELETE"
                    || n == "PARTY_SMALL_WINDOW_DELETE_ALL"
            })
            .collect()
    };
    assert_eq!(
        party_packets(&over_a),
        ["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE_ALL"]
    );
    assert_eq!(
        party_packets(&over_b),
        ["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE"]
    );
    let delete: PartySmallWindowDelete = all(&over_b, "PARTY_SMALL_WINDOW_DELETE").remove(0);
    assert_eq!((delete.object_id, delete.name.as_str()), (a, NAME_A));

    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    let tail_object_id = left_a
        .row_of(m4_data::RAT_TAIL_ID, "INVENTORY")
        .expect("the tail")
        .object_id;
    Played {
        left_a,
        left_b,
        adena_shares,
        tail_object_id,
    }
}

pub fn assert_played(played: &Played) {
    let (a, b) = (&played.left_a, &played.left_b);
    // Exp and SP: the 6 SP the message shows are 7 in the store.
    // NOTE(java-quirk): `PlayerStat.addExpAndSp` shows `(long) addToSp` and adds `Math.round`.
    for left in [a, b] {
        assert_eq!((left.level, left.exp, left.sp), (1, 55, 7));
    }
    let (share_a, share_b) = played.adena_shares;
    assert_eq!(a.count_of(ADENA_ID), m6_data::START_ADENA + share_a);
    assert_eq!(b.count_of(ADENA_ID), m6_data::START_ADENA + share_b);
    // The loot by turn: soulshots to B, the tail to A.
    assert_eq!(a.count_of(SOULSHOT_ID), m5_data::SOULSHOTS_AT_START);
    assert_eq!(
        b.count_of(SOULSHOT_ID),
        m5_data::SOULSHOTS_AT_START + m4_data::RAT_SOULSHOTS
    );
    assert_eq!(a.count_of(m4_data::RAT_TAIL_ID), 1);
    assert_eq!(b.count_of(m4_data::RAT_TAIL_ID), 0);
    // D-39: the item the game made has an id of the Rust range.
    assert!(
        played.tail_object_id >= l2_world::RUST_FIRST_OBJECT_ID,
        "{}",
        played.tail_object_id
    );
    assert_eq!((a.clan_id, b.clan_id), (0, 0));
}

#[tokio::test]
async fn a_party_shares_a_buff_exp_and_loot_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let played = play(&mut world).await;
    assert_played(&played);
}

#[tokio::test]
async fn a_party_shares_a_buff_exp_and_loot_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6p", false).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played);
    net.finish().await;
}
