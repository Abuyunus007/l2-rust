//! Quest 290: Threat Removal (Pinaps in the Town of Gludin sends level 82 players after the sel mahum).
// Java: dist/game/data/scripts/quests/Q00290_ThreatRemoval/Q00290_ThreatRemoval.java
//
// Pattern: a party collect quest that is paid by counting (`docs/design/scripts.md`, "Quest
// patterns"): the sel mahum drop identification tags with a chance per monster; Pinaps takes 400
// of them for one reward picked by a roll of ten. The quest is open to players who completed
// Q00251_NoSecrets, which this server reads through `player_quest_state`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PINAPS: i32 = 30201;

// Items
const ENCHANT_WEAPON_S: i32 = 959;
const ENCHANT_ARMOR_S: i32 = 960;
const FIRE_CRYSTAL: i32 = 9552;
const SEL_MAHUM_ID_TAG: i32 = 15714;

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00251_NoSecrets.class.getSimpleName()`: the quest that must be completed.
const NO_SECRETS: &str = "Q00251_NoSecrets";

/// Java `MOBS_TAG`: the sel mahum monsters and their drop chance out of 1000.
fn tag_chance(npc_id: i32) -> i32 {
    match npc_id {
        22775 => 932, // Sel Mahum Drill Sergeant
        22776 => 397, // Sel Mahum Training Officer
        22777 => 932, // Sel Mahum Drill Sergeant
        22778 => 932, // Sel Mahum Drill Sergeant
        22780 => 363, // Sel Mahum Recruit
        22781 => 483, // Sel Mahum Soldier
        22782 => 363, // Sel Mahum Recruit
        22783 => 352, // Sel Mahum Soldier
        22784 => 363, // Sel Mahum Recruit
        22785 => 169, // Sel Mahum Soldier
        _ => 0,
    }
}

/// Java `Q00290_ThreatRemoval`.
pub struct Q00290ThreatRemoval;

impl Script for Q00290ThreatRemoval {
    fn name(&self) -> &'static str {
        "Q00290_ThreatRemoval"
    }

    fn path(&self) -> &'static str {
        "quests/Q00290_ThreatRemoval"
    }

    fn id(&self) -> i32 {
        290
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PINAPS]);
        s.add_talk_id(&[PINAPS]);
        s.add_kill_id(&[
            22775, 22776, 22777, 22778, 22780, 22781, 22782, 22783, 22784, 22785,
        ]);
        s.register_quest_items(&[SEL_MAHUM_ID_TAG]);
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
            "30201-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30201-06.html" => {
                if qs.is_cond(ctx, 1) {
                    ctx.take_items(player, SEL_MAHUM_ID_TAG, 400);
                    match ctx.get_random(10) {
                        0 => ctx.reward_items(player, ENCHANT_WEAPON_S, 1),
                        1..=3 => ctx.reward_items(player, ENCHANT_ARMOR_S, 1),
                        4..=5 => ctx.reward_items(player, ENCHANT_ARMOR_S, 2),
                        6 => ctx.reward_items(player, ENCHANT_ARMOR_S, 3),
                        7..=8 => ctx.reward_items(player, FIRE_CRYSTAL, 1),
                        _ => ctx.reward_items(player, FIRE_CRYSTAL, 2),
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30201-07.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "exit" => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, SEL_MAHUM_ID_TAG) {
                        htmltext = Some("30201-08.html".to_owned());
                    } else {
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some("30201-09.html".to_owned());
                    }
                }
            }
            "30201-10.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): Java only checks that some party member exists (`partyMember` is not
        // used again); the tag is rewarded to the killer.
        if ctx.get_random_party_member_cond(killer, 1).is_none() {
            return;
        }
        if ctx.get_random(1000) < tag_chance(npc.id) {
            ctx.reward_items(killer, SEL_MAHUM_ID_TAG, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let completed = ctx
                    .player_quest_state(player, NO_SECRETS)
                    .is_some_and(|prev| prev.is_completed());
                htmltext = if ctx.player_level(player) >= MIN_LEVEL && completed {
                    "30201-01.htm"
                } else {
                    "30201-03.html"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = if ctx.get_quest_items_count(player, SEL_MAHUM_ID_TAG) < 400 {
                        "30201-04.html"
                    } else {
                        "30201-05.html"
                    }
                    .to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00290_ThreatRemoval";

    /// The stand-in of Q00251: an NPC of its own, and the quest completed when talked to.
    const STAND_IN_NPC: i32 = 30002;

    const NPCS: &str = r#"
        <npc id="30201" type="Folk" level="85" name="Pinaps"/>
        <npc id="30002" type="Folk" level="70" name="Stand-in"/>
        <npc id="22775" type="Monster" level="80" name="Sel Mahum Drill Sergeant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15714" type="EtcItem" name="Sel Mahum ID Tag"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="959" type="EtcItem" name="Scroll: Enchant Weapon (S)"><set name="is_stackable" val="true"/></item>
        <item id="960" type="EtcItem" name="Scroll: Enchant Armor (S)"><set name="is_stackable" val="true"/></item>
        <item id="9552" type="EtcItem" name="Fire Crystal"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00251_NoSecrets` as far as this quest needs it: its NPC completes it.
    struct NoSecrets;

    impl Script for NoSecrets {
        fn name(&self) -> &'static str {
            NO_SECRETS
        }
        fn path(&self) -> &'static str {
            "quests/Q00251_NoSecrets"
        }
        fn id(&self) -> i32 {
            251
        }
        fn setup(&self, s: &mut ScriptSetup) {
            s.add_start_npc(&[STAND_IN_NPC]);
            s.add_talk_id(&[STAND_IN_NPC]);
        }
        fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
            if let Some(qs) = ctx.get_quest_state(player, true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
            None
        }
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NoSecrets);
        registry.add(Q00290ThreatRemoval);
        let pages = Pages::of(&Q00290ThreatRemoval)
            .page("30201-01.htm", &["30201-02.html"])
            .plain(&[
                "30201-02.html",
                "30201-03.html",
                "30201-04.html",
                "30201-07.html",
                "30201-09.html",
                "30201-10.html",
            ])
            .page("30201-05.html", &["30201-06.html"])
            .page("30201-06.html", &["30201-07.html", "exit"])
            .page("30201-08.html", &["30201-07.html", "30201-10.html"])
            .npcs(&[PINAPS, STAND_IN_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let pinaps = t.spawn(PINAPS, 20);
        let stand_in = t.spawn(STAND_IN_NPC, 30);
        (t, pinaps, stand_in)
    }

    /// Kills sel mahum drill sergeants until `tags` tags are in the inventory.
    fn hunt(t: &mut ScriptTest, tags: i64) {
        for _ in 0..1000 {
            if t.count(1, SEL_MAHUM_ID_TAG) >= tags {
                return;
            }
            let sergeant = t.spawn(22775, 60);
            t.kill(1, sergeant);
        }
        panic!("no {tags} tags after 1000 kills");
    }

    #[test]
    fn without_the_no_secrets_quest_pinaps_says_no() {
        let (mut t, pinaps, _stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, pinaps).contains("30201-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn four_hundred_tags_then_a_reward_and_the_exit_with_tags_left_goes_on() {
        let (mut t, pinaps, stand_in) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        // Complete Q00251 (the stand-in) first.
        t.quest_page(1, stand_in);
        t.take_writes();

        assert!(t.quest_page(1, pinaps).contains("30201-01.htm"));
        assert!(t.event(1, NAME, "30201-02.html").contains("30201-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, pinaps).contains("30201-04.html"));

        // 401 tags: one is left after the 400 are paid.
        hunt(&mut t, 401);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, pinaps).contains("30201-05.html"));
        assert!(t.event(1, NAME, "30201-06.html").contains("30201-06.html"));
        assert_eq!(t.count(1, SEL_MAHUM_ID_TAG), 1);
        let rewards: i64 = [959, 960, 9552].iter().map(|&item| t.count(1, item)).sum();
        assert!(rewards >= 1, "{rewards}");
        assert!(t.take_writes().is_empty());

        // "exit" with a tag left: the page that asks to hand it in; then the 10th page ends it.
        assert!(t.event(1, NAME, "exit").contains("30201-08.html"));
        assert!(t.event(1, NAME, "30201-10.html").contains("30201-10.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
