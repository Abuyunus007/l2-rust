//! Quest 23: Lidia's Heart (High Priest Innocentin, Trader Violet, the tombstone, the ghost of von Hellmann).
// Java: dist/game/data/scripts/quests/Q00023_LidiasHeart/Q00023_LidiasHeart.java
//
// Pattern: a ghost story with the memo state as the step counter (`docs/design/scripts.md`,
// "Quest patterns"). The Java NPC variables `SPAWNED` (on the tombstone) and `npc0` (on the ghost,
// the tombstone that woke it) live in `ctx.state`, keyed by object id.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIGH_PRIEST_INNOCENTIN: i32 = 31328;
const TRADER_VIOLET: i32 = 31386;
const TOMBSTONE: i32 = 31523;
const GHOST_OF_VON_HELLMANN: i32 = 31524;
const BROKEN_BOOKSHELF: i32 = 31526;
const BOX: i32 = 31530;

// Items
const LIDIAS_DIARY: i32 = 7064;
const SILVER_KEY: i32 = 7149;
const SILVER_SPEAR: i32 = 7150;

// Reward
const MAP_FOREST_OF_THE_DEAD: i32 = 7063;
const LIDIAS_HAIRPIN: i32 = 7148;

// Misc
const MIN_LEVEL: i32 = 64;

// Locations
const GHOST_SPAWN: (i32, i32, i32) = (51432, -54570, -3136);

/// Java `Q00022_TragedyInVonHellmannForest.class.getSimpleName()`.
const TRAGEDY: &str = "Q00022_TragedyInVonHellmannForest";
/// Java `Q00024_InhabitantsOfTheForestOfTheDead.class.getSimpleName()`.
const INHABITANTS: &str = "Q00024_InhabitantsOfTheForestOfTheDead";

/// The Java NPC variables of the two NPCs: `SPAWNED` of a tombstone, `npc0` of a ghost.
#[derive(Default)]
struct LidiaVars {
    spawned: HashSet<ObjectId>,
    npc0: HashMap<ObjectId, ObjectId>,
}

/// Java `Q00023_LidiasHeart`.
pub struct Q00023LidiasHeart;

impl Script for Q00023LidiasHeart {
    fn name(&self) -> &'static str {
        "Q00023_LidiasHeart"
    }

    fn path(&self) -> &'static str {
        "quests/Q00023_LidiasHeart"
    }

    fn id(&self) -> i32 {
        23
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIGH_PRIEST_INNOCENTIN]);
        s.add_talk_id(&[
            HIGH_PRIEST_INNOCENTIN,
            TRADER_VIOLET,
            TOMBSTONE,
            GHOST_OF_VON_HELLMANN,
            BROKEN_BOOKSHELF,
            BOX,
        ]);
        s.add_spawn_id(&[GHOST_OF_VON_HELLMANN]);
        s.register_quest_items(&[LIDIAS_DIARY, SILVER_KEY, SILVER_SPEAR]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN" {
            let npc = npc?;
            let npc0 = ctx.state::<LidiaVars>().npc0.get(&npc.object_id).copied();
            if let Some(npc0) = npc0 {
                ctx.state::<LidiaVars>().spawned.remove(&npc0);
            }
            ctx.state::<LidiaVars>().npc0.remove(&npc.object_id);
            ctx.delete_npc(npc);
            // Java `super.onEvent(..)`: null.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if ctx.player_level(player) < MIN_LEVEL {
                    htmltext = Some("31328-02.htm".to_owned());
                } else {
                    if !ctx.has_quest_items(player, MAP_FOREST_OF_THE_DEAD) {
                        ctx.give_items(player, MAP_FOREST_OF_THE_DEAD, 1);
                    }
                    ctx.give_items(player, SILVER_KEY, 1);
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some("31328-03.htm".to_owned());
                }
            }
            "31328-05.html" | "31328-06.html" | "31328-10.html" | "31328-11.html" | "31328-16.html"
            | "31328-17.html" | "31328-18.html" | "31524-03.html" | "31526-04.html" | "31526-05.html"
            | "31526-07a.html" | "31526-09.html" => {
                htmltext = Some(event.to_owned());
            }
            "31328-07.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-12.html" => {
                if qs.is_memo_state(ctx, 5) || qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 5, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-13.html" => {
                if qs.is_memo_state(ctx, 5) || qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-19.html" => {
                ctx.play_sound(player, QuestSound::AmbsoundMtCreak);
                htmltext = Some(event.to_owned());
            }
            "31328-20.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 6, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-21.html" => {
                qs.set_cond(ctx, 5, false);
                htmltext = Some(event.to_owned());
            }
            "31523-02.html" => {
                if qs.is_memo_state(ctx, 8) || qs.is_memo_state(ctx, 9) {
                    ctx.play_sound(player, QuestSound::SkillsoundHorror02);
                    let npc = npc?;
                    if !ctx.state::<LidiaVars>().spawned.contains(&npc.object_id) {
                        ctx.state::<LidiaVars>().spawned.insert(npc.object_id);
                        if let Some(ghost) = ctx.add_spawn(
                            GHOST_OF_VON_HELLMANN,
                            GHOST_SPAWN.0,
                            GHOST_SPAWN.1,
                            GHOST_SPAWN.2,
                            0,
                            false,
                            0,
                            0,
                        ) {
                            ctx.state::<LidiaVars>()
                                .npc0
                                .insert(ghost.object_id, npc.object_id);
                        }
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31523-03.html".to_owned());
                    }
                }
            }
            "31523-06.html" => {
                if qs.is_memo_state(ctx, 9) {
                    ctx.give_items(player, SILVER_KEY, 1);
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 8, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "31524-02.html" => {
                ctx.play_sound(player, QuestSound::ChrsoundMhfighterCry);
                htmltext = Some(event.to_owned());
            }
            "31524-04.html" => {
                if qs.is_memo_state(ctx, 8) {
                    ctx.take_items(player, LIDIAS_DIARY, 1);
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 7, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "31526-02.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, SILVER_KEY) {
                    ctx.take_items(player, SILVER_KEY, -1);
                    qs.set_memo_state(ctx, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "31526-06.html" => {
                if !ctx.has_quest_items(player, LIDIAS_HAIRPIN) {
                    ctx.give_items(player, LIDIAS_HAIRPIN, 1);
                }
                let memo = qs.get_memo_state(ctx);
                qs.set_memo_state(ctx, memo + 1);
                if ctx.has_quest_items(player, LIDIAS_DIARY) {
                    qs.set_cond(ctx, 4, false);
                }
                htmltext = Some(event.to_owned());
            }
            "31526-08.html" => {
                ctx.play_sound(player, QuestSound::ItemsoundArmorLeather);
                htmltext = Some(event.to_owned());
            }
            "31526-10.html" => {
                ctx.play_sound(player, QuestSound::AmbsoundEgDron);
                htmltext = Some(event.to_owned());
            }
            "31526-11.html" => {
                ctx.give_items(player, LIDIAS_DIARY, 1);
                let memo = qs.get_memo_state(ctx);
                qs.set_memo_state(ctx, memo + 1);
                if ctx.has_quest_items(player, LIDIAS_HAIRPIN) {
                    qs.set_cond(ctx, 4, false);
                }
                htmltext = Some(event.to_owned());
            }
            "31530-02.html" => {
                if qs.is_memo_state(ctx, 11) && ctx.has_quest_items(player, SILVER_KEY) {
                    ctx.give_items(player, SILVER_SPEAR, 1);
                    ctx.take_items(player, SILVER_KEY, -1);
                    ctx.play_sound(player, QuestSound::ItemsoundWeaponSpear);
                    qs.set_cond(ctx, 10, false);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == HIGH_PRIEST_INNOCENTIN {
                let q22_done = ctx
                    .player_quest_state(player, TRAGEDY)
                    .is_some_and(|q| q.is_completed());
                if q22_done {
                    htmltext = "31328-01.htm".to_owned();
                } else {
                    htmltext = "31328-01a.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                HIGH_PRIEST_INNOCENTIN => match qs.get_memo_state(ctx) {
                    1 => htmltext = "31328-04.html".to_owned(),
                    2 => htmltext = "31328-08.html".to_owned(),
                    5 => htmltext = "31328-09.html".to_owned(),
                    6 => htmltext = "31328-14.html".to_owned(),
                    7 => htmltext = "31328-15.html".to_owned(),
                    8 => {
                        qs.set_cond(ctx, 6, true);
                        htmltext = "31328-22.html".to_owned();
                    }
                    _ => {}
                },
                TRADER_VIOLET => match qs.get_memo_state(ctx) {
                    10 => {
                        if ctx.has_quest_items(player, SILVER_KEY) {
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 9, true);
                            htmltext = "31386-01.html".to_owned();
                        }
                    }
                    11 => {
                        if !ctx.has_quest_items(player, SILVER_SPEAR) {
                            htmltext = "31386-02.html".to_owned();
                        } else {
                            ctx.give_adena(player, 350_000, true);
                            ctx.add_exp_and_sp(player, 456_893, 42_112);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "31386-03.html".to_owned();
                        }
                    }
                    _ => {}
                },
                TOMBSTONE => match qs.get_memo_state(ctx) {
                    8 => htmltext = "31523-01.html".to_owned(),
                    9 => htmltext = "31523-04.html".to_owned(),
                    10 => htmltext = "31523-05.html".to_owned(),
                    _ => {}
                },
                GHOST_OF_VON_HELLMANN => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 8 {
                        htmltext = "31524-01.html".to_owned();
                    } else if memo_state == 9 {
                        if !ctx.has_quest_items(player, SILVER_KEY) {
                            htmltext = "31524-05.html".to_owned();
                        }
                    } else if memo_state == 9 || memo_state == 10 {
                        // NOTE(java-quirk): the `memo_state == 9` here is dead, 9 went to the branch above.
                        if ctx.has_quest_items(player, SILVER_KEY) {
                            qs.set_memo_state(ctx, 10);
                            htmltext = "31524-06.html".to_owned();
                        }
                    }
                }
                BROKEN_BOOKSHELF => match qs.get_memo_state(ctx) {
                    2 => {
                        if ctx.has_quest_items(player, SILVER_KEY) {
                            qs.set_cond(ctx, 3, true);
                            htmltext = "31526-01.html".to_owned();
                        }
                    }
                    3 => htmltext = "31526-03.html".to_owned(),
                    4 => {
                        if ctx.has_quest_items(player, LIDIAS_HAIRPIN) {
                            htmltext = "31526-07.html".to_owned();
                        } else if ctx.has_quest_items(player, LIDIAS_DIARY) {
                            htmltext = "31526-12.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_all_quest_items(player, &[LIDIAS_HAIRPIN, LIDIAS_DIARY]) {
                            htmltext = "31526-13.html".to_owned();
                        }
                    }
                    _ => {}
                },
                BOX => {
                    if qs.get_memo_state(ctx) == 11 {
                        if ctx.has_quest_items(player, SILVER_KEY) {
                            htmltext = "31530-01.html".to_owned();
                        } else if ctx.has_quest_items(player, SILVER_SPEAR) {
                            htmltext = "31530-03.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        } else if qs.state(ctx) == COMPLETED {
            if npc.id == HIGH_PRIEST_INNOCENTIN {
                htmltext = ctx.get_already_completed_msg(player);
            } else if npc.id == TRADER_VIOLET && ctx.player_quest_state(player, INHABITANTS).is_none() {
                htmltext = "31386-04.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("DESPAWN", 300_000, Some(npc), None, false);
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::WhoAwokeMe, &[]);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00023_LidiasHeart";

    const NPCS: &str = r#"
        <npc id="31328" type="Folk" level="70" name="Innocentin"/>
        <npc id="31386" type="Folk" level="70" name="Violet"/>
        <npc id="31523" type="Folk" level="70" name="Tombstone"/>
        <npc id="31524" type="Folk" level="70" name="Ghost of von Hellmann"/>
        <npc id="31526" type="Folk" level="70" name="Bookshelf"/>
        <npc id="31530" type="Folk" level="70" name="Box"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7063" type="EtcItem" name="Map"/>
        <item id="7148" type="EtcItem" name="Lidia's Hairpin"/>
        <item id="7064" type="EtcItem" name="Diary"><set name="is_questitem" val="true"/></item>
        <item id="7149" type="EtcItem" name="Silver Key"><set name="is_questitem" val="true"/></item>
        <item id="7150" type="EtcItem" name="Silver Spear"><set name="is_questitem" val="true"/></item>"#;

    /// A stand-in for a quest this one reads.
    struct Other(&'static str, i32);

    impl Script for Other {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Other"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Other(TRAGEDY, 22));
        registry.add(Q00023LidiasHeart);
        let pages = Pages::of(&Q00023LidiasHeart)
            .page("31328-01.htm", &["ACCEPT"])
            .plain(&["31328-01a.html", "31328-02.htm", "31328-03.htm"])
            .npcs(&[
                HIGH_PRIEST_INNOCENTIN,
                TRADER_VIOLET,
                TOMBSTONE,
                GHOST_OF_VON_HELLMANN,
                BROKEN_BOOKSHELF,
                BOX,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let innocentin = t.spawn(HIGH_PRIEST_INNOCENTIN, 20);
        (t, innocentin)
    }

    fn finish_tragedy(t: &mut ScriptTest) {
        t.world.run_as_script(TRAGEDY, |ctx| {
            if let Some(qs) = ctx.quest_state_of(TRAGEDY, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00023LidiasHeart.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00023LidiasHeart.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn writes(t: &ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    #[test]
    fn it_needs_the_tragedy_and_level_64() {
        let (mut t, innocentin) = setup();
        t.enter(1, 64, vec![]);
        assert!(t.quest_page(1, innocentin).contains("31328-01a.html"));

        let (mut t, innocentin) = setup();
        t.enter(1, 63, vec![]);
        finish_tragedy(&mut t);
        assert!(t.quest_page(1, innocentin).contains("31328-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("31328-02.htm"));
        assert!(writes(&t).is_empty());
    }

    #[test]
    fn the_whole_chain_to_the_spear_and_the_reward() {
        let (mut t, innocentin) = setup();
        t.enter(1, 64, vec![]);
        finish_tragedy(&mut t);
        assert!(t.quest_page(1, innocentin).contains("31328-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("31328-03.htm"));
        assert_eq!(
            (t.count(1, MAP_FOREST_OF_THE_DEAD), t.count(1, SILVER_KEY)),
            (1, 1)
        );
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Innocentin sends the player to the bookshelf: memo 2, cond 2; the key is used: memo 3.
        assert_eq!(
            ev(&mut t, "31328-07.html", None).as_deref(),
            Some("31328-07.html")
        );
        let shelf = t.spawn(BROKEN_BOOKSHELF, 40);
        assert_eq!(talk(&mut t, shelf).as_deref(), Some("31526-01.html"));
        assert_eq!(
            ev(&mut t, "31526-02.html", None).as_deref(),
            Some("31526-02.html")
        );
        assert_eq!(t.count(1, SILVER_KEY), 0);
        // The hairpin and the diary (memo 3 -> 4 -> 5, cond 4).
        assert_eq!(
            ev(&mut t, "31526-06.html", None).as_deref(),
            Some("31526-06.html")
        );
        assert_eq!(
            ev(&mut t, "31526-11.html", None).as_deref(),
            Some("31526-11.html")
        );
        assert_eq!((t.count(1, LIDIAS_HAIRPIN), t.count(1, LIDIAS_DIARY)), (1, 1));
        writes(&t);

        // Memo 7, 8: the tombstone wakes the ghost, once.
        assert_eq!(
            ev(&mut t, "31328-13.html", None).as_deref(),
            Some("31328-13.html")
        );
        assert_eq!(
            ev(&mut t, "31328-20.html", None).as_deref(),
            Some("31328-20.html")
        );
        let tomb = t.spawn(TOMBSTONE, 60);
        assert_eq!(
            ev(&mut t, "31523-02.html", Some(tomb)).as_deref(),
            Some("31523-02.html")
        );
        assert_eq!(t.npcs_of(GHOST_OF_VON_HELLMANN).len(), 1);
        assert_eq!(
            ev(&mut t, "31523-02.html", Some(tomb)).as_deref(),
            Some("31523-03.html")
        );
        let ghost = t.npcs_of(GHOST_OF_VON_HELLMANN)[0];
        // The ghost takes the diary (memo 9); the tombstone gives the key (memo 10).
        assert_eq!(
            ev(&mut t, "31524-04.html", None).as_deref(),
            Some("31524-04.html")
        );
        assert_eq!(
            ev(&mut t, "31523-06.html", None).as_deref(),
            Some("31523-06.html")
        );
        // The ghost despawns after 5 minutes and frees the tombstone.
        t.wait_ms(301_000);
        assert!(!t.in_world(ghost));
        assert_eq!(ev(&mut t, "31523-02.html", Some(tomb)), None); // memo 10: nothing

        // Violet: memo 11; the box gives the spear (cond 10); Violet pays.
        let violet = t.spawn(TRADER_VIOLET, 80);
        let boxx = t.spawn(BOX, 100);
        assert_eq!(talk(&mut t, violet).as_deref(), Some("31386-01.html"));
        assert_eq!(
            ev(&mut t, "31530-02.html", None).as_deref(),
            Some("31530-02.html")
        );
        assert_eq!(t.count(1, SILVER_SPEAR), 1);
        assert_eq!(talk(&mut t, boxx).as_deref(), Some("31530-03.html"));
        writes(&t);
        assert_eq!(talk(&mut t, violet).as_deref(), Some("31386-03.html"));
        assert_eq!(t.count(1, 57), 350_000);
        assert_eq!(
            writes(&t),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert_eq!(talk(&mut t, violet).as_deref(), Some("31386-04.html"));
    }
}
