//! Scenario: a servitor. The leader summons Kat the Cat with its skill (the watcher sees it appear),
//! selects the target dummy and orders the servitor to attack; the dummy loses HP. The leader
//! logs out with the servitor out, which is stored the way Java stores it.

use l2_protocol_game::server::{Attack, NpcInfo, PetInfo, SetSummonRemainTime, SummonInfo};

use super::*;

pub struct Played {
    left_a: Left,
    summons: Vec<SummonDbRow>,
    owner: i32,
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let a = h.object_id(Who::A);
    let entered_a = h.enter(Who::A).await;
    let dummy = all::<NpcInfo>(&entered_a, "NPC_INFO")
        .iter()
        .find(|n| n.display_id == m5_data::DUMMY_ID)
        .expect("the dummy is in view")
        .object_id;
    h.enter(Who::B).await;
    h.collect_until(Who::A, &|body| is_named(body, "CHAR_INFO")).await;

    // ---- the summon: the owner gets the pet window, everybody else sees an NPC appear -----------------
    h.act(Who::A, act::use_skill(m6_data::SUMMON_KAT)).await;
    let summoned = h
        .collect_until(Who::A, &|body| is_named(body, "SET_SUMMON_REMAIN_TIME"))
        .await;
    assert!(
        message_ids(&summoned).contains(&SystemMessageId::UseS1),
        "{:?}",
        message_ids(&summoned)
    );
    assert_in_order(&names(&summoned), &["MAGIC_SKILL_USE", "PET_INFO", "PET_STATUS_UPDATE"]);
    let info = all::<PetInfo>(&summoned, "PET_INFO").remove(0);
    // A servitor is summon type 1; its title is the owner's name (Java `Summon.getTitle`).
    assert_eq!((info.summon_type, info.display_id), (1, m6_data::KAT_ID));
    assert_eq!(info.title, NAME_A);
    let servitor = info.object_id;
    assert!(servitor >= l2_world::RUST_FIRST_OBJECT_ID, "D-39: {servitor}");
    let remain: SetSummonRemainTime = all(&summoned, "SET_SUMMON_REMAIN_TIME").remove(0);
    assert_eq!(remain.max_time, m6_data::KAT_LIFE_SECONDS * 1000, "{remain:?}");
    let seen_by_b = h.collect_quiet(Who::B).await;
    // (`SummonInfo` has the opcode of `NpcInfo`, so the NPCs in view are skipped.)
    let seen: Vec<SummonInfo> = seen_by_b
        .iter()
        .filter(|body| is_named(body, "NPC_INFO"))
        .filter_map(|body| SummonInfo::decode(&mut PacketReader::new(&body[1..])).ok())
        .collect();
    assert!(
        seen.iter().any(|n| n.object_id == servitor && n.display_id == m6_data::KAT_ID),
        "B sees the servitor: {:?}",
        names(&seen_by_b)
    );

    // ---- the target is chosen, the servitor is told to attack it (action 22) ---------------------------------
    h.act(Who::A, act::click(dummy)).await;
    let selected = h
        .collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    let hp_before = last_status(&selected, dummy, CUR_HP).expect("the HP of the target");
    h.act(Who::A, act::action_use(22)).await;
    let fight = h
        .collect_until(Who::A, &|body| {
            is_named(body, "SYSTEM_MESSAGE")
                && decode_any::<SystemMessage>(body).id == SystemMessageId::C1HasDoneS3PointsOfDamageToC2
        })
        .await;
    let blows: Vec<Attack> = all::<Attack>(&fight, "ATTACK")
        .into_iter()
        .filter(|x| x.attacker_id == servitor)
        .collect();
    assert_eq!(blows[0].first_hit.target_id, dummy);
    assert_in_order(&names(&fight), &["MOVE_TO_PAWN", "ATTACK"]);
    // "Kat the Cat has done N damage to Test Dummy": the servitor is named by its template.
    let damage = system_messages(&fight)
        .into_iter()
        .find(|m| m.id == SystemMessageId::C1HasDoneS3PointsOfDamageToC2)
        .unwrap_or_else(|| panic!("no damage message: {:?}", system_messages(&fight)));
    assert_eq!(damage.params[0], SystemMessageParam::NpcName(m6_data::KAT_ID));
    assert_eq!(damage.params[1], text("Test Dummy"));
    // The dummy is hurt: its next HP update is lower (the servitor keeps hitting).
    let more = h
        .collect_until(Who::A, &|body| {
            is_named(body, "STATUS_UPDATE")
                && decode_any::<StatusUpdate>(body).object_id == dummy
                && decode_any::<StatusUpdate>(body)
                    .attributes
                    .iter()
                    .any(|(id, _)| *id == CUR_HP)
        })
        .await;
    let mut seen_hp = last_status(&fight, dummy, CUR_HP);
    seen_hp = last_status(&more, dummy, CUR_HP).or(seen_hp);
    assert!(
        seen_hp.is_some_and(|hp| hp < hp_before),
        "the dummy lost HP: {seen_hp:?} of {hp_before}"
    );
    let watched = h.collect_quiet(Who::B).await;
    assert!(
        all::<Attack>(&watched, "ATTACK").iter().any(|x| x.attacker_id == servitor),
        "B saw the servitor hit: {:?}",
        names(&watched)
    );

    // ---- the owner logs out: the servitor is stored with what it has left -------------------------------------
    let left_a = h.leave(Who::A).await;
    let summons = h.summon_rows().await;
    Played {
        left_a,
        summons,
        owner: a,
    }
}

pub fn assert_played(played: &Played) {
    // Java `CharSummonTable.saveSummon`: one row of (owner, skill) with the HP and MP the servitor
    // had and the milliseconds of life it had left.
    assert_eq!(played.summons.len(), 1, "{:?}", played.summons);
    let row = &played.summons[0];
    assert_eq!((row.owner_id, row.summon_skill_id), (played.owner, m6_data::SUMMON_KAT));
    assert!(row.cur_hp > 0 && row.cur_mp > 0, "{row:?}");
    assert!(
        (1..m6_data::KAT_LIFE_SECONDS * 1000).contains(&row.time),
        "some life is gone, some is left: {}",
        row.time
    );
    // The leader itself is untouched.
    assert_eq!(played.left_a.exp, 0);
}

#[tokio::test]
async fn a_servitor_fights_and_is_stored_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let played = play(&mut world).await;
    assert_played(&played);
}

#[tokio::test]
async fn a_servitor_fights_and_is_stored_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6u", false).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played);
    net.finish().await;
}
