//! The rift invasions in a test world: `//rift_start`, waves, boss, rewards, the merchant, the
//! weekly cap, the inventory, and switching it off.

use std::sync::Arc;

use l2_data::{RiftWaveDef, RiftZoneDef};
use l2_util::ObjectId;
use l2_world::custom::CustomScriptSettings;
use l2_world::{ClientInput, DamageInfo, L2rSettings, RiftInvasionSettings, ScriptRegistry};

use super::*;
use crate::testing::{ScriptTest, Sent, X, Y, Z, name_of, names_to, pages_to};

const SHARD: i32 = 4405;
const MERCHANT: i32 = 50_030;
const MOB_A: i32 = 21_682;
const MOB_B: i32 = 21_687;
const BOSS: i32 = 25_335;
const GM: i32 = 1;
const OTHER: i32 = 2;

const NPCS: &str = r#"
<npc id="50030" type="Folk" name="Rissbeschwoerer"/>
<npc id="50031" type="Folk" name="Rift"/>
<npc id="21682" level="40" type="Monster" name="Dimension Invader Soldier"/>
<npc id="21687" level="40" type="Monster" name="Dimension Invader Priest"/>
<npc id="25335" level="48" type="Monster" name="Anakazel"/>
"#;

/// A non-stackable item to fill the inventory with.
const JUNK: i32 = 100;

fn items(shard_weight: i64) -> String {
    format!(
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="4405" type="EtcItem" name="Blue Dimensional Stone - Shard"><set name="weight" val="{shard_weight}" /><set name="is_stackable" val="true" /></item>
<item id="100" type="EtcItem" name="Junk" />"#
    )
}

fn zone() -> RiftZoneDef {
    RiftZoneDef {
        name: "Test Field".to_owned(),
        min_level: 40,
        max_level: 46,
        spawns: vec![(X + 300, Y, Z)],
        waves: vec![
            RiftWaveDef {
                count: 3,
                mobs: vec![MOB_A, MOB_B],
            },
            RiftWaveDef {
                count: 4,
                mobs: vec![MOB_B],
            },
        ],
        boss_id: BOSS,
        boss_hp_multiplier: 1.0,
    }
}

fn rift_settings() -> RiftInvasionSettings {
    RiftInvasionSettings {
        shard_item_id: SHARD,
        merchant_npc_id: MERCHANT,
        merchant_loc: (X + 40, Y, Z),
        wave_count: 2,
        interval_min_ms: 3_600_000,
        interval_max_ms: 3_600_000,
        ..RiftInvasionSettings::default()
    }
}

fn scene_with(settings: RiftInvasionSettings, gm_items: Vec<l2_world::ItemData>) -> ScriptTest {
    let mut registry = ScriptRegistry::new();
    registry.add(RiftInvasion);
    let mut t = ScriptTest::new_with(registry, NPCS, &items(1), &[], |world, _| {
        world.set_custom_script_settings(CustomScriptSettings {
            rift_zones: vec![zone()],
            ..CustomScriptSettings::default()
        });
        world.set_l2r_settings(L2rSettings {
            rift: settings,
            ..L2rSettings::default()
        });
    });
    t.enter(GM, 40, gm_items);
    t.wait_ms(1_100);
    t.take();
    t
}

fn scene() -> ScriptTest {
    scene_with(rift_settings(), vec![])
}

fn start_rift(t: &mut ScriptTest, zone: &str) -> bool {
    let mut started = false;
    t.world
        .run_as_script(NAME, |ctx| started = start(ctx, Some(zone), Some(ObjectId(GM))));
    started
}

fn with_state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut RiftState) -> R) -> R {
    let mut out = None;
    t.world
        .run_as_script(NAME, |ctx| out = Some(f(ctx.state::<RiftState>())));
    out.unwrap()
}

/// The living monsters of the last stage that came out.
fn last_stage(t: &mut ScriptTest) -> Vec<ObjectId> {
    with_state(t, |s| {
        s.stages
            .last()
            .map(|r| r.alive.iter().copied().collect())
            .unwrap_or_default()
    })
}

fn hit(t: &mut ScriptTest, attacker: ObjectId, mob: ObjectId, damage: f64) {
    t.world.run_as_script(NAME, |ctx| {
        RiftInvasion.on_creature_damage_received(
            ctx,
            &DamageInfo {
                attacker: Some(attacker),
                target: mob,
                damage,
                skill: None,
                critical: false,
                damage_over_time: false,
            },
        );
    });
}

/// `player` hits every monster of the last stage and kills it; a second passes.
fn beat_stage(t: &mut ScriptTest, player: i32) {
    for mob in last_stage(t) {
        hit(t, ObjectId(player), mob, 500.0);
        t.world.do_die(mob, Some(ObjectId(player)));
    }
    t.wait_ms(1_100);
}

/// The text of every packet a player got (UTF-16 strings, both alignments).
fn texts_to(sent: &Sent, to: i32) -> String {
    let utf16 = |bytes: &[u8]| {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    };
    sent.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, body)| format!("{} {}", utf16(body), utf16(&body[1..])))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The rift monsters alive and the marker (the dead ones decay as any corpse).
fn rift_npcs(t: &ScriptTest) -> usize {
    [MOB_A, MOB_B, BOSS, MARKER_NPC]
        .iter()
        .flat_map(|id| t.npcs_of(*id))
        .filter(|n| !t.world.is_dead(*n))
        .count()
}

#[test]
fn a_gm_rift_brings_its_waves_then_the_boss_then_the_rewards() {
    let mut t = scene();
    // The merchant stands in Giran (here: next to the players).
    assert_eq!(t.npcs_of(MERCHANT).len(), 1);
    assert!(start_rift(&mut t, "test"));
    let out = t.take();
    assert!(
        texts_to(&out, GM).contains("has opened in Test Field"),
        "{}",
        texts_to(&out, GM)
    );
    // The marker and wave 1: 3 monsters, level 40, no drops, normal exp.
    assert_eq!(t.npcs_of(MARKER_NPC).len(), 1);
    let wave1 = last_stage(&mut t);
    assert_eq!(wave1.len(), 3);
    let level = t.world.stats(wave1[0]).unwrap().level();
    assert_eq!(level, 40);
    let rewards = t.world.npc_reward_override(wave1[0]).unwrap();
    assert!(!rewards.drops);
    assert_eq!((rewards.exp_rate, rewards.sp_rate), (1.0, 1.0));
    // A second start is refused while it runs.
    assert!(!start_rift(&mut t, ""));

    // Wave 1 beaten: wave 2 at once (4 monsters, level 46 = the zone's highest).
    beat_stage(&mut t, GM);
    let wave2 = last_stage(&mut t);
    assert_eq!(wave2.len(), 4);
    assert_eq!(t.world.stats(wave2[0]).unwrap().level(), 46);
    assert!(
        t.world.npc_reward_override(wave1[0]).is_none(),
        "a dead monster's rewards go"
    );
    // Wave 2 beaten: the boss, announced to everybody and on the screen near the rift.
    beat_stage(&mut t, GM);
    let out = t.take();
    assert!(texts_to(&out, GM).contains("Anakazel has come out of the rift"));
    assert!(names_to(&out, GM).iter().any(|n| n == "EX_SHOW_SCREEN_MESSAGE"));
    assert_eq!(t.npcs_of(BOSS).len(), 1);
    assert_eq!(t.count(GM, SHARD), 0, "nothing before the end");

    // The boss dies: 2 waves x 5 + boss 15 + top-3 bonus 5.
    beat_stage(&mut t, GM);
    assert_eq!(t.count(GM, SHARD), 30);
    let out = t.take();
    assert!(texts_to(&out, GM).contains("its boss is slain"));
    // Nothing of the rift is left; the next rift is planned; the merchant stays.
    assert_eq!(rift_npcs(&t), 0);
    assert_eq!(t.npcs_of(MERCHANT).len(), 1);
    t.wait_ms(1_100);
    let planned = with_state(&mut t, |s| (s.machine.phase(), s.machine.until_open().is_some()));
    assert_eq!(planned, (Phase::Idle, true));
    // The weekly count holds the 30.
    assert_eq!(
        t.world
            .player(ObjectId(GM))
            .unwrap()
            .data
            .profile
            .variables
            .get(VAR_WEEK_SHARDS)
            .cloned(),
        Some("30".to_owned())
    );
}

#[test]
fn standing_by_is_not_enough_and_the_top_three_get_the_bonus() {
    let mut t = scene();
    t.enter(OTHER, 40, vec![]);
    t.take();
    assert!(start_rift(&mut t, "1"));
    // OTHER only stands in the radius (presence: 1 point a second) while GM fights.
    t.wait_ms(3_100);
    beat_stage(&mut t, GM);
    beat_stage(&mut t, GM);
    beat_stage(&mut t, GM);
    assert_eq!(t.count(GM, SHARD), 30);
    assert_eq!(t.count(OTHER, SHARD), 0);
    // Both were participants.
    let out = t.take();
    assert!(texts_to(&out, GM).contains("1 defenders were rewarded"));
}

#[test]
fn heals_of_a_fighter_count_and_a_summons_damage_counts_for_its_owner() {
    let mut t = scene();
    t.enter(OTHER, 40, vec![]);
    t.take();
    assert!(start_rift(&mut t, "1"));
    for _ in 0..3 {
        // The healer (OTHER) heals the fighter (GM) for a third of the damage.
        for mob in last_stage(&mut t) {
            hit(&mut t, ObjectId(GM), mob, 600.0);
        }
        t.world.note_heal(ObjectId(OTHER), ObjectId(GM), 600.0);
        t.wait_ms(1_100);
        for mob in last_stage(&mut t) {
            t.world.do_die(mob, Some(ObjectId(GM)));
        }
        t.wait_ms(1_100);
    }
    // Both earned every stage; both are in the top three.
    assert_eq!(t.count(GM, SHARD), 30);
    assert_eq!(t.count(OTHER, SHARD), 30);

    // A summon's damage is its owner's.
    let mut t = scene();
    let summon = t.summon(GM, MOB_A);
    assert!(start_rift(&mut t, "1"));
    let mob = last_stage(&mut t)[0];
    hit(&mut t, summon, mob, 250.0);
    let credited = with_state(&mut t, |s| {
        s.stages[0].stage.contribution.get(&ObjectId(GM)).copied()
    });
    assert!(credited.is_some_and(|c| c >= 250.0), "{credited:?}");
}

#[test]
fn a_rift_that_runs_out_of_time_fails_but_pays_the_beaten_waves() {
    let mut t = scene_with(
        RiftInvasionSettings {
            duration_ms: 30_000,
            wave_interval_ms: 10_000,
            ..rift_settings()
        },
        vec![],
    );
    assert!(start_rift(&mut t, "1"));
    beat_stage(&mut t, GM);
    // Wave 2 is not beaten; after its interval the boss comes; the time runs out.
    t.wait_ms(30_000);
    let out = t.take();
    assert!(texts_to(&out, GM).contains("collapsed"));
    // Only wave 1: 5 shards, no bonus.
    assert_eq!(t.count(GM, SHARD), 5);
    assert_eq!(rift_npcs(&t), 0, "no rift monster is left");
}

#[test]
fn a_planned_rift_is_announced_reminded_and_opens_by_itself() {
    let mut t = scene_with(
        RiftInvasionSettings {
            interval_min_ms: 120_000,
            interval_max_ms: 120_000,
            announce_lead_ms: 90_000,
            ..rift_settings()
        },
        vec![],
    );
    // Planned at the first tick: announced 30 s later, reminded at 60 s, opened at 120 s.
    t.wait_ms(30_000);
    let out = t.take();
    assert!(
        texts_to(&out, GM).contains("about to open in Test Field in 2 minutes"),
        "{}",
        texts_to(&out, GM)
    );
    assert_eq!(rift_npcs(&t), 0);
    t.wait_ms(30_000);
    assert!(texts_to(&t.take(), GM).contains("opens in 1 minute"));
    t.wait_ms(60_000);
    assert!(texts_to(&t.take(), GM).contains("has opened in Test Field"));
    assert_eq!(t.npcs_of(MARKER_NPC).len(), 1);
    assert_eq!(last_stage(&mut t).len(), 3);
}

#[test]
fn more_players_bring_more_and_stronger_monsters() {
    let mut t = scene();
    for id in 2..=6 {
        t.enter(id, 40, vec![]);
    }
    t.take();
    assert!(start_rift(&mut t, "1"));
    // Six players: five beyond the first, 5 x 8 % = 40 %: 3 x 1.4 = 4.2, rounded up to 5.
    let wave = last_stage(&mut t);
    assert_eq!(wave.len(), 5);
    let scale = t.world.npc(wave[0]).unwrap().scale.unwrap();
    assert!((scale.hp - 1.4).abs() < 1e-6, "{}", scale.hp);
    // Without scaling: the zone's count.
    let mut t = scene_with(
        RiftInvasionSettings {
            scale_with_players: false,
            ..rift_settings()
        },
        vec![],
    );
    for id in 2..=6 {
        t.enter(id, 40, vec![]);
    }
    assert!(start_rift(&mut t, "1"));
    assert_eq!(last_stage(&mut t).len(), 3);
}

#[test]
fn shards_that_do_not_fit_wait_and_are_collected_at_the_merchant() {
    // The fighter's inventory is full of junk.
    let junk: Vec<_> = (0..300)
        .map(|i| crate::testing::item(10_000 + i, JUNK, 1))
        .collect();
    let mut t = scene_with(rift_settings(), junk);
    assert!(start_rift(&mut t, "1"));
    beat_stage(&mut t, GM);
    beat_stage(&mut t, GM);
    beat_stage(&mut t, GM);
    assert_eq!(t.count(GM, SHARD), 0);
    let out = t.take();
    assert!(texts_to(&out, GM).contains("30 rift shards wait for you"));
    let pending = t
        .world
        .player(ObjectId(GM))
        .unwrap()
        .data
        .profile
        .variables
        .get(VAR_PENDING)
        .cloned();
    assert_eq!(pending, Some("30".to_owned()));
    // Not on the ground either.
    assert!(!names_to(&out, GM).iter().any(|n| n == "DROP_ITEM"));

    // The merchant: still no room.
    let merchant = t.npcs_of(MERCHANT)[0];
    t.talk(GM, merchant);
    let page = pages_to(&t.take(), GM).pop().unwrap_or_default();
    assert!(page.contains("Collect waiting shards (30)"), "{page}");
    assert!(t.event(GM, NAME, "collect").contains("still full"));
    // Room again: collected.
    for i in 0..300 {
        t.input(
            GM,
            ClientInput::DestroyItem {
                object_id: 10_000 + i,
                count: 1,
            },
        );
    }
    t.talk(GM, merchant);
    t.take();
    assert!(
        t.event(GM, NAME, "collect")
            .contains("Here are your 30 rift shards")
    );
    assert_eq!(t.count(GM, SHARD), 30);
    t.talk(GM, merchant);
    t.take();
    assert!(t.event(GM, NAME, "collect").contains("No rift shards"));
}

#[test]
fn waiting_shards_come_at_the_next_login_and_offline_winners_get_theirs_then() {
    let mut t = scene();
    t.world.run_as_script(NAME, |ctx| {
        ctx.set_player_variable(ObjectId(GM), VAR_PENDING, "7");
        ctx.set_global_variable(&format!("{GLOBAL_OFFLINE}{OTHER}"), "12");
    });
    // The enter world hook of a player with waiting shards.
    t.world
        .run_as_script(NAME, |ctx| RiftInvasion.on_enter_world(ctx, ObjectId(GM)));
    assert_eq!(t.count(GM, SHARD), 7);
    t.enter(OTHER, 40, vec![]);
    t.wait_ms(100);
    assert_eq!(t.count(OTHER, SHARD), 12);
    let left = t
        .world
        .global_variables()
        .get_long(&format!("{GLOBAL_OFFLINE}{OTHER}"), 0);
    assert_eq!(left, 0);
}

#[test]
fn the_weekly_cap_limits_the_shards_until_a_gm_resets_it() {
    let mut t = scene_with(
        RiftInvasionSettings {
            weekly_cap: 40,
            ..rift_settings()
        },
        vec![],
    );
    let run = |t: &mut ScriptTest| {
        assert!(start_rift(t, "1"));
        beat_stage(t, GM);
        beat_stage(t, GM);
        beat_stage(t, GM);
    };
    run(&mut t);
    assert_eq!(t.count(GM, SHARD), 30);
    run(&mut t);
    // 10 of the next 30 fit under the cap.
    assert_eq!(t.count(GM, SHARD), 40);
    assert!(texts_to(&t.take(), GM).contains("weekly limit of 40"));
    run(&mut t);
    assert_eq!(t.count(GM, SHARD), 40);
    // `//rift_cap_reset` (the GM without a target: himself).
    t.world
        .run_as_script(NAME, |ctx| cap_reset(ctx, ObjectId(GM), ""));
    run(&mut t);
    assert_eq!(t.count(GM, SHARD), 70);
}

#[test]
fn the_merchant_shows_the_status_and_opens_the_shop() {
    let mut t = scene();
    let multisell = l2_data::MultisellData::from_xml(
        &[(
            RIFT_MULTISELL,
            r#"<list><npcs><npc>-1</npc></npcs><item><ingredient id="4405" count="10"/><production id="57" count="1"/></item></list>"#,
        )],
        &l2_data::ItemTemplates::from_xml(&format!("<list>{}</list>", items(1))).unwrap(),
        &l2_data::MultisellLoadOptions::default(),
    );
    t.world
        .set_multisell_data(Arc::new(multisell), l2_world::MultisellSettings::default());
    let merchant = t.npcs_of(MERCHANT)[0];
    t.talk(GM, merchant);
    let page = pages_to(&t.take(), GM).pop().unwrap_or_default();
    assert!(
        page.contains("Trade rift shards") && page.contains("Next rift / status"),
        "{page}"
    );
    assert!(
        t.event(GM, NAME, "status")
            .contains("The next one opens in about 60 minutes")
    );
    // The shop link is on the main page.
    t.talk(GM, merchant);
    t.take();
    t.bypass(GM, &format!("Script {NAME} shop"));
    let out = t.take();
    assert!(
        out.iter()
            .any(|(to, b)| *to == GM && name_of(b) == "MULTI_SELL_LIST"),
        "{:?}",
        names_to(&out, GM)
    );
    // While a rift is open the status names it.
    assert!(start_rift(&mut t, "1"));
    t.talk(GM, merchant);
    t.take();
    let status = t.event(GM, NAME, "status");
    assert!(
        status.contains("A rift is open in Test Field: wave 1 of 2"),
        "{status}"
    );
}

#[test]
fn switching_it_off_removes_everything_and_missing_ids_keep_it_off() {
    let mut t = scene();
    assert!(start_rift(&mut t, "1"));
    beat_stage(&mut t, GM);
    assert!(rift_npcs(&t) > 0);
    // `RiftEnabled = false` (//reload l2r): the rift closes (wave 1 pays), the merchant goes.
    let mut settings = t.world.l2r_settings().clone();
    settings.rift.enabled = false;
    t.world.set_l2r_settings(settings);
    t.wait_ms(1_100);
    assert_eq!(rift_npcs(&t), 0);
    assert!(t.npcs_of(MERCHANT).is_empty());
    assert_eq!(t.count(GM, SHARD), 5);
    assert!(!start_rift(&mut t, "1"));
    // Nothing comes back by itself.
    t.wait_ms(10_000);
    assert_eq!(rift_npcs(&t), 0);
    assert_eq!(with_state(&mut t, |s| s.machine.until_open()), None);

    // The ids not set: off, no merchant, no start.
    let mut t = scene_with(
        RiftInvasionSettings {
            shard_item_id: 0,
            ..rift_settings()
        },
        vec![],
    );
    assert!(t.npcs_of(MERCHANT).is_empty());
    assert!(!start_rift(&mut t, "1"));
    assert!(texts_to(&t.take(), GM).contains("RiftShardItemId is 0"));
    // An unknown zone name lists the zones.
    let mut t = scene();
    assert!(!start_rift(&mut t, "nowhere"));
    assert!(texts_to(&t.take(), GM).contains("Zones: 1 Test Field"));
}

#[test]
fn a_stop_closes_the_rift_and_the_bound_shard_stays_with_its_owner() {
    let mut t = scene();
    assert!(start_rift(&mut t, "1"));
    beat_stage(&mut t, GM);
    let mut stopped = false;
    t.world.run_as_script(NAME, |ctx| stopped = stop(ctx));
    assert!(stopped);
    assert_eq!(rift_npcs(&t), 0);
    assert_eq!(t.count(GM, SHARD), 5);
    assert!(texts_to(&t.take(), GM).contains("was closed"));

    // The bound shard cannot be dropped.
    let shard = t
        .world
        .player(ObjectId(GM))
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == SHARD)
        .unwrap()
        .object_id;
    t.input(
        GM,
        ClientInput::DropItem {
            object_id: shard.0,
            count: 5,
            x: X + 20,
            y: Y,
            z: Z,
        },
    );
    assert_eq!(t.count(GM, SHARD), 5);
    assert!(t.world.is_l2r_bound_item(SHARD));
}

#[test]
fn the_shipped_shop_pays_in_shards_and_sells_known_items() {
    let Some(root) = crate::testing::datapack() else {
        return;
    };
    let items = l2_data::ItemTemplates::load(root, false);
    let text = std::fs::read_to_string(root.join(format!("data/multisell/{RIFT_MULTISELL}.xml"))).unwrap();
    let data = l2_data::MultisellData::from_xml(
        &[(RIFT_MULTISELL, &text)],
        &items,
        &l2_data::MultisellLoadOptions::default(),
    );
    let list = data.get(RIFT_MULTISELL).expect("the list loads");
    assert!(list.entries.len() >= 20, "{}", list.entries.len());
    for entry in &list.entries {
        assert!(entry.ingredients.iter().all(|i| i.item_id == SHARD));
        assert!(
            entry.products.iter().all(|p| items.get(p.item_id).is_some()),
            "{entry:?}"
        );
    }
    // The shipped templates of the merchant and the marker.
    let npcs = l2_data::NpcTemplates::load(root, &l2_data::NpcLoadOptions::default());
    assert_eq!(npcs.get(MERCHANT).unwrap().display_id, 32_478);
    assert_eq!(npcs.get(MARKER_NPC).unwrap().display_id, 36_571);
    // The shipped L2R.ini turns the rifts on with these ids.
    let ini = std::fs::read_to_string(root.join("config/L2R.ini")).unwrap();
    assert!(ini.contains("RiftShardItemId = 4405") && ini.contains("RiftMerchantNpcId = 50030"));
}
