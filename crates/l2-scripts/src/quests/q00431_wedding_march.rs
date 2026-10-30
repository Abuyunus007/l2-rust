//! Quest 431: Wedding March (Kantabilon; Lienrik and Lienrik Lad drop the silver crystals).
// Java: dist/game/data/scripts/quests/Q00431_WeddingMarch/Q00431_WeddingMarch.java
//
// Pattern: a party kill quest (`get_random_party_member_cond` drops the item for a member at
// cond 1) with a turn-in that gives the reward and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KANTABILON: i32 = 31042;

// Monsters
const MOBS: [i32; 2] = [
    20786, // Lienrik
    20787, // Lienrik Lad
];

// Items
const SILVER_CRYSTAL: i32 = 7540;
const WEDDING_ECHO_CRYSTAL: i32 = 7062;

// Misc
const MIN_LEVEL: i32 = 38;
const CRYSTAL_COUNT: i64 = 50;

/// Java `Q00431_WeddingMarch`.
pub struct Q00431WeddingMarch;

impl Script for Q00431WeddingMarch {
    fn name(&self) -> &'static str {
        "Q00431_WeddingMarch"
    }

    fn path(&self) -> &'static str {
        "quests/Q00431_WeddingMarch"
    }

    fn id(&self) -> i32 {
        431
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KANTABILON]);
        s.add_talk_id(&[KANTABILON]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[SILVER_CRYSTAL]);
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

        // Java `equalsIgnoreCase`: the page names are plain ASCII.
        if event.eq_ignore_ascii_case("31042-02.htm") {
            qs.start_quest(ctx);
            Some(event.to_owned())
        } else if event.eq_ignore_ascii_case("31042-06.html") {
            if ctx.get_quest_items_count(player, SILVER_CRYSTAL) < CRYSTAL_COUNT {
                return Some("31042-05.html".to_owned());
            }
            ctx.give_items(player, WEDDING_ECHO_CRYSTAL, 25);
            qs.exit_quest(ctx, true, true);
            Some(event.to_owned())
        } else {
            None
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(player, 1)`: a party member at cond 1 (the killer alone).
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if ctx.get_random_boolean() {
            ctx.give_items(member, SILVER_CRYSTAL, 1);
            if ctx.get_quest_items_count(member, SILVER_CRYSTAL) >= CRYSTAL_COUNT {
                // Java calls `getQuestState(member, false).setCond(2, true)` without a null
                // check: the member is at cond 1, so its state exists.
                if let Some(qs) = ctx.get_quest_state(member, false) {
                    qs.set_cond(ctx, 2, true);
                }
            } else {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31042-01.htm"
                } else {
                    "31042-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31042-03.html"
                } else {
                    "31042-04.html"
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00431_WeddingMarch";
    const LIENRIK: i32 = 20786;

    const NPCS: &str = r#"
        <npc id="31042" type="Folk" level="70" name="Kantabilon"/>
        <npc id="20786" type="Monster" level="40" name="Lienrik"/>
        <npc id="20787" type="Monster" level="40" name="Lienrik Lad"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7540" type="EtcItem" name="Silver Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7062" type="EtcItem" name="Wedding Echo Crystal"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00431WeddingMarch);
        let pages = Pages::of(&Q00431WeddingMarch)
            .page("31042-01.htm", &["31042-02.htm"])
            .plain(&[
                "31042-00.htm",
                "31042-02.htm",
                "31042-03.html",
                "31042-05.html",
                "31042-06.html",
            ])
            .page("31042-04.html", &["31042-06.html"])
            .npcs(&[KANTABILON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kantabilon = t.spawn(KANTABILON, 20);
        (t, kantabilon)
    }

    /// Kills Lienrik until 50 silver crystals are in the bag; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..1_000 {
            let mob = t.spawn(LIENRIK, 60);
            t.kill(1, mob);
            if t.count(1, SILVER_CRYSTAL) >= CRYSTAL_COUNT {
                return kills;
            }
        }
        panic!("no 50 crystals after 1000 kills");
    }

    #[test]
    fn a_character_below_level_38_is_sent_away() {
        let (mut t, kantabilon) = setup();
        t.enter(1, 37, vec![]);
        assert!(t.quest_page(1, kantabilon).contains("31042-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_silver_crystals_then_the_echo_crystals() {
        let (mut t, kantabilon) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, kantabilon).contains("31042-01.htm"));
        assert!(t.event(1, NAME, "31042-02.htm").contains("31042-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kantabilon).contains("31042-03.html"));

        // The crystals come one at a time; the 50th sets cond 2.
        hunt(&mut t);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, kantabilon).contains("31042-04.html"));

        // The turn-in: 25 echo crystals, `exitQuest(true, true)`. The silver crystals are
        // registered quest items, so the exit takes them away.
        assert!(t.event(1, NAME, "31042-06.html").contains("31042-06.html"));
        assert_eq!(t.count(1, WEDDING_ECHO_CRYSTAL), 25);
        assert_eq!(t.count(1, SILVER_CRYSTAL), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_turn_in_with_too_few_crystals_gets_the_page_only() {
        // The page with the turn-in link only appears at cond 2, so this branch is called
        // directly (`Q00431_WeddingMarch.onEvent` with fewer than 50 crystals).
        let (mut t, kantabilon) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, kantabilon).contains("31042-01.htm"));
        assert!(t.event(1, NAME, "31042-02.htm").contains("31042-02.htm"));
        t.take_writes();
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00431WeddingMarch.on_event(ctx, "31042-06.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("31042-05.html"));
        assert_eq!(t.count(1, WEDDING_ECHO_CRYSTAL), 0);
        assert!(t.take_writes().is_empty());
    }
}
