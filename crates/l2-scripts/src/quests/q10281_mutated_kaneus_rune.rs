//! Quest 10281: Mutated Kaneus - Rune (Mathias sends the party after White Allosce).
// Java: dist/game/data/scripts/quests/Q10281_MutatedKaneusRune/Q10281_MutatedKaneusRune.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MATHIAS: i32 = 31340;
const KAYAN: i32 = 31335;

// Monster
const WHITE_ALLOSCE: i32 = 18577;

// Item
const TISSUE_WA: i32 = 13840;

/// Java `Q10281_MutatedKaneusRune`.
pub struct Q10281MutatedKaneusRune;

/// Java `rewardItem(player)`: the tissue, and the quest item sound.
fn reward_item(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.give_items(player, TISSUE_WA, 1);
    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
}

impl Script for Q10281MutatedKaneusRune {
    fn name(&self) -> &'static str {
        "Q10281_MutatedKaneusRune"
    }

    fn path(&self) -> &'static str {
        "quests/Q10281_MutatedKaneusRune"
    }

    fn id(&self) -> i32 {
        10281
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MATHIAS]);
        s.add_talk_id(&[MATHIAS, KAYAN]);
        s.add_kill_id(&[WHITE_ALLOSCE]);
        s.register_quest_items(&[TISSUE_WA]);
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
            "31340-03.htm" => qs.start_quest(ctx),
            "31335-03.htm" => {
                ctx.give_adena(player, 360_000, true);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        if ctx.world().party_of(killer).is_some() {
            // A party member who has the quest and no tissue yet; one of them is drawn at random.
            let mut party_members = Vec::new();
            for member in ctx.world().party_members_of(killer) {
                if let Some(member_qs) = ctx.get_quest_state(member, false)
                    && member_qs.is_started(ctx)
                    && !ctx.has_quest_items(member, TISSUE_WA)
                {
                    party_members.push(member);
                }
            }
            if let Some(member) = ctx.get_random_entry(&party_members) {
                reward_item(ctx, member);
            }
        } else if qs.is_started(ctx) && !ctx.has_quest_items(killer, TISSUE_WA) {
            reward_item(ctx, killer);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MATHIAS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 67 {
                        "31340-01.htm"
                    } else {
                        "31340-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext = if ctx.has_quest_items(player, TISSUE_WA) {
                        "31340-05.htm"
                    } else {
                        "31340-04.htm"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = "31340-06.htm".to_owned(),
                _ => {}
            },
            KAYAN => match qs.state(ctx) {
                STARTED => {
                    htmltext = if ctx.has_quest_items(player, TISSUE_WA) {
                        "31335-02.htm"
                    } else {
                        "31335-01.htm"
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10281_MutatedKaneusRune";

    const NPCS: &str = r#"
        <npc id="31340" type="Folk" level="70" name="Mathias"/>
        <npc id="31335" type="Folk" level="70" name="Kayan"/>
        <npc id="18577" type="Monster" level="60" name="White Allosce"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13840" type="EtcItem" name="Tissue of White Allosce"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10281MutatedKaneusRune);
        let pages = Pages::of(&Q10281MutatedKaneusRune)
            .plain(&["31340-00.htm", "31340-04.htm", "31340-05.htm", "31340-06.htm"])
            .page("31340-01.htm", &["31340-02.htm"])
            .page("31340-02.htm", &["31340-03.htm"])
            .plain(&["31340-03.htm", "31335-01.htm", "31335-03.htm"])
            .page("31335-02.htm", &["31335-03.htm"])
            .npcs(&[MATHIAS, KAYAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mathias = t.spawn(MATHIAS, 20);
        let kayan = t.spawn(KAYAN, 40);
        (t, mathias, kayan)
    }

    /// Accepts the quest from Mathias (`31340-03.htm`).
    fn accept(t: &mut ScriptTest, player: i32, mathias: Oid) {
        assert!(t.quest_page(player, mathias).contains("31340-01.htm"));
        assert!(t.event(player, NAME, "31340-02.htm").contains("31340-02.htm"));
        assert!(t.event(player, NAME, "31340-03.htm").contains("31340-03.htm"));
    }

    #[test]
    fn a_character_of_level_67_is_sent_away() {
        let (mut t, mathias, _) = setup();
        t.enter(1, 67, vec![]);
        assert!(t.quest_page(1, mathias).contains("31340-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_tissue_then_the_reward_from_kayan() {
        let (mut t, mathias, kayan) = setup();
        t.enter(1, 68, vec![]);
        accept(&mut t, 1, mathias);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, mathias).contains("31340-04.htm"));

        let allosce = t.spawn(WHITE_ALLOSCE, 60);
        t.kill(1, allosce);
        assert_eq!(t.count(1, TISSUE_WA), 1);
        assert!(t.quest_page(1, mathias).contains("31340-05.htm"));
        assert!(t.quest_page(1, kayan).contains("31335-02.htm"));

        // The reward: 360000 adena and `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "31335-03.htm").contains("31335-03.htm"));
        assert_eq!(t.count(1, 57), 360_000);
        assert_eq!(t.count(1, TISSUE_WA), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, mathias).contains("31340-06.htm"));
        assert!(
            t.quest_page(1, kayan)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, mathias, _) = setup();
        t.enter(1, 70, vec![]);
        t.enter(2, 70, vec![]);
        accept(&mut t, 1, mathias);
        accept(&mut t, 2, mathias);
        t.party(1, 2);

        let allosce = t.spawn(WHITE_ALLOSCE, 60);
        t.kill(1, allosce);
        assert_eq!(t.count(1, TISSUE_WA) + t.count(2, TISSUE_WA), 1);
    }
}
