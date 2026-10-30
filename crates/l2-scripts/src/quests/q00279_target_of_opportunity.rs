//! Quest 279: Target of Opportunity (Jerian in the Hunters Village sends level 82 players after the seal components).
// Java: dist/game/data/scripts/quests/Q00279_TargetOfOpportunity/Q00279_TargetOfOpportunity.java
//
// Pattern: a party collect quest (`docs/design/scripts.md`, "Quest patterns"): a random party
// member whose `progress` variable is 1 is the one the four seal components count for. Each
// component drops once per monster kind with a chance; the last one sets cond 2. Jerian gives the
// two seal breakers once all four are there and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const JERIAN: i32 = 32302;

// Monsters (sorted, like Java's `Arrays.binarySearch`)
const MONSTERS: [i32; 4] = [22373, 22374, 22375, 22376];

// Items
const SEAL_COMPONENTS: [i32; 4] = [15517, 15518, 15519, 15520];
const SEAL_BREAKERS: [i32; 2] = [15515, 15516];

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `haveAllExceptThis`: every seal component but `idx` is in the inventory.
fn have_all_except_this(ctx: &ScriptCtx<'_>, player: ObjectId, idx: usize) -> bool {
    SEAL_COMPONENTS
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != idx)
        .all(|(_, &item)| ctx.has_quest_items(player, item))
}

/// Java `Q00279_TargetOfOpportunity`.
pub struct Q00279TargetOfOpportunity;

impl Script for Q00279TargetOfOpportunity {
    fn name(&self) -> &'static str {
        "Q00279_TargetOfOpportunity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00279_TargetOfOpportunity"
    }

    fn id(&self) -> i32 {
        279
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JERIAN]);
        s.add_talk_id(&[JERIAN]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&SEAL_COMPONENTS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = match ctx.get_quest_state(player, false) {
            Some(qs) if ctx.player_level(player) >= MIN_LEVEL => qs,
            _ => return Some(ctx.get_no_quest_msg(player)),
        };
        // NOTE(java-quirk): every event comes back as the text, also the `-h Script ... enter`
        // links of the pages, which Java shows as a chat line.
        let htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("32302-05.html") {
            qs.start_quest(ctx);
            qs.set(ctx, "progress", "1");
        } else if event.eq_ignore_ascii_case("32302-08.html")
            && qs.get_int(ctx, "progress") == 1
            && SEAL_COMPONENTS
                .iter()
                .all(|&item| ctx.has_quest_items(player, item))
        {
            ctx.give_items(player, SEAL_BREAKERS[0], 1);
            ctx.give_items(player, SEAL_BREAKERS[1], 1);
            qs.exit_quest(ctx, true, true);
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(pl) = ctx.get_random_party_member_var(killer, Some("progress"), "1") else {
            return;
        };
        let Ok(idx) = MONSTERS.binary_search(&npc.id) else {
            return;
        };
        let qs = ctx.get_quest_state(pl, false);
        if ctx.get_random(1000) < 311 && !ctx.has_quest_items(killer, SEAL_COMPONENTS[idx]) {
            // NOTE(java-quirk): the component goes to the killer, the cond 2 to the member drawn
            // above (Java gives to `player` and sets the cond on `pl`).
            ctx.give_items(killer, SEAL_COMPONENTS[idx], 1);
            if have_all_except_this(ctx, killer, idx) {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 2, true);
                }
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32302-01.htm"
            } else {
                "32302-02.html"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED && qs.get_int(ctx, "progress") == 1 {
            htmltext = if SEAL_COMPONENTS
                .iter()
                .all(|&item| ctx.has_quest_items(player, item))
            {
                "32302-07.html"
            } else {
                "32302-06.html"
            }
            .to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00279_TargetOfOpportunity";

    const NPCS: &str = r#"
        <npc id="32302" type="Folk" level="85" name="Jerian"/>
        <npc id="22373" type="Monster" level="80" name="Monster 22373"/>
        <npc id="22374" type="Monster" level="80" name="Monster 22374"/>
        <npc id="22375" type="Monster" level="80" name="Monster 22375"/>
        <npc id="22376" type="Monster" level="80" name="Monster 22376"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15515" type="EtcItem" name="Seal Breaker A"/>
        <item id="15516" type="EtcItem" name="Seal Breaker B"/>
        <item id="15517" type="EtcItem" name="Seal Component 1"><set name="is_questitem" val="true"/></item>
        <item id="15518" type="EtcItem" name="Seal Component 2"><set name="is_questitem" val="true"/></item>
        <item id="15519" type="EtcItem" name="Seal Component 3"><set name="is_questitem" val="true"/></item>
        <item id="15520" type="EtcItem" name="Seal Component 4"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00279TargetOfOpportunity);
        let pages = Pages::of(&Q00279TargetOfOpportunity)
            .page("32302-01.htm", &["32302-03.htm"])
            .page("32302-03.htm", &["32302-04.htm"])
            .page("32302-04.htm", &["32302-05.html"])
            .page("32302-07.html", &["32302-08.html"])
            .plain(&["32302-02.html", "32302-05.html", "32302-06.html", "32302-08.html"])
            .npcs(&[JERIAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jerian = t.spawn(JERIAN, 20);
        (t, jerian)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, jerian) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, jerian).contains("32302-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn four_components_then_the_breakers() {
        let (mut t, jerian) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, jerian).contains("32302-01.htm"));
        assert!(t.event(1, NAME, "32302-03.htm").contains("32302-03.htm"));
        assert!(t.event(1, NAME, "32302-04.htm").contains("32302-04.htm"));
        assert!(t.event(1, NAME, "32302-05.html").contains("32302-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "progress", "1"),
            ]
        );
        assert!(t.quest_page(1, jerian).contains("32302-06.html"));

        // Components drop one per kind with a chance; the last one sets cond 2.
        let mut kills = 0;
        while SEAL_COMPONENTS.iter().any(|&item| t.count(1, item) == 0) {
            kills += 1;
            assert!(kills < 3000, "no components after {kills} kills");
            let id = MONSTERS[kills % MONSTERS.len()];
            let monster = t.spawn(id, 60);
            t.kill(1, monster);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Jerian gives both breakers and ends the quest (repeatable).
        assert!(t.quest_page(1, jerian).contains("32302-07.html"));
        assert!(t.event(1, NAME, "32302-08.html").contains("32302-08.html"));
        assert_eq!(t.count(1, SEAL_BREAKERS[0]), 1);
        assert_eq!(t.count(1, SEAL_BREAKERS[1]), 1);
        for item in SEAL_COMPONENTS {
            assert_eq!(t.count(1, item), 0);
        }
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
