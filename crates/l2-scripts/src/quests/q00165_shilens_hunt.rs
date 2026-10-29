//! Quest 165: Shilen's Hunt (Nelsya sends the Dark Elves after the wolves and keltirs for bezoars).
// Java: dist/game/data/scripts/quests/Q00165_ShilensHunt/Q00165_ShilensHunt.java
//
// Pattern: a kill count with a drop chance up to a limit, per monster: a monster drops a bezoar
// when `getRandom(3)` is below its chance (`MONSTERS`). The 13th bezoar sets cond 2 (no sound),
// the start NPC gives the potions and ends the quest. Dark Elves only (race gate).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NELSYA: i32 = 30348;

// Monsters: id, the drop chance bound of `getRandom(3) < chance` (Java `MONSTERS`).
const MONSTERS: [(i32, i32); 4] = [
    (20456, 3), // Ashen Wolf
    (20529, 1), // Young Brown Keltir
    (20532, 1), // Brown Keltir
    (20536, 2), // Elder Brown Keltir
];

// Items
const LESSER_HEALING_POTION: i32 = 1060;
const DARK_BEZOAR: i32 = 1160;

// Misc
const MIN_LEVEL: i32 = 3;
const REQUIRED_COUNT: i64 = 13;

/// Java `Q00165_ShilensHunt`.
pub struct Q00165ShilensHunt;

impl Script for Q00165ShilensHunt {
    fn name(&self) -> &'static str {
        "Q00165_ShilensHunt"
    }

    fn path(&self) -> &'static str {
        "quests/Q00165_ShilensHunt"
    }

    fn id(&self) -> i32 {
        165
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NELSYA]);
        s.add_talk_id(&[NELSYA]);
        let ids: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[DARK_BEZOAR]);
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
            && event.eq_ignore_ascii_case("30348-03.htm")
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        // The listener only fires for the ids of `MONSTERS`.
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.get_random(3) < chance
        {
            ctx.give_items(killer, DARK_BEZOAR, 1);
            if ctx.get_quest_items_count(killer, DARK_BEZOAR) < REQUIRED_COUNT {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            } else {
                qs.set_cond(ctx, 2, true);
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
                        "30348-02.htm".to_owned()
                    } else {
                        "30348-01.htm".to_owned()
                    }
                } else {
                    "30348-00.htm".to_owned()
                };
            }
            STARTED => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, DARK_BEZOAR) >= REQUIRED_COUNT {
                    ctx.give_items(player, LESSER_HEALING_POTION, 5);
                    ctx.add_exp_and_sp(player, 1_000, 0);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30348-05.html".to_owned();
                } else {
                    htmltext = "30348-04.html".to_owned();
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

    const NAME: &str = "Q00165_ShilensHunt";
    const KELTIR: i32 = 20532;

    const NPCS: &str = r#"
        <npc id="30348" type="Folk" level="1" name="Nelsya"/>
        <npc id="20532" type="Monster" level="5" name="Brown Keltir"/>"#;

    const ITEMS: &str = r#"
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="1160" type="EtcItem" name="Dark Bezoar"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00165ShilensHunt);
        let pages = Pages::of(&Q00165ShilensHunt)
            .page("30348-02.htm", &["30348-03.htm"])
            .plain(&[
                "30348-00.htm",
                "30348-01.htm",
                "30348-03.htm",
                "30348-04.html",
                "30348-05.html",
            ])
            .npcs(&[NELSYA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let nelsya = t.spawn(NELSYA, 20);
        (t, nelsya)
    }

    #[test]
    fn only_a_dark_elf_of_level_3_starts_the_hunt() {
        let (mut t, nelsya) = setup();
        t.enter_as(1, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, nelsya).contains("30348-00.htm"));
        t.enter_as(2, 2, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(2, nelsya).contains("30348-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn thirteen_bezoars_and_the_potions() {
        let (mut t, nelsya) = setup();
        t.enter_as(1, 3, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, nelsya).contains("30348-02.htm"));
        assert!(t.event(1, NAME, "30348-03.htm").contains("30348-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Bezoars until 13: the last one sets cond 2 (no pick-up sound for it).
        let mut kills = 0;
        while t.count(1, DARK_BEZOAR) < REQUIRED_COUNT {
            let keltir = t.spawn(KELTIR, 40);
            t.kill(1, keltir);
            kills += 1;
            assert!(kills < 500, "the keltirs never dropped 13 bezoars");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Nelsya: five potions, 1000 exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, nelsya).contains("30348-05.html"));
        assert_eq!(t.count(1, LESSER_HEALING_POTION), 5);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
