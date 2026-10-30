//! Quest 312: Take Advantage of the Crisis! (Filaur in the Dark Elven village, the Lunatic mobs).
// Java: dist/game/data/scripts/quests/Q00312_TakeAdvantageOfTheCrisis/Q00312_TakeAdvantageOfTheCrisis.java
//
// Pattern: a party kill, a random member at cond N (`docs/design/scripts.md`, "Quest patterns"),
// with the fragment going to the member drawn. The NPC has no reward: the quest ends with the
// player's own choice (`30535-11.html`), which also takes the fragments.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const FILAUR: i32 = 30535;

// Monsters (id, drop chance per mille)
const MOBS: [(i32, i32); 13] = [
    (22678, 291), // Grave Robber Summoner (Lunatic)
    (22679, 596), // Grave Robber Magician (Lunatic)
    (22680, 610), // Grave Robber Worker (Lunatic)
    (22681, 626), // Grave Robber Warrior (Lunatic)
    (22682, 692), // Grave Robber Warrior of Light (Lunatic)
    (22683, 650), // Servitor of Darkness
    (22684, 310), // Servitor of Darkness
    (22685, 626), // Servitor of Darkness
    (22686, 626), // Servitor of Darkness
    (22687, 308), // Phantoms of the Mine
    (22688, 416), // Evil Spirits of the Mine
    (22689, 212), // Mine Bug
    (22690, 748), // Earthworm's Descendant
];

// Item
const MINERAL_FRAGMENT: i32 = 14875;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00312_TakeAdvantageOfTheCrisis`.
pub struct Q00312TakeAdvantageOfTheCrisis;

impl Script for Q00312TakeAdvantageOfTheCrisis {
    fn name(&self) -> &'static str {
        "Q00312_TakeAdvantageOfTheCrisis"
    }

    fn path(&self) -> &'static str {
        "quests/Q00312_TakeAdvantageOfTheCrisis"
    }

    fn id(&self) -> i32 {
        312
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FILAUR]);
        s.add_talk_id(&[FILAUR]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[MINERAL_FRAGMENT]);
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30535-02.html" | "30535-03.html" | "30535-04.html" | "30535-05.htm" | "30535-09.html"
            | "30535-10.html" => {}
            "30535-06.htm" => {
                qs.start_quest(ctx);
            }
            "30535-11.html" => {
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        // Java draws the member first, then rolls: both happen only with a member.
        let member = ctx.get_random_party_member_cond(player, 1);
        if let Some(member) = member {
            // Only the registered monsters reach here; any other id never drops.
            let chance = MOBS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0, |(_, chance)| *chance);
            if ctx.get_random(1000) < chance {
                ctx.give_items(member, MINERAL_FRAGMENT, 1);
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30535-01.htm"
            } else {
                "30535-00.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = if ctx.has_quest_items(player, MINERAL_FRAGMENT) {
                "30535-08.html"
            } else {
                "30535-07.html"
            }
            .to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00312_TakeAdvantageOfTheCrisis";

    const NPCS: &str = r#"
        <npc id="30535" type="Folk" level="1" name="Filaur"/>
        <npc id="22689" type="Monster" level="60" name="Mine Bug"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14875" type="EtcItem" name="Mineral Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00312TakeAdvantageOfTheCrisis);
        let pages = Pages::of(&Q00312TakeAdvantageOfTheCrisis)
            .page("30535-01.htm", &["30535-02.html"])
            .page("30535-02.html", &["30535-03.html"])
            .page("30535-03.html", &["30535-04.html"])
            .page("30535-04.html", &["30535-05.htm"])
            .page("30535-05.htm", &["30535-06.htm"])
            .page("30535-07.html", &["30535-09.html", "30535-11.html"])
            .page("30535-08.html", &["30535-09.html", "30535-10.html"])
            .page("30535-10.html", &["30535-11.html"])
            .plain(&["30535-00.htm", "30535-06.htm", "30535-09.html", "30535-11.html"])
            .npcs(&[FILAUR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let filaur = t.spawn(FILAUR, 20);
        (t, filaur)
    }

    /// Kills Mine Bugs until a fragment drops; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let bug = t.spawn(22689, 60);
            t.kill(1, bug);
            if t.count(1, MINERAL_FRAGMENT) >= 1 {
                return kills;
            }
        }
        panic!("no fragment after 400 kills");
    }

    #[test]
    fn a_level_seventy_nine_character_is_sent_away() {
        let (mut t, filaur) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, filaur).contains("30535-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_story_then_the_fragments_and_the_end() {
        let (mut t, filaur) = setup();
        t.enter_as(1, 80, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, filaur).contains("30535-01.htm"));
        for event in ["30535-02.html", "30535-03.html", "30535-04.html", "30535-05.htm"] {
            assert!(t.event(1, NAME, event).contains(event), "{event}");
        }
        assert!(t.event(1, NAME, "30535-06.htm").contains("30535-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No fragment yet: the NPC sends the player out, with the two links of 07.
        assert!(t.quest_page(1, filaur).contains("30535-07.html"));
        assert!(t.event(1, NAME, "30535-09.html").contains("30535-09.html"));
        assert!(t.quest_page(1, filaur).contains("30535-07.html"));

        // The drawn member (the killer) gets a fragment, which turns the NPC's page.
        let kills = hunt(&mut t);
        assert!(kills >= 1, "{kills}");
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, filaur).contains("30535-08.html"));

        // "30535-10.html" then the end: the fragments go, the quest ends repeatable, with the
        // finish sound.
        assert!(t.event(1, NAME, "30535-10.html").contains("30535-10.html"));
        t.bypass(1, &format!("Script {NAME} 30535-11.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30535-11.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, MINERAL_FRAGMENT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
