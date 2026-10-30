//! Quest 601: Watching Eyes (Eye of Argos in the Ketra and Varka villages, Proof of Avenger).
// Java: dist/game/data/scripts/quests/Q00601_WatchingEyes/Q00601_WatchingEyes.java
//
// Pattern: a repeatable collect quest. Each monster drops a Proof of Avenger with a chance; the
// hundredth sets cond 2. The NPC rewards one of three item bundles or the adena alone, drawn
// with the world RNG (`getRandom(4)`), and ends the quest repeatable.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const EYE_OF_ARGOS: i32 = 31683;

// Item
const PROOF_OF_AVENGER: i32 = 7188;

// Monsters: (id, drop chance out of 1000), Java's `MOBS` map.
const MOBS: [(i32, i32); 5] = [
    (21308, 790),
    (21309, 820),
    (21306, 850),
    (21310, 680),
    (21311, 630),
];

// Reward: (item, adena) per draw; the fourth draw gives adena only.
const REWARD: [(i32, i64); 4] = [(6699, 90000), (6698, 80000), (6700, 40000), (0, 230000)];

// Misc
const MIN_LEVEL: i32 = 71;
const PROOF_COUNT: i64 = 100;

/// Java `Q00601_WatchingEyes`.
pub struct Q00601WatchingEyes;

impl Script for Q00601WatchingEyes {
    fn name(&self) -> &'static str {
        "Q00601_WatchingEyes"
    }

    fn path(&self) -> &'static str {
        "quests/Q00601_WatchingEyes"
    }

    fn id(&self) -> i32 {
        601
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EYE_OF_ARGOS]);
        s.add_talk_id(&[EYE_OF_ARGOS]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[PROOF_OF_AVENGER]);
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
            "31683-02.htm" => {
                qs.start_quest(ctx);
            }
            "31683-05.html" => {
                if ctx.get_quest_items_count(player, PROOF_OF_AVENGER) < PROOF_COUNT {
                    return Some("31683-06.html".to_owned());
                }
                let i = ctx.get_random(4);
                if i < 3 {
                    ctx.give_items(player, REWARD[i as usize].0, 5);
                    ctx.add_exp_and_sp(player, 120000, 10000);
                }
                ctx.give_adena(player, REWARD[i as usize].1, true);
                qs.exit_quest(ctx, true, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        let chance = MOBS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, chance)| *chance);
        if qs.is_cond(ctx, 1) && ctx.get_random(1000) < chance {
            ctx.give_items(killer, PROOF_OF_AVENGER, 1);
            if ctx.get_quest_items_count(killer, PROOF_OF_AVENGER) == PROOF_COUNT {
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
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31683-01.htm"
                } else {
                    "31683-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31683-03.html"
                } else {
                    "31683-04.html"
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00601_WatchingEyes";

    const NPCS: &str = r#"
        <npc id="31683" type="Folk" level="70" name="Eye of Argos"/>
        <npc id="21306" type="Monster" level="75" name="Varka Silenos Scout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7188" type="EtcItem" name="Proof of Avenger"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6699" type="EtcItem" name="Gemstone Bundle A"><set name="is_stackable" val="true"/></item>
        <item id="6698" type="EtcItem" name="Gemstone Bundle B"><set name="is_stackable" val="true"/></item>
        <item id="6700" type="EtcItem" name="Gemstone Bundle C"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00601WatchingEyes);
        let pages = Pages::of(&Q00601WatchingEyes)
            .page("31683-01.htm", &["31683-02.htm"])
            .plain(&["31683-00.htm", "31683-02.htm", "31683-03.html", "31683-06.html"])
            .page("31683-04.html", &["31683-05.html"])
            .page("31683-05.html", &[])
            .npcs(&[EYE_OF_ARGOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let eye = t.spawn(EYE_OF_ARGOS, 20);
        (t, eye)
    }

    /// Kills Varka Silenos Scouts (85% each) until the player holds `count` proofs.
    fn gather_proofs(t: &mut ScriptTest, count: i64) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(21306, 60);
            t.kill(1, mob);
            if t.count(1, PROOF_OF_AVENGER) >= count {
                return kills;
            }
        }
        panic!("no {count} proofs after 400 kills");
    }

    #[test]
    fn a_character_below_level_71_is_sent_away() {
        let (mut t, eye) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, eye).contains("31683-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_proofs_then_one_of_the_bundles_or_adena() {
        let (mut t, eye) = setup();
        t.enter(1, 71, vec![]);
        assert!(t.quest_page(1, eye).contains("31683-01.htm"));
        assert!(t.event(1, NAME, "31683-02.htm").contains("31683-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, eye).contains("31683-03.html"));

        // Less than 100 proofs: the NPC asks for more, nothing is taken.
        let kills = gather_proofs(&mut t, PROOF_COUNT);
        assert!(kills >= 50, "{kills}");
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Cond 2: the page with the link to the reward.
        assert!(t.quest_page(1, eye).contains("31683-04.html"));
        assert!(t.event(1, NAME, "31683-05.html").contains("31683-05.html"));
        assert_eq!(t.count(1, PROOF_OF_AVENGER), 0);
        // One of four draws: a bundle of five with the adena, or the adena alone.
        let rewards = [(6699, 90000), (6698, 80000), (6700, 40000), (0, 230000)];
        let got = rewards
            .iter()
            .find(|(id, adena)| t.count(1, 57) == *adena && (*id == 0 || t.count(1, *id) == 5));
        assert!(got.is_some(), "adena {}", t.count(1, 57));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
