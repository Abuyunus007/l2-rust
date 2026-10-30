//! Quest 368: Trespassing into the Holy Ground (Priestess Restina, the Blade Stakatos of the Garden of Eva).
// Java: dist/game/data/scripts/quests/Q00368_TrespassingIntoTheHolyGround/Q00368_TrespassingIntoTheHolyGround.java
//
// Pattern: a repeatable kill-count quest with a drop chance (`docs/design/scripts.md`, "Quest
// patterns"): the party check (`random_party_member_state`, state -1) differs per monster, the
// reward is the fangs turned into adena with a bonus at ten fangs, and the player chooses to go
// on or to quit.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RESTINA: i32 = 30926;

// Item
const BLADE_STAKATO_FANG: i32 = 5881;

// Misc
const MIN_LEVEL: i32 = 36;

// Mobs (drop chance)
const MOBS: [(i32, f64); 4] = [
    (20794, 0.60), // blade_stakato
    (20795, 0.57), // blade_stakato_worker
    (20796, 0.61), // blade_stakato_soldier
    (20797, 0.93), // blade_stakato_drone
];

/// Java `Q00368_TrespassingIntoTheHolyGround`.
pub struct Q00368TrespassingIntoTheHolyGround;

impl Script for Q00368TrespassingIntoTheHolyGround {
    fn name(&self) -> &'static str {
        "Q00368_TrespassingIntoTheHolyGround"
    }

    fn path(&self) -> &'static str {
        "quests/Q00368_TrespassingIntoTheHolyGround"
    }

    fn id(&self) -> i32 {
        368
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RESTINA]);
        s.add_talk_id(&[RESTINA]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[BLADE_STAKATO_FANG]);
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
            "30926-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30926-05.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30926-06.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // The worker and the drone need one started member in the party, the others three
        // (the killer counts that many times).
        let i = match npc.id {
            20795 | 20797 => 1,
            _ => 3,
        };
        let qs = ctx.random_party_member_state(killer, -1, i, Some(npc));
        if qs.is_some() {
            let chance = MOBS.iter().find(|(id, _)| *id == npc.id).map_or(0.0, |(_, c)| *c);
            // NOTE(java-quirk): the fang goes to the killer (`giveItemRandomly(player, ..)`), not
            // to the party member the roll above picked.
            ctx.give_item_randomly(killer, Some(npc), BLADE_STAKATO_FANG, 1, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30926-01.htm"
            } else {
                "30926-03.html"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if ctx.has_quest_items(player, BLADE_STAKATO_FANG) {
                let count = ctx.get_quest_items_count(player, BLADE_STAKATO_FANG);
                let bonus = if count >= 10 { 9450 } else { 2000 };
                ctx.give_adena(player, count * 250 + bonus, true);
                ctx.take_items(player, BLADE_STAKATO_FANG, -1);
                htmltext = "30926-04.html".to_owned();
            } else {
                htmltext = "30926-07.html".to_owned();
            }
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

    const NAME: &str = "Q00368_TrespassingIntoTheHolyGround";

    const NPCS: &str = r#"
        <npc id="30926" type="Folk" level="70" name="Priestess Restina"/>
        <npc id="20794" type="Monster" level="30" name="Blade Stakato"/>
        <npc id="20795" type="Monster" level="30" name="Blade Stakato Worker"/>
        <npc id="20796" type="Monster" level="30" name="Blade Stakato Soldier"/>
        <npc id="20797" type="Monster" level="30" name="Blade Stakato Drone"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5881" type="EtcItem" name="Blade Stakato Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00368TrespassingIntoTheHolyGround);
        let pages = Pages::of(&Q00368TrespassingIntoTheHolyGround)
            .page("30926-01.htm", &["30926-02.htm"])
            .plain(&["30926-02.htm", "30926-03.html"])
            .page("30926-04.html", &["30926-05.html", "30926-06.html"])
            .plain(&["30926-05.html", "30926-06.html"])
            .page("30926-07.html", &["30926-05.html", "30926-06.html"])
            .npcs(&[RESTINA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let restina = t.spawn(RESTINA, 20);
        (t, restina)
    }

    /// Kills the four stakatos in turns until the player has `fangs` fangs.
    fn hunt(t: &mut ScriptTest, fangs: i64) {
        for kills in 0..400 {
            if t.count(1, BLADE_STAKATO_FANG) >= fangs {
                return;
            }
            let mob = t.spawn(MOBS[kills % 4].0, 60);
            t.kill(1, mob);
        }
        panic!("no {fangs} fangs after 400 kills");
    }

    #[test]
    fn a_level_35_character_is_sent_away() {
        let (mut t, restina) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, restina).contains("30926-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_fangs_pay_the_big_bonus_and_the_quest_goes_on() {
        let (mut t, restina) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, restina).contains("30926-01.htm"));
        assert!(t.event(1, NAME, "30926-02.htm").contains("30926-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, restina).contains("30926-07.html"));

        hunt(&mut t, 10);
        let fangs = t.count(1, BLADE_STAKATO_FANG);
        let adena = t.count(1, 57);
        assert!(t.quest_page(1, restina).contains("30926-04.html"));
        assert_eq!(t.count(1, 57), adena + fangs * 250 + 9450);
        assert_eq!(t.count(1, BLADE_STAKATO_FANG), 0);
        assert!(t.take_writes().is_empty());

        // "Death to the blade stakatos!": the page only, the quest goes on.
        assert!(t.quest_page(1, restina).contains("30926-07.html"));
        assert!(t.event(1, NAME, "30926-06.html").contains("30926-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fewer_fangs_pay_the_small_bonus_and_quitting_ends_the_quest() {
        let (mut t, restina) = setup();
        t.enter(1, 36, vec![]);
        t.quest_page(1, restina);
        t.event(1, NAME, "30926-02.htm");
        t.take_writes();

        hunt(&mut t, 1);
        let fangs = t.count(1, BLADE_STAKATO_FANG);
        assert!(fangs < 10);
        let adena = t.count(1, 57);
        assert!(t.quest_page(1, restina).contains("30926-04.html"));
        assert_eq!(t.count(1, 57), adena + fangs * 250 + 2000);

        // "No, I quit."
        assert!(t.quest_page(1, restina).contains("30926-07.html"));
        assert!(t.event(1, NAME, "30926-05.html").contains("30926-05.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, restina).contains("30926-01.htm"));
    }

    #[test]
    fn the_killer_gets_the_fang_even_when_a_member_is_the_one_on_the_quest() {
        let (mut t, restina) = setup();
        t.enter(1, 36, vec![]);
        t.enter(2, 36, vec![]);
        assert!(t.quest_page(2, restina).contains("30926-01.htm"));
        t.event(2, NAME, "30926-02.htm");
        t.party(1, 2);
        let mut kills = 0;
        while t.count(1, BLADE_STAKATO_FANG) == 0 && kills < 200 {
            let drone = t.spawn(20797, 60);
            t.kill(1, drone);
            kills += 1;
        }
        // The killer (player 1) has the fang, the started member does not.
        assert!(t.count(1, BLADE_STAKATO_FANG) > 0);
        assert_eq!(t.count(2, BLADE_STAKATO_FANG), 0);
    }
}
