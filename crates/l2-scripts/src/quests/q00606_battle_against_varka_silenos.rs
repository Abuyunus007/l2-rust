//! Quest 606: Battle against Varka Silenos (Kadun in the Ketra Orc Village, Varka monsters).
// Java: dist/game/data/scripts/quests/Q00606_BattleAgainstVarkaSilenos/Q00606_BattleAgainstVarkaSilenos.java
//
// Pattern: a repeatable collect quest. Each monster drops a mane with a chance per monster (a
// random member of the party at cond 1 gets it); the NPC trades 100 manes for horns.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KADUN: i32 = 31370;

// Monsters: (id, drop chance out of 1000), Java's `MOBS` map in its order.
const MOBS: [(i32, i32); 15] = [
    (21350, 500), // Varka Silenos Recruit
    (21353, 510), // Varka Silenos Scout
    (21354, 522), // Varka Silenos Hunter
    (21355, 519), // Varka Silenos Shaman
    (21357, 529), // Varka Silenos Priest
    (21358, 529), // Varka Silenos Warrior
    (21360, 539), // Varka Silenos Medium
    (21362, 539), // Varka Silenos Officer
    (21364, 558), // Varka Silenos Seer
    (21365, 568), // Varka Silenos Great Magus
    (21366, 568), // Varka Silenos General
    (21368, 568), // Varka Silenos Great Seer
    (21369, 664), // Varka's Commander
    (21371, 713), // Varka's Head Magus
    (21373, 738), // Varka's Prophet
];

// Items
const HORN: i32 = 7186;
const MANE: i32 = 7233;

// Misc
const MIN_LEVEL: i32 = 74;
const MANE_COUNT: i64 = 100;

/// Java `Q00606_BattleAgainstVarkaSilenos`.
pub struct Q00606BattleAgainstVarkaSilenos;

impl Script for Q00606BattleAgainstVarkaSilenos {
    fn name(&self) -> &'static str {
        "Q00606_BattleAgainstVarkaSilenos"
    }

    fn path(&self) -> &'static str {
        "quests/Q00606_BattleAgainstVarkaSilenos"
    }

    fn id(&self) -> i32 {
        606
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KADUN]);
        s.add_talk_id(&[KADUN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[MANE]);
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
            "31370-03.htm" => {
                qs.start_quest(ctx);
            }
            "31370-06.html" => {}
            "31370-07.html" => {
                if ctx.get_quest_items_count(player, MANE) < MANE_COUNT {
                    return Some("31370-08.html".to_owned());
                }
                ctx.take_items(player, MANE, MANE_COUNT);
                ctx.give_items(player, HORN, 20);
            }
            "31370-09.html" => {
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): the mane goes to a random member at cond 1; `getRandom` is only rolled
        // when a member was found (`member != null && getRandom(1000) < chance`).
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let chance = MOBS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if ctx.get_random(1000) < chance {
            ctx.give_items(member, MANE, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31370-01.htm"
                } else {
                    "31370-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(player, MANE) {
                    "31370-04.html"
                } else {
                    "31370-05.html"
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00606_BattleAgainstVarkaSilenos";

    const NPCS: &str = r#"
        <npc id="31370" type="Folk" level="70" name="Kadun"/>
        <npc id="21373" type="Monster" level="80" name="Varka's Prophet"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7233" type="EtcItem" name="Varka's Mane"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7186" type="EtcItem" name="Horn of Varka" ><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00606BattleAgainstVarkaSilenos);
        let pages = Pages::of(&Q00606BattleAgainstVarkaSilenos)
            .page("31370-01.htm", &["31370-03.htm"])
            .plain(&[
                "31370-02.htm",
                "31370-03.htm",
                "31370-05.html",
                "31370-08.html",
                "31370-09.html",
            ])
            .page(
                "31370-04.html",
                &["31370-06.html", "31370-07.html", "31370-09.html"],
            )
            .page("31370-06.html", &["31370-07.html", "31370-09.html"])
            .page(
                "31370-07.html",
                &["31370-06.html", "31370-07.html", "31370-09.html"],
            )
            .npcs(&[KADUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kadun = t.spawn(KADUN, 20);
        (t, kadun)
    }

    /// Kills Varka's Prophets (73.8% each) until the player holds `count` manes.
    fn gather_manes(t: &mut ScriptTest, count: i64) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(21373, 60);
            t.kill(1, mob);
            if t.count(1, MANE) >= count {
                return kills;
            }
        }
        panic!("no {count} manes after 400 kills");
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, kadun) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, kadun).contains("31370-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_manes_for_twenty_horns_then_the_end() {
        let (mut t, kadun) = setup();
        t.enter(1, 74, vec![item(100, MANE, 50)]);
        assert!(t.quest_page(1, kadun).contains("31370-01.htm"));
        assert!(t.event(1, NAME, "31370-03.htm").contains("31370-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // With 50 manes the NPC asks for 100: `31370-08.html`, nothing is taken.
        assert!(t.quest_page(1, kadun).contains("31370-04.html"));
        assert!(t.event(1, NAME, "31370-07.html").contains("31370-08.html"));
        assert_eq!(t.count(1, MANE), 50);

        // The mane drops come with the item sound; the monsters give one at most per kill.
        let kills = gather_manes(&mut t, MANE_COUNT);
        assert!(kills >= 25, "{kills}");
        assert_eq!(t.count(1, MANE), MANE_COUNT);
        assert!(t.take_writes().is_empty());

        assert!(t.quest_page(1, kadun).contains("31370-04.html"));
        assert!(t.event(1, NAME, "31370-07.html").contains("31370-07.html"));
        assert_eq!((t.count(1, MANE), t.count(1, HORN)), (0, 20));

        assert!(t.event(1, NAME, "31370-09.html").contains("31370-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_monster_without_a_party_member_at_cond_one_drops_nothing() {
        let (mut t, _kadun) = setup();
        t.enter(1, 74, vec![]);
        // Not started: nobody at cond 1 in the party, nothing rolls.
        let mob = t.spawn(21373, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, MANE), 0);
    }
}
