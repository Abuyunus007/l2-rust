//! Quest 167: Dwarven Kinship (Carlon sends the player's letter to Haprock, then Norman).
// Java: dist/game/data/scripts/quests/Q00167_DwarvenKinship/Q00167_DwarvenKinship.java
//
// Pattern: a delivery chain over three NPCs. The start NPC hands out the letter; Haprock either
// pays for it at once (cond 1, `exitQuest(false, true)`) or swaps it for Norman's letter and
// moves to cond 2; Norman pays and ends the quest (one-time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NORMAN: i32 = 30210;
const HAPROCK: i32 = 30255;
const CARLON: i32 = 30350;

// Items
const CARLONS_LETTER: i32 = 1076;
const NORMANS_LETTER: i32 = 1106;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00167_DwarvenKinship`.
pub struct Q00167DwarvenKinship;

impl Script for Q00167DwarvenKinship {
    fn name(&self) -> &'static str {
        "Q00167_DwarvenKinship"
    }

    fn path(&self) -> &'static str {
        "quests/Q00167_DwarvenKinship"
    }

    fn id(&self) -> i32 {
        167
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CARLON]);
        s.add_talk_id(&[CARLON, NORMAN, HAPROCK]);
        s.register_quest_items(&[CARLONS_LETTER, NORMANS_LETTER]);
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
            "30210-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, NORMANS_LETTER) {
                    ctx.give_adena(player, 20_000, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30255-02.html" => htmltext = Some(event.to_owned()),
            "30255-03.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, CARLONS_LETTER) {
                    ctx.take_items(player, CARLONS_LETTER, -1);
                    ctx.give_items(player, NORMANS_LETTER, 1);
                    ctx.give_adena(player, 2_000, true);
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30255-04.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, CARLONS_LETTER) {
                    ctx.give_adena(player, 15_000, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30350-03.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, CARLONS_LETTER, 1);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            CARLON => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30350-02.htm".to_owned()
                    } else {
                        "30350-01.htm".to_owned()
                    };
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, CARLONS_LETTER) {
                        htmltext = "30350-04.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            HAPROCK => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, CARLONS_LETTER) {
                    htmltext = "30255-01.html".to_owned();
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, NORMANS_LETTER) {
                    htmltext = "30255-05.html".to_owned();
                }
            }
            NORMAN => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, NORMANS_LETTER) {
                    htmltext = "30210-01.html".to_owned();
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00167_DwarvenKinship";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30210" type="Folk" level="1" name="Norman"/>
        <npc id="30255" type="Folk" level="1" name="Haprock"/>
        <npc id="30350" type="Folk" level="1" name="Carlon"/>"#;

    const ITEMS: &str = r#"
        <item id="1076" type="EtcItem" name="Carlon's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1106" type="EtcItem" name="Norman's Letter"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        norman: Oid,
        haprock: Oid,
        carlon: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00167DwarvenKinship);
        let pages = Pages::of(&Q00167DwarvenKinship)
            .page("30350-02.htm", &["30350-03.htm"])
            .page("30255-01.html", &["30255-02.html"])
            .page("30255-02.html", &["30255-03.html", "30255-04.html"])
            .page("30210-01.html", &["30210-02.html"])
            .plain(&[
                "30350-01.htm",
                "30350-03.htm",
                "30350-04.html",
                "30255-03.html",
                "30255-04.html",
                "30255-05.html",
                "30210-02.html",
            ])
            .npcs(&[NORMAN, HAPROCK, CARLON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            norman: t.spawn(NORMAN, 60),
            haprock: t.spawn(HAPROCK, 40),
            carlon: t.spawn(CARLON, 20),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_15_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, npcs.carlon).contains("30350-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_to_norman_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, npcs.carlon).contains("30350-02.htm"));
        assert!(t.event(1, NAME, "30350-03.htm").contains("30350-03.htm"));
        assert_eq!(t.count(1, CARLONS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Haprock takes Carlon's letter and gives Norman's: 2000 adena, cond 2 (no sound).
        assert!(t.quest_page(1, npcs.haprock).contains("30255-01.html"));
        assert!(t.event(1, NAME, "30255-02.html").contains("30255-02.html"));
        assert!(t.event(1, NAME, "30255-03.html").contains("30255-03.html"));
        assert_eq!(t.count(1, CARLONS_LETTER), 0);
        assert_eq!(t.count(1, NORMANS_LETTER), 1);
        assert_eq!(t.count(1, ADENA), 2_000);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.haprock).contains("30255-05.html"));

        // Norman pays 20000 adena and ends the quest (`exitQuest(false, true)`).
        assert!(t.quest_page(1, npcs.norman).contains("30210-01.html"));
        assert!(t.event(1, NAME, "30210-02.html").contains("30210-02.html"));
        assert_eq!(t.count(1, ADENA), 22_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn haprock_pays_at_once_with_carlons_letter() {
        let (mut t, npcs) = setup();
        let mut data = player_data(1, 20, vec![item(5, CARLONS_LETTER, 1)]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, npcs.haprock).contains("30255-01.html"));
        assert!(t.event(1, NAME, "30255-02.html").contains("30255-02.html"));
        assert!(t.event(1, NAME, "30255-04.html").contains("30255-04.html"));
        assert_eq!(t.count(1, ADENA), 15_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
