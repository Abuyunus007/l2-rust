//! Quest 366: Silver Haired Shaman (Magister Dieter, Sairon's dolls on Devil's Isle).
// Java: dist/game/data/scripts/quests/Q00366_SilverHairedShaman/Q00366_SilverHairedShaman.java
//
// Pattern: a repeatable kill-count quest with a drop chance per monster (`docs/design/scripts.md`,
// "Quest patterns"): the kill goes to a random started party member (`random_party_member_state`,
// `check_party_member_state` = Java `checkPartyMember`), the NPC pays for the hair and the player
// chooses to go on or to quit (`exit_quest(true, true)`).

use l2_util::ObjectId;
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DIETER: i32 = 30111;

// Item
const SAIRONS_SILVER_HAIR: i32 = 5874;

// Misc
const MIN_LEVEL: i32 = 48;

// Mobs (drop chance in percent)
const MOBS: [(i32, i32); 3] = [
    (20986, 80), // saitnn
    (20987, 73), // saitnn_doll
    (20988, 80), // saitnn_puppet
];

/// Java `Q00366_SilverHairedShaman`.
pub struct Q00366SilverHairedShaman;

impl Script for Q00366SilverHairedShaman {
    fn name(&self) -> &'static str {
        "Q00366_SilverHairedShaman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00366_SilverHairedShaman"
    }

    fn id(&self) -> i32 {
        366
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DIETER]);
        s.add_talk_id(&[DIETER]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SAIRONS_SILVER_HAIR]);
    }

    fn check_party_member_state(&self, ctx: &mut ScriptCtx<'_>, qs: Qs, _npc: Option<ScriptNpc>) -> bool {
        qs.is_started(ctx)
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
            "30111-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30111-05.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30111-06.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30111-01.htm"
            } else {
                "30111-03.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if ctx.has_quest_items(player, SAIRONS_SILVER_HAIR) {
                let item_count = ctx.get_quest_items_count(player, SAIRONS_SILVER_HAIR);
                ctx.give_adena(player, item_count * 500 + 29000, true);
                ctx.take_items(player, SAIRONS_SILVER_HAIR, -1);
                htmltext = "30111-04.html".to_owned();
            } else {
                htmltext = "30111-07.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, 1, 3, Some(npc));
        let chance = MOBS.iter().find(|(id, _)| *id == npc.id).map_or(0, |(_, c)| *c);
        if let Some(qs) = qs
            && ctx.get_random(100) < chance
        {
            // The hair goes to the member the roll picked (no limit, every roll that passes).
            ctx.give_item_randomly(qs.player(), Some(npc), SAIRONS_SILVER_HAIR, 1, 1, 0, 1.0, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00366_SilverHairedShaman";

    const NPCS: &str = r#"
        <npc id="30111" type="Folk" level="70" name="Magister Dieter"/>
        <npc id="20986" type="Monster" level="60" name="Saitnn"/>
        <npc id="20987" type="Monster" level="60" name="Saitnn Doll"/>
        <npc id="20988" type="Monster" level="60" name="Saitnn Puppet"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5874" type="EtcItem" name="Sairon's Silver Hair"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00366SilverHairedShaman);
        let pages = Pages::of(&Q00366SilverHairedShaman)
            .page("30111-01.htm", &["30111-02.htm"])
            .plain(&["30111-02.htm", "30111-03.html", "30111-05.html", "30111-06.html"])
            .page("30111-04.html", &["30111-05.html", "30111-06.html"])
            .page("30111-07.html", &["30111-05.html", "30111-06.html"])
            .npcs(&[DIETER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dieter = t.spawn(DIETER, 20);
        (t, dieter)
    }

    /// Kills the Sairon monsters until the player has `hair` silver hairs.
    fn hunt(t: &mut ScriptTest, hair: i64) {
        for kills in 0..400 {
            if t.count(1, SAIRONS_SILVER_HAIR) >= hair {
                return;
            }
            let mob = t.spawn(MOBS[kills % 3].0, 60);
            t.kill(1, mob);
        }
        panic!("no {hair} hairs after 400 kills");
    }

    #[test]
    fn a_level_47_character_is_sent_away() {
        let (mut t, dieter) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, dieter).contains("30111-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hair_is_paid_and_the_player_chooses_to_go_on_or_quit() {
        let (mut t, dieter) = setup();
        t.enter(1, 48, vec![]);
        assert!(t.quest_page(1, dieter).contains("30111-01.htm"));
        assert!(t.event(1, NAME, "30111-02.htm").contains("30111-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, dieter).contains("30111-07.html"));

        // Five hairs; the NPC pays 500 each plus 29000 and the hair goes; the quest goes on.
        hunt(&mut t, 5);
        let hair = t.count(1, SAIRONS_SILVER_HAIR);
        assert!(t.quest_page(1, dieter).contains("30111-04.html"));
        assert_eq!(t.count(1, 57), hair * 500 + 29000);
        assert_eq!(t.count(1, SAIRONS_SILVER_HAIR), 0);
        assert!(t.take_writes().is_empty());

        // "I will continue working for you."
        assert!(t.event(1, NAME, "30111-06.html").contains("30111-06.html"));
        assert!(t.take_writes().is_empty());

        // "I want to quit": the quest ends repeatable (the NPC's page offers the link again).
        assert!(t.quest_page(1, dieter).contains("30111-07.html"));
        assert!(t.event(1, NAME, "30111-05.html").contains("30111-05.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, dieter).contains("30111-01.htm"));
    }

    #[test]
    fn a_kill_goes_to_a_started_party_member() {
        let (mut t, dieter) = setup();
        t.enter(1, 48, vec![]);
        t.enter(2, 48, vec![]);
        assert!(t.quest_page(2, dieter).contains("30111-01.htm"));
        t.event(2, NAME, "30111-02.htm");
        t.party(1, 2);
        for kills in 0..20 {
            let mob = t.spawn(MOBS[kills % 3].0, 60);
            t.kill(1, mob);
        }
        // Only the started member can take a hair: the killer is not on the quest.
        assert_eq!(t.count(1, SAIRONS_SILVER_HAIR), 0);
        assert!(t.count(2, SAIRONS_SILVER_HAIR) > 0);
    }
}
