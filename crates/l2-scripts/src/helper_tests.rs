//! Tests of the `ScriptCtx` helpers of S-DATA-01 (`l2_world::script::player_help`), run through a
//! probe script that calls them from an event the way a quest would.

use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId as Oid;
use l2_world::{
    Intention, ItemData, PlayerClanState, Script, ScriptCtx, ScriptNpc, ScriptRegistry, ScriptSettings,
    ScriptSetup, SkillKey,
};

use crate::testing::{Pages, ScriptTest, item, names_to, player_data};

const NPC_ID: i32 = 31_000;
const NPCS: &str = r#"<npc id="31000" type="Folk" level="70" name="Probe"/>
    <npc id="31001" type="Monster" level="10" name="Mob"/>"#;
const ITEMS: &str = r#"<item id="10280" type="EtcItem" name="Certificate"><set name="is_stackable" val="true"/></item>
    <item id="10281" type="EtcItem" name="Mark"><set name="is_stackable" val="true"/><set name="is_questitem" val="true"/></item>
    <item id="10282" type="Weapon" name="Wand"><set name="bodypart" val="rhand"/><set name="weapon_type" val="SWORD"/></item>"#;

struct Probe;

impl Script for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }

    fn path(&self) -> &'static str {
        "test/Probe"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NPC_ID]);
        s.add_talk_id(&[NPC_ID]);
        s.add_first_talk_id(&[NPC_ID]);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: Oid) -> Option<String> {
        Some("p.html".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<Oid>,
    ) -> Option<String> {
        let player = player?;
        match event {
            "clan" => Some(format!(
                "<html><body>sponsor={} apprentice={} pledge={}</body></html>",
                ctx.player_sponsor(player),
                ctx.player_apprentice(player),
                ctx.player_pledge_type(player)
            )),
            "social" => {
                ctx.send_social_action(player, 3);
                None
            }
            "warehouse" => {
                let before = ctx.private_warehouse_item(player, Oid(900));
                let destroyed = ctx.destroy_private_warehouse_item(player, Oid(900), 1);
                let after = ctx.private_warehouse_item(player, Oid(900));
                let too_many = ctx.destroy_private_warehouse_item(player, Oid(900), 99);
                let missing = ctx.destroy_private_warehouse_item(player, Oid(12345), 1);
                Some(format!(
                    "<html><body>{before:?} {destroyed} {after:?} {too_many} {missing}</body></html>"
                ))
            }
            _ => None,
        }
    }
}

fn setup() -> (ScriptTest, Oid) {
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    let pages = Pages::of(&Probe).page("p.html", &["clan", "social", "warehouse"]);
    let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
    let npc = t.spawn(NPC_ID, 20);
    (t, npc)
}

fn enter(t: &mut ScriptTest, npc: Oid, id: i32, mut data: l2_world::PlayerData) {
    data.object_id = Oid(id);
    t.enter_with(data);
    t.talk(id, npc);
    t.take();
}

#[test]
fn sponsor_apprentice_and_pledge_type_come_from_the_clan_state() {
    let (mut t, npc) = setup();
    // A player without a clan: 0, 0, 0 (`Player._pledgeType` starts at 0).
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    assert!(
        t.event(1, "Probe", "clan")
            .contains("sponsor=0 apprentice=0 pledge=0")
    );
    // An academy member with a sponsor, and its sponsor. The state is set directly (the test world
    // has no clan table, so a real clan id would be reset when the player enters).
    let mut academy = player_data(2, 20, vec![]);
    academy.profile.clan = Some(PlayerClanState {
        clan_id: 0,
        pledge_type: -1,
        sponsor: 3,
        ..Default::default()
    });
    enter(&mut t, npc, 2, academy);
    assert!(
        t.event(2, "Probe", "clan")
            .contains("sponsor=3 apprentice=0 pledge=-1")
    );
    let mut sponsor = player_data(3, 80, vec![]);
    sponsor.profile.clan = Some(PlayerClanState {
        clan_id: 0,
        apprentice: 2,
        ..Default::default()
    });
    enter(&mut t, npc, 3, sponsor);
    assert!(
        t.event(3, "Probe", "clan")
            .contains("sponsor=0 apprentice=2 pledge=0")
    );
}

#[test]
fn a_social_action_goes_to_the_player_only() {
    let (mut t, npc) = setup();
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    enter(&mut t, npc, 2, player_data(2, 80, vec![]));
    t.bypass(1, "Script Probe social");
    let out = t.take();
    assert_eq!(
        names_to(&out, 1).iter().filter(|n| *n == "SOCIAL_ACTION").count(),
        1
    );
    assert!(!names_to(&out, 2).contains(&"SOCIAL_ACTION".to_owned()));
    let packet = out
        .iter()
        .find(|(to, body)| *to == 1 && crate::testing::name_of(body) == "SOCIAL_ACTION")
        .map(|(_, body)| crate::testing::decode::<l2_protocol_game::server::SocialAction>(body))
        .unwrap();
    assert_eq!((packet.object_id, packet.action_id), (1, 3));
}

#[test]
fn a_warehouse_item_is_destroyed_by_count() {
    let (mut t, npc) = setup();
    let mut data = player_data(1, 80, vec![item(901, 10280, 5)]);
    let mut book = item(900, 10280, 2);
    book.loc = "WAREHOUSE".to_owned();
    data.profile.warehouse.private.push(book);
    enter(&mut t, npc, 1, data);
    // 2 -> 1 (`destroyItem` with a smaller count only lowers the stack), then 99 is too many and an
    // unknown object id finds nothing.
    let page = t.event(1, "Probe", "warehouse");
    assert!(
        page.contains("Some((10280, 2)) true Some((10280, 1)) false false"),
        "{page}"
    );
    // The bag is not touched.
    assert_eq!(t.count(1, 10280), 5);
    // The last unit deletes the item.
    assert!(t.world.destroy_private_warehouse_item(Oid(1), Oid(900), 1));
    assert_eq!(t.world.private_warehouse_item(Oid(1), Oid(900)), None);
}

#[test]
fn set_noble_sets_the_flag() {
    let (mut t, probe) = setup();
    enter(&mut t, probe, 1, player_data(1, 80, vec![]));
    assert!(!t.world.is_noble(Oid(1)));
    // The script helper is the one quests call.
    t.world.set_noble(Oid(1), true);
    assert!(t.world.is_noble(Oid(1)));
}

#[test]
fn summoned_npcs_are_counted_until_they_decay() {
    let (mut t, probe) = setup();
    assert_eq!(t.world.summoned_npc_count(probe), 0);
    let a = t.spawn(31_001, 40);
    let b = t.spawn(31_001, 60);
    t.world.add_summoned_npc(probe, a);
    t.world.add_summoned_npc(probe, b);
    assert_eq!(t.world.summoned_npc_count(probe), 2);
    assert_eq!(t.world.summoner_of(a), Some(probe));
    // `Npc.onDecay` removes the NPC from its summoner's list.
    assert!(t.world.delete_npc(a));
    assert_eq!(t.world.summoned_npc_count(probe), 1);
    assert_eq!(t.world.summoner_of(b), Some(probe));
}

#[test]
fn the_killing_blow_weapon_is_the_killers_right_hand_item() {
    let (mut t, _) = setup();
    let mut sword = item(300, 10280, 1);
    sword.loc = "PAPERDOLL".to_owned();
    sword.loc_data = 5;
    t.enter_with(player_data(1, 80, vec![sword]));
    t.enter_with(player_data(2, 80, vec![]));
    let a = t.spawn(31_001, 40);
    let b = t.spawn(31_001, 60);
    assert_eq!(t.world.killing_blow_weapon(a), 0);
    t.kill(1, a);
    assert_eq!(t.world.killing_blow_weapon(a), 10280);
    // No weapon: 0 (the fists item is not in this world's templates).
    t.kill(2, b);
    assert_eq!(t.world.killing_blow_weapon(b), 0);
}

/// A monster and a player next to it, for the helpers that act on an NPC.
fn with_mob() -> (ScriptTest, ScriptNpc) {
    let (mut t, npc) = setup();
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    let object_id = t.spawn(31_001, 40);
    (
        t,
        ScriptNpc {
            object_id,
            id: 31_001,
        },
    )
}

fn skill() -> SkillKey {
    SkillKey::new(4001, 1)
}

#[test]
fn cast_skill_at_sets_the_target_first() {
    let (mut t, mob) = with_mob();
    t.world
        .run_as_script("Probe", |ctx| ctx.cast_skill_at(mob, Some(Oid(1)), skill()));
    assert_eq!(t.world.creature_target(mob.object_id), Some(Oid(1)));
}

#[test]
fn target_of_answers_for_an_npc_and_for_a_player() {
    let (mut t, mob) = with_mob();
    t.world.set_target(Oid(1), Some(mob.object_id));
    let mut seen = (None, None);
    t.world.run_as_script("Probe", |ctx| {
        ctx.world_mut().set_npc_target(mob.object_id, Some(Oid(1)));
        seen = (ctx.target_of(mob.object_id), ctx.target_of(Oid(1)));
    });
    assert_eq!(seen, (Some(Oid(1)), Some(mob.object_id)));
}

#[test]
fn add_skill_cast_desire_sets_the_target() {
    let (mut t, mob) = with_mob();
    t.world.run_as_script("Probe", |ctx| {
        ctx.add_skill_cast_desire(mob, Oid(1), skill(), 100)
    });
    assert_eq!(t.world.creature_target(mob.object_id), Some(Oid(1)));
}

#[test]
fn add_attack_desire_makes_the_npc_run() {
    let (mut t, mob) = with_mob();
    assert!(!t.world.is_running(mob.object_id));
    t.world
        .run_as_script("Probe", |ctx| ctx.add_attack_desire(mob, Oid(1), 100));
    assert!(t.world.is_running(mob.object_id));
}

#[test]
fn set_running_makes_the_npc_run() {
    let (mut t, mob) = with_mob();
    assert!(!t.world.is_running(mob.object_id));
    t.world.run_as_script("Probe", |ctx| ctx.set_running(mob));
    assert!(t.world.is_running(mob.object_id));
}

#[test]
fn npc_move_to_gives_the_move_intention() {
    let (mut t, mob) = with_mob();
    t.world.run_as_script("Probe", |ctx| {
        ctx.npc_move_to(mob, crate::testing::X + 300, crate::testing::Y, crate::testing::Z);
    });
    assert_eq!(t.world.intention_of(mob.object_id), Intention::MoveTo);
}

#[test]
fn teleport_player_offsets_only_when_allowed() {
    let (mut t, npc) = setup();
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    let (x, y, z) = (crate::testing::X + 1_000, crate::testing::Y, crate::testing::Z);
    t.world
        .run_as_script("Probe", |ctx| ctx.teleport_player(Oid(1), x, y, z, 0, false));
    let (px, py, _) = t.world.position_of(Oid(1)).unwrap();
    assert_eq!((px, py), (x, y));

    enter(&mut t, npc, 2, player_data(2, 80, vec![]));
    t.world
        .run_as_script("Probe", |ctx| ctx.teleport_player(Oid(2), x, y, z, 0, true));
    let (ox, oy, _) = t.world.position_of(Oid(2)).unwrap();
    assert_ne!((ox, oy), (x, y));
    // `MaxOffsetOnTeleport` defaults to 50.
    assert!((ox - x).abs() <= 50 && (oy - y).abs() <= 50);
}

#[test]
fn give_items_multiplies_quest_items_only() {
    let (mut t, npc) = setup();
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    t.world.set_script_settings(ScriptSettings {
        quest_item_drop_amount_multiplier: 2.0,
        ..ScriptSettings::default()
    });
    t.world.run_as_script("Probe", |ctx| {
        ctx.give_items(Oid(1), 10_281, 3);
        ctx.give_items(Oid(1), 10_280, 3);
    });
    assert_eq!(t.count(1, 10_281), 6);
    assert_eq!(t.count(1, 10_280), 3);
}

#[test]
fn take_items_unequips_a_worn_item_and_tells_the_others() {
    let (mut t, npc) = setup();
    let wand = ItemData {
        loc: "PAPERDOLL".to_owned(),
        loc_data: paperdoll::RHAND as i32,
        ..item(500, 10_282, 1)
    };
    enter(&mut t, npc, 1, player_data(1, 80, vec![wand]));
    assert_eq!(t.world.paperdoll_object(Oid(1), paperdoll::RHAND), Some(Oid(500)));
    t.world.run_as_script("Probe", |ctx| {
        ctx.take_items(Oid(1), 10_282, 1);
    });
    assert_eq!(t.count(1, 10_282), 0);
    assert_eq!(t.world.paperdoll_object(Oid(1), paperdoll::RHAND), None);
    let names = names_to(&t.take(), 1);
    assert!(names.iter().any(|name| name == "USER_INFO"), "{names:?}");
}
