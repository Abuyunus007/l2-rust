//! Quest 615: Magical Power of Fire, Part 1 (Naran, Udan and Asefa's Box; the Ketra monsters).
// Java: dist/game/data/scripts/quests/Q00615_MagicalPowerOfFirePart1/Q00615_MagicalPowerOfFirePart1.java
//
// Pattern: the same shape as Q00609 (the Ketra monsters start Asefa's eye at cond 2; the spell
// dispels it; the box gives the stolen red totem; Udan gives the red totem at cond 3).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NARAN: i32 = 31378;
const UDAN: i32 = 31379;
const ASEFA_BOX: i32 = 31559;
const ASEFA_EYE: i32 = 31684;

// Monsters
const KETRA_MOBS: [i32; 21] = [
    21324, // Ketra Orc Footman
    21325, // Ketra's War Hound
    21327, // Ketra Orc Raider
    21328, // Ketra Orc Scout
    21329, // Ketra Orc Shaman
    21331, // Ketra Orc Warrior
    21332, // Ketra Orc Lieutenant
    21334, // Ketra Orc Medium
    21335, // Ketra Orc Elite Soldier
    21336, // Ketra Orc White Captain
    21338, // Ketra Orc Seer
    21339, // Ketra Orc General
    21340, // Ketra Orc Battalion Commander
    21342, // Ketra Orc Grand Seer
    21343, // Ketra Commander
    21344, // Ketra Elite Guard
    21345, // Ketra's Head Shaman
    21346, // Ketra's Head Guard
    21347, // Ketra Prophet
    21348, // Prophet's Guard
    21349, // Prophet's Aide
];

// Items
const KEY: i32 = 1661;
const STOLEN_RED_TOTEM: i32 = 7242;
const WISDOM_STONE: i32 = 7081;
const RED_TOTEM: i32 = 7243;
const VARKA_MARKS: [i32; 5] = [7221, 7222, 7223, 7224, 7225];

// Skills: Gaze of Watcher, and its dispel.
const GOW: SkillKey = SkillKey { id: 4547, level: 1 };
const DISPEL_GOW: SkillKey = SkillKey { id: 4548, level: 1 };

// Misc
const MIN_LEVEL: i32 = 74;
/// Java `startQuestTimer("eye_despawn", 10000, ...)`.
const EYE_DESPAWN_MS: i64 = 10_000;
/// The variable that says the eye is out (Java `qs.isSet("spawned")`).
const SPAWNED: &str = "spawned";

/// Java `Q00615_MagicalPowerOfFirePart1`.
pub struct Q00615MagicalPowerOfFirePart1;

impl Script for Q00615MagicalPowerOfFirePart1 {
    fn name(&self) -> &'static str {
        "Q00615_MagicalPowerOfFirePart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00615_MagicalPowerOfFirePart1"
    }

    fn id(&self) -> i32 {
        615
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NARAN]);
        s.add_talk_id(&[UDAN, NARAN, ASEFA_BOX]);
        s.add_attack_id(&KETRA_MOBS);
        s.register_quest_items(&[STOLEN_RED_TOTEM]);
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
            "31378-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "open_box" => {
                if !ctx.has_quest_items(player, KEY) {
                    htmltext = Some("31559-02.html".to_owned());
                } else if qs.is_cond(ctx, 2) {
                    if qs.is_set(ctx, SPAWNED) {
                        ctx.take_items(player, KEY, 1);
                        htmltext = Some("31559-04.html".to_owned());
                    } else {
                        ctx.give_items(player, STOLEN_RED_TOTEM, 1);
                        ctx.take_items(player, KEY, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = Some("31559-03.html".to_owned());
                    }
                }
            }
            "eye_despawn" => {
                // The timer's NPC is the eye (Java never checks it).
                if let Some(npc) = npc {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::AsefaHasAlreadySeenYourFace,
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
            if let Some(eye) = ctx.add_spawn_at(ASEFA_EYE, npc.object_id, false, 0) {
                ctx.broadcast_npc_say(
                    eye,
                    ChatType::NpcGeneral,
                    NpcStringId::YouCanTAvoidTheEyesOfAsefa,
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
            NARAN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_at_least_one_quest_item(player, &VARKA_MARKS) {
                            "31378-01.htm"
                        } else {
                            "31378-00a.html"
                        }
                    } else {
                        "31378-00b.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31378-03.html".to_owned();
                    }
                }
                _ => {}
            },
            UDAN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => {
                            htmltext = "31379-01.html".to_owned();
                            qs.set_cond(ctx, 2, true);
                        }
                        2 => {
                            if qs.is_set(ctx, SPAWNED) {
                                qs.unset(ctx, SPAWNED);
                                ctx.cast_skill(npc, player, DISPEL_GOW);
                                htmltext = "31379-03.html".to_owned();
                            } else {
                                htmltext = "31379-02.html".to_owned();
                            }
                        }
                        3 => {
                            ctx.give_items(player, RED_TOTEM, 1);
                            ctx.give_items(player, WISDOM_STONE, 1);
                            qs.exit_quest(ctx, true, true);
                            htmltext = "31379-04.html".to_owned();
                        }
                        _ => {}
                    }
                }
            }
            ASEFA_BOX => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "31559-01.html".to_owned();
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

    const NAME: &str = "Q00615_MagicalPowerOfFirePart1";

    const NPCS: &str = r#"
        <npc id="31378" type="Folk" level="70" name="Naran"/>
        <npc id="31379" type="Folk" level="70" name="Udan"/>
        <npc id="31559" type="Folk" level="1" name="Asefa's Box"/>
        <npc id="31684" type="Monster" level="1" name="Asefa's Eye"/>
        <npc id="21347" type="Monster" level="70" name="Ketra Prophet"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1661" type="EtcItem" name="Key"><set name="is_stackable" val="true"/></item>
        <item id="7242" type="EtcItem" name="Stolen Red Totem"><set name="is_questitem" val="true"/></item>
        <item id="7081" type="EtcItem" name="Wisdom Stone"/>
        <item id="7243" type="EtcItem" name="Red Totem"/>
        <item id="7221" type="EtcItem" name="Mark of Varka's Alliance"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00615MagicalPowerOfFirePart1);
        let pages = Pages::of(&Q00615MagicalPowerOfFirePart1)
            .page("31378-01.htm", &["31378-02.html"])
            .plain(&[
                "31378-00a.html",
                "31378-00b.html",
                "31378-02.html",
                "31378-03.html",
            ])
            .plain(&["31379-01.html", "31379-02.html", "31379-03.html", "31379-04.html"])
            .page("31559-01.html", &["open_box"])
            .plain(&["31559-02.html", "31559-03.html", "31559-04.html"])
            .npcs(&[NARAN, UDAN, ASEFA_BOX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let naran = t.spawn(NARAN, 20);
        let udan = t.spawn(UDAN, 30);
        let chest = t.spawn(ASEFA_BOX, 40);
        (t, naran, udan, chest)
    }

    #[test]
    fn the_fire_chain_from_naran_to_udan_and_the_red_totem() {
        let (mut t, naran, udan, chest) = setup();
        t.enter(1, 74, vec![item(100, 7221, 1), item(101, KEY, 1)]);
        assert!(t.quest_page(1, naran).contains("31378-01.htm"));
        assert!(t.event(1, NAME, "31378-02.html").contains("31378-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, naran).contains("31378-03.html"));

        assert!(t.quest_page(1, udan).contains("31379-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, chest).contains("31559-01.html"));
        assert!(t.event(1, NAME, "open_box").contains("31559-03.html"));
        assert_eq!((t.count(1, STOLEN_RED_TOTEM), t.count(1, KEY)), (1, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        assert!(t.quest_page(1, udan).contains("31379-04.html"));
        assert_eq!((t.count(1, RED_TOTEM), t.count(1, WISDOM_STONE)), (1, 1));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_ketra_hit_at_cond_two_starts_the_eye_once_and_asefas_spell_dispels_it() {
        let (mut t, naran, udan, _chest) = setup();
        t.enter(1, 74, vec![item(100, 7221, 1)]);
        t.quest_page(1, naran);
        t.event(1, NAME, "31378-02.html");
        t.quest_page(1, udan);
        t.take_writes();

        let mob = t.spawn(21347, 60);
        t.world.run_as_script(NAME, |ctx| {
            Q00615MagicalPowerOfFirePart1.on_attack(
                ctx,
                ScriptNpc {
                    object_id: mob,
                    id: 21347,
                },
                Oid(1),
                10,
                false,
                None,
            );
        });
        assert_eq!(t.take_writes(), [rows::insert(NAME, "spawned", "1")]);
        let eyes = t.npcs_of(ASEFA_EYE);
        assert_eq!(eyes.len(), 1);

        t.world.run_as_script(NAME, |ctx| {
            Q00615MagicalPowerOfFirePart1.on_attack(
                ctx,
                ScriptNpc {
                    object_id: mob,
                    id: 21347,
                },
                Oid(1),
                10,
                false,
                None,
            );
        });
        assert!(t.take_writes().is_empty());
        assert_eq!(t.npcs_of(ASEFA_EYE).len(), 1);

        assert!(t.quest_page(1, udan).contains("31379-03.html"));
        assert_eq!(t.take_writes(), [rows::delete_var(NAME, "spawned")]);
        t.wait_ms(10_000);
        assert!(!t.in_world(eyes[0]));
    }
}
