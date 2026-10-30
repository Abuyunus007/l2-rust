//! Quest 616: Magical Power of Fire - Part 2 (Udan in the Dion/Gludio area; the Ketra totem summons
//! Nastron, the raid boss, whose heart Udan wants; Nastron respawns after a random time).
// Java: dist/game/data/scripts/quests/Q00616_MagicalPowerOfFirePart2/Q00616_MagicalPowerOfFirePart2.java
//
// Pattern: a raid boss that a quest summons (the totem spawned at load and at the respawn timer),
// the timers of the boss (`spawn_npc`, `despawn_nastron`), the respawn time kept in a global
// variable (D-54: the row is written by the engine's global variables), and the kill that gives
// the heart to every player in range (`executeForEachPlayer`, `actionForEachPlayer`).
// The constructor's spawn and timer are ported as `on_load` (the scripts are installed at start).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const UDAN: i32 = 31379;
const KETRA_TOTEM: i32 = 31558;

// Monster
const NASTRON: i32 = 25306;

// Items
const RED_TOTEM: i32 = 7243;
const NASTRON_HEART: i32 = 7244;

// Misc
const MIN_LEVEL: i32 = 75;
/// Java `GlobalVariablesManager` row of the respawn time (epoch ms).
const RESPAWN_VAR: &str = "Q00616_respawn";

/// Java `addSpawn(KETRA_TOTEM, 142368, -82512, -6487, 58000, false, 0, true)`.
const KETRA_TOTEM_SPAWN: (i32, i32, i32, i32) = (142_368, -82_512, -6_487, 58_000);
/// Java `addSpawn(NASTRON, 142528, -82528, -6496, 0, false, 0)`.
const NASTRON_SPAWN: (i32, i32, i32, i32) = (142_528, -82_528, -6_496, 0);

/// Java `Q00616_MagicalPowerOfFirePart2`.
pub struct Q00616MagicalPowerOfFirePart2;

/// Java `addSpawn(KETRA_TOTEM, ...)`: the totem at its place.
fn spawn_ketra_totem(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z, heading) = KETRA_TOTEM_SPAWN;
    ctx.add_spawn(KETRA_TOTEM, x, y, z, heading, false, 0, 0);
}

/// Java `spawnNastron(npc, qs)`.
fn spawn_nastron(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, qs: l2_world::script::Qs) -> String {
    if ctx.has_quest_timer("spawn_npc", None, None) {
        return "31558-03.html".to_owned();
    }
    if qs.is_cond(ctx, 1) {
        ctx.take_items(qs.player(), RED_TOTEM, 1);
        qs.set_cond(ctx, 2, true);
    }
    ctx.delete_npc(npc);
    let (x, y, z, heading) = NASTRON_SPAWN;
    if let Some(nastron) = ctx.add_spawn(NASTRON, x, y, z, heading, false, 0, 0) {
        ctx.broadcast_npc_say(
            nastron,
            ChatType::NpcGeneral,
            NpcStringId::TheMagicalPowerOfFireIsAlsoThePowerOfFlamesAndLavaIfYouDareToConfrontItOnlyDeathWillAwaitYou,
            &[],
        );
        ctx.start_quest_timer("despawn_nastron", 1_200_000, Some(nastron), None, false);
    }
    "31558-02.html".to_owned()
}

impl Script for Q00616MagicalPowerOfFirePart2 {
    fn name(&self) -> &'static str {
        "Q00616_MagicalPowerOfFirePart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00616_MagicalPowerOfFirePart2"
    }

    fn id(&self) -> i32 {
        616
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[UDAN]);
        s.add_talk_id(&[UDAN, KETRA_TOTEM]);
        s.add_kill_id(&[NASTRON]);
        s.register_quest_items(&[RED_TOTEM, NASTRON_HEART]);
    }

    /// Java constructor: the respawn time is still ahead, so the totem comes back with the timer;
    /// otherwise the totem is there now.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let test = ctx.global_variables().get_long(RESPAWN_VAR, 0);
        let remain = if test != 0 { test - ctx.now_ms() } else { 0 };
        if remain > 0 {
            ctx.start_quest_timer("spawn_npc", remain, None, None, false);
        } else {
            spawn_ketra_totem(ctx);
        }
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
            return;
        }
        if npc.id == NASTRON {
            // Java: case 1 takes the red totem and falls through to case 2.
            let cond = qs.get_cond(ctx);
            if cond == 1 {
                ctx.take_items(player, RED_TOTEM, 1);
            }
            if cond == 1 || cond == 2 {
                if !ctx.has_quest_items(player, NASTRON_HEART) {
                    ctx.give_items(player, NASTRON_HEART, 1);
                }
                qs.set_cond(ctx, 3, true);
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let Some(player) = player else {
            // The timers without a player (Java `onEvent` with `player == null`).
            match event {
                "despawn_nastron" => {
                    if let Some(npc) = npc {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::ThePowerOfConstraintIsGettingWeakerYourRitualHasFailed,
                            &[],
                        );
                        ctx.delete_npc(npc);
                    }
                    spawn_ketra_totem(ctx);
                }
                "spawn_npc" => spawn_ketra_totem(ctx),
                _ => {}
            }
            return None;
        };
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "31379-02.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "give_heart" => {
                if ctx.has_quest_items(player, NASTRON_HEART) {
                    ctx.add_exp_and_sp(player, 10_000, 0);
                    qs.exit_quest(ctx, true, true);
                    Some("31379-06.html".to_owned())
                } else {
                    Some("31379-07.html".to_owned())
                }
            }
            "spawn_totem" => {
                let npc = npc?;
                if ctx.has_quest_items(player, RED_TOTEM) {
                    Some(spawn_nastron(ctx, npc, qs))
                } else {
                    Some("31558-04.html".to_owned())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        // Java: the respawn delay from the raid settings (43200000 and 129600000 ms).
        let settings = ctx.world().boss_settings();
        let (min_multiplier, max_multiplier) = (
            settings.raid_min_respawn_multiplier,
            settings.raid_max_respawn_multiplier,
        );
        let min_delay = (43_200_000_f32 * min_multiplier) as i32;
        let max_delay = (129_600_000_f32 * max_multiplier) as i32;
        let respawn_delay = ctx.get_random_range(min_delay, max_delay);
        ctx.cancel_quest_timer("despawn_nastron", Some(npc), None);
        let respawn_at = ctx.now_ms() + i64::from(respawn_delay);
        ctx.set_global_variable(RESPAWN_VAR, &respawn_at.to_string());
        ctx.start_quest_timer("spawn_npc", i64::from(respawn_delay), None, None, false);
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == UDAN {
            let state = qs.state(ctx);
            if state == CREATED {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    if ctx.has_quest_items(player, RED_TOTEM) {
                        "31379-01.htm".to_owned()
                    } else {
                        "31379-00a.html".to_owned()
                    }
                } else {
                    "31379-00b.html".to_owned()
                };
            } else if state == STARTED {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31379-03.html".to_owned()
                } else if ctx.has_quest_items(player, NASTRON_HEART) {
                    "31379-04.html".to_owned()
                } else {
                    "31379-05.html".to_owned()
                };
            }
        } else if npc.id == KETRA_TOTEM && qs.is_started(ctx) {
            htmltext = match qs.get_cond(ctx) {
                1 => "31558-01.html".to_owned(),
                2 => spawn_nastron(ctx, npc, qs),
                3 => "31558-05.html".to_owned(),
                _ => htmltext,
            };
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00616_MagicalPowerOfFirePart2";

    const NPCS: &str = r#"
        <npc id="31379" type="Folk" level="80" name="Udan"/>
        <npc id="31558" type="Folk" level="80" name="Ketra Totem"/>
        <npc id="25306" type="Monster" level="80" name="Nastron"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7243" type="EtcItem" name="Red Totem"><set name="is_questitem" val="true"/></item>
        <item id="7244" type="EtcItem" name="Nastron Heart"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00616MagicalPowerOfFirePart2);
        let pages = Pages::of(&Q00616MagicalPowerOfFirePart2)
            .page("31379-01.htm", &["31379-02.html"])
            .plain(&[
                "31379-00a.html",
                "31379-00b.html",
                "31379-02.html",
                "31379-03.html",
                "31379-05.html",
                "31379-06.html",
                "31379-07.html",
            ])
            .page("31379-04.html", &["give_heart"])
            .page("31558-01.html", &["spawn_totem"])
            .plain(&["31558-02.html", "31558-03.html", "31558-04.html", "31558-05.html"])
            .npcs(&[UDAN, KETRA_TOTEM]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let udan = t.spawn(UDAN, 20);
        (t, udan)
    }

    /// The totem the script spawned at load (its place is the ketra spawn).
    fn ketra_totem(t: &ScriptTest) -> Oid {
        t.npcs_of(KETRA_TOTEM)[0]
    }

    #[test]
    fn udan_sends_a_character_without_the_red_totem_or_the_level_away() {
        let (mut t, udan) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, udan).contains("31379-00b.html"));
        t.enter(2, 75, vec![]);
        assert!(t.quest_page(2, udan).contains("31379-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_totem_summons_nastron_and_the_heart_ends_the_quest() {
        let (mut t, udan) = setup();
        assert_eq!(t.npcs_of(KETRA_TOTEM).len(), 1, "the totem is spawned at load");
        t.enter(1, 75, vec![item(100, RED_TOTEM, 1)]);
        assert!(t.quest_page(1, udan).contains("31379-01.htm"));
        assert!(t.event(1, NAME, "31379-02.html").contains("31379-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The totem at cond 1 hands over: the red totem is spent and Nastron comes.
        // The totem stands far from the test characters, so its talk and its link are run as the
        // hooks (the bypass would need the player in range).
        let totem = ketra_totem(&t);
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(totem).unwrap();
            assert_eq!(
                Q00616MagicalPowerOfFirePart2.on_talk(ctx, npc, Oid(1)).as_deref(),
                Some("31558-01.html")
            );
            assert_eq!(
                Q00616MagicalPowerOfFirePart2
                    .on_event(ctx, "spawn_totem", Some(npc), Some(Oid(1)))
                    .as_deref(),
                Some("31558-02.html")
            );
        });
        assert_eq!(t.count(1, RED_TOTEM), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(t.npcs_of(NASTRON).len(), 1);

        // Nastron dies: the heart for the killer, cond 3; the respawn is scheduled.
        // The heart goes to the characters in range of the boss: the test character goes there.
        let nastron = t.npcs_of(NASTRON)[0];
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(Oid(1), 142_528, -82_528, -6_496, 0, false);
        });
        t.wait_ms(1_000);
        t.kill(1, nastron);
        assert_eq!(t.count(1, NASTRON_HEART), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Udan gives the experience and the quest ends (the character walks back to him).
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(
                Oid(1),
                crate::testing::X,
                crate::testing::Y,
                crate::testing::Z,
                0,
                false,
            );
        });
        t.wait_ms(1_000);
        assert!(t.quest_page(1, udan).contains("31379-04.html"));
        assert!(t.event(1, NAME, "give_heart").contains("31379-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
