//! Quest 10277: Mutated Kaneus - Dion (Lukas sends the party after Crimson Hatu and Seer Flouros).
// Java: dist/game/data/scripts/quests/Q10277_MutatedKaneusDion/Q10277_MutatedKaneusDion.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LUKAS: i32 = 30071;
const MIRIEN: i32 = 30461;

// Monsters
const CRIMSON_HATU: i32 = 18558;
const SEER_FLOUROS: i32 = 18559;

// Items
const TISSUE_CH: i32 = 13832;
const TISSUE_SF: i32 = 13833;

/// Java `Q10277_MutatedKaneusDion`.
pub struct Q10277MutatedKaneusDion;

/// Java `rewardItem(npcId, player)`: one tissue of the monster, once.
fn reward_item(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if npc_id == CRIMSON_HATU && !ctx.has_quest_items(player, TISSUE_CH) {
        ctx.give_items(player, TISSUE_CH, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if npc_id == SEER_FLOUROS && !ctx.has_quest_items(player, TISSUE_SF) {
        ctx.give_items(player, TISSUE_SF, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q10277MutatedKaneusDion {
    fn name(&self) -> &'static str {
        "Q10277_MutatedKaneusDion"
    }

    fn path(&self) -> &'static str {
        "quests/Q10277_MutatedKaneusDion"
    }

    fn id(&self) -> i32 {
        10277
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LUKAS]);
        s.add_talk_id(&[LUKAS, MIRIEN]);
        s.add_kill_id(&[CRIMSON_HATU, SEER_FLOUROS]);
        s.register_quest_items(&[TISSUE_CH, TISSUE_SF]);
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
            "30071-03.html" => qs.start_quest(ctx),
            "30461-03.html" => {
                ctx.give_adena(player, 20000, true);
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
                    && ((npc_id == CRIMSON_HATU && !ctx.has_quest_items(member, TISSUE_CH))
                        || (npc_id == SEER_FLOUROS && !ctx.has_quest_items(member, TISSUE_SF)))
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
            LUKAS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 27 {
                        "30071-01.htm"
                    } else {
                        "30071-00.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_CH) && ctx.has_quest_items(player, TISSUE_SF) {
                            "30071-05.html"
                        } else {
                            "30071-04.html"
                        }
                        .to_owned();
                }
                COMPLETED => htmltext = "30071-06.html".to_owned(),
                _ => {}
            },
            MIRIEN => match qs.state(ctx) {
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_CH) && ctx.has_quest_items(player, TISSUE_SF) {
                            "30461-02.html"
                        } else {
                            "30461-01.html"
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

    const NAME: &str = "Q10277_MutatedKaneusDion";

    const NPCS: &str = r#"
        <npc id="30071" type="Folk" level="50" name="Lukas"/>
        <npc id="30461" type="Folk" level="50" name="Mirien"/>
        <npc id="18558" type="Monster" level="40" name="Crimson Hatu"/>
        <npc id="18559" type="Monster" level="40" name="Seer Flouros"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13832" type="EtcItem" name="Tissue of Crimson Hatu"><set name="is_questitem" val="true"/></item>
        <item id="13833" type="EtcItem" name="Tissue of Seer Flouros"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10277MutatedKaneusDion);
        let pages = Pages::of(&Q10277MutatedKaneusDion)
            .plain(&["30071-00.html", "30071-04.html", "30071-05.html", "30071-06.html"])
            .page("30071-01.htm", &["30071-02.htm"])
            .page("30071-02.htm", &["30071-03.html"])
            .plain(&["30071-03.html", "30461-01.html", "30461-03.html"])
            .page("30461-02.html", &["30461-03.html"])
            .npcs(&[LUKAS, MIRIEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lukas = t.spawn(LUKAS, 20);
        let mirien = t.spawn(MIRIEN, 40);
        (t, lukas, mirien)
    }

    /// Accepts the quest from Lukas (`30071-03.html`).
    fn accept(t: &mut ScriptTest, player: i32, lukas: Oid) {
        assert!(t.quest_page(player, lukas).contains("30071-01.htm"));
        assert!(t.event(player, NAME, "30071-02.htm").contains("30071-02.htm"));
        assert!(t.event(player, NAME, "30071-03.html").contains("30071-03.html"));
    }

    #[test]
    fn a_character_of_level_27_is_sent_away() {
        let (mut t, lukas, _) = setup();
        t.enter(1, 27, vec![]);
        assert!(t.quest_page(1, lukas).contains("30071-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_tissues_then_the_reward_from_mirien() {
        let (mut t, lukas, mirien) = setup();
        t.enter(1, 28, vec![]);
        accept(&mut t, 1, lukas);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, lukas).contains("30071-04.html"));

        // Crimson Hatu and Seer Flouros: one tissue each.
        let hatu = t.spawn(CRIMSON_HATU, 60);
        t.kill(1, hatu);
        assert_eq!(t.count(1, TISSUE_CH), 1);
        let flouros = t.spawn(SEER_FLOUROS, 60);
        t.kill(1, flouros);
        assert_eq!(t.count(1, TISSUE_SF), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));

        assert!(t.quest_page(1, lukas).contains("30071-05.html"));
        assert!(t.quest_page(1, mirien).contains("30461-02.html"));

        // The reward: 20000 adena and `exitQuest(false, true)`; the tissues go with the quest.
        assert!(t.event(1, NAME, "30461-03.html").contains("30461-03.html"));
        assert_eq!(t.count(1, 57), 20000);
        assert_eq!((t.count(1, TISSUE_CH), t.count(1, TISSUE_SF)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, lukas).contains("30071-06.html"));
        assert!(
            t.quest_page(1, mirien)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, lukas, _) = setup();
        t.enter(1, 30, vec![]);
        t.enter(2, 30, vec![]);
        accept(&mut t, 1, lukas);
        accept(&mut t, 2, lukas);
        t.party(1, 2);

        let hatu = t.spawn(CRIMSON_HATU, 60);
        t.kill(1, hatu);
        assert_eq!(t.count(1, TISSUE_CH) + t.count(2, TISSUE_CH), 1);
    }
}
