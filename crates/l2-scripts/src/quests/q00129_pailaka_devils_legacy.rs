//! Quest 129: Pailaka - Devil's Legacy (Survivor, Supporter, the dwarf adventurers and three bosses).
// Java: dist/game/data/scripts/quests/Q00129_PailakaDevilsLegacy/Q00129_PailakaDevilsLegacy.java
//
// Pattern: like Q00128 (the instance NPC pays only players inside the instance) plus a first-talk
// page (`add_first_talk_id`). The instance entry belongs to the instance script (`instances/`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const KAMS: i32 = 18629; // Kams (Panuka)
const ALKASO: i32 = 18631; // Alkaso (Panuka)
const LEMATAN: i32 = 18633; // Lematan
const SURVIVOR: i32 = 32498; // Devil's Isle Survivor
const SUPPORTER: i32 = 32501; // Devil's Isle Supporter
const ADVENTURER1: i32 = 32508; // Dwarf Adventurer
const ADVENTURER2: i32 = 32511; // Dwarf Adventurer

// Items
const SWORD: i32 = 13042; // Ancient Legacy Sword
const ENH_SWORD1: i32 = 13043; // Enhanced Ancient Legacy Sword
const ENH_SWORD2: i32 = 13044; // Complete Ancient Legacy Sword
const SCROLL_1: i32 = 13046; // Pailaka Weapon Upgrade Stage 1
const SCROLL_2: i32 = 13047; // Pailaka Weapon Upgrade Stage 2
const SHIELD: i32 = 13032; // Pailaka Instant Shield
const HEALING_POTION: i32 = 13033; // Quick Healing Potion
const ANTIDOTE_POTION: i32 = 13048; // Pailaka Antidote
const DIVINE_POTION: i32 = 13049; // Divine Soul
const DEFENCE_POTION: i32 = 13059; // Long-Range Defense Increasing Potion
const PAILAKA_KEY: i32 = 13150; // Pailaka All-Purpose Key
const BRACELET: i32 = 13295; // Pailaka Bracelet
const ESCAPE: i32 = 736; // Scroll of Escape

// Skills: Java `VITALITY_REPLENISHING` (`SkillHolder(5774, 2)`)
const VITALITY_REPLENISHING: (i32, i32) = (5774, 2);

// Misc
const MIN_LEVEL: i32 = 61;
const MAX_LEVEL: i32 = 67;
const EXIT_TIME: i32 = 5;

/// Java `Q00129_PailakaDevilsLegacy`.
pub struct Q00129PailakaDevilsLegacy;

impl Script for Q00129PailakaDevilsLegacy {
    fn name(&self) -> &'static str {
        "Q00129_PailakaDevilsLegacy"
    }

    fn path(&self) -> &'static str {
        "quests/Q00129_PailakaDevilsLegacy"
    }

    fn id(&self) -> i32 {
        129
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SURVIVOR]);
        s.add_first_talk_id(&[SURVIVOR, SUPPORTER, ADVENTURER1, ADVENTURER2]);
        s.add_talk_id(&[SURVIVOR, SUPPORTER, ADVENTURER1, ADVENTURER2]);
        s.add_kill_id(&[KAMS, ALKASO, LEMATAN]);
        s.register_quest_items(&[
            SWORD,
            ENH_SWORD1,
            ENH_SWORD2,
            SCROLL_1,
            SCROLL_2,
            SHIELD,
            HEALING_POTION,
            ANTIDOTE_POTION,
            DIVINE_POTION,
            DEFENCE_POTION,
            PAILAKA_KEY,
        ]);
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

        let mut htmltext = None;
        match event {
            "32498-02.htm" | "32498-03.htm" | "32498-04.htm" => htmltext = Some(event.to_owned()),
            "32498-05.htm" => {
                if !qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                    qs.start_quest(ctx);
                }
            }
            "32501-02.htm" | "32501-04.htm" => htmltext = Some(event.to_owned()),
            "32501-03.htm" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, SWORD, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, false);
        if npc.id != ADVENTURER2 || qs.is_none_or(|q| !q.is_completed(ctx)) {
            return Some(format!("{}.htm", npc.id));
        }

        Some("32511-03.htm".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            SURVIVOR => match qs.state(ctx) {
                CREATED => {
                    if ctx.player_level(player) < MIN_LEVEL {
                        htmltext = "32498-11.htm".to_owned();
                    } else if ctx.player_level(player) > MAX_LEVEL {
                        htmltext = "32498-12.htm".to_owned();
                    } else {
                        htmltext = "32498-01.htm".to_owned();
                    }
                }
                STARTED => {
                    htmltext = if qs.get_cond(ctx) > 1 {
                        "32498-08.htm"
                    } else {
                        "32498-06.htm"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = "32498-10.htm".to_owned(),
                _ => htmltext = "32498-01.htm".to_owned(),
            },
            SUPPORTER => {
                htmltext = if qs.get_cond(ctx) > 2 {
                    "32501-04.htm"
                } else {
                    "32501-01.htm"
                }
                .to_owned();
            }
            ADVENTURER1 => {
                if ctx.summon_of(player).is_some() {
                    htmltext = "32508-07.htm".to_owned();
                } else if ctx.has_quest_items(player, SWORD) {
                    if ctx.has_quest_items(player, SCROLL_1) {
                        ctx.take_items(player, SWORD, -1);
                        ctx.take_items(player, SCROLL_1, -1);
                        ctx.give_items(player, ENH_SWORD1, 1);
                        htmltext = "32508-03.htm".to_owned();
                    } else {
                        htmltext = "32508-02.htm".to_owned();
                    }
                } else if ctx.has_quest_items(player, ENH_SWORD1) {
                    if ctx.has_quest_items(player, SCROLL_2) {
                        ctx.take_items(player, ENH_SWORD1, -1);
                        ctx.take_items(player, SCROLL_2, -1);
                        ctx.give_items(player, ENH_SWORD2, 1);
                        htmltext = "32508-05.htm".to_owned();
                    } else {
                        htmltext = "32508-04.htm".to_owned();
                    }
                } else if ctx.has_quest_items(player, ENH_SWORD2) {
                    htmltext = "32508-06.htm".to_owned();
                } else {
                    htmltext = "32508-00.htm".to_owned();
                }
            }
            ADVENTURER2 => {
                if ctx.summon_of(player).is_some() {
                    htmltext = "32511-02.htm".to_owned();
                } else {
                    let instance_id = ctx.world().instance_id_of(npc.object_id);
                    qs.exit_quest(ctx, false, true);
                    // Java `inst.setDuration(EXIT_TIME * 60000)`, `setEmptyDestroyTime(0)`.
                    ctx.world_mut()
                        .set_instance_duration(instance_id, EXIT_TIME * 60_000);
                    ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
                    let inside = ctx
                        .world()
                        .instance(instance_id)
                        .is_some_and(|inst| inst.contains_player(player));
                    if inside {
                        ctx.cast_skill(
                            npc,
                            player,
                            SkillKey::new(VITALITY_REPLENISHING.0, VITALITY_REPLENISHING.1),
                        );
                        ctx.add_exp_and_sp(player, 10_800_000, 950_000);
                        ctx.reward_items(player, BRACELET, 1);
                        ctx.give_items(player, ESCAPE, 1);
                    } else {
                        htmltext = "32511-01.htm".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match npc.id {
            KAMS => {
                if ctx.has_quest_items(player, SWORD) {
                    ctx.give_items(player, SCROLL_1, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
            }
            ALKASO => {
                if ctx.has_quest_items(player, ENH_SWORD1) {
                    ctx.give_items(player, SCROLL_2, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
            }
            LEMATAN => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="18629" type="Monster" level="61" name="Kams"/>
        <npc id="18631" type="Monster" level="61" name="Alkaso"/>
        <npc id="18633" type="Monster" level="61" name="Lematan"/>
        <npc id="32498" type="Folk" level="61" name="Survivor"/>
        <npc id="32501" type="Folk" level="61" name="Supporter"/>
        <npc id="32508" type="Folk" level="61" name="Adventurer"/>
        <npc id="32511" type="Folk" level="61" name="Adventurer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13042" type="Weapon" name="Sword"><set name="is_questitem" val="true"/></item>
        <item id="13043" type="Weapon" name="Enhanced Sword"><set name="is_questitem" val="true"/></item>
        <item id="13044" type="Weapon" name="Complete Sword"><set name="is_questitem" val="true"/></item>
        <item id="13046" type="EtcItem" name="Scroll 1"><set name="is_questitem" val="true"/></item>
        <item id="13047" type="EtcItem" name="Scroll 2"><set name="is_questitem" val="true"/></item>
        <item id="13295" type="Armor" name="Bracelet"/>
        <item id="736" type="EtcItem" name="Scroll of Escape"/>"#;

    const NAME: &str = "Q00129_PailakaDevilsLegacy";

    struct Npcs {
        survivor: Oid,
        supporter: Oid,
        adventurer1: Oid,
        adventurer2: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00129PailakaDevilsLegacy);
        let pages = Pages::of(&Q00129PailakaDevilsLegacy)
            .page("32498-01.htm", &["32498-02.htm"])
            .page("32498-02.htm", &["32498-03.htm"])
            .page("32498-03.htm", &["32498-04.htm"])
            .page("32498-04.htm", &["32498-05.htm"])
            .plain(&["32498-05.htm", "32498-06.htm", "32498-08.htm", "32498-10.htm"])
            .plain(&["32498-11.htm", "32498-12.htm"])
            .page("32501-01.htm", &["32501-02.htm"])
            .page("32501-02.htm", &["32501-03.htm"])
            .plain(&["32501-03.htm", "32501-04.htm"])
            .plain(&[
                "32508-00.htm",
                "32508-02.htm",
                "32508-03.htm",
                "32508-04.htm",
                "32508-05.htm",
                "32508-06.htm",
            ])
            .plain(&["32511-01.htm", "32511-03.htm"])
            .npcs(&[SURVIVOR, SUPPORTER, ADVENTURER1, ADVENTURER2])
            .file(
                "data/scripts/quests/Q00129_PailakaDevilsLegacy/32498.htm",
                r#"<html><body>32498.htm<a action="bypass Script">Quest</a></body></html>"#,
            )
            .file(
                "data/scripts/quests/Q00129_PailakaDevilsLegacy/32501.htm",
                r#"<html><body>32501.htm<a action="bypass Script">Quest</a></body></html>"#,
            )
            .file(
                "data/scripts/quests/Q00129_PailakaDevilsLegacy/32508.htm",
                r#"<html><body>32508.htm<a action="bypass Script">Quest</a></body></html>"#,
            )
            .file(
                "data/scripts/quests/Q00129_PailakaDevilsLegacy/32511.htm",
                r#"<html><body>32511.htm<a action="bypass Script">Quest</a></body></html>"#,
            )
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            survivor: t.spawn(SURVIVOR, 20),
            supporter: t.spawn(SUPPORTER, 30),
            adventurer1: t.spawn(ADVENTURER1, 40),
            adventurer2: t.spawn(ADVENTURER2, 50),
        };
        (t, npcs)
    }

    #[test]
    fn the_survivor_gates_the_level_61_to_67() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![]);
        let p = t.quest_page(1, npcs.survivor);
        assert!(p.contains("32498-11.htm"), "{p}");
        t.enter(2, 68, vec![]);
        assert!(t.quest_page(2, npcs.survivor).contains("32498-12.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_first_talk_page_is_the_npc_id() {
        let (mut t, npcs) = setup();
        t.enter(1, 64, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(npcs.adventurer2).unwrap();
            let first = Q00129PailakaDevilsLegacy.on_first_talk(ctx, npc, Oid(1));
            assert_eq!(first.as_deref(), Some("32511.htm"));
        });
    }

    #[test]
    fn the_sword_chain_and_the_kill_drops() {
        let (mut t, npcs) = setup();
        t.enter(1, 64, vec![]);
        assert!(t.quest_page(1, npcs.survivor).contains("32498-01.htm"));
        for page in ["32498-02.htm", "32498-03.htm", "32498-04.htm", "32498-05.htm"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // The supporter hands out the sword at cond 2.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.supporter).contains("32501-01.htm"));
        assert!(t.event(1, NAME, "32501-02.htm").contains("32501-02.htm"));
        assert!(t.event(1, NAME, "32501-03.htm").contains("32501-03.htm"));
        assert_eq!(t.count(1, SWORD), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Kams drops the first scroll (only with the sword), the adventurer upgrades the sword.
        let kams = t.spawn(KAMS, 60);
        t.kill(1, kams);
        assert_eq!(t.count(1, SCROLL_1), 1);
        assert!(t.quest_page(1, npcs.adventurer1).contains("32508-03.htm"));
        assert_eq!((t.count(1, SWORD), t.count(1, ENH_SWORD1)), (0, 1));

        // Alkaso drops the second scroll (only with the enhanced sword).
        let alkaso = t.spawn(ALKASO, 60);
        t.kill(1, alkaso);
        assert!(t.quest_page(1, npcs.adventurer1).contains("32508-05.htm"));
        assert_eq!(t.count(1, ENH_SWORD2), 1);

        // Lematan sets cond 4.
        t.take_writes();
        let lematan = t.spawn(LEMATAN, 60);
        t.kill(1, lematan);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "4")]);
    }

    #[test]
    fn the_second_adventurer_outside_the_instance_only_closes_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 64, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 4, false);
        });
        t.take_writes();
        // Java reads `inst.setDuration(..)` on the instance of the NPC; the test world has none.
        assert!(t.quest_page(1, npcs.adventurer2).contains("32511-01.htm"));
        assert_eq!(t.count(1, 736), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
