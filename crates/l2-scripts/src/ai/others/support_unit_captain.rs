//! The Support Unit Captains of the fortresses: talismans for Knight's Epaulettes, the squad skills.
// Java: dist/game/data/scripts/ai/others/SupportUnitCaptain/SupportUnitCaptain.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const UNIT_CAPTAIN: [i32; 21] = [
    35662, // Shanty Fortress
    35694, // Southern Fortress
    35731, // Hive Fortress
    35763, // Valley Fortress
    35800, // Ivory Fortress
    35831, // Narsell Fortress
    35863, // Bayou Fortress
    35900, // White Sands Fortress
    35932, // Borderland Fortress
    35970, // Swamp Fortress
    36007, // Archaic Fortress
    36039, // Floran Fortress
    36077, // Cloud Mountain
    36114, // Tanor Fortress
    36145, // Dragonspine Fortress
    36177, // Antharas's Fortress
    36215, // Western Fortress
    36253, // Hunter's Fortress
    36290, // Aaru Fortress
    36322, // Demon Fortress
    36360, // Monastic Fortress
];

// Items
const EPAULETTE: i32 = 9912; // Knight's Epaulette
const RED_MEDITATION: i32 = 9931; // Red Talisman of Meditation
const BLUE_DIV_PROTECTION: i32 = 9932; // Blue Talisman - Divine Protection
const BLUE_EXPLOSION: i32 = 10416; // Blue Talisman - Explosion
const BLUE_M_EXPLOSION: i32 = 10417; // Blue Talisman - Magic Explosion
const RED_MIN_CLARITY: i32 = 9917; // Red Talisman of Minimum Clarity
const RED_MAX_CLARITY: i32 = 9918; // Red Talisman of Maximum Clarity
const RED_MENTAL_REG: i32 = 9928; // Red Talisman of Mental Regeneration
const BLUE_PROTECTION: i32 = 9929; // Blue Talisman of Protection
const BLUE_INVIS: i32 = 9920; // Blue Talisman of Invisibility
const BLUE_DEFENSE: i32 = 9916; // Blue Talisman of Defense
const BLACK_ESCAPE: i32 = 9923; // Black Talisman - Escape
const BLUE_HEALING: i32 = 9924; // Blue Talisman of Healing
const RED_RECOVERY: i32 = 9925; // Red Talisman of Recovery
const BLUE_DEFENSE2: i32 = 9926; // Blue Talisman of Defense
const BLUE_M_DEFENSE: i32 = 9927; // Blue Talisman of Magic Defense
const RED_LIFE_FORCE: i32 = 10518; // Red Talisman - Life Force
const BLUE_GREAT_HEALING: i32 = 10424; // Blue Talisman - Greater Healing
const WHITE_FIRE: i32 = 10421; // White Talisman - Fire
const COMMON_TALISMANS: [i32; 42] = [
    9914,  // Blue Talisman of Power
    9915,  // Blue Talisman of Wild Magic
    9920,  // Blue Talisman of Invisibility
    9921,  // Blue Talisman - Shield Protection
    9922,  // Black Talisman - Mending
    9933,  // Yellow Talisman of Power
    9934,  // Yellow Talisman of Violent Haste
    9935,  // Yellow Talisman of Arcane Defense
    9936,  // Yellow Talisman of Arcane Power
    9937,  // Yellow Talisman of Arcane Haste
    9938,  // Yellow Talisman of Accuracy
    9939,  // Yellow Talisman of Defense
    9940,  // Yellow Talisman of Alacrity
    9941,  // Yellow Talisman of Speed
    9942,  // Yellow Talisman of Critical Reduction
    9943,  // Yellow Talisman of Critical Damage
    9944,  // Yellow Talisman of Critical Dodging
    9945,  // Yellow Talisman of Evasion
    9946,  // Yellow Talisman of Healing
    9947,  // Yellow Talisman of CP Regeneration
    9948,  // Yellow Talisman of Physical Regeneration
    9949,  // Yellow Talisman of Mental Regeneration
    9950,  // Grey Talisman of Weight Training
    9952,  // Orange Talisman - Hot Springs CP Potion
    9953,  // Orange Talisman - Elixir of Life
    9954,  // Orange Talisman - Elixir of Mental Strength
    9955,  // Black Talisman - Vocalization
    9956,  // Black Talisman - Arcane Freedom
    9957,  // Black Talisman - Physical Freedom
    9958,  // Black Talisman - Rescue
    9959,  // Black Talisman - Free Speech
    9960,  // White Talisman of Bravery
    9961,  // White Talisman of Motion
    9962,  // White Talisman of Grounding
    9963,  // White Talisman of Attention
    9964,  // White Talisman of Bandages
    9965,  // White Talisman of Protection
    10418, // White Talisman - Storm
    10420, // White Talisman - Water
    10519, // White Talisman - Earth
    10422, // White Talisman - Light
    10423, // Blue Talisman - Self-Destruction
];

/// Java `npc.getFort().getOwnerClan() == null ? 0 : ...getId()`, and "the player's clan is it".
fn is_fort_clan(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> bool {
    let fort_owner = ctx.world().npc_fort_owner_clan_id(npc.object_id);
    // `player.getClan() != null` (clan ids are above 0).
    ctx.clan_id_of(player) != 0 && ctx.clan_id_of(player) == fort_owner
}

/// Java `ai.others.SupportUnitCaptain.SupportUnitCaptain`.
pub struct SupportUnitCaptain;

impl Script for SupportUnitCaptain {
    fn name(&self) -> &'static str {
        "SupportUnitCaptain"
    }

    fn path(&self) -> &'static str {
        "ai/others/SupportUnitCaptain"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&UNIT_CAPTAIN);
        s.add_talk_id(&UNIT_CAPTAIN);
        s.add_first_talk_id(&UNIT_CAPTAIN);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);
        if !is_fort_clan(ctx, npc, player) {
            return Some("unitcaptain-04.html".to_owned());
        }

        let mut htmltext = None;
        let item_id;
        match event {
            "unitcaptain.html" | "unitcaptain-01.html" => {
                htmltext = Some(event.to_owned());
            }
            "giveTalisman" => {
                if ctx.get_quest_items_count(player, EPAULETTE) < 10 {
                    return Some("unitcaptain-05.html".to_owned());
                }

                let category_chance = ctx.get_random(100);
                if category_chance <= 5 {
                    let chance = ctx.get_random(100);
                    item_id = if chance <= 25 {
                        RED_MEDITATION
                    } else if chance <= 50 {
                        BLUE_DIV_PROTECTION
                    } else if chance <= 75 {
                        BLUE_EXPLOSION
                    } else {
                        BLUE_M_EXPLOSION
                    };
                } else if category_chance <= 15 {
                    let chance = ctx.get_random(100);
                    item_id = if chance <= 20 {
                        RED_MIN_CLARITY
                    } else if chance <= 40 {
                        RED_MAX_CLARITY
                    } else if chance <= 60 {
                        RED_MENTAL_REG
                    } else if chance <= 80 {
                        BLUE_PROTECTION
                    } else {
                        BLUE_INVIS
                    };
                } else if category_chance <= 30 {
                    let chance = ctx.get_random(100);
                    item_id = if chance <= 12 {
                        BLUE_DEFENSE
                    } else if chance <= 25 {
                        BLACK_ESCAPE
                    } else if chance <= 37 {
                        BLUE_HEALING
                    } else if chance <= 50 {
                        RED_RECOVERY
                    } else if chance <= 62 {
                        BLUE_DEFENSE2
                    } else if chance <= 75 {
                        BLUE_M_DEFENSE
                    } else if chance <= 87 {
                        RED_LIFE_FORCE
                    } else {
                        BLUE_GREAT_HEALING
                    };
                } else {
                    let chance = ctx.get_random(46);
                    item_id = if chance <= 41 {
                        COMMON_TALISMANS[chance as usize]
                    } else {
                        WHITE_FIRE
                    };
                }

                ctx.take_items(player, EPAULETTE, 10);
                ctx.give_items(player, item_id, 1);
                htmltext = Some("unitcaptain-02.html".to_owned());
            }
            "squadSkill" => {
                if ctx.is_clan_leader(player) || ctx.has_member_fame_access(player) {
                    ctx.world_mut().show_sub_unit_skill_list(player);
                } else {
                    htmltext = Some("unitcaptain-03.html".to_owned());
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(
            if is_fort_clan(ctx, npc, player) {
                "unitcaptain.html"
            } else {
                "unitcaptain-04.html"
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="35662" type="Folk" level="70" name="Support Unit Captain"/>"#;
    const ITEMS: &str = r#"<item id="9912" type="EtcItem" name="Knight's Epaulette"><set name="is_stackable" val="true"/></item>"#;

    fn event(t: &mut ScriptTest, name: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("SupportUnitCaptain", |ctx| {
            let npc = ctx.script_npc(t_npc(ctx));
            out = SupportUnitCaptain.on_event(ctx, name, npc, Some(ObjectId(1)));
        });
        out
    }

    fn t_npc(ctx: &ScriptCtx<'_>) -> ObjectId {
        *ctx.world().npc_ids().first().unwrap()
    }

    fn setup(items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SupportUnitCaptain);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.spawn(35662, 20);
        t.enter(1, 60, items);
        t
    }

    #[test]
    fn a_player_without_the_owner_clan_gets_the_stranger_page_everywhere() {
        let mut t = setup(vec![item(5000, 9912, 10)]);
        assert_eq!(event(&mut t, "giveTalisman").as_deref(), Some("unitcaptain-04.html"));
        assert_eq!(event(&mut t, "squadSkill").as_deref(), Some("unitcaptain-04.html"));
        assert_eq!(t.count(1, 9912), 10);
        let mut page = None;
        t.world.run_as_script("SupportUnitCaptain", |ctx| {
            let npc = ctx.script_npc(t_npc(ctx)).unwrap();
            page = SupportUnitCaptain.on_first_talk(ctx, npc, ObjectId(1));
        });
        assert_eq!(page.as_deref(), Some("unitcaptain-04.html"));
    }

    #[test]
    fn the_tables_are_javas() {
        assert_eq!(COMMON_TALISMANS[41], 10423);
        assert_eq!(UNIT_CAPTAIN.len(), 21);
    }
}
