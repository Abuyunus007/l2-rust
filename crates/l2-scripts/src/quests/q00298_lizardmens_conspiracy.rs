//! Quest 298: Lizardmen's Conspiracy (Guard Praga and Magister Rohmer in the Gludio area; maille lizardmen and araneids).
// Java: dist/game/data/scripts/quests/Q00298_LizardmensConspiracy/Q00298_LizardmensConspiracy.java
//
// Pattern: two NPCs and a party collect quest (`docs/design/scripts.md`, "Quest patterns"): Praga
// hands out the patrol report, Rohmer takes it (cond 2) and wants 50 shining gems and 50 shining
// red gems from the lizardmen and araneids; a random party member in cond 2 gets the gems, the
// moment both are complete sets cond 3 and Rohmer pays 42000 exp and ends the quest.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GUARD_PRAGA: i32 = 30333;
const MAGISTER_ROHMER: i32 = 30344;

// Items
const PATROLS_REPORT: i32 = 7182;
const SHINING_GEM: i32 = 7183;
const SHINING_RED_GEM: i32 = 7184;

// Misc
const MIN_LEVEL: i32 = 25;

/// Java `MONSTERS`: the item, the drop chance and the amount of each monster.
fn gem_drop(npc_id: i32) -> Option<(i32, f64)> {
    match npc_id {
        20922 => Some((SHINING_GEM, 0.49)),     // Maille Lizardman Warrior
        20923 => Some((SHINING_GEM, 0.70)),     // Maille Lizardman Shaman
        20924 => Some((SHINING_GEM, 0.75)),     // Maille Lizardman Matriarch
        20926 => Some((SHINING_RED_GEM, 0.54)), // Giant Araneid
        20927 => Some((SHINING_RED_GEM, 0.64)), // King Araneid
        _ => None,
    }
}

/// Java `Q00298_LizardmensConspiracy`.
pub struct Q00298LizardmensConspiracy;

impl Script for Q00298LizardmensConspiracy {
    fn name(&self) -> &'static str {
        "Q00298_LizardmensConspiracy"
    }

    fn path(&self) -> &'static str {
        "quests/Q00298_LizardmensConspiracy"
    }

    fn id(&self) -> i32 {
        298
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_PRAGA]);
        s.add_talk_id(&[GUARD_PRAGA, MAGISTER_ROHMER]);
        s.add_kill_id(&[20922, 20923, 20924, 20926, 20927]);
        s.register_quest_items(&[PATROLS_REPORT, SHINING_GEM, SHINING_RED_GEM]);
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
            "30333-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, PATROLS_REPORT, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30344-04.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, PATROLS_REPORT) {
                    ctx.take_items(player, PATROLS_REPORT, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30344-06.html" => {
                if qs.is_cond(ctx, 3) {
                    ctx.add_exp_and_sp(player, 0, 42000);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30344-07.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.random_party_member_state(killer, 2, 3, Some(npc)) else {
            return;
        };
        let Some((gem, chance)) = gem_drop(npc.id) else {
            return;
        };
        let member_player = member.player();
        if ctx.give_item_randomly(member_player, Some(npc), gem, 1, 1, 50, chance, true)
            && ctx.get_quest_items_count(member_player, SHINING_GEM) >= 50
            && ctx.get_quest_items_count(member_player, SHINING_RED_GEM) >= 50
        {
            member.set_cond(ctx, 3, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        if qs.is_created(ctx) && npc.id == GUARD_PRAGA {
            htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                "30333-01.htm"
            } else {
                "30333-02.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if npc.id == GUARD_PRAGA && ctx.has_quest_items(talker, PATROLS_REPORT) {
                htmltext = "30333-04.html".to_owned();
            } else if npc.id == MAGISTER_ROHMER {
                match qs.get_cond(ctx) {
                    1 => htmltext = "30344-01.html".to_owned(),
                    2 => htmltext = "30344-02.html".to_owned(),
                    3 => htmltext = "30344-03.html".to_owned(),
                    _ => {}
                }
            }
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

    const NAME: &str = "Q00298_LizardmensConspiracy";

    const NPCS: &str = r#"
        <npc id="30333" type="Folk" level="70" name="Guard Praga"/>
        <npc id="30344" type="Folk" level="70" name="Magister Rohmer"/>
        <npc id="20922" type="Monster" level="36" name="Maille Lizardman Warrior"/>
        <npc id="20926" type="Monster" level="36" name="Giant Araneid"/>"#;

    const ITEMS: &str = r#"
        <item id="7182" type="EtcItem" name="Patrol's Report"><set name="is_questitem" val="true"/></item>
        <item id="7183" type="EtcItem" name="Shining Gem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7184" type="EtcItem" name="Shining Red Gem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00298LizardmensConspiracy);
        let pages = Pages::of(&Q00298LizardmensConspiracy)
            .page("30333-01.htm", &["30333-03.htm"])
            .plain(&["30333-02.htm", "30333-03.htm", "30333-04.html"])
            .page("30344-01.html", &["30344-04.html"])
            .plain(&["30344-02.html", "30344-04.html"])
            .page("30344-03.html", &["30344-06.html"])
            .plain(&["30344-06.html", "30344-07.html"])
            .npcs(&[GUARD_PRAGA, MAGISTER_ROHMER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let praga = t.spawn(GUARD_PRAGA, 20);
        let rohmer = t.spawn(MAGISTER_ROHMER, 40);
        (t, praga, rohmer)
    }

    #[test]
    fn a_level_twenty_four_character_is_sent_away() {
        let (mut t, praga, _rohmer) = setup();
        t.enter_as(1, 24, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, praga).contains("30333-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn report_then_fifty_and_fifty_gems_then_the_experience() {
        let (mut t, praga, rohmer) = setup();
        t.enter_as(1, 25, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, praga).contains("30333-01.htm"));
        assert!(t.event(1, NAME, "30333-03.htm").contains("30333-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, 7182), 1);

        // Praga has nothing more to say while the report is in the bag; Rohmer wants it.
        assert!(t.quest_page(1, praga).contains("30333-04.html"));
        assert!(t.quest_page(1, rohmer).contains("30344-01.html"));
        assert!(t.event(1, NAME, "30344-04.html").contains("30344-04.html"));
        assert_eq!(t.count(1, 7182), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, rohmer).contains("30344-02.html"));

        // Gems from the lizardmen and the araneids; the moment both are complete cond 3 comes.
        let mut kills = 0;
        while t.take_writes().is_empty() {
            kills += 1;
            assert!(kills < 2000, "no cond 3 after {kills} kills");
            let monster = t.spawn(if kills % 2 == 0 { 20922 } else { 20926 }, 60);
            t.kill(1, monster);
        }
        assert!(t.count(1, SHINING_GEM) >= 50 && t.count(1, SHINING_RED_GEM) >= 50);

        // Rohmer's last page: 42000 exp, `exitQuest(true, true)`.
        assert!(t.quest_page(1, rohmer).contains("30344-03.html"));
        assert!(t.event(1, NAME, "30344-06.html").contains("30344-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
