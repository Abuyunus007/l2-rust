//! Quest 602: Shadow of Light (Eye of Argos in the Ketra and Varka villages, Eye of Darkness).
// Java: dist/game/data/scripts/quests/Q00602_ShadowOfLight/Q00602_ShadowOfLight.java
//
// Pattern: the same shape as Q00601 (a repeatable collect quest with a drop chance per monster;
// the hundredth eye sets cond 2; a random bundle or the adena alone is the reward). The second
// monster has its own chance, and the reward gives experience after the adena.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const EYE_OF_ARGOS: i32 = 31683;

// Item
const EYE_OF_DARKNESS: i32 = 7189;

// Monsters
const MOBS: [i32; 2] = [21299, 21304];

// Reward: (item, adena, exp, sp) per draw; the fourth draw gives adena only.
const REWARD: [(i32, i64, i64, i32); 4] = [
    (6699, 40000, 120000, 20000),
    (6698, 60000, 110000, 15000),
    (6700, 40000, 150000, 10000),
    (0, 100000, 140000, 11250),
];

// Misc
const MIN_LEVEL: i32 = 68;
const EYE_COUNT: i64 = 100;

/// Java `Q00602_ShadowOfLight`.
pub struct Q00602ShadowOfLight;

impl Script for Q00602ShadowOfLight {
    fn name(&self) -> &'static str {
        "Q00602_ShadowOfLight"
    }

    fn path(&self) -> &'static str {
        "quests/Q00602_ShadowOfLight"
    }

    fn id(&self) -> i32 {
        602
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EYE_OF_ARGOS]);
        s.add_talk_id(&[EYE_OF_ARGOS]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[EYE_OF_DARKNESS]);
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
                if ctx.get_quest_items_count(player, EYE_OF_DARKNESS) < EYE_COUNT {
                    return Some("31683-06.html".to_owned());
                }
                let i = ctx.get_random(4);
                let (item, adena, exp, sp) = REWARD[i as usize];
                if i < 3 {
                    ctx.give_items(player, item, 3);
                }
                ctx.give_adena(player, adena, true);
                ctx.add_exp_and_sp(player, exp, sp);
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
        let chance = if npc.id == MOBS[0] { 560 } else { 800 };
        if qs.is_cond(ctx, 1) && ctx.get_random(1000) < chance {
            ctx.give_items(killer, EYE_OF_DARKNESS, 1);
            if ctx.get_quest_items_count(killer, EYE_OF_DARKNESS) == EYE_COUNT {
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

    const NAME: &str = "Q00602_ShadowOfLight";

    const NPCS: &str = r#"
        <npc id="31683" type="Folk" level="70" name="Eye of Argos"/>
        <npc id="21304" type="Monster" level="70" name="Varka Silenos Hunter"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7189" type="EtcItem" name="Eye of Darkness"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6699" type="EtcItem" name="Gemstone Bundle A"><set name="is_stackable" val="true"/></item>
        <item id="6698" type="EtcItem" name="Gemstone Bundle B"><set name="is_stackable" val="true"/></item>
        <item id="6700" type="EtcItem" name="Gemstone Bundle C"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00602ShadowOfLight);
        let pages = Pages::of(&Q00602ShadowOfLight)
            .page("31683-01.htm", &["31683-02.htm"])
            .plain(&["31683-00.htm", "31683-02.htm", "31683-03.html", "31683-06.html"])
            .page("31683-04.html", &["31683-05.html"])
            .page("31683-05.html", &[])
            .npcs(&[EYE_OF_ARGOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let eye = t.spawn(EYE_OF_ARGOS, 20);
        (t, eye)
    }

    /// Kills Varka Silenos Hunters (80% each) until the player holds `count` eyes.
    fn gather_eyes(t: &mut ScriptTest, count: i64) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(21304, 60);
            t.kill(1, mob);
            if t.count(1, EYE_OF_DARKNESS) >= count {
                return kills;
            }
        }
        panic!("no {count} eyes after 400 kills");
    }

    #[test]
    fn a_character_below_level_68_is_sent_away() {
        let (mut t, eye) = setup();
        t.enter(1, 67, vec![]);
        assert!(t.quest_page(1, eye).contains("31683-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_eyes_then_one_of_the_bundles_or_adena() {
        let (mut t, eye) = setup();
        t.enter(1, 68, vec![]);
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

        let kills = gather_eyes(&mut t, EYE_COUNT);
        assert!(kills >= 50, "{kills}");
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, eye).contains("31683-04.html"));
        assert!(t.event(1, NAME, "31683-05.html").contains("31683-05.html"));
        assert_eq!(t.count(1, EYE_OF_DARKNESS), 0);
        // One of four draws: three bundles with the adena, or the adena alone.
        let got = REWARD
            .iter()
            .find(|(id, adena, _, _)| t.count(1, 57) == *adena && (*id == 0 || t.count(1, *id) == 3));
        assert!(got.is_some(), "adena {}", t.count(1, 57));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
