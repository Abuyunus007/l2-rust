//! Quest 641: Attack Sailren! (Shilen's Stone Statue in the Gludin/Dragon Valley area; the
//! velociraptors and the pterosaur drop the Gazkh fragments).
// Java: dist/game/data/scripts/quests/Q00641_AttackSailren/Q00641_AttackSailren.java
//
// Pattern: a party kill count with a random member at cond 2 (`getRandomPartyMember(player, 1)`),
// and the start gate of another quest (Q00126 completed) read through `player_quest_state`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SHILENS_STONE_STATUE: i32 = 32109;

// Items
const GAZKH_FRAGMENT: i32 = 8782;
const GAZKH: i32 = 8784;

/// Java `Q00126_TheNameOfEvil2.class.getSimpleName()`.
const THE_NAME_OF_EVIL2: &str = "Q00126_TheNameOfEvil2";

// Monsters
const MOBS: [i32; 6] = [
    22196, // Velociraptor
    22197, // Velociraptor
    22198, // Velociraptor
    22218, // Velociraptor
    22223, // Velociraptor
    22199, // Pterosaur
];

/// Java `Q00641_AttackSailren`.
pub struct Q00641AttackSailren;

impl Script for Q00641AttackSailren {
    fn name(&self) -> &'static str {
        "Q00641_AttackSailren"
    }

    fn path(&self) -> &'static str {
        "quests/Q00641_AttackSailren"
    }

    fn id(&self) -> i32 {
        641
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SHILENS_STONE_STATUE]);
        s.add_talk_id(&[SHILENS_STONE_STATUE]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[GAZKH_FRAGMENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            // NOTE(java-quirk): without a state the no-quest page is the answer, not null.
            return Some(ctx.get_no_quest_msg(player));
        };
        match event {
            "32109-1.html" => qs.start_quest(ctx),
            "32109-2a.html" => {
                if ctx.get_quest_items_count(player, GAZKH_FRAGMENT) >= 30 {
                    ctx.give_items(player, GAZKH, 1);
                    qs.exit_quest(ctx, true, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if let Some(qs) = ctx.get_quest_state(party_member, false) {
            ctx.give_items(party_member, GAZKH_FRAGMENT, 1);
            if ctx.get_quest_items_count(party_member, GAZKH_FRAGMENT) < 30 {
                ctx.play_sound(party_member, QuestSound::ItemsoundQuestItemget);
            } else {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;
        match qs.state(ctx) {
            CREATED => {
                if ctx.player_level(player) < 77 {
                    htmltext = "32109-0.htm".to_owned();
                } else {
                    let completed = ctx
                        .player_quest_state(player, THE_NAME_OF_EVIL2)
                        .is_some_and(|other| other.is_completed());
                    htmltext = if completed {
                        "32109-0a.htm".to_owned()
                    } else {
                        "32109-0b.htm".to_owned()
                    };
                }
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "32109-1a.html".to_owned()
                } else {
                    "32109-2.html".to_owned()
                };
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::COMPLETED;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00641_AttackSailren";

    const NPCS: &str = r#"
        <npc id="32109" type="Folk" level="80" name="Shilen's Stone Statue"/>
        <npc id="22196" type="Monster" level="75" name="Velociraptor"/>
        <npc id="22199" type="Monster" level="75" name="Pterosaur"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8782" type="EtcItem" name="Gazkh Fragment"><set name="is_questitem" val="true"/></item>
        <item id="8784" type="EtcItem" name="Gazkh"/>"#;

    /// Java `Q00126_TheNameOfEvil2`, as far as this quest reads it: a script of that name.
    struct NameOfEvil2;

    impl Script for NameOfEvil2 {
        fn name(&self) -> &'static str {
            THE_NAME_OF_EVIL2
        }
        fn path(&self) -> &'static str {
            "quests/Q00126_TheNameOfEvil2"
        }
        fn id(&self) -> i32 {
            126
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NameOfEvil2);
        registry.add(Q00641AttackSailren);
        let pages = Pages::of(&Q00641AttackSailren)
            .page("32109-0a.htm", &["32109-0c.htm"])
            .page("32109-0c.htm", &["32109-1.html"])
            .plain(&["32109-0.htm", "32109-0b.htm", "32109-1.html", "32109-1a.html"])
            .page("32109-2.html", &["32109-2a.html"])
            .plain(&["32109-2a.html"])
            .npcs(&[SHILENS_STONE_STATUE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let statue = t.spawn(SHILENS_STONE_STATUE, 20);
        (t, statue)
    }

    /// The character enters; `Q00126_TheNameOfEvil2` is completed when `done`.
    fn enter(t: &mut ScriptTest, level: i32, done: bool) {
        t.enter(1, level, vec![]);
        if done {
            t.world.run_as_script(THE_NAME_OF_EVIL2, |ctx| {
                let qs = ctx.get_quest_state(Oid(1), true).unwrap();
                qs.set_state(ctx, COMPLETED);
            });
            t.take_writes();
        }
    }

    /// Kills one velociraptor next to the player and lets the kill delay pass.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(22196, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_77_is_sent_away() {
        let (mut t, statue) = setup();
        enter(&mut t, 76, true);
        assert!(t.quest_page(1, statue).contains("32109-0.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_gate_is_the_completed_name_of_evil_2() {
        let (mut t, statue) = setup();
        enter(&mut t, 77, false);
        assert!(t.quest_page(1, statue).contains("32109-0b.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_starts_and_thirty_fragments_end_it() {
        let (mut t, statue) = setup();
        enter(&mut t, 77, true);
        assert!(t.quest_page(1, statue).contains("32109-0a.htm"));
        assert!(t.event(1, NAME, "32109-0c.htm").contains("32109-0c.htm"));
        assert!(t.event(1, NAME, "32109-1.html").contains("32109-1.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, statue).contains("32109-1a.html"));

        // Kills before 30 fragments give one each and no step.
        for _ in 0..29 {
            kill_one(&mut t);
        }
        assert_eq!(t.count(1, GAZKH_FRAGMENT), 29);
        assert_eq!(t.take_writes(), []);

        // The 30th fragment moves the quest to cond 2.
        kill_one(&mut t);
        assert_eq!(t.count(1, GAZKH_FRAGMENT), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, statue).contains("32109-2.html"));
        assert!(t.event(1, NAME, "32109-2a.html").contains("32109-2a.html"));
        assert_eq!(t.count(1, GAZKH), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_kill_before_the_quest_gives_nothing() {
        let (mut t, _) = setup();
        enter(&mut t, 77, true);
        kill_one(&mut t);
        assert_eq!(t.count(1, GAZKH_FRAGMENT), 0);
        assert!(t.take_writes().is_empty());
    }
}
