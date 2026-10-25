//! Scenario: a GM-started fortress siege of Shanty. The raider registers his clan at the suspicious
//! merchant (250,000 adena), the GM starts the siege (`//startfortsiege 101`), the GM kills the
//! commanders one after the other, the raider picks up the combat flag and tries to display it on
//! the flag pole, and the fortress changes hands. Java: `FortressSiegeManager` (script),
//! `FortSiege`, `Fort`, `CombatFlag`, `FortCommander`, `TakeFort`, `AdminFortSiege`.
//!
//! With `SiegeDatabaseWrites = False` (D-51) the world plays all of it but writes no fortress row
//! (the registration, the dates, the owner); with `True` the rows are Java's.
//!
//! NOTE: the fortress is not taken by the skill in the world as merged. Flag Display targets the
//! flag pole, a static object, and the cast launches only at creatures: the escape-range check of
//! `onMagicLaunchedTimer` (`effectRange` 400) leaves no target and aborts the cast, like Java's
//! (whose target list holds a `null`). The shipped condition `canTakeFort` is not modelled either.
//! So the scenario ends the way a GM does, `//setfort 101` on the raider, which calls the same
//! `Fort.endOfSiege(clan)` as the effect; `flag_display_takes_the_fortress` keeps the real way as
//! an ignored test.

use l2_protocol_game::server::{
    ExShowFortressInfo, NpcHtmlMessage, NpcInfo, NpcSay, PledgeShowInfoUpdate, SpawnItem, StaticObjectInfo,
};

use super::*;

const WOLVES: i32 = m7_seed::WOLVES;
const SHANTY: i32 = m7_seed::SHANTY;
/// The fee of the registration, from the player's adena (`FortSiege.addAttacker`).
const REGISTRATION_FEE: i64 = 250_000;

pub fn opts(siege_writes: bool, flag_condition: bool) -> Opts {
    Opts {
        siege_writes,
        positions: [m7_data::GM_POS, m7_data::FORT_REGISTER_POS, m7_data::FORT_GM_POS],
        auction_ms: 86_400_000,
        flag_condition,
    }
}

fn distance(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    dx.hypot(dy)
}

/// The player walks from `from` to `to` and the scenario waits for the arrival (the run speed is
/// at least 115 units a second). Returns what the client got meanwhile.
async fn walk<H: Harness>(h: &mut H, who: Who, from: (i32, i32, i32), to: (i32, i32, i32)) -> Vec<Vec<u8>> {
    h.act(who, act::move_to(to, from)).await;
    let seconds = (distance(from, to) / 100.0).ceil() as u64 + 2;
    h.wait_seconds(seconds).await;
    h.collect_quiet(who).await
}

/// Clicks `npc` twice (select, then talk) and returns the page it shows.
async fn talk_to<H: Harness>(h: &mut H, who: Who, npc: i32) -> NpcHtmlMessage {
    let origin = h.start_position(who);
    h.act(who, act::click(npc, origin)).await;
    h.collect_until(who, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    h.act(who, act::click(npc, origin)).await;
    let page = h
        .collect_until(who, &|body| is_named(body, "NPC_HTML_MESSAGE"))
        .await;
    all(&page, "NPC_HTML_MESSAGE").remove(0)
}

fn fort_info_of(batch: &[Vec<u8>]) -> l2_protocol_game::server::fort::FortressInfo {
    let mut infos: Vec<ExShowFortressInfo> = all(batch, "EX_SHOW_FORTRESS_INFO");
    assert!(!infos.is_empty(), "no fortress list in {:?}", names(batch));
    let mut forts = infos.remove(0).forts;
    assert!(!forts.is_empty(), "an empty fortress list");
    forts.remove(0)
}

/// How the scenario ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// The GM gives the fortress to the clan of his target (`//setfort 101`).
    Gm,
    /// The flag is displayed with the skill (does not work in the world as merged).
    Skill,
}

pub struct Played {
    pub rows_start: Rows,
    pub rows_end: Rows,
    pub ended_at: i64,
    pub raider: Left,
}

pub async fn play<H: Harness>(h: &mut H, writes: bool, finish: Finish) -> Played {
    // ---- the raider and the GM enter; the raider's clan comes from the tables (D-50) ----------------------
    let entered = h.enter(Who::Raider).await;
    let npcs = all::<NpcInfo>(&entered, "NPC_INFO");
    let npc_of = |template: i32| {
        npcs.iter()
            .find(|n| n.display_id == template)
            .unwrap_or_else(|| panic!("NPC {template} in view: {npcs:?}"))
            .object_id
    };
    // The foreman always stands there, the suspicious merchant before a siege.
    let merchant = npc_of(m7_data::FORT_MERCHANT_ID);
    let _foreman = npc_of(m7_data::FORT_FOREMAN_ID);
    let pole = all::<StaticObjectInfo>(&entered, "STATIC_OBJECT")
        .into_iter()
        .find(|s| s.static_object_id == m7_data::FLAG_POLE_STATIC_ID)
        .expect("the flag pole is in view")
        .object_id;
    h.enter(Who::Gm).await;
    h.collect_quiet(Who::Raider).await;
    let rows_start = h.rows().await;
    assert_eq!(rows_start.fort(SHANTY).owner, 0);

    // ---- the fortress list (RequestAllFortressInfo): Shanty has no owner, no siege ------------------------------
    h.act(Who::Raider, act::all_fortress_info()).await;
    let info = fort_info_of(&h.collect_quiet(Who::Raider).await);
    assert_eq!(
        (
            info.residence_id,
            info.owner_clan_name.as_str(),
            info.siege_in_progress
        ),
        (SHANTY, "", false)
    );

    // ---- the merchant's page offers the registration; the clan pays 250,000 --------------------------------------
    let page = talk_to(h, Who::Raider, merchant).await;
    assert!(
        page.html.contains("bypass Script FortressSiegeManager register"),
        "{}",
        page.html
    );
    h.act(Who::Raider, act::bypass("Script FortressSiegeManager register"))
        .await;
    let registered = h.collect_quiet(Who::Raider).await;
    assert_eq!(
        message_of(&registered, SystemMessageId::S1AdenaDisappeared).params,
        [SystemMessageParam::LongNumber(REGISTRATION_FEE)]
    );
    assert_eq!(
        message_of(
            &registered,
            SystemMessageId::YourClanHasBeenRegisteredToS1SFortressBattle
        )
        .params,
        [text("Shanty")]
    );
    let reply: NpcHtmlMessage = all(&registered, "NPC_HTML_MESSAGE").remove(0);
    assert!(reply.html.contains("Your clan is registered."), "{}", reply.html);
    // The rows of `FortSiege.saveSiegeClan` and `saveSiegeDate`: the switch (D-51).
    let rows_registered = h.rows().await;
    if writes {
        assert_eq!(rows_registered.fort_clans, [(SHANTY, WOLVES)]);
        // The date is an hour ahead (`setSiegeDateTime`).
        let date = rows_registered.fort(SHANTY).siege_date;
        assert!(date > now_ms() && date <= now_ms() + 3_700_000, "{date}");
    } else {
        assert_eq!(rows_registered, rows_start, "D-51: nothing global is written");
    }
    // The second registration is refused ("registered already"): the merchant's page again (the
    // client may only send back the links of the page it shows), the same command.
    h.act(Who::Raider, act::click(merchant, h.start_position(Who::Raider)))
        .await;
    let page = h
        .collect_until(Who::Raider, &|body| is_named(body, "NPC_HTML_MESSAGE"))
        .await;
    let page: NpcHtmlMessage = all(&page, "NPC_HTML_MESSAGE").remove(0);
    assert!(
        page.html.contains("bypass Script FortressSiegeManager register"),
        "{}",
        page.html
    );
    h.act(Who::Raider, act::bypass("Script FortressSiegeManager register"))
        .await;
    let again = h.collect_quiet(Who::Raider).await;
    let reply: NpcHtmlMessage = all(&again, "NPC_HTML_MESSAGE").remove(0);
    assert!(
        reply.html.contains("Your clan is registered already."),
        "{}",
        reply.html
    );

    // ---- the GM starts the siege ---------------------------------------------------------------------------------------
    h.collect_quiet(Who::Gm).await;
    h.act(Who::Gm, act::admin("startfortsiege 101")).await;
    h.wait_seconds(1).await;
    let gm_started = h.collect_quiet(Who::Gm).await;
    let raider_started = h.collect_quiet(Who::Raider).await;
    assert_eq!(
        message_of(&raider_started, SystemMessageId::TheFortressBattleS1HasBegun).params,
        [SystemMessageParam::CastleName(SHANTY)],
        "the participants hear it"
    );
    // The commanders and the guard stand in the fortress (the GM is inside).
    let seen = all::<NpcInfo>(&gm_started, "NPC_INFO");
    let commanders: Vec<&NpcInfo> = seen
        .iter()
        .filter(|n| n.display_id == m7_data::FORT_COMMANDER_1 || n.display_id == m7_data::FORT_COMMANDER_2)
        .collect();
    assert_eq!(commanders.len(), 2, "{seen:?}");
    assert!(
        seen.iter().any(|n| n.display_id == m7_data::FORT_GUARD_ID),
        "{seen:?}"
    );
    // The suspicious merchant has gone (`spawnType` 2).
    assert!(
        all::<l2_protocol_game::server::DeleteObject>(&raider_started, "DELETE_OBJECT")
            .iter()
            .any(|d| d.object_id == merchant),
        "the merchant leaves when the siege starts"
    );
    h.act(Who::Raider, act::all_fortress_info()).await;
    assert!(fort_info_of(&h.collect_quiet(Who::Raider).await).siege_in_progress);
    // `startSiege` saves the siege date (`saveFortSiege`) and nothing else.
    let rows_started = h.rows().await;
    if writes {
        assert_eq!(rows_started.fort_clans, rows_registered.fort_clans);
        assert_eq!(rows_started.castles, rows_registered.castles);
        assert_eq!(rows_started.clans, rows_registered.clans);
        assert_eq!(rows_started.fort(SHANTY).owner, 0);
    } else {
        assert_eq!(rows_started, rows_registered, "D-51: nothing global is written");
    }

    // ---- the GM kills the commanders: the barracks, then all of them --------------------------------------------------
    let mut barracks = Vec::new();
    for commander in &commanders {
        h.act(Who::Gm, act::click(commander.object_id, m7_data::FORT_GM_POS))
            .await;
        h.collect_quiet(Who::Gm).await;
        h.act(Who::Gm, act::admin("kill")).await;
        h.wait_seconds(1).await;
        barracks.extend(h.collect_quiet(Who::Raider).await);
    }
    // The first commander shouts as he falls and the barracks are seized; the last one opens the
    // fortress and the flags appear.
    assert_eq!(
        message_ids(&barracks)
            .into_iter()
            .filter(|m| matches!(
                m,
                SystemMessageId::TheBarracksHaveBeenSeized | SystemMessageId::AllBarracksAreOccupied
            ))
            .collect::<Vec<_>>(),
        [
            SystemMessageId::TheBarracksHaveBeenSeized,
            SystemMessageId::AllBarracksAreOccupied
        ]
    );
    assert_eq!(count_named(&barracks, "DIE"), 2);
    assert!(count_named(&barracks, "NPC_SAY") >= 2, "a dying commander shouts");
    let _: Vec<NpcSay> = all(&barracks, "NPC_SAY");
    let flags: Vec<SpawnItem> = all(&barracks, "SPAWN_ITEM");
    assert_eq!(flags.len(), 2, "two combat flags lie in the fortress");
    assert!(flags.iter().all(|f| f.display_id == m7_data::COMBAT_FLAG_ITEM));

    // ---- the raider walks in and picks up the flag: only a clan leader of an attacker may --------------------
    let at = (
        m7_data::FORT_FLAG_POS.0 - 30,
        m7_data::FORT_FLAG_POS.1,
        m7_data::Z,
    );
    walk(h, Who::Raider, m7_data::FORT_REGISTER_POS, at).await;
    let flag = flags
        .iter()
        .find(|f| f.y == m7_data::FORT_FLAG_POS.1)
        .expect("the first flag");
    h.act(Who::Raider, act::click(flag.object_id, at)).await;
    h.wait_seconds(2).await;
    let picked = h.collect_quiet(Who::Raider).await;
    assert_eq!(
        message_of(&picked, SystemMessageId::C1HasAcquiredTheFlag).params,
        [text(Seed::name_of(Who::Raider))]
    );
    assert!(has_message(&picked, SystemMessageId::YouHaveEquippedYourS1));
    // The flag brings its skill.
    let skills: l2_protocol_game::server::SkillList = all(&picked, "SKILL_LIST").pop().expect("a skill list");
    assert!(skills.skills.iter().any(|s| s.id == m7_data::FLAG_DISPLAY));
    // The others in the siege saw it picked up.
    let gm_saw = h.collect_quiet(Who::Gm).await;
    assert!(has_message(&gm_saw, SystemMessageId::AttentionC1HasPickedUpS2));

    // ---- the flag pole: target it and display the flag --------------------------------------------------------------
    walk(h, Who::Raider, at, m7_data::FLAG_POLE_STAND).await;
    h.act(Who::Raider, act::click(pole, m7_data::FLAG_POLE_STAND))
        .await;
    h.collect_quiet(Who::Raider).await;
    h.act(Who::Raider, act::use_skill(m7_data::FLAG_DISPLAY)).await;
    h.wait_seconds(4).await;
    let displaying = h.collect_quiet(Who::Raider).await;
    // `TakeFortStart`: the clan is announced to the siege when the cast begins.
    assert_eq!(
        message_of(&displaying, SystemMessageId::S1ClanIsTryingToDisplayAFlag).params,
        [text(m7_seed::WOLVES_NAME)]
    );
    let ended_at;
    match finish {
        Finish::Skill => {
            // The cast ends with `TakeFort` and the clan has the fortress.
            let taken = h
                .collect_until(Who::Raider, &|b| {
                    is_named(b, "SYSTEM_MESSAGE")
                        && decode_any::<SystemMessage>(b).id
                            == SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2
                })
                .await;
            ended_at = now_ms();
            assert_eq!(
                message_of(&taken, SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2).params,
                [text(m7_seed::WOLVES_NAME), SystemMessageParam::CastleName(SHANTY)]
            );
        }
        Finish::Gm => {
            // The GM gives the fortress to his target's clan, as `TakeFort` would (`Fort.endOfSiege`).
            h.collect_quiet(Who::Gm).await;
            h.act(
                Who::Gm,
                act::click(h.object_id(Who::Raider), m7_data::FORT_GM_POS),
            )
            .await;
            h.collect_quiet(Who::Gm).await;
            h.act(Who::Gm, act::admin("setfort 101")).await;
            h.wait_seconds(1).await;
            ended_at = now_ms();
        }
    }
    let won = h.collect_quiet(Who::Raider).await;
    // `Fort.endOfSiege` -> `setOwner(clan, true)`: "S1 is victorious in the fortress battle of S2" to
    // everybody, the siege over, the fortress on the clan's window (`TakeFortPoints`: reputation).
    assert_eq!(
        message_of(&won, SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2).params,
        [text(m7_seed::WOLVES_NAME), SystemMessageParam::CastleName(SHANTY)]
    );
    assert!(has_message(
        &won,
        SystemMessageId::TheFortressBattleOfS1HasFinished
    ));
    let clan_window: PledgeShowInfoUpdate = all(&won, "PLEDGE_SHOW_INFO_UPDATE")
        .pop()
        .expect("the clan window");
    assert_eq!(clan_window.fort_id, SHANTY);
    h.act(Who::Raider, act::all_fortress_info()).await;
    let info = fort_info_of(&h.collect_quiet(Who::Raider).await);
    assert_eq!(
        (
            info.residence_id,
            info.owner_clan_name.as_str(),
            info.siege_in_progress
        ),
        (SHANTY, m7_seed::WOLVES_NAME, false)
    );
    let rows_end = h.rows().await;
    let raider = h.leave(Who::Raider).await;
    h.leave(Who::Gm).await;
    Played {
        rows_start,
        rows_end,
        ended_at,
        raider,
    }
}

/// The rows after the siege: with the switch Java's statements (`Fort.setOwner`, `endSiege`,
/// `clearSiegeClan`), without it nothing global.
pub fn assert_played(played: &Played, writes: bool) {
    // The registration fee comes out of the leader's adena in both modes; the flag does not stay.
    assert_eq!(
        played.raider.count_of(ADENA_ID),
        m6_data::START_ADENA - REGISTRATION_FEE
    );
    assert_eq!(
        played.raider.count_of(m7_data::COMBAT_FLAG_ITEM),
        0,
        "the flag is gone with the siege"
    );
    if writes {
        let fort = played.rows_end.fort(SHANTY);
        // `Fort.updateOwnerInDB`: the owner, the time, state and castle 0.
        assert_eq!(fort.owner, WOLVES);
        assert!(
            (fort.last_owned_time - played.ended_at).abs() < 60_000,
            "the time the clan took it: {fort:?}"
        );
        assert_eq!((fort.state, fort.castle_id), (0, 0));
        // `clearSiegeClan`: the registrations are gone.
        assert!(played.rows_end.fort_clans.is_empty());
    } else {
        // D-51: no row of the fortress changed.
        assert_eq!(played.rows_end.forts, played.rows_start.forts);
        assert_eq!(played.rows_end.fort_clans, played.rows_start.fort_clans);
        assert_eq!(played.rows_end, played.rows_start);
    }
}

#[tokio::test]
async fn a_gm_started_fortress_siege_runs_in_a_bare_world_without_global_writes() {
    let mut world = BareWorld::start(&opts(false, false));
    let played = play(&mut world, false, Finish::Gm).await;
    assert_played(&played, false);
}

#[tokio::test]
async fn a_gm_started_fortress_siege_runs_in_a_bare_world_and_writes_javas_rows() {
    let mut world = BareWorld::start(&opts(true, false));
    let played = play(&mut world, true, Finish::Gm).await;
    assert_played(&played, true);
}

#[tokio::test]
async fn a_gm_started_fortress_siege_runs_on_the_real_servers_without_global_writes() {
    let options = opts(false, false);
    let Some(mut net) = Net::start("e2em7f0", &options).await else {
        return;
    };
    let played = play(&mut net, false, Finish::Gm).await;
    assert_played(&played, false);
    net.finish().await;
}

#[tokio::test]
async fn a_gm_started_fortress_siege_runs_on_the_real_servers_and_writes_javas_rows() {
    let options = opts(true, false);
    let Some(mut net) = Net::start("e2em7f1", &options).await else {
        return;
    };
    let played = play(&mut net, true, Finish::Gm).await;
    assert_played(&played, true);
    net.finish().await;
}

/// The real way: the flag is displayed with the skill and `TakeFort` gives the clan the fortress.
#[tokio::test]
#[ignore = "Flag Display does not take the fortress in the world as merged: its target is the flag \
            pole (a static object), the escape-range check of the cast drops targets that are no \
            creatures and aborts the cast (Java's list holds a null there), and the shipped \
            condition canTakeFort is not modelled (stats/conditions.rs). Needs its own card."]
async fn flag_display_takes_the_fortress() {
    let mut world = BareWorld::start(&opts(true, true));
    let played = play(&mut world, true, Finish::Skill).await;
    assert_played(&played, true);
}
