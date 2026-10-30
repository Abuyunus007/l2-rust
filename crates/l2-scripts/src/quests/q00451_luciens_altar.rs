//! Quest 451: Lucien's Altar (Daichir and the five altars that take the replenished beads).
// Java: dist/game/data/scripts/quests/Q00451_LuciensAltar/Q00451_LuciensAltar.java
//
// Pattern: a daily quest (`exit_quest_type(Daily)`) whose talk NPCs are many: each altar takes a
// bead once (the variable named after the altar's id), five discharged beads move the quest to
// cond 2, and the NPC of the start hands out the reward.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DAICHIR: i32 = 30537;
const ALTARS: [i32; 5] = [32706, 32707, 32708, 32709, 32710];

// Items
const REPLENISHED_BEAD: i32 = 14877;
const DISCHARGED_BEAD: i32 = 14878;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00451_LuciensAltar`.
pub struct Q00451LuciensAltar;

impl Script for Q00451LuciensAltar {
    fn name(&self) -> &'static str {
        "Q00451_LuciensAltar"
    }

    fn path(&self) -> &'static str {
        "quests/Q00451_LuciensAltar"
    }

    fn id(&self) -> i32 {
        451
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DAICHIR]);
        s.add_talk_id(&ALTARS);
        s.add_talk_id(&[DAICHIR]);
        s.register_quest_items(&[REPLENISHED_BEAD, DISCHARGED_BEAD]);
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
        if event == "30537-04.htm" {
            htmltext = Some(event.to_owned());
        } else if event == "30537-05.htm" {
            qs.start_quest(ctx);
            ctx.give_items(player, REPLENISHED_BEAD, 5);
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        if npc.id == DAICHIR {
            let mut state = qs.state(ctx);
            // Java: `case COMPLETED` falls through to `CREATED` once the quest is available again.
            if state == COMPLETED {
                if !qs.is_now_available(ctx) {
                    return Some("30537-03.html".to_owned());
                }
                qs.set_state(ctx, CREATED);
                state = CREATED;
            }
            match state {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30537-01.htm"
                    } else {
                        "30537-02.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = if ALTARS.iter().any(|altar| qs.is_set(ctx, &altar.to_string())) {
                            "30537-10.html"
                        } else {
                            "30537-09.html"
                        }
                        .to_owned();
                    } else {
                        // Tauti reward: 13 773 960 exp, 16 232 820 sp, 742 800 Adena.
                        ctx.give_adena(player, 255_380, true);
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        htmltext = "30537-08.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, REPLENISHED_BEAD) {
            // Each altar takes one bead: its own id is the variable that remembers it.
            let var = npc.id.to_string();
            if qs.get_int(ctx, &var) == 0 {
                qs.set(ctx, &var, "1");
                ctx.take_items(player, REPLENISHED_BEAD, 1);
                ctx.give_items(player, DISCHARGED_BEAD, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                if ctx.get_quest_items_count(player, DISCHARGED_BEAD) >= 5 {
                    qs.set_cond(ctx, 2, true);
                }
                htmltext = "recharge.html".to_owned();
            } else {
                htmltext = "findother.html".to_owned();
            }
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

    const NAME: &str = "Q00451_LuciensAltar";

    const NPCS: &str = r#"
        <npc id="30537" type="Folk" level="70" name="Daichir"/>
        <npc id="32706" type="Folk" level="70" name="Altar of Lucien 1"/>
        <npc id="32707" type="Folk" level="70" name="Altar of Lucien 2"/>
        <npc id="32708" type="Folk" level="70" name="Altar of Lucien 3"/>
        <npc id="32709" type="Folk" level="70" name="Altar of Lucien 4"/>
        <npc id="32710" type="Folk" level="70" name="Altar of Lucien 5"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14877" type="EtcItem" name="Replenished Bead"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14878" type="EtcItem" name="Discharged Bead"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00451LuciensAltar);
        let pages = Pages::of(&Q00451LuciensAltar)
            .page("30537-01.htm", &["30537-04.htm"])
            .page("30537-04.htm", &["30537-05.htm"])
            .plain(&[
                "30537-02.htm",
                "30537-03.html",
                "30537-05.htm",
                "30537-08.html",
                "30537-09.html",
                "30537-10.html",
                "findother.html",
                "recharge.html",
            ])
            .npcs(&[DAICHIR, 32706, 32707, 32708, 32709, 32710]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let daichir = t.spawn(DAICHIR, 20);
        (t, daichir)
    }

    fn insert(var: &str, value: &str) -> l2_world::QuestWrite {
        rows::insert(NAME, var, value)
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, daichir) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, daichir).contains("30537-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_five_beads_then_the_reward() {
        let (mut t, daichir) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, daichir).contains("30537-01.htm"));
        assert!(t.event(1, NAME, "30537-04.htm").contains("30537-04.htm"));
        assert!(t.event(1, NAME, "30537-05.htm").contains("30537-05.htm"));
        assert_eq!(t.count(1, REPLENISHED_BEAD), 5);
        assert_eq!(
            t.take_writes(),
            [insert("cond", "1"), insert("<state>", "Started")]
        );

        // Each altar takes one bead; a second visit to the same altar finds nothing to do.
        let altars: Vec<Oid> = ALTARS.iter().map(|id| t.spawn(*id, 40)).collect();
        assert!(t.quest_page(1, altars[0]).contains("recharge.html"));
        assert_eq!(t.count(1, DISCHARGED_BEAD), 1);
        assert_eq!(t.take_writes(), [insert("32706", "1")]);
        assert!(t.quest_page(1, altars[0]).contains("findother.html"));
        assert!(t.take_writes().is_empty());

        // One altar is done, so Daichir gives the second page (`isSet` of an altar id).
        assert!(t.quest_page(1, daichir).contains("30537-10.html"));

        // The other four altars: the fifth discharged bead sets cond 2.
        for altar in &altars[1..] {
            assert!(t.quest_page(1, *altar).contains("recharge.html"));
        }
        assert_eq!(t.count(1, DISCHARGED_BEAD), 5);
        assert_eq!(t.take_writes().last(), Some(&rows::update(NAME, "cond", "2")));

        // The reward: 255 380 adena, `exitQuest(DAILY, true)`.
        let adena = t.count(1, 57);
        assert!(t.quest_page(1, daichir).contains("30537-08.html"));
        assert_eq!(t.count(1, 57) - adena, 255_380);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(matches!(&writes[2], l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done for today: the daily page.
        assert!(t.quest_page(1, daichir).contains("30537-03.html"));
    }

    #[test]
    fn with_no_altar_done_daichir_asks_for_the_altars() {
        let (mut t, daichir) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, daichir);
        t.event(1, NAME, "30537-04.htm");
        t.event(1, NAME, "30537-05.htm");
        assert!(t.quest_page(1, daichir).contains("30537-09.html"));
    }
}
