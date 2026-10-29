//! Quest 272: Wrath of Ancestors (Livina in the Orc Village sends the orcs after the grave robbers).
// Java: dist/game/data/scripts/quests/Q00272_WrathOfAncestors/Q00272_WrathOfAncestors.java
//
// Pattern: a kill-count quest (`docs/design/scripts.md`, "Quest patterns"): every goblin grave
// robber gives one head while cond 1 lasts; the 50th sets cond 2, the NPC pays adena and ends the
// quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LIVINA: i32 = 30572;

// Items
const GRAVE_ROBBERS_HEAD: i32 = 1474;

// Monsters
const MONSTERS: [i32; 2] = [
    20319, // Goblin Grave Robber
    20320, // Goblin Tomb Raider Leader
];

// Misc
const MIN_LEVEL: i32 = 5;

/// Java `Q00272_WrathOfAncestors`.
pub struct Q00272WrathOfAncestors;

impl Script for Q00272WrathOfAncestors {
    fn name(&self) -> &'static str {
        "Q00272_WrathOfAncestors"
    }

    fn path(&self) -> &'static str {
        "quests/Q00272_WrathOfAncestors"
    }

    fn id(&self) -> i32 {
        272
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LIVINA]);
        s.add_talk_id(&[LIVINA]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[GRAVE_ROBBERS_HEAD]);
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
        if event.eq_ignore_ascii_case("30572-04.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            ctx.give_items(killer, GRAVE_ROBBERS_HEAD, 1);
            if ctx.get_quest_items_count(killer, GRAVE_ROBBERS_HEAD) >= 50 {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ORC {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30572-03.htm"
                    } else {
                        "30572-02.htm"
                    }
                } else {
                    "30572-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30572-05.html".to_owned(),
                2 => {
                    ctx.give_adena(player, 1500, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30572-06.html".to_owned();
                }
                _ => {}
            },
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

    const NAME: &str = "Q00272_WrathOfAncestors";

    const NPCS: &str = r#"
        <npc id="30572" type="Folk" level="70" name="Livina"/>
        <npc id="20319" type="Monster" level="5" name="Goblin Grave Robber"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1474" type="EtcItem" name="Grave Robber's Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00272WrathOfAncestors);
        let pages = Pages::of(&Q00272WrathOfAncestors)
            .page("30572-03.htm", &["30572-04.htm"])
            .plain(&[
                "30572-01.htm",
                "30572-02.htm",
                "30572-04.htm",
                "30572-05.html",
                "30572-06.html",
            ])
            .npcs(&[LIVINA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let livina = t.spawn(LIVINA, 20);
        (t, livina)
    }

    #[test]
    fn only_orcs_of_level_five_can_take_it() {
        let (mut t, livina) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, livina).contains("30572-01.htm"));
        t.enter_as(2, 4, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, livina).contains("30572-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_heads_then_adena_and_the_quest_repeats() {
        let (mut t, livina) = setup();
        t.enter_as(1, 5, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, livina).contains("30572-03.htm"));
        assert!(t.event(1, NAME, "30572-04.htm").contains("30572-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, livina).contains("30572-05.html"));

        // The 50th head sets cond 2 (`setCond(2, true)`).
        for _ in 0..50 {
            let robber = t.spawn(20319, 60);
            t.kill(1, robber);
        }
        assert_eq!(t.count(1, GRAVE_ROBBERS_HEAD), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Livina pays 1500 adena; `exitQuest(true, true)`.
        assert!(t.quest_page(1, livina).contains("30572-06.html"));
        assert_eq!(t.count(1, 57), 1500);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
