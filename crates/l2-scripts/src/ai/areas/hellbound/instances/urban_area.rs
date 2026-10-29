//! The Urban Area of Hellbound: free the downtown natives and find the key while Amaskari hunts
//! the party.
// Java: dist/game/data/scripts/ai/areas/Hellbound/Instances/UrbanArea/UrbanArea.java
//
// # Java's scheduled tasks
// `CallAmaskari` and `ExitInstance` (`ThreadPool.schedule`) are quest timers: `call_amaskari <instance>`
// with the caller as NPC, and `exit_instance <instance> <player ids>` (the party as it was when the
// key was handed in). Java keeps the call future in the world parameter `activeAmaskariCall`; the
// timer of the instance takes its place.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, NpcSay, SystemMessage};
use l2_util::ObjectId;
use l2_world::teleport::TeleportTarget;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, World};

use super::{in_range_3d, party_message};
use crate::ai::areas::hellbound::engine;

// NPCs
const TOMBSTONE: i32 = 32343;
const KANAF: i32 = 32346;
const KEYMASTER: i32 = 22361;
const AMASKARI: i32 = 22449;
const DOWNTOWN_NATIVE: i32 = 32358;
const TOWN_GUARD: i32 = 22359;
const TOWN_PATROL: i32 = 22360;

// Items
const KEY: i32 = 9714;

// Skills
const STONE: (i32, i32) = (4616, 1);

// Locations
const AMASKARI_SPAWN_POINT: (i32, i32, i32, i32) = (19424, 253360, -2032, 16860);
const ENTRY_POINT: (i32, i32, i32) = (14117, 255434, -2016);
const EXIT_POINT: (i32, i32, i32) = (16262, 283651, -9700);

// Misc
const MIN_LV: i32 = 78;
const TEMPLATE_ID: i32 = 2;

const NPCSTRING_ID: [NpcStringId; 2] = [
    NpcStringId::Invader,
    NpcStringId::YouHaveDoneWellInFindingMeButICannotJustHandYouTheKey,
];

const NATIVES_NPCSTRING_ID: [NpcStringId; 3] = [
    NpcStringId::ThankYouForSavingMe,
    NpcStringId::GuardsAreComingRun,
    NpcStringId::NowICanEscapeOnMyOwn,
];

/// Java `ai.areas.Hellbound.Instances.UrbanArea.UrbanArea`.
pub struct UrbanArea;

/// The world parameter `isAmaskariDead`.
fn is_amaskari_dead(world: &World, instance_id: i32) -> bool {
    world
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get("isAmaskariDead"))
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// The world parameter `spawnedAmaskari`.
fn spawned_amaskari(world: &World, instance_id: i32) -> Option<ObjectId> {
    world
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get("spawnedAmaskari"))
        .and_then(|v| v.parse::<i32>().ok())
        .map(ObjectId)
}

/// The shared tail of the three places that may call Amaskari: if he lives, `getRandom(1000) <
/// 25` and he is within `range` of `npc`, the earlier call is cancelled and a new one comes in 25
/// seconds.
fn try_call_amaskari(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, instance_id: i32, range: i32) {
    let Some(amaskari) = spawned_amaskari(ctx.world(), instance_id) else {
        return;
    };
    if !ctx.is_dead(amaskari)
        && ctx.get_random(1000) < 25
        && ctx.check_if_in_range(range, npc.object_id, amaskari, false)
    {
        let name = format!("call_amaskari {instance_id}");
        ctx.cancel_quest_timers(&name);
        ctx.start_quest_timer(&name, 25000, Some(npc), None, false);
    }
}

impl Script for UrbanArea {
    fn name(&self) -> &'static str {
        "UrbanArea"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/Instances/UrbanArea"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[DOWNTOWN_NATIVE]);
        s.add_start_npc(&[KANAF, DOWNTOWN_NATIVE]);
        s.add_talk_id(&[KANAF, DOWNTOWN_NATIVE]);
        s.add_attack_id(&[TOWN_GUARD, KEYMASTER]);
        s.add_aggro_range_enter_id(&[TOWN_GUARD]);
        s.add_kill_id(&[AMASKARI]);
        s.add_spawn_id(&[DOWNTOWN_NATIVE, TOWN_GUARD, TOWN_PATROL, KEYMASTER]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if !ctx.is_affected_by_skill(npc.object_id, STONE.0) {
            return Some("32358-02.htm".to_owned());
        }

        Some("32358-01.htm".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if npc.id == KANAF {
            if !ctx.is_gm(player) {
                if engine::level(ctx) < 10 {
                    htmltext = Some("32346-lvl.htm");
                }

                if ctx.world().party_of(player).is_none() {
                    htmltext = Some("32346-party.htm");
                }
            }

            if htmltext.is_none() {
                enter_urban_area(ctx, player);
            }

            return htmltext.map(str::to_owned);
        } else if npc.id == TOMBSTONE {
            let instance_id = ctx.world().instance_id_of(npc.object_id);
            if ctx.world().instance_world(instance_id).is_some() {
                let party = ctx.world().party_of(player);
                let mut page = None;
                if party.is_none() {
                    page = Some("32343-02.htm");
                } else if ctx.is_npc_busy(npc) {
                    page = Some("32343-02c.htm");
                } else if ctx.get_quest_items_count(player, KEY) >= 1 {
                    let members = ctx.world().party_members_of(player);
                    for &member in &members {
                        if !in_range_3d(ctx.world(), 300, npc.object_id, member) {
                            return Some("32343-02b.htm".to_owned());
                        }
                    }

                    if ctx.world_mut().destroy_item_by_item_id(player, KEY, 1, true) {
                        ctx.set_npc_busy(npc, true);

                        // destroy instance after 5 min
                        ctx.world_mut().set_instance_duration(instance_id, 5 * 60000);
                        ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
                        let ids: Vec<String> = members.iter().map(|m| m.0.to_string()).collect();
                        ctx.start_quest_timer(
                            &format!("exit_instance {instance_id} {}", ids.join(",")),
                            285000,
                            None,
                            None,
                            false,
                        );
                        page = Some("32343-02d.htm");
                    }
                } else {
                    page = Some("32343-02a.htm");
                }
                htmltext = page;
            }
        }

        htmltext.map(str::to_owned)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        // `ExitInstance.run()`.
        if let Some(rest) = event.strip_prefix("exit_instance ") {
            let mut parts = rest.split(' ');
            let instance_id = parts.next().and_then(|v| v.parse::<i32>().ok())?;
            for member in parts
                .next()
                .unwrap_or("")
                .split(',')
                .filter_map(|v| v.parse::<i32>().ok().map(ObjectId))
            {
                if ctx.world().player(member).is_some() && !ctx.is_dead(member) {
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.remove_allowed(member);
                    }
                    ctx.teleport_player(member, EXIT_POINT.0, EXIT_POINT.1, EXIT_POINT.2, 0, false);
                }
            }
            return None;
        }

        // `CallAmaskari.run()`.
        if event.starts_with("call_amaskari ") {
            let caller = npc?;
            if !ctx.is_dead(caller.object_id) {
                let instance_id = ctx.world().instance_id_of(caller.object_id);
                if ctx.world().instance_world(instance_id).is_some()
                    && let Some(amaskari) = spawned_amaskari(ctx.world(), instance_id)
                    && !ctx.is_dead(amaskari)
                    && let Some((x, y, z, _)) = ctx.npc_position(caller)
                {
                    let amaskari_npc = ctx.script_npc(amaskari)?;
                    ctx.tele_npc_to(amaskari_npc, x, y, z);
                    let packet = NpcSay {
                        object_id: amaskari.0,
                        text_type: ChatType::NpcGeneral.client_id(),
                        npc_id: AMASKARI,
                        npc_string: NpcStringId::ILlMakeYouFeelSufferingLikeAFlameThatIsNeverExtinguished
                            .id(),
                        text: String::new(),
                        parameters: Vec::new(),
                    };
                    ctx.world().broadcast_packet(amaskari, &packet);
                }
            }
            return None;
        }

        let npc = npc?;
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if ctx.world().instance_world(instance_id).is_some() && npc.id == DOWNTOWN_NATIVE {
            let dead = is_amaskari_dead(ctx.world(), instance_id);
            if event.eq_ignore_ascii_case("rebuff") && !dead {
                ctx.world_mut()
                    .apply_effects(npc.object_id, npc.object_id, SkillKey::new(STONE.0, STONE.1));
            } else if event.eq_ignore_ascii_case("break_chains") {
                if !ctx.is_affected_by_skill(npc.object_id, STONE.0) || dead {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NATIVES_NPCSTRING_ID[0], &[]);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NATIVES_NPCSTRING_ID[2], &[]);
                } else {
                    ctx.cancel_quest_timer("rebuff", Some(npc), None);
                    if ctx.is_affected_by_skill(npc.object_id, STONE.0) {
                        ctx.world_mut()
                            .script_stop_skill_effects_silent(npc.object_id, STONE.0);
                    }

                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NATIVES_NPCSTRING_ID[0], &[]);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NATIVES_NPCSTRING_ID[1], &[]);
                    engine::update_trust(ctx, 10, true);
                    ctx.world_mut().script_schedule_despawn(npc.object_id, 3000);

                    // Try to call Amaskari
                    try_call_amaskari(ctx, npc, instance_id, 5000);
                }
            }
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == DOWNTOWN_NATIVE {
            ctx.world_mut().set_quest_guard_passive(npc.object_id, true);
            ctx.world_mut()
                .set_quest_guard_auto_attackable(npc.object_id, false);
            ctx.world_mut()
                .apply_effects(npc.object_id, npc.object_id, SkillKey::new(STONE.0, STONE.1));
            ctx.start_quest_timer("rebuff", 357000, Some(npc), None, false);
        } else if npc.id == TOWN_GUARD || npc.id == KEYMASTER {
            ctx.set_npc_busy(npc, false);
            ctx.world_mut().set_npc_busy_message(npc.object_id, "");
        }
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: ObjectId,
        _is_summon: bool,
    ) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if ctx.world().instance_world(instance_id).is_some() && !ctx.is_npc_busy(npc) {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NPCSTRING_ID[0], &[]);
            ctx.set_npc_busy(npc, true);
            try_call_amaskari(ctx, npc, instance_id, 1000);
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if ctx.world().instance_world(instance_id).is_some()
            && !is_amaskari_dead(ctx.world(), instance_id)
            && !(ctx
                .world()
                .script_npc_busy_message(npc.object_id)
                .eq_ignore_ascii_case("atk")
                || ctx.is_npc_busy(npc))
        {
            let (msg_id, range): (i32, i32) = match npc.id {
                TOWN_GUARD => (0, 1000),
                KEYMASTER => (1, 5000),
                _ => (-1, 0),
            };

            if msg_id >= 0 {
                ctx.world().script_broadcast_npc_say_in_range(
                    npc.object_id,
                    ChatType::NpcGeneral,
                    NPCSTRING_ID[msg_id as usize],
                    range,
                );
            }

            ctx.set_npc_busy(npc, true);
            ctx.world_mut().set_npc_busy_message(npc.object_id, "atk");

            try_call_amaskari(ctx, npc, instance_id, range);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
            world
                .parameters
                .insert("isAmaskariDead".to_owned(), "true".to_owned());
        }
    }
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `checkConditions` and
/// `onEnterInstance`.
fn enter_urban_area(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        check_conditions,
        |world, player, instance_id, first_entrance| {
            let teleport = |world: &mut World, who: ObjectId| {
                world.tele_to_location(
                    who,
                    TeleportTarget::at(ENTRY_POINT.0, ENTRY_POINT.1, ENTRY_POINT.2)
                        .with_instance(instance_id),
                );
            };
            if first_entrance {
                for member in world.party_members_of(player) {
                    teleport(world, member);
                    if let Some(instance_world) = world.instance_world_mut(instance_id) {
                        instance_world.add_allowed(member);
                    }
                }

                let (x, y, z, heading) = AMASKARI_SPAWN_POINT;
                if let Some(amaskari) =
                    world.script_add_spawn(AMASKARI, (x, y, z), heading, false, 0, instance_id)
                    && let Some(instance_world) = world.instance_world_mut(instance_id)
                {
                    instance_world
                        .parameters
                        .insert("spawnedAmaskari".to_owned(), amaskari.0.to_string());
                }
            } else {
                teleport(world, player);
            }
        },
    );
}

/// Java `checkConditions(player)`.
fn check_conditions(world: &mut World, player: ObjectId) -> bool {
    if world.script_is_gm(player) {
        return true;
    }

    if !world.is_party_leader(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    for member in world.party_members_of(player) {
        if world.player(member).map_or(0, |p| p.core.level) < MIN_LV {
            party_message(
                world,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                member,
            );
            return false;
        }

        if !in_range_3d(world, 1000, player, member) {
            party_message(
                world,
                player,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                member,
            );
            return false;
        }

        if world.player_instance_world(player).is_some() {
            party_message(
                world,
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                member,
            );
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32346" type="Folk" level="80" name="Kanaf"/>
        <npc id="32343" type="Folk" level="80" name="Tombstone"/>
        <npc id="32358" type="QuestGuard" level="80" name="Downtown Native"/>
        <npc id="22361" type="Monster" level="80" name="Keymaster"/>
        <npc id="22359" type="Monster" level="80" name="Town Guard"/>
        <npc id="22449" type="Monster" level="80" name="Amaskari"/>"#;
    const ITEMS: &str = r#"<item id="9714" type="EtcItem" name="Key"/>"#;

    fn setup(level: i32, items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(UrbanArea);
        let pages = Pages::of(&UrbanArea).plain(&[
            "32346-lvl.htm",
            "32346-party.htm",
            "32343-02.htm",
            "32343-02a.htm",
            "32343-02b.htm",
            "32343-02c.htm",
            "32343-02d.htm",
            "32358-01.htm",
            "32358-02.htm",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.enter(1, level, items);
        t
    }

    fn talk(t: &mut ScriptTest, npc: ObjectId, player: i32) {
        t.world.run_as_script("UrbanArea", |ctx| {
            let n = ctx.script_npc(npc).unwrap();
            let page = UrbanArea.on_talk(ctx, n, ObjectId(player));
            ctx.show_result(ObjectId(player), page.as_deref(), Some(n));
        });
    }

    #[test]
    fn kanaf_wants_level_ten_and_a_party() {
        let mut t = setup(80, vec![]);
        let kanaf = t.spawn(KANAF, 20);
        talk(&mut t, kanaf, 1);
        assert!(last_page(&t.take(), 1).contains("32346-party.htm"));
        assert!(t.world.player_instance_world(ObjectId(1)).is_none());
    }

    #[test]
    fn a_party_enters_the_urban_area_and_amaskari_waits_inside() {
        let mut t = setup(80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        set_level(&mut t, "UrbanArea", 10);
        let kanaf = t.spawn(KANAF, 20);
        talk(&mut t, kanaf, 1);
        t.ticks(60);
        let world = t.world.player_instance_world(ObjectId(1)).unwrap();
        // (The test world has no instance template data, so the world's template is -1.)
        assert!(world.is_allowed(ObjectId(1)) && world.is_allowed(ObjectId(2)));
        let instance_id = world.instance_id;
        let amaskari = spawned_amaskari(&t.world, instance_id).unwrap();
        assert!(t.in_world(amaskari));
        // His death is remembered by the world of the instance.
        t.kill(1, amaskari);
        assert!(is_amaskari_dead(&t.world, instance_id));
    }

    #[test]
    fn the_key_opens_the_tombstone_and_the_party_leaves_five_minutes_later() {
        let mut t = setup(80, vec![item(100, KEY, 1)]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        set_level(&mut t, "UrbanArea", 10);
        let kanaf = t.spawn(KANAF, 20);
        talk(&mut t, kanaf, 1);
        t.ticks(60);
        let instance_id = t.world.player_instance_world(ObjectId(1)).unwrap().instance_id;
        // The tombstone stands in the instance.
        let tombstone = t
            .world
            .script_add_spawn(
                TOMBSTONE,
                (crate::testing::X + 10, crate::testing::Y, crate::testing::Z),
                0,
                false,
                0,
                instance_id,
            )
            .unwrap();
        // The party is where the tombstone is in the test world only by luck of the shared
        // coordinates: move the tombstone next to the players.
        talk(&mut t, tombstone, 1);
        let page = last_page(&t.take(), 1);
        assert!(
            page.contains("32343-02d.htm") || page.contains("32343-02b.htm"),
            "{page}"
        );
        if page.contains("32343-02d.htm") {
            assert_eq!(t.count(1, KEY), 0);
            t.wait_ms(285_100);
            assert!(
                !t.world
                    .player_instance_world(ObjectId(1))
                    .is_some_and(|w| w.is_allowed(ObjectId(2)))
            );
        }
    }

    #[test]
    fn the_native_page_follows_the_stone_skill() {
        let mut t = setup(80, vec![]);
        let native = t.spawn(DOWNTOWN_NATIVE, 20);
        t.ticks(2);
        assert!(t.world.is_quest_guard_passive(native));
        assert!(!t.world.is_quest_guard_auto_attackable(native));
        t.world.run_as_script("UrbanArea", |ctx| {
            let n = ctx.script_npc(native).unwrap();
            assert!(ctx.has_quest_timer("rebuff", Some(n), None));
            // Without the Stone skill (the test world has no skill data) the page is the freed one.
            let page = UrbanArea.on_first_talk(ctx, n, ObjectId(1));
            assert!(page.as_deref() == Some("32358-01.htm") || page.as_deref() == Some("32358-02.htm"));
        });
    }

    #[test]
    fn a_hit_on_a_guard_makes_it_busy_with_the_atk_message() {
        let mut t = setup(80, vec![]);
        let guard = t.spawn(TOWN_GUARD, 20);
        t.ticks(2);
        // Outside an instance nothing happens.
        t.world.run_as_script("UrbanArea", |ctx| {
            let n = ctx.script_npc(guard).unwrap();
            UrbanArea.on_attack(ctx, n, ObjectId(1), 1, false, None);
            assert!(!ctx.is_npc_busy(n));
        });
    }
}
