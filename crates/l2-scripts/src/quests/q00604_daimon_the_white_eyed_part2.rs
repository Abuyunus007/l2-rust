//! Quest 604: Daimon the White-Eyed - Part 2 (the Eye of Argos sends the character to Daimon's
//! Altar; the summon crystal calls the raid boss, whose essence is traded for a greater dye).
// Java: dist/game/data/scripts/quests/Q00604_DaimonTheWhiteEyedPart2/Q00604_DaimonTheWhiteEyedPart2.java
//
// Pattern: as Q00625 (a boss that the quest summons, the respawn time in a global variable, D-54,
// the "is it alive" check over the world and the kill that pays the players in range).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DAIMONS_ALTAR: i32 = 31541;
const EYE_OF_ARGOS: i32 = 31683;

// Raid Boss
const DAIMON_THE_WHITE_EYED: i32 = 25290;

// Items
const UNFINISHED_SUMMON_CRYSTAL: i32 = 7192;
const SUMMON_CRYSTAL: i32 = 7193;
const ESSENCE_OF_DAIMON: i32 = 7194;

// Rewards
const DYE_I2M2_C: i32 = 4595; // Greater Dye of INT <Int+2 Men-2>
const DYE_I2W2_C: i32 = 4596; // Greater Dye of INT <Int+2 Wit-2>
const DYE_M2I2_C: i32 = 4597; // Greater Dye of MEN <Men+2 Int-2>
const DYE_M2W2_C: i32 = 4598; // Greater Dye of MEN <Men+2 Wit-2>
const DYE_W2I2_C: i32 = 4599; // Greater Dye of WIT <Wit+2 Int-2>
const DYE_W2M2_C: i32 = 4600; // Greater Dye of WIT <Wit+2 Men-2>

// Location
const DAIMON_THE_WHITE_EYED_LOC: (i32, i32, i32) = (186_320, -43_904, -3_175);

// Misc
const DAIMON_THE_WHITE_EYED_RESPAWN_TIME: &str = "DAIMON_THE_WHITE_EYED_RESPAWN_TIME";
const MIN_LEVEL: i32 = 73;

/// Java `Q00604_DaimonTheWhiteEyedPart2`.
pub struct Q00604DaimonTheWhiteEyedPart2;

/// Java `isDaimonSpawned()`: `true` while the respawn time is ahead, or when the boss is in the
/// world (the visible objects).
fn is_daimon_spawned(ctx: &ScriptCtx<'_>) -> bool {
    if ctx.now_ms()
        > ctx
            .global_variables()
            .get_long(DAIMON_THE_WHITE_EYED_RESPAWN_TIME, 0)
    {
        let world = ctx.world();
        return world.npc_ids().into_iter().any(|id| {
            world
                .npc(id)
                .is_some_and(|n| n.npc_id == DAIMON_THE_WHITE_EYED && !n.decayed)
        });
    }
    true
}

/// Java `addSpawn(DAIMON_THE_WHITE_EYED, DAIMON_THE_WHITE_EYED_LOC)`.
fn spawn_daimon(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z) = DAIMON_THE_WHITE_EYED_LOC;
    ctx.add_spawn(DAIMON_THE_WHITE_EYED, x, y, z, 0, false, 0, 0);
}

impl Script for Q00604DaimonTheWhiteEyedPart2 {
    fn name(&self) -> &'static str {
        "Q00604_DaimonTheWhiteEyedPart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00604_DaimonTheWhiteEyedPart2"
    }

    fn id(&self) -> i32 {
        604
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EYE_OF_ARGOS]);
        s.add_talk_id(&[EYE_OF_ARGOS, DAIMONS_ALTAR]);
        s.add_spawn_id(&[DAIMON_THE_WHITE_EYED]);
        s.add_kill_id(&[DAIMON_THE_WHITE_EYED]);
        s.register_quest_items(&[SUMMON_CRYSTAL, ESSENCE_OF_DAIMON]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN" {
            if is_daimon_spawned(ctx)
                && let Some(npc) = npc
            {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::CanLightExistWithoutDarkness,
                    &[],
                );
                ctx.delete_npc(npc);
            }
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31683-04.htm" => {
                ctx.take_items(player, UNFINISHED_SUMMON_CRYSTAL, 1);
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 11);
                ctx.give_items(player, SUMMON_CRYSTAL, 1);
                htmltext = Some(event.to_owned());
            }
            "31683-07.html" => {
                if ctx.has_quest_items(player, ESSENCE_OF_DAIMON) {
                    let random = ctx.get_random(1000);
                    ctx.take_items(player, ESSENCE_OF_DAIMON, 1);
                    let reward = if random < 167 {
                        DYE_I2M2_C
                    } else if random < 334 {
                        DYE_I2W2_C
                    } else if random < 501 {
                        DYE_M2I2_C
                    } else if random < 668 {
                        DYE_M2W2_C
                    } else if random < 835 {
                        DYE_W2I2_C
                    } else {
                        DYE_W2M2_C
                    };
                    ctx.reward_items(player, reward, 5);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31683-08.html".to_owned());
                }
            }
            "31541-02.html" => {
                if ctx.has_quest_items(player, SUMMON_CRYSTAL) {
                    if !is_daimon_spawned(ctx) {
                        ctx.take_items(player, SUMMON_CRYSTAL, 1);
                        htmltext = Some(event.to_owned());
                        spawn_daimon(ctx);
                        qs.set_memo_state(ctx, 21);
                        qs.set_cond(ctx, 2, true);
                    } else {
                        htmltext = Some("31541-03.html".to_owned());
                    }
                } else {
                    htmltext = Some("31541-04.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if ctx.player_level(player) < MIN_LEVEL {
                htmltext = "31683-01.htm".to_owned();
            } else if !ctx.has_quest_items(player, UNFINISHED_SUMMON_CRYSTAL) {
                htmltext = "31683-02.htm".to_owned();
            } else {
                htmltext = "31683-03.htm".to_owned();
            }
        } else if state == STARTED {
            if npc.id == EYE_OF_ARGOS {
                if qs.is_memo_state(ctx, 11) {
                    htmltext = "31683-05.html".to_owned();
                } else if qs.get_memo_state(ctx) >= 22 && ctx.has_quest_items(player, ESSENCE_OF_DAIMON) {
                    htmltext = "31683-06.html".to_owned();
                } else {
                    htmltext = "31683-09.html".to_owned();
                }
            } else if qs.is_memo_state(ctx, 11) {
                // DAIMONS_ALTAR
                if is_daimon_spawned(ctx) {
                    htmltext = "31541-05.html".to_owned();
                } else if ctx.has_quest_items(player, SUMMON_CRYSTAL) {
                    htmltext = "31541-01.html".to_owned();
                } else {
                    htmltext = "31541-04.html".to_owned();
                }
            } else if qs.is_memo_state(ctx, 21) {
                if !is_daimon_spawned(ctx) {
                    if ctx.has_quest_items(player, SUMMON_CRYSTAL) {
                        ctx.take_items(player, SUMMON_CRYSTAL, 1);
                        spawn_daimon(ctx);
                        htmltext = "31541-02.html".to_owned();
                    } else {
                        htmltext = "31541-04.html".to_owned();
                    }
                } else {
                    htmltext = "31541-03.html".to_owned();
                }
            } else if qs.get_memo_state(ctx) >= 22 {
                htmltext = "31541-05.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("DESPAWN", 1_200_000, Some(npc), None, false);
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::WhoIsCallingMe, &[]);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);

        let settings = ctx.world().boss_settings();
        let (min_multiplier, max_multiplier) = (
            settings.raid_min_respawn_multiplier,
            settings.raid_max_respawn_multiplier,
        );
        let respawn_min_delay = (43_200_000_f32 * min_multiplier) as i32;
        let respawn_max_delay = (129_600_000_f32 * max_multiplier) as i32;
        let respawn_delay = ctx.get_random_range(respawn_min_delay, respawn_max_delay);
        let respawn_at = ctx.now_ms() + i64::from(respawn_delay);
        ctx.set_global_variable(DAIMON_THE_WHITE_EYED_RESPAWN_TIME, &respawn_at.to_string());
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
        if qs.get_memo_state(ctx) >= 11
            && qs.get_memo_state(ctx) <= 21
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            qs.set_cond(ctx, 3, true);
            qs.set_memo_state(ctx, 22);
            ctx.give_items(player, ESSENCE_OF_DAIMON, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00604_DaimonTheWhiteEyedPart2";

    const NPCS: &str = r#"
        <npc id="31541" type="Folk" level="70" name="Daimon's Altar"/>
        <npc id="31683" type="Folk" level="70" name="Eye of Argos"/>
        <npc id="25290" type="Monster" level="80" name="Daimon the White-Eyed"/>"#;

    const ITEMS: &str = r#"
        <item id="7192" type="EtcItem" name="Unfinished Summon Crystal"><set name="is_questitem" val="true"/></item>
        <item id="7193" type="EtcItem" name="Summon Crystal"><set name="is_questitem" val="true"/></item>
        <item id="7194" type="EtcItem" name="Essence of Daimon"><set name="is_questitem" val="true"/></item>
        <item id="4595" type="EtcItem" name="Dye 1"><set name="is_stackable" val="true"/></item>
        <item id="4596" type="EtcItem" name="Dye 2"><set name="is_stackable" val="true"/></item>
        <item id="4597" type="EtcItem" name="Dye 3"><set name="is_stackable" val="true"/></item>
        <item id="4598" type="EtcItem" name="Dye 4"><set name="is_stackable" val="true"/></item>
        <item id="4599" type="EtcItem" name="Dye 5"><set name="is_stackable" val="true"/></item>
        <item id="4600" type="EtcItem" name="Dye 6"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00604DaimonTheWhiteEyedPart2);
        let pages = Pages::of(&Q00604DaimonTheWhiteEyedPart2)
            .page("31541-01.html", &["31541-02.html"])
            .plain(&[
                "31541-02.html",
                "31541-03.html",
                "31541-04.html",
                "31541-05.html",
                "31683-01.htm",
                "31683-02.htm",
                "31683-04.htm",
                "31683-05.html",
                "31683-07.html",
                "31683-08.html",
                "31683-09.html",
            ])
            .page("31683-03.htm", &["31683-04.htm"])
            .page("31683-06.html", &["31683-07.html"])
            .npcs(&[DAIMONS_ALTAR, EYE_OF_ARGOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let eye = t.spawn(EYE_OF_ARGOS, 20);
        let altar = t.spawn(DAIMONS_ALTAR, 30);
        (t, eye, altar)
    }

    fn teleport(t: &mut ScriptTest, player: i32, to: (i32, i32, i32)) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(Oid(player), to.0, to.1, to.2, 0, false);
        });
        t.wait_ms(1_000);
    }

    #[test]
    fn level_and_the_unfinished_crystal_gate_the_start() {
        let (mut t, eye, _) = setup();
        t.enter(1, 72, vec![item(100, UNFINISHED_SUMMON_CRYSTAL, 1)]);
        assert!(t.quest_page(1, eye).contains("31683-01.htm"));
        t.enter(2, 73, vec![]);
        assert!(t.quest_page(2, eye).contains("31683-02.htm"));
        t.enter(3, 73, vec![item(101, UNFINISHED_SUMMON_CRYSTAL, 1)]);
        assert!(t.quest_page(3, eye).contains("31683-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_altar_summons_daimon_and_the_essence_buys_a_dye() {
        let (mut t, eye, altar) = setup();
        t.enter(1, 73, vec![item(100, UNFINISHED_SUMMON_CRYSTAL, 1)]);
        t.quest_page(1, eye);
        assert!(t.event(1, NAME, "31683-04.htm").contains("31683-04.htm"));
        assert_eq!(
            (t.count(1, UNFINISHED_SUMMON_CRYSTAL), t.count(1, SUMMON_CRYSTAL)),
            (0, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "11")
            ]
        );
        assert!(t.quest_page(1, eye).contains("31683-05.html"));

        // The altar: the crystal is spent and the boss appears.
        assert!(t.quest_page(1, altar).contains("31541-01.html"));
        assert!(t.event(1, NAME, "31541-02.html").contains("31541-02.html"));
        assert_eq!(t.count(1, SUMMON_CRYSTAL), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "21"),
                rows::update(NAME, "cond", "2")
            ]
        );
        let boss = t.npcs_of(DAIMON_THE_WHITE_EYED)[0];
        // Another character is told that the boss is out already.
        t.enter(2, 73, vec![item(101, UNFINISHED_SUMMON_CRYSTAL, 1)]);
        assert!(t.quest_page(1, altar).contains("31541-03.html"));

        // It dies next to the character: the essence, cond 3, memo 22.
        teleport(&mut t, 1, DAIMON_THE_WHITE_EYED_LOC);
        t.take_writes();
        t.kill(1, boss);
        assert_eq!(t.count(1, ESSENCE_OF_DAIMON), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "3"),
                rows::update(NAME, "memoState", "22")
            ]
        );
        assert!(
            t.world
                .global_variables()
                .get_long(DAIMON_THE_WHITE_EYED_RESPAWN_TIME, 0)
                .gt(&0)
        );

        // Back at the Eye of Argos: five of a dye, the quest ends and can be repeated.
        teleport(
            &mut t,
            1,
            (crate::testing::X, crate::testing::Y, crate::testing::Z),
        );
        assert!(t.quest_page(1, eye).contains("31683-06.html"));
        assert!(t.event(1, NAME, "31683-07.html").contains("31683-07.html"));
        let dyes: i64 = (4595..=4600).map(|id| t.count(1, id)).sum();
        assert_eq!(dyes, 5);
        assert_eq!(t.count(1, ESSENCE_OF_DAIMON), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn daimon_leaves_after_twenty_minutes() {
        let (mut t, eye, altar) = setup();
        t.enter(1, 73, vec![item(100, UNFINISHED_SUMMON_CRYSTAL, 1)]);
        t.quest_page(1, eye);
        t.event(1, NAME, "31683-04.htm");
        t.quest_page(1, altar);
        t.event(1, NAME, "31541-02.html");
        let boss = t.npcs_of(DAIMON_THE_WHITE_EYED)[0];
        assert!(t.in_world(boss));
        t.wait_ms(1_200_000);
        assert!(!t.in_world(boss));
    }
}
