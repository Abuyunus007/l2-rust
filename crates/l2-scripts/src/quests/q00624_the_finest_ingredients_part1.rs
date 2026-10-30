//! Quest 624: The Finest Ingredients - Part 1 (Jeremy in the Rune Town/Hot Springs area; the
//! Hot Springs monsters drop the three ingredients, 50 of each make the first half of the dish).
// Java: dist/game/data/scripts/quests/Q00624_TheFinestIngredientsPart1/Q00624_TheFinestIngredientsPart1.java
//
// Pattern: a kill count per item with a fill to 50 (`getRandomPartyMember(player, 1)` and the
// radius check of the member), cond 2 when the three items reach 150 together, and the reward
// of two items that ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const JEREMY: i32 = 31521;

// Items
const TRUNK_OF_NEPENTHES: i32 = 7202;
const FOOT_OF_BANDERSNATCHLING: i32 = 7203;
const SECRET_SPICE: i32 = 7204;

// Rewards
const ICE_CRYSTAL: i32 = 7080;
const SOY_SAUCE_JAR: i32 = 7205;

// Misc
const MIN_LEVEL: i32 = 73;

/// Java `MONSTER_DROPS`: the item each monster drops.
const MONSTER_DROPS: [(i32, i32); 4] = [
    (21314, FOOT_OF_BANDERSNATCHLING), // Hot Springs Bandersnatchling
    (21317, SECRET_SPICE),             // Hot Springs Atroxspawn
    (21319, TRUNK_OF_NEPENTHES),       // Hot Springs Nepenthes
    (21321, SECRET_SPICE),             // Hot Springs Atrox
];

/// Java `getRegisteredItemIds()`.
const REGISTERED: [i32; 3] = [TRUNK_OF_NEPENTHES, FOOT_OF_BANDERSNATCHLING, SECRET_SPICE];

/// Java `Q00624_TheFinestIngredientsPart1`.
pub struct Q00624TheFinestIngredientsPart1;

impl Script for Q00624TheFinestIngredientsPart1 {
    fn name(&self) -> &'static str {
        "Q00624_TheFinestIngredientsPart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00624_TheFinestIngredientsPart1"
    }

    fn id(&self) -> i32 {
        624
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JEREMY]);
        s.add_talk_id(&[JEREMY]);
        let mobs: Vec<i32> = MONSTER_DROPS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&REGISTERED);
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
            "31521-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31521-05.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count_of(player, &REGISTERED) == 150 {
                    ctx.give_items(player, ICE_CRYSTAL, 1);
                    ctx.give_items(player, SOY_SAUCE_JAR, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("31521-05.html".to_owned());
                } else {
                    htmltext = Some("31521-06.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(killer, 1)`: a member at cond 1, then `isInsideRadius3D`.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, member, true) {
            return;
        }
        let Some(&(_, item)) = MONSTER_DROPS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        let count = ctx.get_quest_items_count(member, item);
        if count + 1 >= 50 {
            if count < 50 {
                ctx.give_items(member, item, 50 - count);
                ctx.play_sound(member, QuestSound::ItemsoundQuestFanfareMiddle);
            }
            if ctx.get_quest_items_count_of(member, &REGISTERED) == 150
                && let Some(qs) = ctx.get_quest_state(member, false)
            {
                qs.set_cond(ctx, 2, true);
            }
        } else {
            ctx.give_items(member, item, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31521-01.htm".to_owned()
            } else {
                "31521-00.htm".to_owned()
            };
        } else if state == STARTED {
            if qs.is_cond(ctx, 1) {
                htmltext = "31521-03.html".to_owned();
            } else if qs.is_cond(ctx, 2) {
                htmltext = "31521-04.html".to_owned();
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

    const NAME: &str = "Q00624_TheFinestIngredientsPart1";

    const NPCS: &str = r#"
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="21314" type="Monster" level="70" name="Hot Springs Bandersnatchling"/>
        <npc id="21317" type="Monster" level="70" name="Hot Springs Atroxspawn"/>
        <npc id="21319" type="Monster" level="70" name="Hot Springs Nepenthes"/>
        <npc id="21321" type="Monster" level="70" name="Hot Springs Atrox"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7202" type="EtcItem" name="Trunk of Nepenthes"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7203" type="EtcItem" name="Foot of Bandersnatchling"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7204" type="EtcItem" name="Secret Spice"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7080" type="EtcItem" name="Ice Crystal"><set name="is_stackable" val="true"/></item>
        <item id="7205" type="EtcItem" name="Soy Sauce Jar"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00624TheFinestIngredientsPart1);
        let pages = Pages::of(&Q00624TheFinestIngredientsPart1)
            .page("31521-01.htm", &["31521-02.htm"])
            .plain(&["31521-00.htm", "31521-02.htm", "31521-03.html", "31521-06.html"])
            .page("31521-04.html", &["31521-05.html"])
            .plain(&["31521-05.html"])
            .npcs(&[JEREMY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jeremy = t.spawn(JEREMY, 20);
        (t, jeremy)
    }

    /// Kills `mob` next to the player until the player has `target` of `item`.
    fn kill_until(t: &mut ScriptTest, mob: i32, item: i32, target: i64) {
        let mut kills = 0;
        while t.count(1, item) < target {
            let npc = t.spawn(mob, 40);
            t.kill(1, npc);
            kills += 1;
            assert!(kills < 200, "no {item} dropped");
        }
    }

    #[test]
    fn a_character_below_level_73_is_sent_away() {
        let (mut t, jeremy) = setup();
        t.enter(1, 72, vec![]);
        assert!(t.quest_page(1, jeremy).contains("31521-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_ingredients_to_150_and_the_reward() {
        let (mut t, jeremy) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, jeremy).contains("31521-01.htm"));
        assert!(t.event(1, NAME, "31521-02.htm").contains("31521-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, jeremy).contains("31521-03.html"));

        // Fill the feet and the trunks to 50 each; the spice from both atrox kinds.
        kill_until(&mut t, 21314, FOOT_OF_BANDERSNATCHLING, 50);
        kill_until(&mut t, 21319, TRUNK_OF_NEPENTHES, 50);
        kill_until(&mut t, 21317, SECRET_SPICE, 25);
        assert!(t.take_writes().is_empty());
        // The last spice that makes 150 sets cond 2.
        kill_until(&mut t, 21321, SECRET_SPICE, 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, jeremy).contains("31521-04.html"));
        assert!(t.event(1, NAME, "31521-05.html").contains("31521-05.html"));
        assert_eq!((t.count(1, 7080), t.count(1, 7205)), (1, 1));
        assert_eq!(t.count(1, TRUNK_OF_NEPENTHES), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
