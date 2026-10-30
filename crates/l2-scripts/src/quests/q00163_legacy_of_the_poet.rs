//! Quest 163: Legacy of the Poet (Starden wants four poems from the Baraq orcs).
// Java: dist/game/data/scripts/quests/Q00163_LegacyOfThePoet/Q00163_LegacyOfThePoet.java
//
// Pattern: a one-time kill quest with four separate drops, each rolled on every kill. Dark Elves
// are sent away (race gate).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const STARDEN: i32 = 30220;

// Monsters
const MONSTERS: [i32; 2] = [
    20372, // Baraq Orc Fighter
    20373, // Baraq Orc Warrior Leader
];

// Items
const RUMIELS_1ST_POEM: i32 = 1038;
const RUMIELS_2ND_POEM: i32 = 1039;
const RUMIELS_3RD_POEM: i32 = 1040;
const RUMIELS_4TH_POEM: i32 = 1041;

// Misc
const MIN_LEVEL: i32 = 11;

/// Java `Q00163_LegacyOfThePoet`.
pub struct Q00163LegacyOfThePoet;

/// One poem of `onKill`: give it, then either cond 2 (the other three are in hand) or the sound.
fn give_poem(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::script::Qs,
    killer: ObjectId,
    poem: i32,
    others: [i32; 3],
) {
    ctx.give_items(killer, poem, 1);
    if ctx.has_all_quest_items(killer, &others) {
        qs.set_cond(ctx, 2, true);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q00163LegacyOfThePoet {
    fn name(&self) -> &'static str {
        "Q00163_LegacyOfThePoet"
    }

    fn path(&self) -> &'static str {
        "quests/Q00163_LegacyOfThePoet"
    }

    fn id(&self) -> i32 {
        163
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STARDEN]);
        s.add_talk_id(&[STARDEN]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[
            RUMIELS_1ST_POEM,
            RUMIELS_2ND_POEM,
            RUMIELS_3RD_POEM,
            RUMIELS_4TH_POEM,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "30220-03.html" | "30220-04.html" => htmltext = Some(event.to_owned()),
            "30220-05.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            if ctx.get_random(10) == 0 && !ctx.has_quest_items(killer, RUMIELS_1ST_POEM) {
                give_poem(
                    ctx,
                    qs,
                    killer,
                    RUMIELS_1ST_POEM,
                    [RUMIELS_2ND_POEM, RUMIELS_3RD_POEM, RUMIELS_4TH_POEM],
                );
            }

            if ctx.get_random(10) > 7 && !ctx.has_quest_items(killer, RUMIELS_2ND_POEM) {
                give_poem(
                    ctx,
                    qs,
                    killer,
                    RUMIELS_2ND_POEM,
                    [RUMIELS_1ST_POEM, RUMIELS_3RD_POEM, RUMIELS_4TH_POEM],
                );
            }

            if ctx.get_random(10) > 7 && !ctx.has_quest_items(killer, RUMIELS_3RD_POEM) {
                give_poem(
                    ctx,
                    qs,
                    killer,
                    RUMIELS_3RD_POEM,
                    [RUMIELS_1ST_POEM, RUMIELS_2ND_POEM, RUMIELS_4TH_POEM],
                );
            }

            if ctx.get_random(10) > 5 && !ctx.has_quest_items(killer, RUMIELS_4TH_POEM) {
                give_poem(
                    ctx,
                    qs,
                    killer,
                    RUMIELS_4TH_POEM,
                    [RUMIELS_1ST_POEM, RUMIELS_2ND_POEM, RUMIELS_3RD_POEM],
                );
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) != race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30220-02.htm".to_owned()
                    } else {
                        "30220-01.htm".to_owned()
                    }
                } else {
                    "30220-00.htm".to_owned()
                };
            }
            STARTED => {
                if ctx.has_all_quest_items(
                    player,
                    &[
                        RUMIELS_1ST_POEM,
                        RUMIELS_2ND_POEM,
                        RUMIELS_3RD_POEM,
                        RUMIELS_4TH_POEM,
                    ],
                ) {
                    ctx.add_exp_and_sp(player, 21_643, 943);
                    ctx.give_adena(player, 13_890, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30220-07.html".to_owned();
                } else {
                    htmltext = "30220-06.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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

    const NAME: &str = "Q00163_LegacyOfThePoet";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30220" type="Folk" level="1" name="Starden"/>
        <npc id="20372" type="Monster" level="11" name="Baraq Orc Fighter"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1038" type="EtcItem" name="Rumiel's 1st Poem"><set name="is_questitem" val="true"/></item>
        <item id="1039" type="EtcItem" name="Rumiel's 2nd Poem"><set name="is_questitem" val="true"/></item>
        <item id="1040" type="EtcItem" name="Rumiel's 3rd Poem"><set name="is_questitem" val="true"/></item>
        <item id="1041" type="EtcItem" name="Rumiel's 4th Poem"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00163LegacyOfThePoet);
        let pages = Pages::of(&Q00163LegacyOfThePoet)
            .page("30220-02.htm", &["30220-03.html"])
            .page("30220-03.html", &["30220-04.html"])
            .page("30220-04.html", &["30220-05.htm"])
            .plain(&[
                "30220-00.htm",
                "30220-01.htm",
                "30220-05.htm",
                "30220-06.html",
                "30220-07.html",
            ])
            .npcs(&[STARDEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let starden = t.spawn(STARDEN, 20);
        (t, starden)
    }

    #[test]
    fn a_dark_elf_and_a_low_level_character_are_sent_away() {
        let (mut t, starden) = setup();
        t.enter_as(1, 11, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, starden).contains("30220-00.htm"));
        t.enter(2, 10, vec![]);
        assert!(t.quest_page(2, starden).contains("30220-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn four_poems_and_the_reward() {
        let (mut t, starden) = setup();
        t.enter(1, 11, vec![]);
        assert!(t.quest_page(1, starden).contains("30220-02.htm"));
        assert!(t.event(1, NAME, "30220-03.html").contains("30220-03.html"));
        assert!(t.event(1, NAME, "30220-04.html").contains("30220-04.html"));
        assert!(t.take_writes().is_empty());
        assert!(t.event(1, NAME, "30220-05.htm").contains("30220-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, starden).contains("30220-06.html"));

        // Kill until all four poems are in hand: the last one sets cond 2.
        let all = |t: &ScriptTest| {
            [
                RUMIELS_1ST_POEM,
                RUMIELS_2ND_POEM,
                RUMIELS_3RD_POEM,
                RUMIELS_4TH_POEM,
            ]
            .iter()
            .all(|&p| t.count(1, p) == 1)
        };
        let mut kills = 0;
        while !all(&t) {
            let orc = t.spawn(20372, 60);
            t.kill(1, orc);
            kills += 1;
            assert!(kills < 1000, "the orcs never dropped four poems");
        }
        let writes = t.take_writes();
        assert_eq!(writes.last(), Some(&rows::update(NAME, "cond", "2")));

        assert!(t.quest_page(1, starden).contains("30220-07.html"));
        assert_eq!(t.count(1, ADENA), 13_890);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
