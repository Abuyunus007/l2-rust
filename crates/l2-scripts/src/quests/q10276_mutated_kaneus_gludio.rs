//! Quest 10276: Mutated Kaneus - Gludio (Bathis sends the party after Tomlan Kamos and Ol Arios).
// Java: dist/game/data/scripts/quests/Q10276_MutatedKaneusGludio/Q10276_MutatedKaneusGludio.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BATHIS: i32 = 30332;
const ROHMER: i32 = 30344;

// Monsters
const TOMLAN_KAMOS: i32 = 18554;
const OL_ARIOSH: i32 = 18555;

// Items
const TISSUE_TK: i32 = 13830;
const TISSUE_OA: i32 = 13831;

/// Java `Q10276_MutatedKaneusGludio`.
pub struct Q10276MutatedKaneusGludio;

/// Java `rewardItem(npcId, player)`: one tissue of the monster, once.
fn reward_item(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if npc_id == TOMLAN_KAMOS && !ctx.has_quest_items(player, TISSUE_TK) {
        ctx.give_items(player, TISSUE_TK, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if npc_id == OL_ARIOSH && !ctx.has_quest_items(player, TISSUE_OA) {
        ctx.give_items(player, TISSUE_OA, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q10276MutatedKaneusGludio {
    fn name(&self) -> &'static str {
        "Q10276_MutatedKaneusGludio"
    }

    fn path(&self) -> &'static str {
        "quests/Q10276_MutatedKaneusGludio"
    }

    fn id(&self) -> i32 {
        10276
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BATHIS]);
        s.add_talk_id(&[BATHIS, ROHMER]);
        s.add_kill_id(&[TOMLAN_KAMOS, OL_ARIOSH]);
        s.register_quest_items(&[TISSUE_TK, TISSUE_OA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "30332-03.htm" => qs.start_quest(ctx),
            "30344-03.htm" => {
                ctx.give_adena(player, 8500, true);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        let npc_id = npc.id;
        if ctx.world().party_of(killer).is_some() {
            // A party member who still lacks this tissue and has the quest gets it; one of them
            // is drawn at random.
            let mut party_members = Vec::new();
            for member in ctx.world().party_members_of(killer) {
                if let Some(member_qs) = ctx.get_quest_state(member, false)
                    && member_qs.is_started(ctx)
                    && ((npc_id == TOMLAN_KAMOS && !ctx.has_quest_items(member, TISSUE_TK))
                        || (npc_id == OL_ARIOSH && !ctx.has_quest_items(member, TISSUE_OA)))
                {
                    party_members.push(member);
                }
            }
            if let Some(member) = ctx.get_random_entry(&party_members) {
                reward_item(ctx, npc_id, member);
            }
        } else if qs.is_started(ctx) {
            reward_item(ctx, npc_id, killer);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            BATHIS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 17 {
                        "30332-01.htm"
                    } else {
                        "30332-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_TK) && ctx.has_quest_items(player, TISSUE_OA) {
                            "30332-05.htm"
                        } else {
                            "30332-04.htm"
                        }
                        .to_owned();
                }
                COMPLETED => htmltext = "30332-06.htm".to_owned(),
                _ => {}
            },
            ROHMER => match qs.state(ctx) {
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_TK) && ctx.has_quest_items(player, TISSUE_OA) {
                            "30344-02.htm"
                        } else {
                            "30344-01.htm"
                        }
                        .to_owned();
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q10276_MutatedKaneusGludio";

    const NPCS: &str = r#"
        <npc id="30332" type="Folk" level="50" name="Bathis"/>
        <npc id="30344" type="Folk" level="50" name="Rohmer"/>
        <npc id="18554" type="Monster" level="40" name="Tomlan Kamos"/>
        <npc id="18555" type="Monster" level="40" name="Ol Arios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13830" type="EtcItem" name="Tissue of Tomlan Kamos"><set name="is_questitem" val="true"/></item>
        <item id="13831" type="EtcItem" name="Tissue of Ol Arios"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10276MutatedKaneusGludio);
        let pages = Pages::of(&Q10276MutatedKaneusGludio)
            .plain(&["30332-00.htm", "30332-04.htm", "30332-05.htm", "30332-06.htm"])
            .page("30332-01.htm", &["30332-02.htm"])
            .page("30332-02.htm", &["30332-03.htm"])
            .plain(&["30332-03.htm", "30344-01.htm", "30344-03.htm"])
            .page("30344-02.htm", &["30344-03.htm"])
            .npcs(&[BATHIS, ROHMER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let bathis = t.spawn(BATHIS, 20);
        let rohmer = t.spawn(ROHMER, 40);
        (t, bathis, rohmer)
    }

    /// Accepts the quest from Bathis (`30332-03.htm`).
    fn accept(t: &mut ScriptTest, player: i32, bathis: Oid) {
        assert!(t.quest_page(player, bathis).contains("30332-01.htm"));
        assert!(t.event(player, NAME, "30332-02.htm").contains("30332-02.htm"));
        assert!(t.event(player, NAME, "30332-03.htm").contains("30332-03.htm"));
    }

    #[test]
    fn a_character_of_level_17_is_sent_away() {
        let (mut t, bathis, _) = setup();
        t.enter(1, 17, vec![]);
        assert!(t.quest_page(1, bathis).contains("30332-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_tissues_then_the_reward_from_rohmer() {
        let (mut t, bathis, rohmer) = setup();
        t.enter(1, 18, vec![]);
        accept(&mut t, 1, bathis);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, bathis).contains("30332-04.htm"));

        // Tomlan Kamos and Ol Arios: one tissue each.
        let tomlan = t.spawn(TOMLAN_KAMOS, 60);
        t.kill(1, tomlan);
        assert_eq!(t.count(1, TISSUE_TK), 1);
        let ol_arios = t.spawn(OL_ARIOSH, 60);
        t.kill(1, ol_arios);
        assert_eq!(t.count(1, TISSUE_OA), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));

        assert!(t.quest_page(1, bathis).contains("30332-05.htm"));
        assert!(t.quest_page(1, rohmer).contains("30344-02.htm"));

        // The reward: 8500 adena and `exitQuest(false, true)`; the tissues go with the quest.
        assert!(t.event(1, NAME, "30344-03.htm").contains("30344-03.htm"));
        assert_eq!(t.count(1, 57), 8500);
        assert_eq!((t.count(1, TISSUE_TK), t.count(1, TISSUE_OA)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, bathis).contains("30332-06.htm"));
        assert!(
            t.quest_page(1, rohmer)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, bathis, _) = setup();
        t.enter(1, 20, vec![]);
        t.enter(2, 20, vec![]);
        accept(&mut t, 1, bathis);
        accept(&mut t, 2, bathis);
        t.party(1, 2);

        let tomlan = t.spawn(TOMLAN_KAMOS, 60);
        t.kill(1, tomlan);
        assert_eq!(t.count(1, TISSUE_TK) + t.count(2, TISSUE_TK), 1);
    }
}
