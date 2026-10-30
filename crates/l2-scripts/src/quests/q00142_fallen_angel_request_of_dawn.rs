//! Quest 142: Fallen Angel - Request of Dawn (started by Q00998_FallenAngelSelect).
// Java: dist/game/data/scripts/quests/Q00142_FallenAngelRequestOfDawn/Q00142_FallenAngelRequestOfDawn.java
//
// Pattern: no start NPC (the select quest notifies "30894-01.html"), a `talk` memo, ant kills
// for fragments at cond 4, a monster spawned from a rock and the `isAngelSpawned` field.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAYMOND: i32 = 30289;
const CASIAN: i32 = 30612;
const NATOOLS: i32 = 30894;
const ROCK: i32 = 32368;

// Monsters
const FALLEN_ANGEL: i32 = 27338;
const MOBS: [(i32, i32); 10] = [
    (20079, 338),  // Ant
    (20080, 363),  // Ant Captain
    (20081, 611),  // Ant Overseer
    (20082, 371),  // Ant Recruit
    (20084, 421),  // Ant Patrol
    (20086, 371),  // Ant Guard
    (20087, 900),  // Ant Soldier
    (20088, 1000), // Ant Warrior Captain
    (20089, 431),  // Noble Ant
    (20090, 917),  // Noble Ant Leader
];

// Items
const CRYPTOGRAM_OF_THE_ANGEL_SEARCH: i32 = 10351;
const PROPHECY_FRAGMENT: i32 = 10352;
const FALLEN_ANGEL_BLOOD: i32 = 10353;

// Misc
const MAX_REWARD_LEVEL: i32 = 43;
const FRAGMENT_COUNT: i64 = 30;

/// Java's field `isAngelSpawned`, kept by the world (D-10).
#[derive(Default)]
struct Fields {
    is_angel_spawned: bool,
}

/// Java `Q00142_FallenAngelRequestOfDawn`.
pub struct Q00142FallenAngelRequestOfDawn;

impl Script for Q00142FallenAngelRequestOfDawn {
    fn name(&self) -> &'static str {
        "Q00142_FallenAngelRequestOfDawn"
    }

    fn path(&self) -> &'static str {
        "quests/Q00142_FallenAngelRequestOfDawn"
    }

    fn id(&self) -> i32 {
        142
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[NATOOLS, RAYMOND, CASIAN, ROCK]);
        let mobs: Vec<i32> = MOBS.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&mobs);
        s.add_kill_id(&[FALLEN_ANGEL]);
        s.register_quest_items(&[
            CRYPTOGRAM_OF_THE_ANGEL_SEARCH,
            PROPHECY_FRAGMENT,
            FALLEN_ANGEL_BLOOD,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = Some(event.to_owned());
        match event {
            "30894-02.html" | "30289-03.html" | "30289-04.html" | "30612-03.html" | "30612-04.html"
            | "30612-06.html" | "30612-07.html" => {}
            "30894-01.html" => qs.start_quest(ctx),
            "30894-03.html" => {
                ctx.give_items(player, CRYPTOGRAM_OF_THE_ANGEL_SEARCH, 1);
                qs.set_cond(ctx, 2, true);
            }
            "30289-05.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 3, true);
            }
            "30612-05.html" => qs.set(ctx, "talk", "2"),
            "30612-08.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 4, true);
            }
            "32368-04.html" => {
                if ctx.state::<Fields>().is_angel_spawned {
                    return Some("32368-03.html".to_owned());
                }

                let npc = npc?;
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.add_spawn(FALLEN_ANGEL, x + 100, y + 100, z, 0, false, 120_000, 0);
                }
                ctx.state::<Fields>().is_angel_spawned = true;
                ctx.start_quest_timer("despawn", 120_000, None, Some(player), false);
            }
            "despawn" => {
                // NOTE(java-quirk): the answer of a timer event is the event name; the world does
                // not show a timer's answer.
                if ctx.state::<Fields>().is_angel_spawned {
                    ctx.state::<Fields>().is_angel_spawned = false;
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if npc.id == FALLEN_ANGEL {
            // NOTE(java-quirk): Java does not null-check this state (a killer without the quest
            // throws); here nothing happens.
            if let Some(qs) = ctx.get_quest_state(player, false)
                && qs.is_cond(ctx, 5)
            {
                ctx.give_items(player, FALLEN_ANGEL_BLOOD, 1);
                qs.set_cond(ctx, 6, true);
                ctx.state::<Fields>().is_angel_spawned = false;
            }
        } else if let Some(member) = ctx.get_random_party_member_cond(player, 4) {
            let qs = ctx.get_quest_state(member, false);
            let chance = MOBS.iter().find(|&&(id, _)| id == npc.id).map_or(0, |&(_, c)| c);
            if ctx.get_random(1000) < chance {
                // NOTE(java-quirk): the fragment goes to the killer, not to the member drawn.
                ctx.give_items(player, PROPHECY_FRAGMENT, 1);
                if ctx.get_quest_items_count(player, PROPHECY_FRAGMENT) >= FRAGMENT_COUNT {
                    ctx.take_items(player, PROPHECY_FRAGMENT, -1);
                    if let Some(qs) = qs {
                        qs.set_cond(ctx, 5, true);
                    }
                } else {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            NATOOLS => match qs.state(ctx) {
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30894-01.html".to_owned(),
                    _ => htmltext = "30894-04.html".to_owned(),
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            RAYMOND => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30289-01.html".to_owned(),
                        2 => {
                            if qs.is_set(ctx, "talk") {
                                htmltext = "30289-03.html".to_owned();
                            } else {
                                ctx.take_items(player, CRYPTOGRAM_OF_THE_ANGEL_SEARCH, -1);
                                qs.set(ctx, "talk", "1");
                                htmltext = "30289-02.html".to_owned();
                            }
                        }
                        3..=5 => htmltext = "30289-06.html".to_owned(),
                        6 => {
                            ctx.give_adena(player, 92_676, true);
                            if ctx.player_level(player) <= MAX_REWARD_LEVEL {
                                ctx.add_exp_and_sp(player, 223_036, 13_091);
                            }
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30289-07.html".to_owned();
                        }
                        _ => {}
                    }
                }
            }
            CASIAN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 | 2 => htmltext = "30612-01.html".to_owned(),
                        3 => {
                            if qs.get_int(ctx, "talk") == 1 {
                                htmltext = "30612-03.html".to_owned();
                            } else if qs.get_int(ctx, "talk") == 2 {
                                htmltext = "30612-06.html".to_owned();
                            } else {
                                htmltext = "30612-02.html".to_owned();
                                qs.set(ctx, "talk", "1");
                            }
                        }
                        4..=6 => htmltext = "30612-09.html".to_owned(),
                        _ => {}
                    }
                }
            }
            ROCK => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        5 => htmltext = "32368-02.html".to_owned(),
                        6 => htmltext = "32368-05.html".to_owned(),
                        _ => htmltext = "32368-01.html".to_owned(),
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00142_FallenAngelRequestOfDawn";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30289" type="Folk" level="1" name="Raymond"/>
        <npc id="30612" type="Folk" level="1" name="Casian"/>
        <npc id="30894" type="Folk" level="1" name="Natools"/>
        <npc id="32368" type="Folk" level="1" name="Rock"/>
        <npc id="20088" type="Monster" level="40" name="Ant Warrior Captain"/>
        <npc id="27338" type="Monster" level="40" name="Fallen Angel"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10351" type="EtcItem" name="Cryptogram"><set name="is_questitem" val="true"/></item>
        <item id="10352" type="EtcItem" name="Prophecy Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10353" type="EtcItem" name="Fallen Angel Blood"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        raymond: Oid,
        casian: Oid,
        natools: Oid,
        rock: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00142FallenAngelRequestOfDawn);
        let pages = Pages::of(&Q00142FallenAngelRequestOfDawn)
            .page("30289-02.html", &["30289-03.html"])
            .page("30289-03.html", &["30289-04.html"])
            .page("30289-04.html", &["30289-05.html"])
            .page("30612-02.html", &["30612-03.html"])
            .page("30612-03.html", &["30612-04.html"])
            .page("30612-04.html", &["30612-05.html"])
            .page("30612-05.html", &["30612-06.html"])
            .page("30612-06.html", &["30612-07.html"])
            .page("30612-07.html", &["30612-08.html"])
            .page("30894-01.html", &["30894-02.html"])
            .page("30894-02.html", &["30894-03.html"])
            .page("32368-02.html", &["32368-04.html"])
            .plain(&[
                "30289-01.html",
                "30289-05.html",
                "30289-06.html",
                "30289-07.html",
                "30612-01.html",
                "30612-08.html",
                "30612-09.html",
                "30894-03.html",
                "30894-04.html",
                "32368-01.html",
                "32368-03.html",
                "32368-04.html",
                "32368-05.html",
            ])
            .npcs(&[NATOOLS, RAYMOND, CASIAN, ROCK]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            raymond: t.spawn(RAYMOND, 20),
            casian: t.spawn(CASIAN, 30),
            natools: t.spawn(NATOOLS, 40),
            rock: t.spawn(ROCK, 50),
        };
        (t, npcs)
    }

    /// Runs a chain of page links as the player clicks them.
    fn click(t: &mut ScriptTest, events: &[&str]) {
        for e in events {
            assert!(t.event(1, NAME, e).contains(e), "{e}");
        }
    }

    #[test]
    fn the_quest_has_no_start_npc_and_an_unstarted_player_gets_the_default_page() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        assert!(!t.quest_page(1, npcs.natools).contains("30894-0"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_chain_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        // Q00998 makes the state (`startQuest(name, player)`) and notifies this event.
        t.world.run_as_script(NAME, |ctx| {
            let _ = ctx.get_quest_state(Oid(1), true);
        });
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let _ = Q00142FallenAngelRequestOfDawn.on_event(ctx, "30894-01.html", None, Some(Oid(1)));
        });
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.natools).contains("30894-01.html"));
        click(&mut t, &["30894-02.html", "30894-03.html"]);
        assert_eq!(t.count(1, CRYPTOGRAM_OF_THE_ANGEL_SEARCH), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.natools).contains("30894-04.html"));

        // Raymond takes the cryptogram on the first visit.
        assert!(t.quest_page(1, npcs.raymond).contains("30289-02.html"));
        assert_eq!(t.count(1, CRYPTOGRAM_OF_THE_ANGEL_SEARCH), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        click(&mut t, &["30289-03.html", "30289-04.html", "30289-05.html"]);
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "3")]
        );
        assert!(t.quest_page(1, npcs.raymond).contains("30289-06.html"));

        // Casian: talk 1, then 2 at "30612-05.html".
        assert!(t.quest_page(1, npcs.casian).contains("30612-02.html"));
        click(&mut t, &["30612-03.html", "30612-04.html", "30612-05.html"]);
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "talk", "1"), rows::update(NAME, "talk", "2")]
        );
        assert!(t.quest_page(1, npcs.casian).contains("30612-06.html"));
        click(&mut t, &["30612-07.html", "30612-08.html"]);
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "4")]
        );

        // Thirty fragments (the captain drops 100 %): cond 5, the fragments are taken.
        for _ in 0..30 {
            let ant = t.spawn(20088, 60);
            t.kill(1, ant);
        }
        assert_eq!(t.count(1, PROPHECY_FRAGMENT), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The rock spawns the angel once; a second try says so.
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        click(&mut t, &["32368-04.html"]);
        let angels = t.npcs_of(FALLEN_ANGEL);
        assert_eq!(angels.len(), 1);
        assert!(t.quest_page(1, npcs.rock).contains("32368-02.html"));
        assert!(t.event(1, NAME, "32368-04.html").contains("32368-03.html"));
        t.kill(1, angels[0]);
        assert_eq!(t.count(1, FALLEN_ANGEL_BLOOD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.rock).contains("32368-05.html"));

        assert!(t.quest_page(1, npcs.raymond).contains("30289-07.html"));
        assert_eq!(t.count(1, ADENA), 92_676);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
