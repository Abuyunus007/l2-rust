//! Quest 164: Blood Fiend (Creamees sends the player to kill Kirunak).
// Java: dist/game/data/scripts/quests/Q00164_BloodFiend/Q00164_BloodFiend.java
//
// Pattern: a one-time kill quest. The monster's death sets cond 2 and gives the skull; the start
// NPC gives the reward when the skull is in hand. Dark Elves are sent away (race gate).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const CREAMEES: i32 = 30149;

// Monster
const KIRUNAK: i32 = 27021;

// Item
const KIRUNAK_SKULL: i32 = 1044;

// Misc
const MIN_LEVEL: i32 = 21;

/// Java `Q00164_BloodFiend`.
pub struct Q00164BloodFiend;

impl Script for Q00164BloodFiend {
    fn name(&self) -> &'static str {
        "Q00164_BloodFiend"
    }

    fn path(&self) -> &'static str {
        "quests/Q00164_BloodFiend"
    }

    fn id(&self) -> i32 {
        164
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CREAMEES]);
        s.add_talk_id(&[CREAMEES]);
        s.add_kill_id(&[KIRUNAK]);
        s.register_quest_items(&[KIRUNAK_SKULL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30149-04.htm"
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::IHaveFulfilledMyContractWithTraderCreamees,
                &[],
            );
            ctx.give_items(killer, KIRUNAK_SKULL, 1);
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) != race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30149-03.htm".to_owned()
                    } else {
                        "30149-02.htm".to_owned()
                    }
                } else {
                    "30149-00.htm".to_owned()
                };
            }
            STARTED => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, KIRUNAK_SKULL) {
                    ctx.give_adena(player, 42_130, true);
                    ctx.add_exp_and_sp(player, 35_637, 1_854);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30149-06.html".to_owned();
                } else {
                    htmltext = "30149-05.html".to_owned();
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

    const NAME: &str = "Q00164_BloodFiend";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30149" type="Folk" level="1" name="Creamees"/>
        <npc id="27021" type="Monster" level="21" name="Kirunak"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1044" type="EtcItem" name="Kirunak's Skull"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00164BloodFiend);
        let pages = Pages::of(&Q00164BloodFiend)
            .page("30149-03.htm", &["30149-04.htm"])
            .plain(&[
                "30149-00.htm",
                "30149-02.htm",
                "30149-04.htm",
                "30149-05.html",
                "30149-06.html",
            ])
            .npcs(&[CREAMEES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let creamees = t.spawn(CREAMEES, 20);
        let kirunak = t.spawn(KIRUNAK, 40);
        (t, creamees, kirunak)
    }

    #[test]
    fn a_dark_elf_and_a_low_level_character_are_sent_away() {
        let (mut t, creamees, _) = setup();
        t.enter_as(1, 25, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, creamees).contains("30149-00.htm"));
        t.enter(2, 20, vec![]);
        assert!(t.quest_page(2, creamees).contains("30149-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_skull_of_kirunak_and_the_reward() {
        let (mut t, creamees, kirunak) = setup();
        t.enter(1, 21, vec![]);
        assert!(t.quest_page(1, creamees).contains("30149-03.htm"));
        assert!(t.event(1, NAME, "30149-04.htm").contains("30149-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Creamees before the kill: nothing to hand in.
        assert!(t.quest_page(1, creamees).contains("30149-05.html"));
        assert!(t.take_writes().is_empty());

        // Kirunak dies: the skull and cond 2.
        t.kill(1, kirunak);
        assert_eq!(t.count(1, KIRUNAK_SKULL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Creamees: adena, exp and sp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, creamees).contains("30149-06.html"));
        assert_eq!(t.count(1, ADENA), 42_130);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, creamees)
                .contains("This quest has already been completed.")
        );
    }
}
