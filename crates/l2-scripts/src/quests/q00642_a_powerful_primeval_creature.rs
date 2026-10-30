//! Quest 642: A Powerful Primeval Creature (Dinn in the Ruins of Dion area; the dinosaurs of the
//! Dinosaur Tracks drop tissues and the ancient eggs drop the dinosaur eggs; 5000 adena a tissue).
// Java: dist/game/data/scripts/quests/Q00642_APowerfulPrimevalCreature/Q00642_APowerfulPrimevalCreature.java
//
// Pattern: a party kill with a drop chance per monster (a random started member, counted three
// times), the tissues paid out per piece (`giveAdena` with the rates), and the exit that pays
// what is left.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DINN: i32 = 32105;

// Items
const DINOSAUR_TISSUE: i32 = 8774;
const DINOSAUR_EGG: i32 = 8775;

// Misc
const MIN_LEVEL: i32 = 75;
const ADENA_PER_TISSUE: i64 = 5000;

// Mobs
const ANCIENT_EGG: i32 = 18344;

/// Java `MOBS_TISSUE`: the tissue drop chance of each dinosaur.
const MOBS_TISSUE: [(i32, f64); 9] = [
    (22196, 0.309), // Velociraptor
    (22197, 0.309), // Velociraptor
    (22198, 0.309), // Velociraptor
    (22199, 0.309), // Pterosaur
    (22215, 0.988), // Tyrannosaurus
    (22216, 0.988), // Tyrannosaurus
    (22217, 0.988), // Tyrannosaurus
    (22218, 0.309), // Velociraptor
    (22223, 0.309), // Velociraptor
];

/// Java `Q00642_APowerfulPrimevalCreature`.
pub struct Q00642APowerfulPrimevalCreature;

impl Script for Q00642APowerfulPrimevalCreature {
    fn name(&self) -> &'static str {
        "Q00642_APowerfulPrimevalCreature"
    }

    fn path(&self) -> &'static str {
        "quests/Q00642_APowerfulPrimevalCreature"
    }

    fn id(&self) -> i32 {
        642
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DINN]);
        s.add_talk_id(&[DINN]);
        s.add_kill_id(&[ANCIENT_EGG]);
        let mobs: Vec<i32> = MOBS_TISSUE.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[DINOSAUR_TISSUE, DINOSAUR_EGG]);
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
            "32105-05.html" => qs.start_quest(ctx),
            "32105-06.htm" => qs.exit_quest(ctx, true, false),
            "32105-09.html" => {
                if ctx.has_quest_items(player, DINOSAUR_TISSUE) {
                    let tissues = ctx.get_quest_items_count(player, DINOSAUR_TISSUE);
                    ctx.give_adena(player, ADENA_PER_TISSUE * tissues, true);
                    ctx.take_items(player, DINOSAUR_TISSUE, -1);
                } else {
                    htmltext = Some("32105-14.html".to_owned());
                }
            }
            "exit" => {
                if ctx.has_quest_items(player, DINOSAUR_TISSUE) {
                    let tissues = ctx.get_quest_items_count(player, DINOSAUR_TISSUE);
                    ctx.give_adena(player, ADENA_PER_TISSUE * tissues, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("32105-12.html".to_owned());
                } else {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("32105-13.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 3, npc)`: the killer counts three times.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let member = qs.player();
        match MOBS_TISSUE.iter().find(|(id, _)| *id == npc.id) {
            Some(&(_, chance)) => {
                ctx.give_item_randomly(member, Some(npc), DINOSAUR_TISSUE, 1, 1, 0, chance, true);
            }
            None => {
                ctx.give_item_randomly(member, Some(npc), DINOSAUR_EGG, 1, 1, 0, 1.0, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) < MIN_LEVEL {
                "32105-01.htm".to_owned()
            } else {
                "32105-02.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext = if ctx.has_at_least_one_quest_item(player, &[DINOSAUR_TISSUE, DINOSAUR_EGG]) {
                "32105-08.html".to_owned()
            } else {
                "32105-07.html".to_owned()
            };
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

    const NAME: &str = "Q00642_APowerfulPrimevalCreature";

    const NPCS: &str = r#"
        <npc id="32105" type="Folk" level="75" name="Dinn"/>
        <npc id="22215" type="Monster" level="75" name="Tyrannosaurus"/>
        <npc id="18344" type="Monster" level="75" name="Ancient Egg"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8774" type="EtcItem" name="Dinosaur Tissue"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8775" type="EtcItem" name="Dinosaur Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00642APowerfulPrimevalCreature);
        let pages = Pages::of(&Q00642APowerfulPrimevalCreature)
            .page("32105-02.htm", &["32105-03.htm"])
            .page("32105-03.htm", &["32105-04.htm"])
            .page("32105-04.htm", &["32105-05.html", "32105-06.htm"])
            .page("32105-08.html", &["32105-09.html", "32105-10.html"])
            .page("32105-09.html", &["exit"])
            .plain(&[
                "32105-01.htm",
                "32105-05.html",
                "32105-06.htm",
                "32105-07.html",
                "32105-10.html",
                "32105-12.html",
                "32105-13.html",
                "32105-14.html",
            ])
            .npcs(&[DINN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dinn = t.spawn(DINN, 20);
        (t, dinn)
    }

    /// A tyrannosaurus (tissue chance 0.988) dies next to the player.
    fn kill_tyrannosaurus(t: &mut ScriptTest) {
        let mob = t.spawn(22215, 40);
        t.kill(1, mob);
    }

    /// Starts the quest from Dinn's page chain and leaves the character on the start page.
    fn start(t: &mut ScriptTest, dinn: Oid) {
        assert!(t.quest_page(1, dinn).contains("32105-02.htm"));
        assert!(t.event(1, NAME, "32105-03.htm").contains("32105-03.htm"));
        assert!(t.event(1, NAME, "32105-04.htm").contains("32105-04.htm"));
        assert!(t.event(1, NAME, "32105-05.html").contains("32105-05.html"));
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, dinn) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, dinn).contains("32105-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_tissues_are_paid_at_5000_each_and_the_quest_ends_at_exit() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        start(&mut t, dinn);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Two tissues, the pay-out and the tissues gone.
        for _ in 0..2 {
            kill_tyrannosaurus(&mut t);
        }
        let tissues = t.count(1, DINOSAUR_TISSUE);
        assert!(tissues >= 1, "no tissue dropped");
        assert!(t.quest_page(1, dinn).contains("32105-08.html"));
        assert!(t.event(1, NAME, "32105-09.html").contains("32105-09.html"));
        assert_eq!(t.count(1, DINOSAUR_TISSUE), 0);
        assert_eq!(t.count(1, 57), ADENA_PER_TISSUE * tissues);
        assert!(t.take_writes().is_empty());

        // One more tissue after the page: `exit` pays it and ends the quest.
        kill_tyrannosaurus(&mut t);
        assert_eq!(t.count(1, DINOSAUR_TISSUE), 1);
        assert!(t.event(1, NAME, "exit").contains("32105-12.html"));
        assert_eq!(t.count(1, 57), ADENA_PER_TISSUE * (tissues + 1));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn exit_without_tissues_ends_the_quest_with_the_other_page() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        start(&mut t, dinn);
        t.take_writes();
        kill_tyrannosaurus(&mut t);
        assert!(t.quest_page(1, dinn).contains("32105-08.html"));
        assert!(t.event(1, NAME, "32105-09.html").contains("32105-09.html"));
        assert_eq!(t.count(1, DINOSAUR_TISSUE), 0);
        t.take_writes();
        assert!(t.event(1, NAME, "exit").contains("32105-13.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
