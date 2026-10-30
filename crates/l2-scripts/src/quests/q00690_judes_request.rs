//! Quest 690: Jude's Request (Jude in the Aden Steel Citadel area, the Evil Weapons of the Monster Foundry).
// Java: dist/game/data/scripts/quests/Q00690_JudesRequest/Q00690_JudesRequest.java
//
// Pattern: a repeatable kill-and-collect quest with two exchanges: 200 Evil Weapons for an S80
// recipe, or 5 for three S80 pieces, the item drawn at random from a list (`get_random(9)`).
// Java's `htmltext = event` default means every event of this script is shown as a page, and
// with no quest state the event answers the no-quest message.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JUDE: i32 = 32356;
const LESSER_EVIL: i32 = 22398;
const GREATER_EVIL: i32 = 22399;

// Items
const EVIL_WEAPON: i32 = 10327;
/// Java `REWARDS[0]`: the S80 recipes (the 200-weapon exchange).
const REWARDS_RECIPE: [i32; 9] = [10373, 10374, 10375, 10376, 10377, 10378, 10379, 10380, 10381];
/// Java `REWARDS[1]`: the S80 pieces (the 5-weapon exchange).
const REWARDS_PIECES: [i32; 9] = [10397, 10398, 10399, 10400, 10401, 10402, 10403, 10404, 10405];

// Misc
const MIN_LEVEL: i32 = 78;

/// Java `Q00690_JudesRequest`.
pub struct Q00690JudesRequest;

impl Script for Q00690JudesRequest {
    fn name(&self) -> &'static str {
        "Q00690_JudesRequest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00690_JudesRequest"
    }

    fn id(&self) -> i32 {
        690
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JUDE]);
        s.add_talk_id(&[JUDE]);
        s.add_kill_id(&[LESSER_EVIL, GREATER_EVIL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java: `htmltext = event` is the default; without a quest state the no-quest message.
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("32356-03.htm") {
            qs.start_quest(ctx);
        } else if event.eq_ignore_ascii_case("32356-07.htm") {
            if ctx.get_quest_items_count(player, EVIL_WEAPON) >= 200 {
                let index = ctx.get_random(REWARDS_RECIPE.len() as i32) as usize;
                ctx.give_items(player, REWARDS_RECIPE[index], 1);
                ctx.take_items(player, EVIL_WEAPON, 200);
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                htmltext = "32356-07.htm".to_owned();
            } else {
                htmltext = "32356-07a.htm".to_owned();
            }
        } else if event.eq_ignore_ascii_case("32356-08.htm") {
            ctx.take_items(player, EVIL_WEAPON, -1);
            qs.exit_quest(ctx, true, true);
        } else if event.eq_ignore_ascii_case("32356-09.htm") {
            if ctx.get_quest_items_count(player, EVIL_WEAPON) >= 5 {
                let index = ctx.get_random(REWARDS_PIECES.len() as i32) as usize;
                ctx.give_items(player, REWARDS_PIECES[index], 1);
                ctx.take_items(player, EVIL_WEAPON, 5);
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                htmltext = "32356-09.htm".to_owned();
            } else {
                htmltext = "32356-09a.htm".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_random_party_member_cond(killer, 1).is_none() {
            return;
        }
        let chance = if npc.id == LESSER_EVIL {
            173
        } else if npc.id == GREATER_EVIL {
            246
        } else {
            0
        };
        // Java: `chance %= 1000` (a no-op here) and one weapon: `max(chance / 1000, 1)`.
        if ctx.get_random(1000) <= chance {
            // NOTE(java-quirk): the weapon goes to the killer, not to the member the random pick
            // returned (Java passes `player`, not `partyMember`).
            ctx.give_items(killer, EVIL_WEAPON, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32356-01.htm"
                } else {
                    "32356-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                let weapons = ctx.get_quest_items_count(player, EVIL_WEAPON);
                htmltext = if weapons >= 200 {
                    "32356-04.htm"
                } else if weapons < 5 {
                    "32356-05a.htm"
                } else {
                    "32356-05.htm"
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

    const NAME: &str = "Q00690_JudesRequest";

    const NPCS: &str = r#"
        <npc id="32356" type="Folk" level="1" name="Jude"/>
        <npc id="22398" type="Monster" level="80" name="Lesser Evil"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10327" type="EtcItem" name="Evil Weapon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10373" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10374" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10375" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10376" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10377" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10378" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10379" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10380" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10381" type="EtcItem" name="S80 Grade Weapon Recipe"/>
        <item id="10397" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10398" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10399" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10400" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10401" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10402" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10403" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10404" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>
        <item id="10405" type="EtcItem" name="S80 Grade Weapon Piece"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00690JudesRequest);
        let pages = Pages::of(&Q00690JudesRequest)
            .page("32356-01.htm", &["32356-03.htm"])
            .plain(&["32356-02.htm", "32356-03.htm"])
            .page("32356-04.htm", &["32356-06.htm", "32356-07.htm", "32356-09.htm"])
            .page("32356-05.htm", &["32356-09.htm", "32356-06.htm", "32356-08.htm"])
            .page("32356-05a.htm", &["32356-06.htm", "32356-08.htm"])
            .plain(&[
                "32356-06.htm",
                "32356-07.htm",
                "32356-07a.htm",
                "32356-08.htm",
                "32356-09.htm",
                "32356-09a.htm",
            ])
            .npcs(&[JUDE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jude = t.spawn(JUDE, 20);
        (t, jude)
    }

    fn give_weapons(t: &mut ScriptTest, count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), EVIL_WEAPON, count);
        });
    }

    fn count_of(t: &ScriptTest, ids: &[i32]) -> i64 {
        ids.iter().map(|id| t.count(1, *id)).sum()
    }

    #[test]
    fn a_level_78_character_starts_the_quest() {
        let (mut t, jude) = setup();
        t.enter(1, 77, vec![]);
        assert!(t.quest_page(1, jude).contains("32356-02.htm"));
        assert!(t.take_writes().is_empty());

        t.enter(2, 78, vec![]);
        let jude2 = t.spawn(JUDE, 25);
        assert!(t.quest_page(2, jude2).contains("32356-01.htm"));
        assert!(t.event(2, NAME, "32356-03.htm").contains("32356-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started"))
            ]
        );
    }

    #[test]
    fn the_monsters_drop_weapons_and_five_become_a_piece() {
        let (mut t, jude) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, jude);
        t.event(1, NAME, "32356-03.htm");
        t.take_writes();

        // Fewer than five: 05a.
        assert!(t.quest_page(1, jude).contains("32356-05a.htm"));

        let mut kills = 0;
        while t.count(1, EVIL_WEAPON) < 5 {
            let lesser = t.spawn(22398, 60);
            t.kill(1, lesser);
            kills += 1;
            assert!(kills < 300, "the lesser evils never dropped the weapons");
        }
        assert!(t.quest_page(1, jude).contains("32356-05.htm"));

        // The five-for-a-piece exchange: one piece, five weapons gone.
        assert!(t.event(1, NAME, "32356-09.htm").contains("32356-09.htm"));
        assert_eq!(count_of(&t, &REWARDS_PIECES), 1);
        assert_eq!(t.count(1, EVIL_WEAPON), 0);
    }

    #[test]
    fn two_hundred_weapons_get_a_recipe_and_quit_takes_the_rest() {
        let (mut t, jude) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, jude);
        t.event(1, NAME, "32356-03.htm");
        t.take_writes();

        give_weapons(&mut t, 199);
        assert!(t.quest_page(1, jude).contains("32356-05.htm"));
        assert!(t.event(1, NAME, "32356-06.htm").contains("32356-06.htm"));
        assert!(t.quest_page(1, jude).contains("32356-05.htm"));

        give_weapons(&mut t, 1);
        assert!(t.quest_page(1, jude).contains("32356-04.htm"));
        assert!(t.event(1, NAME, "32356-07.htm").contains("32356-07.htm"));
        assert_eq!(count_of(&t, &REWARDS_RECIPE), 1);
        assert_eq!(t.count(1, EVIL_WEAPON), 0);
        assert!(t.take_writes().is_empty());

        // Stop: 08 takes the rest and ends the quest.
        give_weapons(&mut t, 7);
        assert!(t.quest_page(1, jude).contains("32356-05.htm"));
        assert!(t.event(1, NAME, "32356-08.htm").contains("32356-08.htm"));
        assert_eq!(t.count(1, EVIL_WEAPON), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn an_exchange_with_too_few_weapons_says_so() {
        let (mut t, jude) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, jude);
        t.event(1, NAME, "32356-03.htm");
        t.take_writes();

        give_weapons(&mut t, 4);
        assert!(t.quest_page(1, jude).contains("32356-05a.htm"));
        assert!(t.event(1, NAME, "32356-06.htm").contains("32356-06.htm"));
        assert!(t.quest_page(1, jude).contains("32356-05a.htm"));
        assert_eq!(t.count(1, EVIL_WEAPON), 4);
    }
}
