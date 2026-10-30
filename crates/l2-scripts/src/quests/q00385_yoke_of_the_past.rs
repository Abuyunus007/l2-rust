//! Quest 385: Yoke of the Past (the Ziggurats of the Ancient Ruins, Scroll of Ancient Magic).
// Java: dist/game/data/scripts/quests/Q00385_YokeOfThePast/Q00385_YokeOfThePast.java
//
// Pattern: a kill-count quest with a drop chance per monster and no limit (`docs/design/scripts.md`,
// "Quest patterns"): the scrolls drop to a random started party member; the NPC exchanges every
// scroll for a blank scroll. Twenty-eight ziggurats start the quest.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ZIGGURATS: [i32; 28] = [
    31095, 31096, 31097, 31098, 31099, 31100, 31101, 31102, 31103, 31104, 31105, 31106, 31107, 31108, 31109,
    31110, 31114, 31115, 31116, 31117, 31118, 31119, 31120, 31121, 31122, 31123, 31124, 31125,
];

// Item
const SCROLL_OF_ANCIENT_MAGIC: i32 = 5902;

// Reward
const BLANK_SCROLL: i32 = 5965;

// Monsters (drop chance)
const MONSTER_CHANCES: [(i32, f64); 43] = [
    (21144, 0.306), // Catacomb Shadow
    (21156, 0.994), // Purgatory Shadow
    (21208, 0.146), // Hallowed Watchman
    (21209, 0.166), // Hallowed Seer
    (21210, 0.202), // Vault Guardian
    (21211, 0.212), // Vault Seer
    (21213, 0.274), // Hallowed Monk
    (21214, 0.342), // Vault Sentinel
    (21215, 0.360), // Vault Monk
    (21217, 0.460), // Hallowed Priest
    (21218, 0.558), // Vault Overlord
    (21219, 0.578), // Vault Priest
    (21221, 0.710), // Sepulcher Inquisitor
    (21222, 0.842), // Sepulcher Archon
    (21223, 0.862), // Sepulcher Inquisitor
    (21224, 0.940), // Sepulcher Guardian
    (21225, 0.970), // Sepulcher Sage
    (21226, 0.202), // Sepulcher Guardian
    (21227, 0.290), // Sepulcher Sage
    (21228, 0.316), // Sepulcher Guard
    (21229, 0.426), // Sepulcher Preacher
    (21230, 0.646), // Sepulcher Guard
    (21231, 0.654), // Sepulcher Preacher
    (21236, 0.238), // Barrow Sentinel
    (21237, 0.274), // Barrow Monk
    (21238, 0.342), // Grave Sentinel
    (21239, 0.360), // Grave Monk
    (21240, 0.410), // Barrow Overlord
    (21241, 0.460), // Barrow Priest
    (21242, 0.558), // Grave Overlord
    (21243, 0.578), // Grave Priest
    (21244, 0.642), // Crypt Archon
    (21245, 0.700), // Crypt Inquisitor
    (21246, 0.842), // Tomb Archon
    (21247, 0.862), // Tomb Inquisitor
    (21248, 0.940), // Crypt Guardian
    (21249, 0.970), // Crypt Sage
    (21250, 0.798), // Tomb Guardian
    (21251, 0.710), // Tomb Sage
    (21252, 0.684), // Crypt Guard
    (21253, 0.574), // Crypt Preacher
    (21254, 0.354), // Tomb Guard
    (21255, 0.250), // Tomb Preacher
];

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `Q00385_YokeOfThePast`.
pub struct Q00385YokeOfThePast;

impl Script for Q00385YokeOfThePast {
    fn name(&self) -> &'static str {
        "Q00385_YokeOfThePast"
    }

    fn path(&self) -> &'static str {
        "quests/Q00385_YokeOfThePast"
    }

    fn id(&self) -> i32 {
        385
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&ZIGGURATS);
        s.add_talk_id(&ZIGGURATS);
        let mobs: Vec<i32> = MONSTER_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SCROLL_OF_ANCIENT_MAGIC]);
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
            "ziggurat-03.htm" | "ziggurat-04.htm" | "ziggurat-06.htm" | "ziggurat-07.htm" => {
                htmltext = Some(event.to_owned());
            }
            "ziggurat-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                }
                htmltext = Some(event.to_owned());
            }
            "ziggurat-10.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                "ziggurat-01.htm"
            } else {
                "ziggurat-02.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if ctx.has_quest_items(talker, SCROLL_OF_ANCIENT_MAGIC) {
                let scrolls = ctx.get_quest_items_count(talker, SCROLL_OF_ANCIENT_MAGIC);
                ctx.reward_items(talker, BLANK_SCROLL, scrolls);
                ctx.take_items(talker, SCROLL_OF_ANCIENT_MAGIC, -1);
                htmltext = "ziggurat-09.html".to_owned();
            } else {
                htmltext = "ziggurat-08.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) {
            let chance = MONSTER_CHANCES
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0.0, |(_, c)| *c);
            ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                SCROLL_OF_ANCIENT_MAGIC,
                1,
                1,
                0,
                chance,
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00385_YokeOfThePast";

    const NPCS: &str = r#"
        <npc id="31095" type="Folk" level="70" name="Ziggurat"/>
        <npc id="21156" type="Monster" level="60" name="Purgatory Shadow"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5902" type="EtcItem" name="Scroll of Ancient Magic"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5965" type="EtcItem" name="Blank Scroll"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00385YokeOfThePast);
        let pages = Pages::of(&Q00385YokeOfThePast)
            .page("ziggurat-01.htm", &["ziggurat-03.htm"])
            .page("ziggurat-03.htm", &["ziggurat-04.htm"])
            .page("ziggurat-04.htm", &["ziggurat-05.htm", "ziggurat-07.htm"])
            .page("ziggurat-07.htm", &["ziggurat-05.htm"])
            .page("ziggurat-05.htm", &["ziggurat-06.htm"])
            .plain(&["ziggurat-02.htm", "ziggurat-06.htm"])
            .page(
                "ziggurat-08.html",
                &["ziggurat-10.html", "ziggurat-05.htm", "ziggurat-06.htm"],
            )
            .page(
                "ziggurat-09.html",
                &["ziggurat-10.html", "ziggurat-05.htm", "ziggurat-06.htm"],
            )
            .plain(&["ziggurat-10.html", "ziggurat-11.html"])
            .npcs(&[31095]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ziggurat = t.spawn(31095, 20);
        (t, ziggurat)
    }

    #[test]
    fn a_level_19_character_is_sent_away() {
        let (mut t, ziggurat) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, ziggurat).contains("ziggurat-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn scrolls_are_exchanged_for_blank_scrolls_and_the_quest_ends_repeatable() {
        let (mut t, ziggurat) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, ziggurat).contains("ziggurat-01.htm"));
        assert!(t.event(1, NAME, "ziggurat-03.htm").contains("ziggurat-03.htm"));
        assert!(t.event(1, NAME, "ziggurat-04.htm").contains("ziggurat-04.htm"));
        assert!(t.event(1, NAME, "ziggurat-05.htm").contains("ziggurat-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Without a scroll: "ziggurat-08".
        assert!(t.quest_page(1, ziggurat).contains("ziggurat-08.html"));
        assert!(t.event(1, NAME, "ziggurat-06.htm").contains("ziggurat-06.htm"));

        // The scrolls drop with no limit; the Purgatory Shadow almost always.
        for _ in 0..3 {
            let shadow = t.spawn(21156, 60);
            t.kill(1, shadow);
        }
        let scrolls = t.count(1, SCROLL_OF_ANCIENT_MAGIC);
        assert!(scrolls >= 1);
        assert!(t.take_writes().is_empty());

        // One blank scroll per scroll; the quest goes on until "I want to quit" (10).
        assert!(t.quest_page(1, ziggurat).contains("ziggurat-09.html"));
        assert_eq!(t.count(1, BLANK_SCROLL), scrolls);
        assert_eq!(t.count(1, SCROLL_OF_ANCIENT_MAGIC), 0);
        assert!(t.event(1, NAME, "ziggurat-10.html").contains("ziggurat-10.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
