//! Quest 277: Gatekeeper's Offering (Tamil in the Gludio area sends the player to the Greystone Golems).
// Java: dist/game/data/scripts/quests/Q00277_GatekeepersOffering/Q00277_GatekeepersOffering.java
//
// Pattern: a kill-count quest (`docs/design/scripts.md`, "Quest patterns"): every Greystone Golem
// gives one starstone until 20 are in, the 20th sets cond 2; the NPC pays and the quest repeats.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TAMIL: i32 = 30576;

// Monster
const GREYSTONE_GOLEM: i32 = 20333;

// Items
const STARSTONE: i32 = 1572;
const GATEKEEPER_CHARM: i32 = 1658;

// Misc
const MIN_LEVEL: i32 = 15;
const STARSTONE_COUT: i64 = 20;

/// Java `Q00277_GatekeepersOffering`.
pub struct Q00277GatekeepersOffering;

impl Script for Q00277GatekeepersOffering {
    fn name(&self) -> &'static str {
        "Q00277_GatekeepersOffering"
    }

    fn path(&self) -> &'static str {
        "quests/Q00277_GatekeepersOffering"
    }

    fn id(&self) -> i32 {
        277
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TAMIL]);
        s.add_talk_id(&[TAMIL]);
        s.add_kill_id(&[GREYSTONE_GOLEM]);
        s.register_quest_items(&[STARSTONE]);
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
        if event.eq_ignore_ascii_case("30576-03.htm") {
            if ctx.player_level(player) < MIN_LEVEL {
                return Some("30576-01.htm".to_owned());
            }
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && ctx.get_quest_items_count(killer, STARSTONE) < STARSTONE_COUT
        {
            ctx.give_items(killer, STARSTONE, 1);
            if ctx.get_quest_items_count(killer, STARSTONE) >= STARSTONE_COUT {
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
            CREATED => htmltext = "30576-02.htm".to_owned(),
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30576-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, STARSTONE) >= STARSTONE_COUT
                {
                    ctx.give_items(player, GATEKEEPER_CHARM, 2);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30576-05.html".to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00277_GatekeepersOffering";

    const NPCS: &str = r#"
        <npc id="30576" type="Folk" level="70" name="Tamil"/>
        <npc id="20333" type="Monster" level="30" name="Greystone Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1572" type="EtcItem" name="Starstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1658" type="EtcItem" name="Gatekeeper's Charm"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00277GatekeepersOffering);
        let pages = Pages::of(&Q00277GatekeepersOffering)
            .page("30576-02.htm", &["30576-03.htm"])
            .plain(&["30576-01.htm", "30576-03.htm", "30576-04.html", "30576-05.html"])
            .npcs(&[TAMIL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tamil = t.spawn(TAMIL, 20);
        (t, tamil)
    }

    /// Kills Greystone Golems until the quest is at cond 2; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let golem = t.spawn(GREYSTONE_GOLEM, 60);
            t.kill(1, golem);
            if t.count(1, STARSTONE) >= STARSTONE_COUT {
                return kills;
            }
        }
        panic!("no 20 starstones after 200 kills");
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, tamil) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, tamil).contains("30576-02.htm"));
        assert!(t.event(1, NAME, "30576-03.htm").contains("30576-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn twenty_starstones_then_two_charms() {
        let (mut t, tamil) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, tamil).contains("30576-02.htm"));
        assert!(t.event(1, NAME, "30576-03.htm").contains("30576-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, tamil).contains("30576-04.html"));

        // One kill gives one stone; the 20th sets cond 2 (`setCond(2, true)`).
        let kills = hunt(&mut t);
        assert_eq!(kills, 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Tamil gives the charms and ends the quest, repeatable (`exitQuest(true, true)`).
        assert!(t.quest_page(1, tamil).contains("30576-05.html"));
        assert_eq!(t.count(1, GATEKEEPER_CHARM), 2);
        assert_eq!(t.count(1, STARSTONE), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
