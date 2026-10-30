//! Quest 625: The Finest Ingredients - Part 2 (Jeremy in the Town of Gludin area; the Yeti's Table
//! summons the Icicle Emperor Bumbalump, whose meat finishes the dish for a random dye).
// Java: dist/game/data/scripts/quests/Q00625_TheFinestIngredientsPart2/Q00625_TheFinestIngredientsPart2.java
//
// Pattern: a boss that a quest summons with a spawn location, a respawn time kept in a global
// variable (D-54), the "is it alive" check over the world (`World.getVisibleObjects`), the boss
// timer that makes it talk (`NPC_TALK`), and the kill that gives the meat to the players in range.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JEREMY: i32 = 31521;
const YETIS_TABLE: i32 = 31542;

// Monster
const ICICLE_EMPEROR_BUMBALUMP: i32 = 25296;

// Items
const SOY_SOURCE_JAR: i32 = 7205;
const FOOD_FOR_BUMBALUMP: i32 = 7209;
const SPECIAL_YETI_MEAT: i32 = 7210;

// Rewards: the greater dyes, five of each.
const GREATER_DYE_OF_STR_1: i32 = 4589;
const GREATER_DYE_OF_STR_2: i32 = 4590;
const GREATER_DYE_OF_CON_1: i32 = 4591;
const GREATER_DYE_OF_CON_2: i32 = 4592;
const GREATER_DYE_OF_DEX_1: i32 = 4593;
const GREATER_DYE_OF_DEX_2: i32 = 4594;
const DYE_COUNT: i64 = 5;

/// Java `ICICLE_EMPEROR_BUMBALUMP_LOC`.
const BUMBALUMP_LOC: (i32, i32, i32) = (158_240, -121_536, -2_222);

/// Java `ICICLE_EMPEROR_BUMBALUMP_RESPAWN_TIME`: the global variable of the respawn time.
const RESPAWN_TIME_VAR: &str = "ICICLE_EMPEROR_BUMBALUMP_RESPAWN_TIME";

// Misc
const MIN_LEVEL: i32 = 73;

/// Java `isBumbalumpSpawned()`: `true` while the respawn time is ahead, or when the boss is in the
/// world (the visible objects).
fn is_bumbalump_spawned(ctx: &ScriptCtx<'_>) -> bool {
    if ctx.now_ms() > ctx.global_variables().get_long(RESPAWN_TIME_VAR, 0) {
        let world = ctx.world();
        return world.npc_ids().into_iter().any(|id| {
            world
                .npc(id)
                .is_some_and(|n| n.npc_id == ICICLE_EMPEROR_BUMBALUMP && !n.decayed)
        });
    }
    true
}

/// Java `addSpawn(ICICLE_EMPEROR_BUMBALUMP, ICICLE_EMPEROR_BUMBALUMP_LOC)`.
fn spawn_bumbalump(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z) = BUMBALUMP_LOC;
    ctx.add_spawn(ICICLE_EMPEROR_BUMBALUMP, x, y, z, 0, false, 0, 0);
}

/// Java `Q00625_TheFinestIngredientsPart2`.
pub struct Q00625TheFinestIngredientsPart2;

impl Script for Q00625TheFinestIngredientsPart2 {
    fn name(&self) -> &'static str {
        "Q00625_TheFinestIngredientsPart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00625_TheFinestIngredientsPart2"
    }

    fn id(&self) -> i32 {
        625
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JEREMY]);
        s.add_talk_id(&[JEREMY, YETIS_TABLE]);
        s.add_spawn_id(&[ICICLE_EMPEROR_BUMBALUMP]);
        s.add_kill_id(&[ICICLE_EMPEROR_BUMBALUMP]);
        s.register_quest_items(&[FOOD_FOR_BUMBALUMP, SPECIAL_YETI_MEAT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "NPC_TALK" {
            // The boss talks while it is in the world.
            if let Some(npc) = npc
                && is_bumbalump_spawned(ctx)
            {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::Oooh, &[]);
            }
            return None;
        }
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31521-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.take_items(player, SOY_SOURCE_JAR, 1);
                    ctx.give_items(player, FOOD_FOR_BUMBALUMP, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "31521-08.html" => {
                if qs.is_cond(ctx, 3) {
                    if ctx.has_quest_items(player, SPECIAL_YETI_MEAT) {
                        let random = ctx.get_random(1000);
                        let dye = if random < 167 {
                            Some(GREATER_DYE_OF_STR_1)
                        } else if random < 334 {
                            Some(GREATER_DYE_OF_STR_2)
                        } else if random < 501 {
                            Some(GREATER_DYE_OF_CON_1)
                        } else if random < 668 {
                            Some(GREATER_DYE_OF_CON_2)
                        } else if random < 835 {
                            Some(GREATER_DYE_OF_DEX_1)
                        } else if random < 1000 {
                            Some(GREATER_DYE_OF_DEX_2)
                        } else {
                            None
                        };
                        if let Some(dye) = dye {
                            ctx.reward_items(player, dye, DYE_COUNT);
                        }
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31521-09.html".to_owned());
                    }
                }
            }
            "31542-02.html" => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, FOOD_FOR_BUMBALUMP) {
                        qs.set_cond(ctx, 2, true);
                        if !is_bumbalump_spawned(ctx) {
                            ctx.take_items(player, FOOD_FOR_BUMBALUMP, 1);
                            spawn_bumbalump(ctx);
                            htmltext = Some(event.to_owned());
                        } else {
                            htmltext = Some("31542-03.html".to_owned());
                        }
                    } else {
                        htmltext = Some("31542-04.html".to_owned());
                    }
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, FOOD_FOR_BUMBALUMP) {
                    if !is_bumbalump_spawned(ctx) {
                        ctx.take_items(player, FOOD_FOR_BUMBALUMP, 1);
                        spawn_bumbalump(ctx);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31542-03.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            JEREMY => {
                let state = qs.state(ctx);
                if state == CREATED {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, SOY_SOURCE_JAR) {
                            "31521-01.htm".to_owned()
                        } else {
                            "31521-02.htm".to_owned()
                        }
                    } else {
                        "31521-03.htm".to_owned()
                    };
                } else if state == STARTED {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "31521-05.html".to_owned(),
                        2 => htmltext = "31521-06.html".to_owned(),
                        3 => htmltext = "31521-07.html".to_owned(),
                        _ => {}
                    }
                } else if state == COMPLETED {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            YETIS_TABLE => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if !is_bumbalump_spawned(ctx) {
                        "31542-01.html".to_owned()
                    } else {
                        "31542-05.html".to_owned()
                    };
                }
                2 => {
                    if !is_bumbalump_spawned(ctx) {
                        if ctx.has_quest_items(player, FOOD_FOR_BUMBALUMP) {
                            ctx.take_items(player, FOOD_FOR_BUMBALUMP, 1);
                            spawn_bumbalump(ctx);
                            htmltext = "31542-02.html".to_owned();
                        } else {
                            htmltext = "31542-04.html".to_owned();
                        }
                    } else {
                        htmltext = "31542-03.html".to_owned();
                    }
                }
                3 => htmltext = "31542-05.html".to_owned(),
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("NPC_TALK", 1_200_000, Some(npc), None, false);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::ISmellSomethingDelicious,
            &[],
        );
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
        let settings = ctx.world().boss_settings();
        let (min_multiplier, max_multiplier) = (
            settings.raid_min_respawn_multiplier,
            settings.raid_max_respawn_multiplier,
        );
        let min_delay = (43_200_000_f32 * min_multiplier) as i32;
        let max_delay = (129_600_000_f32 * max_multiplier) as i32;
        let respawn_delay = ctx.get_random_range(min_delay, max_delay);
        let respawn_at = ctx.now_ms() + i64::from(respawn_delay);
        ctx.set_global_variable(RESPAWN_TIME_VAR, &respawn_at.to_string());
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
        if qs.is_cond(ctx, 2) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, true) {
            qs.set_cond(ctx, 3, true);
            ctx.give_items(player, SPECIAL_YETI_MEAT, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00625_TheFinestIngredientsPart2";

    const NPCS: &str = r#"
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="31542" type="Folk" level="70" name="Yeti's Table"/>
        <npc id="25296" type="Monster" level="80" name="Icicle Emperor Bumbalump"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7205" type="EtcItem" name="Soy Sauce Jar"><set name="is_stackable" val="true"/></item>
        <item id="7209" type="EtcItem" name="Food for Bumbalump"><set name="is_questitem" val="true"/></item>
        <item id="7210" type="EtcItem" name="Special Yeti Meat"><set name="is_questitem" val="true"/></item>
        <item id="4589" type="EtcItem" name="Greater Dye of STR"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00625TheFinestIngredientsPart2);
        let pages = Pages::of(&Q00625TheFinestIngredientsPart2)
            .page("31521-01.htm", &["31521-04.htm"])
            .plain(&[
                "31521-02.htm",
                "31521-03.htm",
                "31521-04.htm",
                "31521-05.html",
                "31521-06.html",
                "31521-09.html",
            ])
            .page("31521-07.html", &["31521-08.html"])
            .plain(&["31521-08.html"])
            .page("31542-01.html", &["31542-02.html"])
            .plain(&["31542-02.html", "31542-03.html", "31542-04.html", "31542-05.html"])
            .npcs(&[JEREMY, YETIS_TABLE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jeremy = t.spawn(JEREMY, 20);
        let table = t.spawn(YETIS_TABLE, 30);
        (t, jeremy, table)
    }

    #[test]
    fn a_character_below_level_73_or_without_the_soy_jar_is_sent_away() {
        let (mut t, jeremy, _) = setup();
        t.enter(1, 72, vec![item(100, 7205, 1)]);
        assert!(t.quest_page(1, jeremy).contains("31521-03.htm"));
        t.enter(2, 73, vec![]);
        assert!(t.quest_page(2, jeremy).contains("31521-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_bumbalump_comes_from_the_table_and_its_meat_ends_the_quest() {
        let (mut t, jeremy, table) = setup();
        t.enter(1, 73, vec![item(100, 7205, 1)]);
        assert!(t.quest_page(1, jeremy).contains("31521-01.htm"));
        assert!(t.event(1, NAME, "31521-04.htm").contains("31521-04.htm"));
        assert_eq!(t.count(1, SOY_SOURCE_JAR), 0);
        assert_eq!(t.count(1, FOOD_FOR_BUMBALUMP), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The table at cond 1: the food goes, the boss comes, cond 2.
        assert!(t.quest_page(1, table).contains("31542-01.html"));
        assert!(t.event(1, NAME, "31542-02.html").contains("31542-02.html"));
        assert_eq!(t.count(1, FOOD_FOR_BUMBALUMP), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        let boss = *t
            .npcs_of(ICICLE_EMPEROR_BUMBALUMP)
            .first()
            .expect("the boss spawned");

        // The boss dies next to the player: the meat and cond 3 (the player is taken there).
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(Oid(1), 158_240, -121_536, -2_222, 0, false);
        });
        t.wait_ms(1_000);
        t.kill(1, boss);
        assert_eq!(t.count(1, SPECIAL_YETI_MEAT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Jeremy's reward: one of the dyes, and the quest ends.
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
        assert!(t.quest_page(1, jeremy).contains("31521-07.html"));
        assert!(t.event(1, NAME, "31521-08.html").contains("31521-08.html"));
        assert_eq!(t.count(1, SPECIAL_YETI_MEAT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
