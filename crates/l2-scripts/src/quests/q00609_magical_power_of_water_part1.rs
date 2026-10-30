//! Quest 609: Magical Power of Water, Part 1 (Wahkan, Asefa and Udan's Box; the Varka monsters).
// Java: dist/game/data/scripts/quests/Q00609_MagicalPowerOfWaterPart1/Q00609_MagicalPowerOfWaterPart1.java
//
// Pattern: a monster-triggered quest. Attacking the Varka monsters at cond 2 once calls Udan's
// eye (a spawn at the monster that casts the gaze on the attacker and despawns after 10 s); the
// Asefa's spell dispels it. The box gives the stolen green totem.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WAHKAN: i32 = 31371;
const ASEFA: i32 = 31372;
const UDANS_BOX: i32 = 31561;
const UDANS_EYE: i32 = 31684;

// Monsters
const VARKA_MOBS: [i32; 21] = [
    21350, // Varka Silenos Recruit
    21351, // Varka Silenos Footman
    21353, // Varka Silenos Scout
    21354, // Varka Silenos Hunter
    21355, // Varka Silenos Shaman
    21357, // Varka Silenos Priest
    21358, // Varka Silenos Warrior
    21360, // Varka Silenos Medium
    21361, // Varka Silenos Magus
    21362, // Varka Silenos Officer
    21364, // Varka Silenos Seer
    21365, // Varka Silenos Great Magus
    21366, // Varka Silenos General
    21368, // Varka Silenos Great Seer
    21369, // Varka's Commander
    21370, // Varka's Elite Guard
    21371, // Varka's Head Magus
    21372, // Varka's Head Guard
    21373, // Varka's Prophet
    21374, // Prophet's Guard
    21375, // Disciple of Prophet
];

// Items
const KEY: i32 = 1661;
const STOLEN_GREEN_TOTEM: i32 = 7237;
const WISDOM_STONE: i32 = 7081;
const GREEN_TOTEM: i32 = 7238;
const KETRA_MARKS: [i32; 5] = [7211, 7212, 7213, 7214, 7215];

// Skills: (id, level). Gaze of Watcher, and its dispel.
const GOW: SkillKey = SkillKey { id: 4547, level: 1 };
const DISPEL_GOW: SkillKey = SkillKey { id: 4548, level: 1 };

// Misc
const MIN_LEVEL: i32 = 74;
/// Java `startQuestTimer("eye_despawn", 10000, ...)`.
const EYE_DESPAWN_MS: i64 = 10_000;
/// The variable that says the eye is out (Java `qs.isSet("spawned")`).
const SPAWNED: &str = "spawned";

/// Java `Q00609_MagicalPowerOfWaterPart1`.
pub struct Q00609MagicalPowerOfWaterPart1;

impl Script for Q00609MagicalPowerOfWaterPart1 {
    fn name(&self) -> &'static str {
        "Q00609_MagicalPowerOfWaterPart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00609_MagicalPowerOfWaterPart1"
    }

    fn id(&self) -> i32 {
        609
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAHKAN]);
        s.add_talk_id(&[ASEFA, WAHKAN, UDANS_BOX]);
        s.add_attack_id(&VARKA_MOBS);
        s.register_quest_items(&[STOLEN_GREEN_TOTEM]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31371-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "open_box" => {
                if !ctx.has_quest_items(player, KEY) {
                    htmltext = Some("31561-02.html".to_owned());
                } else if qs.is_cond(ctx, 2) {
                    if qs.is_set(ctx, SPAWNED) {
                        ctx.take_items(player, KEY, 1);
                        htmltext = Some("31561-04.html".to_owned());
                    } else {
                        ctx.give_items(player, STOLEN_GREEN_TOTEM, 1);
                        ctx.take_items(player, KEY, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = Some("31561-03.html".to_owned());
                    }
                }
            }
            "eye_despawn" => {
                // The timer's NPC is the eye (Java never checks it).
                if let Some(npc) = npc {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::UdanHasAlreadySeenYourFace,
                        &[],
                    );
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if let Some(qs) = ctx.get_quest_state(attacker, false)
            && qs.is_cond(ctx, 2)
            && !qs.is_set(ctx, SPAWNED)
        {
            qs.set(ctx, SPAWNED, "1");
            ctx.cast_skill(npc, attacker, GOW);
            if let Some(eye) = ctx.add_spawn_at(UDANS_EYE, npc.object_id, false, 0) {
                ctx.broadcast_npc_say(
                    eye,
                    ChatType::NpcGeneral,
                    NpcStringId::YouCanTAvoidTheEyesOfUdan,
                    &[],
                );
                ctx.start_quest_timer("eye_despawn", EYE_DESPAWN_MS, Some(eye), Some(attacker), false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            WAHKAN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_at_least_one_quest_item(player, &KETRA_MARKS) {
                            "31371-01.htm"
                        } else {
                            "31371-00a.html"
                        }
                    } else {
                        "31371-00b.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31371-03.html".to_owned();
                    }
                }
                _ => {}
            },
            ASEFA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => {
                            htmltext = "31372-01.html".to_owned();
                            qs.set_cond(ctx, 2, true);
                        }
                        2 => {
                            if qs.is_set(ctx, SPAWNED) {
                                qs.unset(ctx, SPAWNED);
                                ctx.cast_skill(npc, player, DISPEL_GOW);
                                htmltext = "31372-03.html".to_owned();
                            } else {
                                htmltext = "31372-02.html".to_owned();
                            }
                        }
                        3 => {
                            ctx.give_items(player, GREEN_TOTEM, 1);
                            ctx.give_items(player, WISDOM_STONE, 1);
                            qs.exit_quest(ctx, true, true);
                            htmltext = "31372-04.html".to_owned();
                        }
                        _ => {}
                    }
                }
            }
            UDANS_BOX => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "31561-01.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00609_MagicalPowerOfWaterPart1";

    const NPCS: &str = r#"
        <npc id="31371" type="Folk" level="70" name="Wahkan"/>
        <npc id="31372" type="Folk" level="70" name="Asefa"/>
        <npc id="31561" type="Folk" level="1" name="Udan's Box"/>
        <npc id="31684" type="Monster" level="1" name="Udan's Eye"/>
        <npc id="21373" type="Monster" level="70" name="Varka's Prophet"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1661" type="EtcItem" name="Key"><set name="is_stackable" val="true"/></item>
        <item id="7237" type="EtcItem" name="Stolen Green Totem"><set name="is_questitem" val="true"/></item>
        <item id="7081" type="EtcItem" name="Wisdom Stone"/>
        <item id="7238" type="EtcItem" name="Green Totem"/>
        <item id="7211" type="EtcItem" name="Mark of Ketra's Alliance"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00609MagicalPowerOfWaterPart1);
        let pages = Pages::of(&Q00609MagicalPowerOfWaterPart1)
            .page("31371-01.htm", &["31371-02.html"])
            .plain(&[
                "31371-00a.html",
                "31371-00b.html",
                "31371-02.html",
                "31371-03.html",
            ])
            .plain(&["31372-01.html", "31372-02.html", "31372-03.html", "31372-04.html"])
            .page("31561-01.html", &["open_box"])
            .plain(&["31561-02.html", "31561-03.html", "31561-04.html"])
            .npcs(&[WAHKAN, ASEFA, UDANS_BOX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wahkan = t.spawn(WAHKAN, 20);
        let asefa = t.spawn(ASEFA, 30);
        let chest = t.spawn(UDANS_BOX, 40);
        (t, wahkan, asefa, chest)
    }

    #[test]
    fn a_character_without_a_ketra_mark_is_sent_away() {
        let (mut t, wahkan, _asefa, _chest) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, wahkan).contains("31371-00a.html"));
        t.enter(2, 73, vec![item(200, 7211, 1)]);
        assert!(t.quest_page(2, wahkan).contains("31371-00b.html"));
    }

    #[test]
    fn the_totem_chain_from_wahkan_to_asefa() {
        let (mut t, wahkan, asefa, chest) = setup();
        t.enter(1, 74, vec![item(100, 7211, 1), item(101, KEY, 1)]);
        assert!(t.quest_page(1, wahkan).contains("31371-01.htm"));
        assert!(t.event(1, NAME, "31371-02.html").contains("31371-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, wahkan).contains("31371-03.html"));

        // Asefa at cond 1: the spell is for later, cond 2.
        assert!(t.quest_page(1, asefa).contains("31372-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The chest with the key: the green totem, cond 3.
        assert!(t.quest_page(1, chest).contains("31561-01.html"));
        assert!(t.event(1, NAME, "open_box").contains("31561-03.html"));
        assert_eq!((t.count(1, STOLEN_GREEN_TOTEM), t.count(1, KEY)), (1, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Asefa at cond 3: the green totem and the wisdom stone; the quest ends repeatable.
        assert!(t.quest_page(1, asefa).contains("31372-04.html"));
        assert_eq!((t.count(1, GREEN_TOTEM), t.count(1, WISDOM_STONE)), (1, 1));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_box_without_a_key_says_so() {
        let (mut t, wahkan, asefa, chest) = setup();
        t.enter(1, 74, vec![item(100, 7211, 1)]);
        t.quest_page(1, wahkan);
        t.event(1, NAME, "31371-02.html");
        t.take_writes();
        t.quest_page(1, asefa);
        t.take_writes();
        assert!(t.quest_page(1, chest).contains("31561-01.html"));
        assert!(t.event(1, NAME, "open_box").contains("31561-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_eye_comes_once_and_leaves_after_ten_seconds() {
        let (mut t, wahkan, asefa, _chest) = setup();
        t.enter(1, 74, vec![item(100, 7211, 1)]);
        t.quest_page(1, wahkan);
        t.event(1, NAME, "31371-02.html");
        t.quest_page(1, asefa);
        t.take_writes();

        // A hit at cond 2 sets `spawned` (a plain hit, no skill), the eye appears at the monster.
        let mob = t.spawn(21373, 60);
        t.world.run_as_script(NAME, |ctx| {
            Q00609MagicalPowerOfWaterPart1.on_attack(
                ctx,
                ScriptNpc {
                    object_id: mob,
                    id: 21373,
                },
                Oid(1),
                10,
                false,
                None,
            );
        });
        assert_eq!(t.take_writes(), [rows::insert(NAME, "spawned", "1")]);
        let eyes = t.npcs_of(UDANS_EYE);
        assert_eq!(eyes.len(), 1);

        // A second hit does nothing: `spawned` is set.
        t.world.run_as_script(NAME, |ctx| {
            Q00609MagicalPowerOfWaterPart1.on_attack(
                ctx,
                ScriptNpc {
                    object_id: mob,
                    id: 21373,
                },
                Oid(1),
                10,
                false,
                None,
            );
        });
        assert!(t.take_writes().is_empty());
        assert_eq!(t.npcs_of(UDANS_EYE).len(), 1);

        // Asefa dispels the gaze: `spawned` is unset, and the eye leaves with its timer.
        assert!(t.quest_page(1, asefa).contains("31372-03.html"));
        assert_eq!(t.take_writes(), [rows::delete_var(NAME, "spawned")]);
        t.wait_ms(10_000);
        assert!(!t.in_world(eyes[0]));
    }
}
