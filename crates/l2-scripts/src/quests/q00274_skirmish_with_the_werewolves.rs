//! Quest 274: Skirmish with the Werewolves (Brukurse in the Orc Village sends the orcs after the Maraku werewolves).
// Java: dist/game/data/scripts/quests/Q00274_SkirmishWithTheWerewolves/Q00274_SkirmishWithTheWerewolves.java
//
// Pattern: a kill-count quest with a rare second drop (`docs/design/scripts.md`, "Quest patterns"):
// every Maraku werewolf gives a head, 5% of the time also a totem; the 40th head sets cond 2 and
// Brukurse pays for the heads and the totems and ends the quest. Only players who own a necklace
// of valor or of courage may take it.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BRUKURSE: i32 = 30569;

// Monsters
const MARAKU_WEREWOLF: i32 = 20363;
const MARAKU_WEREWOLF_CHIEFTAIN: i32 = 20364;

// Items
const NECKLACE_OF_COURAGE: i32 = 1506;
const NECKLACE_OF_VALOR: i32 = 1507;
const WEREWOLF_HEAD: i32 = 1477;
const WEREWOLF_TOTEM: i32 = 1501;

// Misc
const MIN_LEVEL: i32 = 9;

/// Java `Q00274_SkirmishWithTheWerewolves`.
pub struct Q00274SkirmishWithTheWerewolves;

impl Script for Q00274SkirmishWithTheWerewolves {
    fn name(&self) -> &'static str {
        "Q00274_SkirmishWithTheWerewolves"
    }

    fn path(&self) -> &'static str {
        "quests/Q00274_SkirmishWithTheWerewolves"
    }

    fn id(&self) -> i32 {
        274
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BRUKURSE]);
        s.add_talk_id(&[BRUKURSE]);
        s.add_kill_id(&[MARAKU_WEREWOLF, MARAKU_WEREWOLF_CHIEFTAIN]);
        s.register_quest_items(&[WEREWOLF_HEAD, WEREWOLF_TOTEM]);
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
        if event.eq_ignore_ascii_case("30569-04.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            ctx.give_items(killer, WEREWOLF_HEAD, 1);
            if ctx.get_random(100) <= 5 {
                ctx.give_items(killer, WEREWOLF_TOTEM, 1);
            }
            if ctx.get_quest_items_count(killer, WEREWOLF_HEAD) >= 40 {
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
                if ctx.has_at_least_one_quest_item(player, &[NECKLACE_OF_VALOR, NECKLACE_OF_COURAGE]) {
                    htmltext = if ctx.player_race(player) == race::ORC {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30569-03.htm"
                        } else {
                            "30569-02.html"
                        }
                    } else {
                        "30569-01.html"
                    }
                    .to_owned();
                } else {
                    htmltext = "30569-08.html".to_owned();
                }
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30569-05.html".to_owned(),
                2 => {
                    let heads = ctx.get_quest_items_count(player, WEREWOLF_HEAD);
                    if heads >= 40 {
                        let totems = ctx.get_quest_items_count(player, WEREWOLF_TOTEM);
                        ctx.give_adena(player, heads * 30 + totems * 600 + 2300, true);
                        qs.exit_quest(ctx, true, true);
                        htmltext = if totems > 0 {
                            "30569-07.html"
                        } else {
                            "30569-06.html"
                        }
                        .to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00274_SkirmishWithTheWerewolves";

    const NPCS: &str = r#"
        <npc id="30569" type="Folk" level="70" name="Brukurse"/>
        <npc id="20363" type="Monster" level="20" name="Maraku Werewolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1477" type="EtcItem" name="Werewolf Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1501" type="EtcItem" name="Werewolf Totem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1506" type="Armor" name="Necklace of Courage"><set name="bodypart" val="neck"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00274SkirmishWithTheWerewolves);
        let pages = Pages::of(&Q00274SkirmishWithTheWerewolves)
            .page("30569-03.htm", &["30569-04.htm"])
            .plain(&[
                "30569-01.html",
                "30569-02.html",
                "30569-04.htm",
                "30569-05.html",
                "30569-06.html",
                "30569-07.html",
                "30569-08.html",
            ])
            .npcs(&[BRUKURSE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let brukurse = t.spawn(BRUKURSE, 20);
        (t, brukurse)
    }

    /// Kills Maraku Werewolves with `player` until it has 40 heads; returns the kills.
    fn hunt(t: &mut ScriptTest, player: i32) -> usize {
        for kills in 1..400 {
            if t.count(player, WEREWOLF_HEAD) >= 40 {
                return kills;
            }
            let werewolf = t.spawn(MARAKU_WEREWOLF, 60);
            t.kill(player, werewolf);
        }
        panic!("no 40 heads after 400 kills");
    }

    #[test]
    fn only_orcs_with_a_necklace_of_level_nine_can_take_it() {
        let (mut t, brukurse) = setup();
        t.enter_as(1, 10, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, brukurse).contains("30569-08.html"));
        t.enter_as(
            2,
            9,
            race::ORC,
            player_class::ORC_FIGHTER,
            vec![item(900, NECKLACE_OF_COURAGE, 1)],
        );
        assert!(t.quest_page(2, brukurse).contains("30569-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn forty_heads_then_adena_for_the_heads_and_totems() {
        let (mut t, brukurse) = setup();
        t.enter_as(
            1,
            9,
            race::ORC,
            player_class::ORC_FIGHTER,
            vec![item(900, NECKLACE_OF_COURAGE, 1)],
        );
        assert!(t.quest_page(1, brukurse).contains("30569-03.htm"));
        assert!(t.event(1, NAME, "30569-04.htm").contains("30569-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, brukurse).contains("30569-05.html"));

        // The 40th head sets cond 2 (`setCond(2, true)`).
        hunt(&mut t, 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // 30 adena a head, 600 a totem, 2300 more; the page says whether there was a totem.
        let heads = t.count(1, WEREWOLF_HEAD);
        let totems = t.count(1, WEREWOLF_TOTEM);
        assert!(t.quest_page(1, brukurse).contains(if totems > 0 {
            "30569-07.html"
        } else {
            "30569-06.html"
        }));
        assert_eq!(t.count(1, 57), heads * 30 + totems * 600 + 2300);
        assert_eq!((t.count(1, WEREWOLF_HEAD), t.count(1, WEREWOLF_TOTEM)), (0, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
