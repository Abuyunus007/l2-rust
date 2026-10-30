//! Quest 902: Reclaim Our Era (Mathias sends the party after the Varka, Ketra, Stakato and Anais bosses).
// Java: dist/game/data/scripts/quests/Q00902_ReclaimOurEra/Q00902_ReclaimOurEra.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MATHIAS: i32 = 31340;

// Misc
const MIN_LEVEL: i32 = 80;

// Items
const SHATTERED_BONES: i32 = 21997;
const CANNIBALISTIC_STAKATO_LDR_CLAW: i32 = 21998;
const ANAIS_SCROLL: i32 = 21999;
const PROOF_OF_CHALLENGE: i32 = 21750;

/// Java `MONSTER_DROPS`: the boss and the item it drops for the quest.
const MONSTER_DROPS: [(i32, i32); 11] = [
    (25309, SHATTERED_BONES),                // Varka's Hero Shadith
    (25312, SHATTERED_BONES),                // Varka's Commander Mos
    (25315, SHATTERED_BONES),                // Varka's Chief Horus
    (25299, SHATTERED_BONES),                // Ketra's Hero Hekaton
    (25302, SHATTERED_BONES),                // Ketra's Commander Tayr
    (25305, SHATTERED_BONES),                // Ketra's Chief Brakki
    (25667, CANNIBALISTIC_STAKATO_LDR_CLAW), // Cannibalistic Stakato Chief
    (25668, CANNIBALISTIC_STAKATO_LDR_CLAW), // Cannibalistic Stakato Chief
    (25669, CANNIBALISTIC_STAKATO_LDR_CLAW), // Cannibalistic Stakato Chief
    (25670, CANNIBALISTIC_STAKATO_LDR_CLAW), // Cannibalistic Stakato Chief
    (25701, ANAIS_SCROLL),                   // Anais - Master of Splendor
];

/// Java `giveItem(npc, player)`: the boss's item, once, and cond 5 (for a started member in range).
fn give_item(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    let Some(qs) = ctx.get_quest_state(player, false) else {
        return;
    };
    if qs.is_started(ctx)
        && !qs.is_cond(ctx, 5)
        && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
    {
        let Some(item_id) = MONSTER_DROPS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map(|(_, item)| *item)
        else {
            return;
        };
        ctx.give_items(player, item_id, 1);
        qs.set_cond(ctx, 5, true);
    }
}

/// Java `Q00902_ReclaimOurEra`.
pub struct Q00902ReclaimOurEra;

impl Script for Q00902ReclaimOurEra {
    fn name(&self) -> &'static str {
        "Q00902_ReclaimOurEra"
    }

    fn path(&self) -> &'static str {
        "quests/Q00902_ReclaimOurEra"
    }

    fn id(&self) -> i32 {
        902
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MATHIAS]);
        s.add_talk_id(&[MATHIAS]);
        let bosses: Vec<i32> = MONSTER_DROPS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&bosses);
        s.register_quest_items(&[SHATTERED_BONES, CANNIBALISTIC_STAKATO_LDR_CLAW, ANAIS_SCROLL]);
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
            "31340-04.htm" => {
                if qs.is_created(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31340-05.html" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31340-06.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31340-07.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31340-08.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31340-10.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.world().party_of(killer).is_some() {
            for member in ctx.world().party_members_of(killer) {
                give_item(ctx, npc, member);
            }
        } else {
            give_item(ctx, npc, killer);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        let mut state = qs.state(ctx);
        if state == COMPLETED {
            if !qs.is_now_available(ctx) {
                return Some("31340-02.htm".to_owned());
            }
            qs.set_state(ctx, CREATED);
            state = CREATED;
        }
        match state {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31340-01.htm"
                } else {
                    "31340-03.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "31340-09.html".to_owned(),
                2 => htmltext = "31340-11.html".to_owned(),
                3 => htmltext = "31340-12.html".to_owned(),
                4 => htmltext = "31340-13.html".to_owned(),
                5 => {
                    if ctx.has_quest_items(player, SHATTERED_BONES) {
                        ctx.give_items(player, PROOF_OF_CHALLENGE, 1);
                        ctx.give_adena(player, 134_038, true);
                    } else if ctx.has_quest_items(player, CANNIBALISTIC_STAKATO_LDR_CLAW) {
                        ctx.give_items(player, PROOF_OF_CHALLENGE, 3);
                        ctx.give_adena(player, 210_119, true);
                    } else if ctx.has_quest_items(player, ANAIS_SCROLL) {
                        ctx.give_items(player, PROOF_OF_CHALLENGE, 3);
                        ctx.give_adena(player, 348_155, true);
                    }
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "31340-14.html".to_owned();
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00902_ReclaimOurEra";

    const NPCS: &str = r#"
        <npc id="31340" type="Folk" level="80" name="Mathias"/>
        <npc id="25309" type="Monster" level="80" name="Varka's Hero Shadith"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="21997" type="EtcItem" name="Shattered Bones"><set name="is_questitem" val="true"/></item>
        <item id="21750" type="EtcItem" name="Proof of Challenge"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00902ReclaimOurEra);
        let pages = Pages::of(&Q00902ReclaimOurEra)
            .page("31340-01.htm", &["31340-04.htm"])
            .page("31340-04.htm", &["31340-05.html"])
            .page("31340-05.html", &["31340-10.html"])
            .page(
                "31340-09.html",
                &["31340-06.html", "31340-07.html", "31340-08.html"],
            )
            .plain(&["31340-02.htm", "31340-03.htm", "31340-06.html", "31340-07.html"])
            .plain(&["31340-08.html", "31340-10.html", "31340-11.html", "31340-12.html"])
            .plain(&["31340-13.html", "31340-14.html"])
            .npcs(&[MATHIAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mathias = t.spawn(MATHIAS, 20);
        (t, mathias)
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, mathias) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, mathias).contains("31340-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_boss_gives_the_bones_then_the_proof_and_the_adena_once_a_day() {
        let (mut t, mathias) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, mathias).contains("31340-01.htm"));
        assert!(t.event(1, NAME, "31340-04.htm").contains("31340-04.htm"));
        assert!(t.event(1, NAME, "31340-05.html").contains("31340-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // Cond 3 from the page 07 (the middle of the quest); the boss still counts. Cond 1 to 3
        // skips step 2, so `setCond` keeps the skipped step in `__compltdStateFlags` (as Java).
        assert!(t.quest_page(1, mathias).contains("31340-09.html"));
        assert!(t.event(1, NAME, "31340-07.html").contains("31340-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643"),
            ]
        );

        // Varka's Hero Shadith: the bones, cond 5.
        let shadith = t.spawn(25309, 60);
        t.kill(1, shadith);
        assert_eq!(t.count(1, SHATTERED_BONES), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "5"),
                rows::update(NAME, "__compltdStateFlags", "-2147483627"),
            ]
        );

        // The reward: the proof and 134038 adena, `exitQuest(DAILY, true)`.
        assert!(t.quest_page(1, mathias).contains("31340-14.html"));
        assert_eq!(t.count(1, PROOF_OF_CHALLENGE), 1);
        assert_eq!(t.count(1, 57), 134_038);
        assert_eq!(t.count(1, SHATTERED_BONES), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, mathias).contains("31340-02.htm"));
    }
}
