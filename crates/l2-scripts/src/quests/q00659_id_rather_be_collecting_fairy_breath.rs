//! Quest 659: I'd Rather Be Collecting Fairy Breath (High Summoner Galatea in Gludio, the Windy Hill winds).
// Java: dist/game/data/scripts/quests/Q00659_IdRatherBeCollectingFairyBreath/Q00659_IdRatherBeCollectingFairyBreath.java
//
// Pattern: a repeatable quest of kills and a drop chance per monster (`give_item_randomly` with
// no limit), paid in adena per breath with a bonus at ten or more. The reward link (`REWARD`) and
// the "keep going / quit" links are events of the same script.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GALATEA: i32 = 30634;

// Item
const FAIRY_BREATH: i32 = 8286;

// Misc
const MIN_LEVEL: i32 = 26;

/// Java `MOBS`: the Windy Hill winds and their drop chance.
const MOBS: [(i32, f64); 5] = [
    (20078, 0.98), // whispering_wind
    (21023, 0.82), // sobing_wind
    (21024, 0.86), // babbleing_wind
    (21025, 0.90), // giggleing_wind
    (21026, 0.96), // singing_wind
];

/// Java `Q00659_IdRatherBeCollectingFairyBreath`.
pub struct Q00659IdRatherBeCollectingFairyBreath;

impl Script for Q00659IdRatherBeCollectingFairyBreath {
    fn name(&self) -> &'static str {
        "Q00659_IdRatherBeCollectingFairyBreath"
    }

    fn path(&self) -> &'static str {
        "quests/Q00659_IdRatherBeCollectingFairyBreath"
    }

    fn id(&self) -> i32 {
        659
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GALATEA]);
        s.add_talk_id(&[GALATEA]);
        s.add_kill_id(&MOBS.map(|(id, _)| id));
        s.register_quest_items(&[FAIRY_BREATH]);
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
            "30634-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "REWARD" => {
                if ctx.has_quest_items(player, FAIRY_BREATH) {
                    let count = ctx.get_quest_items_count(player, FAIRY_BREATH);
                    let bonus = if count >= 10 { 5365 } else { 0 };
                    ctx.take_items(player, FAIRY_BREATH, -1);
                    ctx.give_adena(player, count * 50 + bonus, true);
                    htmltext = Some("30634-05.html".to_owned());
                } else {
                    htmltext = Some("30634-06.html".to_owned());
                }
            }
            "30634-07.html" => htmltext = Some(event.to_owned()),
            "30634-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.random_party_member_state(killer, -1, 3, Some(npc)).is_some() {
            // NOTE(java-quirk): the breath goes to the killer, not to the member the random
            // pick returned (Java passes `player`, not `qs.getPlayer()`).
            let chance = MOBS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0.0, |(_, chance)| *chance);
            ctx.give_item_randomly(killer, Some(npc), FAIRY_BREATH, 1, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30634-01.htm"
                } else {
                    "30634-03.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(player, FAIRY_BREATH) {
                    "30634-04.html"
                } else {
                    "30634-09.html"
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00659_IdRatherBeCollectingFairyBreath";

    const NPCS: &str = r#"
        <npc id="30634" type="Folk" level="1" name="High Summoner Galatea"/>
        <npc id="20078" type="Monster" level="20" name="Whispering Wind"/>
        <npc id="21023" type="Monster" level="20" name="Sobbing Wind"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8286" type="EtcItem" name="Fairy Breath"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00659IdRatherBeCollectingFairyBreath);
        let pages = Pages::of(&Q00659IdRatherBeCollectingFairyBreath)
            .page("30634-01.htm", &["30634-02.htm"])
            .plain(&["30634-02.htm", "30634-03.html", "30634-06.html", "30634-09.html"])
            .page("30634-04.html", &["REWARD"])
            .page("30634-05.html", &["30634-07.html", "30634-08.html"])
            .plain(&["30634-07.html", "30634-08.html"])
            .npcs(&[GALATEA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let galatea = t.spawn(GALATEA, 20);
        (t, galatea)
    }

    #[test]
    fn a_level_25_character_is_told_to_come_back() {
        let (mut t, galatea) = setup();
        t.enter(1, 25, vec![]);
        assert!(t.quest_page(1, galatea).contains("30634-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_26_character_starts_the_quest() {
        let (mut t, galatea) = setup();
        t.enter(1, 26, vec![]);
        assert!(t.quest_page(1, galatea).contains("30634-01.htm"));
        assert!(t.event(1, NAME, "30634-02.htm").contains("30634-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Started, no breath yet.
        assert!(t.quest_page(1, galatea).contains("30634-09.html"));
    }

    #[test]
    fn ten_breath_pay_50_each_and_the_bonus_then_the_player_goes_on() {
        let (mut t, galatea) = setup();
        t.enter(1, 30, vec![]);
        t.quest_page(1, galatea);
        t.event(1, NAME, "30634-02.htm");
        t.take_writes();

        let mut kills = 0;
        while t.count(1, FAIRY_BREATH) < 10 {
            let wind = t.spawn(if kills % 2 == 0 { 20078 } else { 21023 }, 60);
            t.kill(1, wind);
            kills += 1;
            assert!(kills < 200, "the winds never dropped the breath");
        }
        assert!(t.take_writes().is_empty());

        // Ten or more breath: 50 each and the bonus of 5 365.
        assert!(t.quest_page(1, galatea).contains("30634-04.html"));
        assert!(t.event(1, NAME, "REWARD").contains("30634-05.html"));
        assert_eq!(t.count(1, FAIRY_BREATH), 0);
        assert_eq!(t.count(1, 57), 10 * 50 + 5365);
        assert!(t.take_writes().is_empty());

        // "I'll do it" keeps the quest going.
        assert!(t.event(1, NAME, "30634-07.html").contains("30634-07.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn quitting_after_a_reward_ends_the_quest() {
        let (mut t, galatea) = setup();
        t.enter(1, 30, vec![]);
        t.quest_page(1, galatea);
        t.event(1, NAME, "30634-02.htm");
        t.take_writes();

        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), FAIRY_BREATH, 1);
        });
        assert!(t.quest_page(1, galatea).contains("30634-04.html"));
        assert!(t.event(1, NAME, "REWARD").contains("30634-05.html"));
        // Fewer than ten: no bonus.
        assert_eq!(t.count(1, 57), 50);
        t.take_writes();

        assert!(t.event(1, NAME, "30634-08.html").contains("30634-08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
