//! Quest 610: Magical Power of Water - Part 2 (Asefa; the Varka totem summons
//! Ashutar, the raid boss, whose heart Asefa wants; Ashutar respawns after a random time).
// Java: dist/game/data/scripts/quests/Q00610_MagicalPowerOfWaterPart2/Q00610_MagicalPowerOfWaterPart2.java
//
// Pattern: a raid boss that a quest summons (the totem spawned at load and at the respawn timer),
// the timers of the boss (`spawn_npc`, `despawn_ashutar`), the respawn time kept in a global
// variable (D-54: the row is written by the engine's global variables), and the kill that gives
// the heart to every player in range (`executeForEachPlayer`, `actionForEachPlayer`).
// The constructor's spawn and timer are ported as `on_load` (the scripts are installed at start).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ASEFA: i32 = 31372;
const VARKA_TOTEM: i32 = 31560;

// Monster
const ASHUTAR: i32 = 25316;

// Items
const GREEN_TOTEM: i32 = 7238;
const ASHUTAR_HEART: i32 = 7239;

// Misc
const MIN_LEVEL: i32 = 75;
/// Java `GlobalVariablesManager` row of the respawn time (epoch ms).
const RESPAWN_VAR: &str = "Q00610_respawn";

/// Java `addSpawn(VARKA_TOTEM, 105452, -36775, -1050, 34000, false, 0, true)`.
const VARKA_TOTEM_SPAWN: (i32, i32, i32, i32) = (105_452, -36_775, -1_050, 34_000);
/// Java `addSpawn(ASHUTAR, 104825, -36926, -1136, 0, false, 0)`.
const ASHUTAR_SPAWN: (i32, i32, i32, i32) = (104_825, -36_926, -1_136, 0);

/// Java `Q00610_MagicalPowerOfWaterPart2`.
pub struct Q00610MagicalPowerOfWaterPart2;

/// Java `addSpawn(VARKA_TOTEM, ...)`: the totem at its place.
fn spawn_varka_totem(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z, heading) = VARKA_TOTEM_SPAWN;
    ctx.add_spawn(VARKA_TOTEM, x, y, z, heading, false, 0, 0);
}

/// Java `spawnAshutar(npc, qs)`.
fn spawn_ashutar(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, qs: l2_world::script::Qs) -> String {
    if ctx.has_quest_timer("spawn_npc", None, None) {
        return "31560-03.html".to_owned();
    }
    if qs.is_cond(ctx, 1) {
        ctx.take_items(qs.player(), GREEN_TOTEM, 1);
        qs.set_cond(ctx, 2, true);
    }
    ctx.delete_npc(npc);
    let (x, y, z, heading) = ASHUTAR_SPAWN;
    if let Some(ashutar) = ctx.add_spawn(ASHUTAR, x, y, z, heading, false, 0, 0) {
        ctx.broadcast_npc_say(
            ashutar,
            ChatType::NpcGeneral,
            NpcStringId::TheMagicalPowerOfWaterComesFromThePowerOfStormAndHailIfYouDareToConfrontItOnlyDeathWillAwaitYou,
            &[],
        );
        ctx.start_quest_timer("despawn_ashutar", 1_200_000, Some(ashutar), None, false);
    }
    "31560-02.html".to_owned()
}

impl Script for Q00610MagicalPowerOfWaterPart2 {
    fn name(&self) -> &'static str {
        "Q00610_MagicalPowerOfWaterPart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00610_MagicalPowerOfWaterPart2"
    }

    fn id(&self) -> i32 {
        610
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASEFA]);
        s.add_talk_id(&[ASEFA, VARKA_TOTEM]);
        s.add_kill_id(&[ASHUTAR]);
        s.register_quest_items(&[GREEN_TOTEM, ASHUTAR_HEART]);
    }

    /// Java constructor: the respawn time is still ahead, so the totem comes back with the timer;
    /// otherwise the totem is there now.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let test = ctx.global_variables().get_long(RESPAWN_VAR, 0);
        let remain = if test != 0 { test - ctx.now_ms() } else { 0 };
        if remain > 0 {
            ctx.start_quest_timer("spawn_npc", remain, None, None, false);
        } else {
            spawn_varka_totem(ctx);
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
        if npc.id == ASHUTAR {
            // Java: case 1 takes the red totem and falls through to case 2.
            let cond = qs.get_cond(ctx);
            if cond == 1 {
                ctx.take_items(player, GREEN_TOTEM, 1);
            }
            if cond == 1 || cond == 2 {
                if !ctx.has_quest_items(player, ASHUTAR_HEART) {
                    ctx.give_items(player, ASHUTAR_HEART, 1);
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
                "despawn_ashutar" => {
                    if let Some(npc) = npc {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::ThePowerOfConstraintIsGettingWeakerYourRitualHasFailed,
                            &[],
                        );
                        ctx.delete_npc(npc);
                    }
                    spawn_varka_totem(ctx);
                }
                "spawn_npc" => spawn_varka_totem(ctx),
                _ => {}
            }
            return None;
        };
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "31372-02.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "give_heart" => {
                if ctx.has_quest_items(player, ASHUTAR_HEART) {
                    ctx.add_exp_and_sp(player, 10_000, 0);
                    qs.exit_quest(ctx, true, true);
                    Some("31372-06.html".to_owned())
                } else {
                    Some("31372-07.html".to_owned())
                }
            }
            "spawn_totem" => {
                let npc = npc?;
                if ctx.has_quest_items(player, GREEN_TOTEM) {
                    Some(spawn_ashutar(ctx, npc, qs))
                } else {
                    Some("31560-04.html".to_owned())
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
        ctx.cancel_quest_timer("despawn_ashutar", Some(npc), None);
        let respawn_at = ctx.now_ms() + i64::from(respawn_delay);
        ctx.set_global_variable(RESPAWN_VAR, &respawn_at.to_string());
        ctx.start_quest_timer("spawn_npc", i64::from(respawn_delay), None, None, false);
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == ASEFA {
            let state = qs.state(ctx);
            if state == CREATED {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    if ctx.has_quest_items(player, GREEN_TOTEM) {
                        "31372-01.htm".to_owned()
                    } else {
                        "31372-00a.html".to_owned()
                    }
                } else {
                    "31372-00b.html".to_owned()
                };
            } else if state == STARTED {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31372-03.html".to_owned()
                } else if ctx.has_quest_items(player, ASHUTAR_HEART) {
                    "31372-04.html".to_owned()
                } else {
                    "31372-05.html".to_owned()
                };
            }
        } else if npc.id == VARKA_TOTEM && qs.is_started(ctx) {
            htmltext = match qs.get_cond(ctx) {
                1 => "31560-01.html".to_owned(),
                2 => spawn_ashutar(ctx, npc, qs),
                3 => "31560-05.html".to_owned(),
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

    const NAME: &str = "Q00610_MagicalPowerOfWaterPart2";

    const NPCS: &str = r#"
        <npc id="31372" type="Folk" level="80" name="Asefa"/>
        <npc id="31560" type="Folk" level="80" name="Varka Totem"/>
        <npc id="25316" type="Monster" level="80" name="Ashutar"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7238" type="EtcItem" name="Green Totem"><set name="is_questitem" val="true"/></item>
        <item id="7239" type="EtcItem" name="Ashutar Heart"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00610MagicalPowerOfWaterPart2);
        let pages = Pages::of(&Q00610MagicalPowerOfWaterPart2)
            .page("31372-01.htm", &["31372-02.html"])
            .plain(&[
                "31372-00a.html",
                "31372-00b.html",
                "31372-02.html",
                "31372-03.html",
                "31372-05.html",
                "31372-06.html",
                "31372-07.html",
            ])
            .page("31372-04.html", &["give_heart"])
            .page("31560-01.html", &["spawn_totem"])
            .plain(&["31560-02.html", "31560-03.html", "31560-04.html", "31560-05.html"])
            .npcs(&[ASEFA, VARKA_TOTEM]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let udan = t.spawn(ASEFA, 20);
        (t, udan)
    }

    /// The totem the script spawned at load (its place is the ketra spawn).
    fn varka_totem(t: &ScriptTest) -> Oid {
        t.npcs_of(VARKA_TOTEM)[0]
    }

    #[test]
    fn asefa_sends_a_character_without_the_green_totem_or_the_level_away() {
        let (mut t, udan) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, udan).contains("31372-00b.html"));
        t.enter(2, 75, vec![]);
        assert!(t.quest_page(2, udan).contains("31372-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_totem_summons_ashutar_and_the_heart_ends_the_quest() {
        let (mut t, udan) = setup();
        assert_eq!(t.npcs_of(VARKA_TOTEM).len(), 1, "the totem is spawned at load");
        t.enter(1, 75, vec![item(100, GREEN_TOTEM, 1)]);
        assert!(t.quest_page(1, udan).contains("31372-01.htm"));
        assert!(t.event(1, NAME, "31372-02.html").contains("31372-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The totem at cond 1 hands over: the red totem is spent and Ashutar comes.
        // The totem stands far from the test characters, so its talk and its link are run as the
        // hooks (the bypass would need the player in range).
        let totem = varka_totem(&t);
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(totem).unwrap();
            assert_eq!(
                Q00610MagicalPowerOfWaterPart2
                    .on_talk(ctx, npc, Oid(1))
                    .as_deref(),
                Some("31560-01.html")
            );
            assert_eq!(
                Q00610MagicalPowerOfWaterPart2
                    .on_event(ctx, "spawn_totem", Some(npc), Some(Oid(1)))
                    .as_deref(),
                Some("31560-02.html")
            );
        });
        assert_eq!(t.count(1, GREEN_TOTEM), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert_eq!(t.npcs_of(ASHUTAR).len(), 1);

        // Ashutar dies: the heart for the killer, cond 3; the respawn is scheduled.
        // The heart goes to the characters in range of the boss: the test character goes there.
        let ashutar = t.npcs_of(ASHUTAR)[0];
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(Oid(1), 104_825, -36_926, -1_136, 0, false);
        });
        t.wait_ms(1_000);
        t.kill(1, ashutar);
        assert_eq!(t.count(1, ASHUTAR_HEART), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Asefa gives the experience and the quest ends (the character walks back to him).
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
        assert!(t.quest_page(1, udan).contains("31372-04.html"));
        assert!(t.event(1, NAME, "give_heart").contains("31372-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
