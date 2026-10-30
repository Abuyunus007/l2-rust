//! Quest 263: Orc Subjugation (Kayleen in the Dark Elven area pays for the balor orc amulets and necklaces).
// Java: dist/game/data/scripts/quests/Q00263_OrcSubjugation/Q00263_OrcSubjugation.java
//
// Pattern: a repeatable collect quest (`docs/design/scripts.md`, "Quest patterns"): the balor orcs
// drop an amulet (archers) or a necklace (fighters) half of the time; Kayleen pays per item, with
// a bonus at 10 items or more, takes them all and the quest stays open until the player leaves.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KAYLEEN: i32 = 30346;

// Items
const ORC_AMULET: i32 = 1116;
const ORC_NECKLACE: i32 = 1117;

// Misc
const MIN_LEVEL: i32 = 8;

/// Java `MONSTERS`: the balor orcs and the item each one drops.
fn drop_item(npc_id: i32) -> Option<i32> {
    match npc_id {
        20385 => Some(ORC_AMULET),   // Balor Orc Archer
        20386 => Some(ORC_NECKLACE), // Balor Orc Fighter
        20387 => Some(ORC_NECKLACE), // Balor Orc Fighter Leader
        20388 => Some(ORC_NECKLACE), // Balor Orc Lieutenant
        _ => None,
    }
}

/// Java `Q00263_OrcSubjugation`.
pub struct Q00263OrcSubjugation;

impl Script for Q00263OrcSubjugation {
    fn name(&self) -> &'static str {
        "Q00263_OrcSubjugation"
    }

    fn path(&self) -> &'static str {
        "quests/Q00263_OrcSubjugation"
    }

    fn id(&self) -> i32 {
        263
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KAYLEEN]);
        s.add_talk_id(&[KAYLEEN]);
        s.add_kill_id(&[20385, 20386, 20387, 20388]);
        s.register_quest_items(&[ORC_AMULET, ORC_NECKLACE]);
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
            "30346-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30346-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30346-08.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.get_random(10) > 4 {
            if let Some(item) = drop_item(npc.id) {
                ctx.give_items(killer, item, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30346-03.htm"
                    } else {
                        "30346-02.htm"
                    }
                } else {
                    "30346-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                let registered = [ORC_AMULET, ORC_NECKLACE];
                if ctx.has_at_least_one_quest_item(player, &registered) {
                    let amulets = ctx.get_quest_items_count(player, ORC_AMULET);
                    let necklaces = ctx.get_quest_items_count(player, ORC_NECKLACE);
                    let bonus = if amulets + necklaces >= 10 { 1100 } else { 0 };
                    ctx.give_adena(player, amulets * 20 + necklaces * 30 + bonus, true);
                    ctx.take_items_many(player, -1, &registered);
                    htmltext = "30346-06.html".to_owned();
                } else {
                    htmltext = "30346-05.html".to_owned();
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
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00263_OrcSubjugation";

    const NPCS: &str = r#"
        <npc id="30346" type="Folk" level="70" name="Kayleen"/>
        <npc id="20385" type="Monster" level="30" name="Balor Orc Archer"/>
        <npc id="20386" type="Monster" level="30" name="Balor Orc Fighter"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1116" type="EtcItem" name="Orc Amulet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1117" type="EtcItem" name="Orc Necklace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00263OrcSubjugation);
        let pages = Pages::of(&Q00263OrcSubjugation)
            .page("30346-03.htm", &["30346-04.htm"])
            .page("30346-05.html", &["30346-07.html", "30346-08.html"])
            .page("30346-06.html", &["30346-07.html", "30346-08.html"])
            .plain(&[
                "30346-01.htm",
                "30346-02.htm",
                "30346-04.htm",
                "30346-07.html",
                "30346-08.html",
            ])
            .npcs(&[KAYLEEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kayleen = t.spawn(KAYLEEN, 20);
        (t, kayleen)
    }

    #[test]
    fn only_dark_elves_of_level_eight_can_take_it() {
        let (mut t, kayleen) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, kayleen).contains("30346-01.htm"));
        t.enter_as(2, 7, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(2, kayleen).contains("30346-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn amulets_and_necklaces_are_paid_with_the_bonus_from_ten_items() {
        let (mut t, kayleen) = setup();
        t.enter_as(1, 8, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, kayleen).contains("30346-03.htm"));
        assert!(t.event(1, NAME, "30346-04.htm").contains("30346-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing in the bag yet.
        assert!(t.quest_page(1, kayleen).contains("30346-05.html"));

        // Archers drop amulets, fighters necklaces; no condition changes.
        for _ in 0..200 {
            if t.count(1, ORC_AMULET) + t.count(1, ORC_NECKLACE) >= 10 {
                break;
            }
            let archer = t.spawn(20385, 60);
            t.kill(1, archer);
            let fighter = t.spawn(20386, 60);
            t.kill(1, fighter);
        }
        let amulets = t.count(1, ORC_AMULET);
        let necklaces = t.count(1, ORC_NECKLACE);
        assert!(amulets + necklaces >= 10);
        assert!(t.take_writes().is_empty());

        // Kayleen pays 20 per amulet, 30 per necklace and 1100 more from 10 items on.
        assert!(t.quest_page(1, kayleen).contains("30346-06.html"));
        assert_eq!(t.count(1, 57), amulets * 20 + necklaces * 30 + 1100);
        assert_eq!((t.count(1, ORC_AMULET), t.count(1, ORC_NECKLACE)), (0, 0));
        assert!(t.take_writes().is_empty());

        // The quest stays open until the player leaves it (`exitQuest(true, true)`).
        assert!(t.event(1, NAME, "30346-07.html").contains("30346-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
