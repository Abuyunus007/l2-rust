//! Quest 637: Through the Gate Once More (Flauron at the Tower of Insolence; the necromancers
//! of the Tower drop the necro hearts, the faded visitor's mark from Q00636 is the entry).
// Java: dist/game/data/scripts/quests/Q00637_ThroughOnceMore/Q00637_ThroughOnceMore.java
//
// Pattern: a kill count with a fixed drop rate (`DROP_CHANCE` 90: one heart with 90 %, computed
// as Java does with `chance / 100` and `chance % 100`) up to 10 hearts, a cond 2 at the limit,
// and a reward that trades the faded mark and the hearts for the mark.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const FLAURON: i32 = 32010;

// Monsters
const MOBS: [i32; 3] = [
    21565, // Necromancer
    21566, // Necromancer
    21567, // Necromancer
];

// Items
const VISITOR_MARK: i32 = 8064;
const FADED_MARK: i32 = 8065;
const NECRO_HEART: i32 = 8066;
const MARK: i32 = 8067;
const REWARD: i32 = 8273;

// Misc
/// Java `DROP_CHANCE` (percent, may be above 100).
const DROP_CHANCE: i32 = 90;

/// Java `Q00637_ThroughOnceMore`.
pub struct Q00637ThroughOnceMore;

impl Script for Q00637ThroughOnceMore {
    fn name(&self) -> &'static str {
        "Q00637_ThroughOnceMore"
    }

    fn path(&self) -> &'static str {
        "quests/Q00637_ThroughOnceMore"
    }

    fn id(&self) -> i32 {
        637
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FLAURON]);
        s.add_talk_id(&[FLAURON]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[NECRO_HEART]);
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
        match event {
            "32010-03.htm" => qs.start_quest(ctx),
            "32010-10.htm" => qs.exit_quest(ctx, true, false),
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        let count = ctx.get_quest_items_count(killer, NECRO_HEART);
        if count >= 10 {
            return;
        }
        let mut chance = DROP_CHANCE;
        let mut num_items = chance / 100;
        chance %= 100;
        if ctx.get_random(100) < chance {
            num_items += 1;
        }
        if num_items > 0 {
            if count + i64::from(num_items) >= 10 {
                num_items = 10 - count as i32;
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
            ctx.give_items(killer, NECRO_HEART, i64::from(num_items));
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let state = qs.state(ctx);
        if state == CREATED {
            if ctx.player_level(player) > 72 {
                if ctx.has_quest_items(player, FADED_MARK) {
                    return Some("32010-02.htm".to_owned());
                }
                if ctx.has_quest_items(player, VISITOR_MARK) {
                    qs.exit_quest(ctx, true, false);
                    return Some("32010-01a.htm".to_owned());
                }
                if ctx.has_quest_items(player, MARK) {
                    qs.exit_quest(ctx, true, false);
                    return Some("32010-0.htm".to_owned());
                }
            }
            qs.exit_quest(ctx, true, false);
            return Some("32010-01.htm".to_owned());
        } else if state == STARTED {
            if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, NECRO_HEART) == 10 {
                ctx.take_items(player, NECRO_HEART, 10);
                ctx.take_items(player, FADED_MARK, 1);
                ctx.give_items(player, MARK, 1);
                ctx.give_items(player, REWARD, 10);
                qs.exit_quest(ctx, true, true);
                return Some("32010-05.htm".to_owned());
            }
            return Some("32010-04.htm".to_owned());
        }
        Some(ctx.get_no_quest_msg(player))
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00637_ThroughOnceMore";

    const NPCS: &str = r#"
        <npc id="32010" type="Folk" level="70" name="Flauron"/>
        <npc id="21565" type="Monster" level="70" name="Necromancer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8064" type="EtcItem" name="Visitor's Mark"><set name="is_questitem" val="true"/></item>
        <item id="8065" type="EtcItem" name="Faded Visitor's Mark"><set name="is_questitem" val="true"/></item>
        <item id="8066" type="EtcItem" name="Necro Heart"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8067" type="EtcItem" name="Mark of Truth"><set name="is_questitem" val="true"/></item>
        <item id="8273" type="EtcItem" name="Reward"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00637ThroughOnceMore);
        let pages = Pages::of(&Q00637ThroughOnceMore)
            .page("32010-02.htm", &["32010-06.htm", "32010-08.htm"])
            .page("32010-06.htm", &["32010-09.htm"])
            .page("32010-08.htm", &["32010-09.htm"])
            .page("32010-09.htm", &["32010-07.htm", "32010-10.htm"])
            .page("32010-07.htm", &["32010-03.htm"])
            .plain(&[
                "32010-0.htm",
                "32010-01.htm",
                "32010-01a.htm",
                "32010-03.htm",
                "32010-04.htm",
                "32010-05.htm",
                "32010-10.htm",
            ])
            .npcs(&[FLAURON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let flauron = t.spawn(FLAURON, 20);
        (t, flauron)
    }

    /// A necromancer dies next to the player and the kill delay passes.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(21565, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_of_level_72_is_sent_away_even_with_the_faded_mark() {
        let (mut t, flauron) = setup();
        t.enter(1, 72, vec![item(100, 8065, 1)]);
        assert!(t.quest_page(1, flauron).contains("32010-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_faded_mark_opens_the_gate_and_the_hearts_end_the_visit() {
        let (mut t, flauron) = setup();
        t.enter(1, 73, vec![item(100, 8065, 1)]);
        assert!(t.quest_page(1, flauron).contains("32010-02.htm"));
        assert!(t.event(1, NAME, "32010-06.htm").contains("32010-06.htm"));
        assert!(t.event(1, NAME, "32010-09.htm").contains("32010-09.htm"));
        assert!(t.event(1, NAME, "32010-07.htm").contains("32010-07.htm"));
        assert!(t.event(1, NAME, "32010-03.htm").contains("32010-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Talking at cond 1 asks for the hearts.
        assert!(t.quest_page(1, flauron).contains("32010-04.htm"));

        // Each kill gives a heart with 90 %; the tenth sets cond 2.
        let mut kills = 0;
        while t.count(1, NECRO_HEART) < 10 {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 100, "no hearts dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The hearts and the faded mark are traded for the mark and ten rewards.
        assert!(t.quest_page(1, flauron).contains("32010-05.htm"));
        assert_eq!(t.count(1, NECRO_HEART), 0);
        assert_eq!(t.count(1, FADED_MARK), 0);
        assert_eq!(t.count(1, MARK), 1);
        assert_eq!(t.count(1, REWARD), 10);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_character_with_the_mark_from_before_is_sent_away() {
        let (mut t, flauron) = setup();
        t.enter(1, 80, vec![item(100, 8067, 1)]);
        assert!(t.quest_page(1, flauron).contains("32010-0.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_character_with_the_visitor_mark_is_sent_away() {
        let (mut t, flauron) = setup();
        t.enter(1, 80, vec![item(100, 8064, 1)]);
        assert!(t.quest_page(1, flauron).contains("32010-01a.htm"));
        assert!(t.take_writes().is_empty());
    }
}
