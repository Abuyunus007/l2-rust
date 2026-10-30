//! Quest 154: Sacrifice to the Sea (Rockswell, Cristel and Rollfnan, ten fox furs for an earring).
// Java: dist/game/data/scripts/quests/Q00154_SacrificeToTheSea/Q00154_SacrificeToTheSea.java
//
// Pattern: a delivery chain whose first step is a party kill count (`getRandomPartyMemberState`).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROCKSWELL: i32 = 30312;
const CRISTEL: i32 = 30051;
const ROLLFNAN: i32 = 30055;

// Items
const FOX_FUR: i32 = 1032;
const FOX_FUR_YAM: i32 = 1033;
const MAIDEN_DOLL: i32 = 1034;

// Monsters
const ELDER_KELTIR: i32 = 20544;
const YOUNG_KELTIR: i32 = 20545;
const KELTIR: i32 = 20481;

// Reward
const MAGE_EARING: i32 = 113;

// Misc
const MIN_LEVEL: i32 = 2;

/// Java `Q00154_SacrificeToTheSea`.
pub struct Q00154SacrificeToTheSea;

impl Script for Q00154SacrificeToTheSea {
    fn name(&self) -> &'static str {
        "Q00154_SacrificeToTheSea"
    }

    fn path(&self) -> &'static str {
        "quests/Q00154_SacrificeToTheSea"
    }

    fn id(&self) -> i32 {
        154
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROCKSWELL]);
        s.add_talk_id(&[ROCKSWELL, CRISTEL, ROLLFNAN]);
        s.add_kill_id(&[ELDER_KELTIR, YOUNG_KELTIR, KELTIR]);
        s.register_quest_items(&[FOX_FUR, FOX_FUR_YAM, MAIDEN_DOLL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30312-03.htm"
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);

        match npc.id {
            ROCKSWELL => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30312-01.htm".to_owned()
                    } else {
                        "30312-02.htm".to_owned()
                    };
                } else if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30312-04.html".to_owned(),
                        2 => htmltext = "30312-07.html".to_owned(),
                        3 => htmltext = "30312-05.html".to_owned(),
                        4 => {
                            ctx.take_items(talker, MAIDEN_DOLL, -1);
                            ctx.reward_items(talker, MAGE_EARING, 1);
                            ctx.add_exp_and_sp(talker, 0, 1_000);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30312-06.html".to_owned();
                        }
                        _ => {}
                    }
                } else {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            CRISTEL => match qs.get_cond(ctx) {
                1 => htmltext = "30051-02.html".to_owned(),
                2 => {
                    ctx.take_items(talker, FOX_FUR, -1);
                    ctx.give_items(talker, FOX_FUR_YAM, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = "30051-01.html".to_owned();
                }
                3 => htmltext = "30051-03.html".to_owned(),
                4 => htmltext = "30051-04.html".to_owned(),
                _ => {}
            },
            ROLLFNAN => match qs.get_cond(ctx) {
                1 | 2 => htmltext = "30055-03.html".to_owned(),
                3 => {
                    ctx.take_items(talker, FOX_FUR_YAM, -1);
                    ctx.give_items(talker, MAIDEN_DOLL, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = "30055-01.html".to_owned();
                }
                4 => htmltext = "30055-02.html".to_owned(),
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, 1, 3, Some(npc));
        if let Some(qs) = qs
            && ctx.give_item_randomly(qs.player(), Some(npc), FOX_FUR, 1, 1, 10, 0.3, true)
        {
            qs.set_cond(ctx, 2, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00154_SacrificeToTheSea";

    const NPCS: &str = r#"
        <npc id="30312" type="Folk" level="1" name="Rockswell"/>
        <npc id="30051" type="Folk" level="1" name="Cristel"/>
        <npc id="30055" type="Folk" level="1" name="Rollfnan"/>
        <npc id="20481" type="Monster" level="5" name="Keltir"/>"#;

    const ITEMS: &str = r#"
        <item id="113" type="Armor" name="Mage Earing"/>
        <item id="1032" type="EtcItem" name="Fox Fur"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1033" type="EtcItem" name="Fox Fur Yarn"><set name="is_questitem" val="true"/></item>
        <item id="1034" type="EtcItem" name="Maiden Doll"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        rockswell: Oid,
        cristel: Oid,
        rollfnan: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00154SacrificeToTheSea);
        let pages = Pages::of(&Q00154SacrificeToTheSea)
            .page("30312-01.htm", &["30312-03.htm"])
            .plain(&[
                "30312-02.htm",
                "30312-03.htm",
                "30312-04.html",
                "30312-05.html",
                "30312-06.html",
                "30312-07.html",
                "30051-01.html",
                "30051-02.html",
                "30051-03.html",
                "30051-04.html",
                "30055-01.html",
                "30055-02.html",
                "30055-03.html",
            ])
            .npcs(&[ROCKSWELL, CRISTEL, ROLLFNAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            rockswell: t.spawn(ROCKSWELL, 20),
            cristel: t.spawn(CRISTEL, 30),
            rollfnan: t.spawn(ROLLFNAN, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_level_1_character_does_not_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 1, vec![]);
        assert!(t.quest_page(1, npcs.rockswell).contains("30312-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn furs_yarn_doll_and_the_earring() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, npcs.rockswell).contains("30312-01.htm"));
        assert!(t.event(1, NAME, "30312-03.htm").contains("30312-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.rockswell).contains("30312-04.html"));
        assert!(t.quest_page(1, npcs.cristel).contains("30051-02.html"));
        assert!(t.quest_page(1, npcs.rollfnan).contains("30055-03.html"));

        // Ten furs (30 %), `setCond(2)` without a sound.
        let mut kills = 0;
        while t.count(1, FOX_FUR) < 10 {
            let keltir = t.spawn(KELTIR, 60);
            t.kill(1, keltir);
            kills += 1;
            assert!(kills < 500, "the keltirs never dropped 10 furs");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.rockswell).contains("30312-07.html"));

        assert!(t.quest_page(1, npcs.cristel).contains("30051-01.html"));
        assert_eq!(t.count(1, FOX_FUR), 0);
        assert_eq!(t.count(1, FOX_FUR_YAM), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.rockswell).contains("30312-05.html"));

        assert!(t.quest_page(1, npcs.rollfnan).contains("30055-01.html"));
        assert_eq!(t.count(1, MAIDEN_DOLL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.cristel).contains("30051-04.html"));

        assert!(t.quest_page(1, npcs.rockswell).contains("30312-06.html"));
        assert_eq!(t.count(1, MAGE_EARING), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
