//! Scenario: a GM-started castle siege of Gludio. The Lions own the castle, the Wolves attack.
//!
//! The lord posts a mercenary with a ticket, the raider registers, the GM starts the siege
//! (`//startsiege Gludio`), the mercenary stands as a guard, the raider builds the headquarters,
//! breaks the gate, engraves the holy artefact (the castle changes hands in the middle of the
//! siege) and the GM ends the siege. Java: `Siege`, `Castle`, `MercTicket`, `HeadquarterCreate`,
//! `TakeCastle`, `AdminSiege`, `RequestJoinSiege`.
//!
//! With `SiegeDatabaseWrites = False` (D-51) the world plays all of it but the global rows do not
//! change (the registration, the owner, the next siege date, the cleared lists); what a player did
//! himself (the hire of the mercenary) is written. With `True` the rows are Java's.

use l2_protocol_game::server::siege::{ExShowCastleInfo, SiegeAttackerList, SiegeInfo, SiegeTime};
use l2_protocol_game::server::{
    DeleteObject, DoorStatusUpdate, NpcInfo, PledgeShowInfoUpdate, SpawnItem, StaticObjectInfo,
};

use super::*;

const LIONS: i32 = m7_seed::LIONS;
const WOLVES: i32 = m7_seed::WOLVES;
const GLUDIO: i32 = m7_data::GLUDIO;

pub fn opts(siege_writes: bool) -> Opts {
    Opts {
        siege_writes,
        positions: [m7_data::LORD_POS, m7_data::RAIDER_START, m7_data::GM_POS],
        // The hall's auction is not part of this scenario: far away.
        auction_ms: 86_400_000,
        flag_condition: false,
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

fn castle_info_of(batch: &[Vec<u8>]) -> l2_protocol_game::server::siege::CastleInfo {
    let mut infos: Vec<ExShowCastleInfo> = all(batch, "EX_SHOW_CASTLE_INFO");
    let mut castles = infos.remove(0).castles;
    castles.remove(0)
}

fn is_message(id: SystemMessageId) -> impl Fn(&[u8]) -> bool {
    move |body| is_named(body, "SYSTEM_MESSAGE") && decode_any::<SystemMessage>(body).id == id
}

/// The `PledgeShowInfoUpdate` of the last one in `batch`.
fn last_pledge_info(batch: &[Vec<u8>]) -> PledgeShowInfoUpdate {
    all::<PledgeShowInfoUpdate>(batch, "PLEDGE_SHOW_INFO_UPDATE")
        .pop()
        .expect("a PledgeShowInfoUpdate")
}

/// What the scenario hands to the checks after it.
pub struct Played {
    /// The rows before anything happened (the seed), after the hire, and at the end.
    pub rows_start: Rows,
    pub rows_hired: Rows,
    pub rows_end: Rows,
    /// The clock when the siege ended.
    pub ended_at: i64,
    /// The owner the castle window shows after the raider logged in again.
    pub owner_after_relog: String,
    pub raider: Left,
    pub lord: Left,
}

pub async fn play<H: Harness>(h: &mut H, writes: bool) -> Played {
    // ---- everybody enters: the lord's clan and castle come from the database (D-50, D-51) ----------
    let lord_enter = h.enter(Who::Lord).await;
    let gate = all::<StaticObjectInfo>(&lord_enter, "STATIC_OBJECT")
        .into_iter()
        .find(|s| s.kind == StaticObjectInfo::KIND_DOOR)
        .expect("the gate is in view of the lord");
    assert_eq!(gate.static_object_id, m7_data::GATE_ID);
    assert_eq!(
        (gate.current_hp, gate.max_hp),
        (m7_data::GATE_HP, m7_data::GATE_HP)
    );
    assert!(
        gate.show_hp && gate.closed && !gate.is_enemy,
        "peace: the gate is no enemy"
    );
    let artefact = all::<NpcInfo>(&lord_enter, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m7_data::ARTEFACT_ID)
        .expect("the holy artefact is in view of the lord")
        .object_id;
    // The clan came with the character: its window, skills and members.
    assert_in_order(
        &names(&lord_enter),
        &["PLEDGE_SKILL_LIST", "PLEDGE_SHOW_MEMBER_LIST_ALL", "USER_INFO"],
    );
    h.enter(Who::Raider).await;
    h.enter(Who::Gm).await;
    h.collect_quiet(Who::Lord).await;
    let rows_start = h.rows().await;

    // ---- the castle window (RequestAllCastleInfo): the owner the world read at CharacterSelect -----
    h.act(Who::Raider, act::all_castle_info()).await;
    let info = castle_info_of(&h.collect_quiet(Who::Raider).await);
    assert_eq!(
        (info.residence_id, info.owner_name.as_str(), info.tax_percent),
        (GLUDIO, m7_seed::LIONS_NAME, m7_data::CASTLE_TAX)
    );
    assert_eq!(
        i64::from(info.siege_date_secs),
        rows_start.castle(GLUDIO).siege_date / 1000
    );

    // ---- the lord posts a mercenary (MercTicket): a player's own write, with the switch off too ------
    h.act(Who::Lord, act::use_item(m7_seed::LORD_TICKET_ITEM)).await;
    let hired = h.collect_quiet(Who::Lord).await;
    // The mercenary shows for 3 seconds, the ticket lies where the lord stands.
    let merc_shown = all::<NpcInfo>(&hired, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m7_data::MERC_ID)
        .expect("the mercenary shows");
    let ticket_on_ground: SpawnItem = all(&hired, "SPAWN_ITEM").remove(0);
    assert_eq!(ticket_on_ground.display_id, m7_data::MERC_TICKET);
    assert_eq!(
        message_of(
            &hired,
            SystemMessageId::PlaceS1InTheCurrentLocationAndDirectionDoYouWishToContinue
        )
        .params,
        [SystemMessageParam::ItemName(m7_data::MERC_TICKET)]
    );
    // The player's ticket is used up.
    assert!(count_named(&hired, "INVENTORY_UPDATE") > 0);
    h.wait_seconds(4).await;
    let merc_gone = h.collect_quiet(Who::Lord).await;
    assert!(
        all::<DeleteObject>(&merc_gone, "DELETE_OBJECT")
            .iter()
            .any(|d| d.object_id == merc_shown.object_id),
        "the mercenary leaves after 3 seconds"
    );
    let rows_hired = h.rows().await;
    assert_eq!(
        rows_hired.guards,
        [m7_db::GuardRow {
            castle_id: GLUDIO,
            npc_id: m7_data::MERC_ID,
            hired: true
        }],
        "`SiegeGuardManager.saveSiegeGuard` is a player's write"
    );
    assert_eq!(rows_hired.castles, rows_start.castles);

    // ---- the lord cannot register: the owner defends by itself ---------------------------------------
    h.act(Who::Lord, act::join_siege(GLUDIO, 1, 1)).await;
    let refused = h.collect_quiet(Who::Lord).await;
    assert!(has_message(
        &refused,
        SystemMessageId::CastleOwningClansAreAutomaticallyRegisteredOnTheDefendingSide
    ));

    // ---- the raider registers as an attacker: the registrations are read again first (D-50) -------
    h.act(Who::Raider, act::join_siege(GLUDIO, 1, 1)).await;
    let joined = h.collect_quiet(Who::Raider).await;
    let siege_info: SiegeInfo = all(&joined, "CASTLE_SIEGE_INFO").remove(0);
    assert_eq!(siege_info.residence_id, GLUDIO);
    assert_eq!(siege_info.owner_id, LIONS);
    assert!(!siege_info.viewer_is_owner_leader);
    assert_eq!(
        siege_info.time,
        SiegeTime::Date(info.siege_date_secs),
        "the siege is announced for the date of the castle"
    );
    h.act(Who::Raider, act::siege_attacker_list(GLUDIO)).await;
    let attackers: SiegeAttackerList =
        all(&h.collect_quiet(Who::Raider).await, "CASTLE_SIEGE_ATTACKER_LIST").remove(0);
    assert_eq!(attackers.clans.len(), 1);
    assert_eq!(attackers.clans[0].as_ref().unwrap().name, m7_seed::WOLVES_NAME);
    // The row of `Siege.saveSiegeClan` only with the switch (D-51).
    let rows_registered = h.rows().await;
    assert_eq!(
        rows_registered.siege_clans,
        if writes { vec![(GLUDIO, WOLVES, 1)] } else { vec![] }
    );

    // ---- the GM starts the siege: `//startsiege Gludio` --------------------------------------------------
    h.act(Who::Gm, act::admin("startsiege Gludio")).await;
    // The end task counts down from a second after the start.
    h.wait_seconds(2).await;
    let lord_started = h.collect_quiet(Who::Lord).await;
    let raider_started = h.collect_quiet(Who::Raider).await;
    let gm_started = h.collect_quiet(Who::Gm).await;
    for batch in [&lord_started, &raider_started, &gm_started] {
        assert_eq!(
            message_of(batch, SystemMessageId::TheS1SiegeHasStarted).params,
            [SystemMessageParam::CastleName(GLUDIO)],
            "everybody online hears it"
        );
    }
    // The two sides hear how long it lasts: `SiegeLength` 120 minutes.
    for batch in [&lord_started, &raider_started] {
        assert_eq!(
            message_of(batch, SystemMessageId::S1HourSUntilCastleSiegeConclusion).params,
            [SystemMessageParam::IntNumber(2)]
        );
    }
    // The hired mercenary stands as a guard, the tickets leave the ground.
    let guard = all::<NpcInfo>(&lord_started, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m7_data::MERC_ID)
        .expect("the mercenary stands as a guard");
    assert_ne!(guard.object_id, merc_shown.object_id);
    assert!(
        all::<DeleteObject>(&lord_started, "DELETE_OBJECT")
            .iter()
            .any(|d| d.object_id == ticket_on_ground.object_id),
        "the ticket leaves the ground when the siege starts"
    );
    // The gate is an enemy now (`Door.isEnemy` while the zone is on).
    h.act(Who::Lord, act::all_castle_info()).await;
    h.collect_quiet(Who::Lord).await;
    assert_eq!(h.rows().await, rows_registered, "starting writes nothing");

    // ---- the raider walks in and builds the headquarters (Build Headquarters) -------------------------
    walk(h, Who::Raider, m7_data::RAIDER_START, m7_data::RAIDER_POS).await;
    h.act(Who::Raider, act::use_skill(m7_data::BUILD_HQ)).await;
    let built = h.collect_until(Who::Raider, &|b| is_named(b, "NPC_INFO")).await;
    assert!(has_message(&built, SystemMessageId::UseS1));
    // The headquarters stands 50 above the leader; the clan's name is its title.
    let mut seen = built.clone();
    seen.extend(h.collect_quiet(Who::Raider).await);
    let flag_info = all::<NpcInfo>(&seen, "NPC_INFO")
        .into_iter()
        .rfind(|n| n.display_id == m7_data::HQ_ID)
        .expect("the headquarters");
    assert_eq!(flag_info.title, m7_seed::WOLVES_NAME);
    assert_eq!(flag_info.z, m7_data::RAIDER_POS.2 + 50);

    // ---- the gate: the attackers break it down -----------------------------------------------------------
    walk(h, Who::Raider, m7_data::RAIDER_POS, m7_data::GATE_APPROACH).await;
    h.act(Who::Raider, act::click(gate.object_id, m7_data::GATE_APPROACH))
        .await;
    h.collect_quiet(Who::Raider).await;
    h.act(Who::Raider, act::click(gate.object_id, m7_data::GATE_APPROACH))
        .await;
    let broken = h
        .collect_until(
            Who::Raider,
            &is_message(SystemMessageId::TheCastleGateHasBeenDestroyed),
        )
        .await;
    let status: Vec<DoorStatusUpdate> = all(&broken, "DOOR_STATUS_UPDATE");
    assert!(status.iter().all(|s| s.is_enemy && s.door_id == m7_data::GATE_ID));
    assert!(
        status.windows(2).all(|w| w[1].current_hp <= w[0].current_hp),
        "the gate only loses HP: {status:?}"
    );
    assert_eq!(status.last().map(|s| s.current_hp), Some(0));
    // The defenders see the gate fall too.
    let lord_saw = h.collect_quiet(Who::Lord).await;
    assert!(has_message(
        &lord_saw,
        SystemMessageId::TheCastleGateHasBeenDestroyed
    ));

    // ---- the artefact: target it and cast Seal of Ruler -------------------------------------------------------
    walk(h, Who::Raider, m7_data::GATE_APPROACH, m7_data::RAIDER_POS).await;
    h.act(Who::Raider, act::click(artefact, m7_data::RAIDER_POS))
        .await;
    let targeted = h.collect_quiet(Who::Raider).await;
    assert!(names(&targeted).contains(&"MY_TARGET_SELECTED".to_owned()));
    h.act(Who::Raider, act::use_skill(m7_data::SEAL_OF_RULER)).await;
    let engraved = h
        .collect_until(
            Who::Raider,
            &is_message(SystemMessageId::ClanS1HasSuccessfullyEngravedTheHolyArtifact),
        )
        .await;
    assert_eq!(
        message_of(
            &engraved,
            SystemMessageId::ClanS1HasSuccessfullyEngravedTheHolyArtifact
        )
        .params,
        [text(m7_seed::WOLVES_NAME)]
    );
    // The new owner's clan window shows the castle at once.
    assert_eq!(last_pledge_info(&engraved).castle_id, GLUDIO);
    // The defenders heard the engraving start.
    let lord_saw = h.collect_quiet(Who::Lord).await;
    assert!(has_message(
        &lord_saw,
        SystemMessageId::TheOpposingClanHasStartedToEngraveTheHolyArtifact
    ));
    assert!(has_message(
        &lord_saw,
        SystemMessageId::ClanS1HasSuccessfullyEngravedTheHolyArtifact
    ));
    h.act(Who::Raider, act::all_castle_info()).await;
    let info = castle_info_of(&h.collect_quiet(Who::Raider).await);
    assert_eq!(
        info.owner_name,
        m7_seed::WOLVES_NAME,
        "the castle changed hands in this world"
    );
    // `Castle.setOwner` writes the owner, `removeMercs` the old owner's mercenaries: siege jobs.
    let rows_engraved = h.rows().await;
    if writes {
        assert_eq!(rows_engraved.clan(WOLVES).has_castle, GLUDIO);
        assert_eq!(rows_engraved.clan(LIONS).has_castle, 0);
        assert!(
            rows_engraved.guards.is_empty(),
            "the mercenaries went with the owner"
        );
        assert_eq!(rows_engraved.siege_clans, rows_registered.siege_clans);
    } else {
        assert_eq!(rows_engraved, rows_registered, "D-51: no global row changed");
    }

    // ---- the GM ends the siege: the winners, the points, the next siege -----------------------------------
    h.act(Who::Gm, act::admin("endsiege Gludio")).await;
    let raider_end = h.collect_quiet(Who::Raider).await;
    let lord_end = h.collect_quiet(Who::Lord).await;
    let ended_at = now_ms();
    for batch in [&raider_end, &lord_end] {
        assert_eq!(
            message_of(batch, SystemMessageId::TheS1SiegeHasFinished).params,
            [SystemMessageParam::CastleName(GLUDIO)]
        );
        assert_eq!(
            message_of(batch, SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege).params,
            [text(m7_seed::WOLVES_NAME), text("Gludio")]
        );
        assert!(has_message(
            batch,
            SystemMessageId::S1HasAnnouncedTheNextCastleSiegeTime
        ));
        assert!(has_message(batch, SystemMessageId::YouHaveLeftACombatZone));
    }
    // `Castle.updateClansReputation`: the Wolves take `TakeCastlePoints` (1500) from the Lions, who
    // lose `LooseCastlePoints` (3000). The new owner's window shows it.
    assert_eq!(
        message_of(
            &raider_end,
            SystemMessageId::SinceYourClanEmergedVictoriousFromTheSiegeS1PointsHaveBeenAddedToYourClanSReputationScore
        )
        .params,
        [SystemMessageParam::IntNumber(1500)]
    );
    assert_eq!(
        message_of(
            &lord_end,
            SystemMessageId::SinceYourClanWasDefeatedInASiegeS1PointsHaveBeenDeductedFromYourClanSReputationScoreAndGivenToTheOpposingClan
        )
        .params,
        [SystemMessageParam::IntNumber(3000)]
    );
    assert_eq!(
        (
            last_pledge_info(&raider_end).castle_id,
            last_pledge_info(&raider_end).reputation_score
        ),
        (GLUDIO, m7_seed::CLAN_REPUTATION + 1500)
    );
    assert_eq!(
        (
            last_pledge_info(&lord_end).castle_id,
            last_pledge_info(&lord_end).reputation_score
        ),
        (0, m7_seed::CLAN_REPUTATION - 3000)
    );
    let rows_end = h.rows().await;

    // ---- the raider logs in again: the world reads his clan's residences again (D-50, D-51) ---------------------
    let raider = h.leave(Who::Raider).await;
    h.enter(Who::Raider).await;
    h.act(Who::Raider, act::all_castle_info()).await;
    let owner_after_relog = castle_info_of(&h.collect_quiet(Who::Raider).await).owner_name;
    h.leave(Who::Raider).await;
    let lord = h.leave(Who::Lord).await;
    h.leave(Who::Gm).await;
    Played {
        rows_start,
        rows_hired,
        rows_end,
        ended_at,
        owner_after_relog,
        raider,
        lord,
    }
}

/// The rows after the siege: with the switch Java's statements (`Siege.endSiege`, `saveCastleSiege`,
/// `Castle.setOwner`), without it nothing global.
pub fn assert_played(played: &Played, writes: bool) {
    let (start, end) = (&played.rows_start, &played.rows_end);
    if writes {
        // `Castle.updateOwnerInDB`: the Wolves have the castle, the Lions do not.
        assert_eq!(end.clan(WOLVES).has_castle, GLUDIO);
        assert_eq!(end.clan(LIONS).has_castle, 0);
        // `Siege.saveCastleSiege`: the next siege on the schedule (a Sunday two weeks on), the
        // registration open for a day; `Castle.setTicketBuyCount(0)` for the new owner.
        let castle = end.castle(GLUDIO);
        assert!(
            castle.siege_date > played.ended_at && castle.siege_date < played.ended_at + 15 * m7_data::DAY_MS,
            "{castle:?}"
        );
        assert!(!castle.reg_time_over);
        // A calendar day: 24 hours, 23 or 25 when the clocks change (the zone of the machine).
        let hour = m7_data::DAY_MS / 24;
        let open_for = castle.reg_time_end - played.ended_at;
        assert!(
            (23 * hour - 60_000..=25 * hour + 60_000).contains(&open_for),
            "the registration for the next siege is open for a day: {castle:?}"
        );
        assert_eq!(castle.ticket_buy_count, 0);
        assert_eq!(
            (castle.tax_percent, castle.treasury),
            (m7_data::CASTLE_TAX, 1_000)
        );
        // `clearSiegeClan`: the registrations go; `removeMercs`: so do the old owner's mercenaries.
        assert!(end.siege_clans.is_empty());
        assert!(end.guards.is_empty());
        assert_eq!(
            played.owner_after_relog,
            m7_seed::WOLVES_NAME,
            "the database agrees"
        );
    } else {
        // D-51: the only row that changed is the hire of the player, which was written when it
        // happened.
        assert_eq!(end.castles, start.castles);
        assert_eq!(end.clans, start.clans);
        assert_eq!(end.siege_clans, start.siege_clans);
        assert_eq!(end, &played.rows_hired);
        assert_ne!(end.guards, start.guards);
        // The raider logs in again and the world reads his clan's residences: the database says the
        // Wolves have no castle (Sieghardt decides, D-51), so the castle is not theirs any more.
        assert_ne!(played.owner_after_relog, m7_seed::WOLVES_NAME);
    }
    // The characters: nobody paid anything (the headquarters costs MP here), the ticket is used.
    assert_eq!(played.lord.count_of(m7_data::MERC_TICKET), 0);
    assert_eq!(played.raider.count_of(ADENA_ID), m6_data::START_ADENA);
    assert_eq!(played.raider.clan_id, WOLVES);
    assert_eq!(played.lord.clan_id, LIONS);
}

#[tokio::test]
async fn a_gm_started_castle_siege_runs_in_a_bare_world_without_global_writes() {
    let mut world = BareWorld::start(&opts(false));
    let played = play(&mut world, false).await;
    assert_played(&played, false);
}

#[tokio::test]
async fn a_gm_started_castle_siege_runs_in_a_bare_world_and_writes_javas_rows() {
    let mut world = BareWorld::start(&opts(true));
    let played = play(&mut world, true).await;
    assert_played(&played, true);
}

/// D-50, D-51: Sieghardt registered the Wolves for the siege while this server runs. The raider's
/// request reads the registrations first and finds his clan on the list: Java's "You have already
/// requested a castle siege." and no second row.
pub async fn play_sieghardt_registered_first<H: Harness>(h: &mut H) {
    h.enter(Who::Lord).await;
    h.enter(Who::Raider).await;
    h.collect_quiet(Who::Raider).await;
    let before = h.rows().await;
    assert!(before.siege_clans.is_empty());

    h.sieghardt(Sieghardt::RegisterAttacker {
        castle_id: GLUDIO,
        clan_id: WOLVES,
    })
    .await;
    h.act(Who::Raider, act::join_siege(GLUDIO, 1, 1)).await;
    let refused = h.collect_quiet(Who::Raider).await;
    assert!(has_message(
        &refused,
        SystemMessageId::YouHaveAlreadyRequestedACastleSiege
    ));
    assert_eq!(
        h.rows().await.siege_clans,
        [(GLUDIO, WOLVES, 1)],
        "the one row of Sieghardt"
    );

    // The list shows what the database has.
    h.act(Who::Raider, act::siege_attacker_list(GLUDIO)).await;
    let attackers: SiegeAttackerList =
        all(&h.collect_quiet(Who::Raider).await, "CASTLE_SIEGE_ATTACKER_LIST").remove(0);
    assert_eq!(attackers.clans.len(), 1);
    assert_eq!(attackers.clans[0].as_ref().unwrap().name, m7_seed::WOLVES_NAME);
    h.leave(Who::Raider).await;
    h.leave(Who::Lord).await;
}

#[tokio::test]
async fn a_registration_of_sieghardt_is_found_by_the_reread_in_a_bare_world() {
    let mut world = BareWorld::start(&opts(false));
    play_sieghardt_registered_first(&mut world).await;
}

#[tokio::test]
async fn a_registration_of_sieghardt_is_found_by_the_reread_on_the_real_servers() {
    let options = opts(false);
    let Some(mut net) = Net::start("e2em7c2", &options).await else {
        return;
    };
    play_sieghardt_registered_first(&mut net).await;
    net.finish().await;
}

/// D-50: a registration is acted on only after the registrations were read again; a read that
/// fails changes nothing and refuses it (the world never falls back to its memory).
#[tokio::test]
async fn a_failed_reread_refuses_the_registration_in_a_bare_world() {
    let mut world = BareWorld::start(&opts(true));
    world.enter(Who::Lord).await;
    world.enter(Who::Raider).await;
    world.collect_quiet(Who::Raider).await;

    world.fail_reads = true;
    world.act(Who::Raider, act::join_siege(GLUDIO, 1, 1)).await;
    let refused = world.collect_quiet(Who::Raider).await;
    assert!(
        !names(&refused).contains(&"CASTLE_SIEGE_INFO".to_owned()),
        "nothing is shown for a refused registration: {:?}",
        names(&refused)
    );
    assert!(world.rows().await.siege_clans.is_empty());
    world.act(Who::Raider, act::siege_attacker_list(GLUDIO)).await;
    let list = world.collect_quiet(Who::Raider).await;
    assert!(
        !names(&list).contains(&"CASTLE_SIEGE_ATTACKER_LIST".to_owned()),
        "the list waits for the read too"
    );

    // The next read works: the same request is accepted and written.
    world.fail_reads = false;
    world.act(Who::Raider, act::join_siege(GLUDIO, 1, 1)).await;
    let joined = world.collect_quiet(Who::Raider).await;
    assert!(names(&joined).contains(&"CASTLE_SIEGE_INFO".to_owned()));
    assert_eq!(world.rows().await.siege_clans, [(GLUDIO, WOLVES, 1)]);
}

#[tokio::test]
async fn a_gm_started_castle_siege_runs_on_the_real_servers_without_global_writes() {
    let options = opts(false);
    let Some(mut net) = Net::start("e2em7c0", &options).await else {
        return;
    };
    let played = play(&mut net, false).await;
    assert_played(&played, false);
    net.finish().await;
}

#[tokio::test]
async fn a_gm_started_castle_siege_runs_on_the_real_servers_and_writes_javas_rows() {
    let options = opts(true);
    let Some(mut net) = Net::start("e2em7c1", &options).await else {
        return;
    };
    let played = play(&mut net, true).await;
    assert_played(&played, true);
    net.finish().await;
}
