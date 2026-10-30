//! Quest 626: A Dark Twilight (Hierarch in the Gludio/Hot Springs area; the Splendor monsters
//! drop the blood of saints, 300 of them buy the experience and the sp or 100000 adena).
// Java: dist/game/data/scripts/quests/Q00626_ADarkTwilight/Q00626_ADarkTwilight.java
//
// Pattern: a party kill count with a drop chance per monster (per mille, the member at cond 1,
// no range check), cond 2 at 300 items, and a choice of reward on the page (`Exp` or `Adena`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HIERARCH: i32 = 31517;

// Item
const BLOOD_OF_SAINT: i32 = 7169;

/// Java `MONSTERS`: the blood drop chance (per mille) of each splendor monster.
const MONSTERS: [(i32, i32); 14] = [
    (21520, 641), // Eye of Splendor
    (21523, 648), // Flash of Splendor
    (21524, 692), // Blade of Splendor
    (21525, 710), // Blade of Splendor
    (21526, 772), // Wisdom of Splendor
    (21529, 639), // Soul of Splendor
    (21530, 683), // Victory of Splendor
    (21531, 767), // Punishment of Splendor
    (21532, 795), // Shout of Splendor
    (21535, 802), // Signet of Splendor
    (21536, 774), // Crown of Splendor
    (21539, 848), // Wailing of Splendor
    (21540, 880), // Wailing of Splendor
    (21658, 790), // Punishment of Splendor
];

// Misc
const MIN_LEVEL_REQUIRED: i32 = 60;
const ITEMS_COUNT_REQUIRED: i64 = 300;

// Rewards
const ADENA_COUNT: i64 = 100_000;
const XP_COUNT: i64 = 162_773;
const SP_COUNT: i32 = 12_500;

/// Java `Q00626_ADarkTwilight`.
pub struct Q00626ADarkTwilight;

impl Script for Q00626ADarkTwilight {
    fn name(&self) -> &'static str {
        "Q00626_ADarkTwilight"
    }

    fn path(&self) -> &'static str {
        "quests/Q00626_ADarkTwilight"
    }

    fn id(&self) -> i32 {
        626
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIERARCH]);
        s.add_talk_id(&[HIERARCH]);
        let mobs: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[BLOOD_OF_SAINT]);
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
            "31517-05.html" => {}
            "31517-02.htm" => qs.start_quest(ctx),
            "Exp" => {
                if ctx.get_quest_items_count(player, BLOOD_OF_SAINT) < ITEMS_COUNT_REQUIRED {
                    return Some("31517-06.html".to_owned());
                }
                ctx.add_exp_and_sp(player, XP_COUNT, SP_COUNT);
                qs.exit_quest(ctx, true, true);
                htmltext = Some("31517-07.html".to_owned());
            }
            "Adena" => {
                if ctx.get_quest_items_count(player, BLOOD_OF_SAINT) < ITEMS_COUNT_REQUIRED {
                    return Some("31517-06.html".to_owned());
                }
                ctx.give_adena(player, ADENA_COUNT, true);
                qs.exit_quest(ctx, true, true);
                htmltext = Some("31517-07.html".to_owned());
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(killer, 1)`: no range check here.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let Some(&(_, chance)) = MONSTERS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        if ctx.get_random(1000) < chance {
            ctx.give_items(member, BLOOD_OF_SAINT, 1);
            if ctx.get_quest_items_count(member, BLOOD_OF_SAINT) < ITEMS_COUNT_REQUIRED {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            } else if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL_REQUIRED {
                "31517-01.htm".to_owned()
            } else {
                "31517-00.htm".to_owned()
            };
        } else if state == STARTED {
            if qs.is_cond(ctx, 1) {
                htmltext = "31517-03.html".to_owned();
            } else if qs.is_cond(ctx, 2) {
                htmltext = "31517-04.html".to_owned();
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

    const NAME: &str = "Q00626_ADarkTwilight";

    const NPCS: &str = r#"
        <npc id="31517" type="Folk" level="70" name="Hierarch"/>
        <npc id="21520" type="Monster" level="70" name="Eye of Splendor"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7169" type="EtcItem" name="Blood of Saint"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00626ADarkTwilight);
        let pages = Pages::of(&Q00626ADarkTwilight)
            .page("31517-01.htm", &["31517-02.htm"])
            .plain(&[
                "31517-00.htm",
                "31517-02.htm",
                "31517-03.html",
                "31517-06.html",
                "31517-07.html",
            ])
            .page("31517-04.html", &["31517-05.html"])
            .page("31517-05.html", &["Exp", "Adena"])
            .npcs(&[HIERARCH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let hierarch = t.spawn(HIERARCH, 20);
        (t, hierarch)
    }

    /// An eye of splendor (drop chance 641 per mille) dies next to the player.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(21520, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_60_is_sent_away() {
        let (mut t, hierarch) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, hierarch).contains("31517-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_hundred_blood_of_saints_pay_the_adena_and_end_the_quest() {
        let (mut t, hierarch) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, hierarch).contains("31517-01.htm"));
        assert!(t.event(1, NAME, "31517-02.htm").contains("31517-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, hierarch).contains("31517-03.html"));

        // 299 blood, then a kill that gives the 300th and sets cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BLOOD_OF_SAINT, 299);
        });
        t.take();
        kill_one(&mut t);
        assert_eq!(t.count(1, BLOOD_OF_SAINT), 300);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, hierarch).contains("31517-04.html"));
        assert!(t.event(1, NAME, "31517-05.html").contains("31517-05.html"));
        assert!(t.event(1, NAME, "Adena").contains("31517-07.html"));
        assert_eq!(t.count(1, 57), ADENA_COUNT);
        assert_eq!(t.count(1, BLOOD_OF_SAINT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_exp_reward_waits_for_300_blood_of_saints() {
        let (mut t, hierarch) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, hierarch);
        t.event(1, NAME, "31517-02.htm");
        t.take_writes();
        // Cond 2 with 299 blood: the "Exp" page refuses and nothing is written.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BLOOD_OF_SAINT, 299);
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        t.take();
        t.take_writes();
        assert!(t.quest_page(1, hierarch).contains("31517-04.html"));
        assert!(t.event(1, NAME, "31517-05.html").contains("31517-05.html"));
        assert!(t.event(1, NAME, "Exp").contains("31517-06.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, BLOOD_OF_SAINT), 299);
    }
}
