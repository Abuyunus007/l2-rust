//! The NPCs of the Varka Silenos Barracks: what they say and sell follows the character's
//! Mark of Varka's Alliance.
// Java: dist/game/data/scripts/ai/areas/VarkaSilenosBarracks/VarkaSilenosSupport/VarkaSilenosSupport.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

/// Java `BuffsData`: a skill (level 1) and the seeds it costs.
struct BuffsData {
    skill: i32,
    cost: i64,
}

// NPCs
const ASHAS: i32 = 31377; // Hierarch
const NARAN: i32 = 31378; // Messenger
const UDAN: i32 = 31379; // Buffer
const DIYABU: i32 = 31380; // Grocer
const HAGOS: i32 = 31381; // Warehouse Keeper
const SHIKON: i32 = 31382; // Trader
const TERANU: i32 = 31383; // Teleporter
// Items
const SEED: i32 = 7187;
const VARKA_MARKS: [i32; 5] = [
    7221, // Mark of Varka's Alliance - Level 1
    7222, // Mark of Varka's Alliance - Level 2
    7223, // Mark of Varka's Alliance - Level 3
    7224, // Mark of Varka's Alliance - Level 4
    7225, // Mark of Varka's Alliance - Level 5
];
// Misc: `BUFF` by the key 1..=8.
const BUFF: [BuffsData; 8] = [
    BuffsData { skill: 4359, cost: 2 }, // Focus: Requires 2 Nepenthese Seeds
    BuffsData { skill: 4360, cost: 2 }, // Death Whisper: Requires 2 Nepenthese Seeds
    BuffsData { skill: 4345, cost: 3 }, // Might: Requires 3 Nepenthese Seeds
    BuffsData { skill: 4355, cost: 3 }, // Acumen: Requires 3 Nepenthese Seeds
    BuffsData { skill: 4352, cost: 3 }, // Berserker: Requires 3 Nepenthese Seeds
    BuffsData { skill: 4354, cost: 3 }, // Vampiric Rage: Requires 3 Nepenthese Seeds
    BuffsData { skill: 4356, cost: 6 }, // Empower: Requires 6 Nepenthese Seeds
    BuffsData { skill: 4357, cost: 6 }, // Haste: Requires 6 Nepenthese Seeds
];

/// Java `ai.areas.VarkaSilenosBarracks.VarkaSilenosSupport.VarkaSilenosSupport`.
pub struct VarkaSilenosSupport;

/// Java `getAllianceLevel(player)`: minus the level of the mark the player carries, 0 for none.
fn get_alliance_level(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    for (i, mark) in VARKA_MARKS.iter().enumerate() {
        if ctx.has_quest_items(player, *mark) {
            return -(i32::try_from(i).unwrap_or(0) + 1);
        }
    }
    0
}

impl Script for VarkaSilenosSupport {
    fn name(&self) -> &'static str {
        "VarkaSilenosSupport"
    }

    fn path(&self) -> &'static str {
        "ai/areas/VarkaSilenosBarracks/VarkaSilenosSupport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ASHAS, NARAN, UDAN, DIYABU, HAGOS, SHIKON, TERANU]);
        s.add_talk_id(&[UDAN, HAGOS, TERANU]);
        s.add_start_npc(&[HAGOS, TERANU]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        let (npc, player) = (npc?, player?);
        // `StringUtil.isNumeric(event) && BUFF.containsKey(Integer.parseInt(event))`.
        let buff = if !event.is_empty() && event.chars().all(|c| c.is_ascii_digit()) {
            event
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=BUFF.len()).contains(n))
                .map(|n| &BUFF[n - 1])
        } else {
            None
        };
        if let Some(buff) = buff {
            if ctx.get_quest_items_count(player, SEED) >= buff.cost {
                ctx.take_items(player, SEED, buff.cost);
                ctx.cast_skill(
                    npc,
                    player,
                    SkillKey {
                        id: buff.skill,
                        level: 1,
                    },
                );
                let max_hp = f64::from(ctx.npc_max_hp(npc));
                let max_mp = ctx
                    .world()
                    .stats(npc.object_id)
                    .map_or(0.0, |s| f64::from(s.max_mp()));
                ctx.world_mut().set_current_hp_mp(npc.object_id, max_hp, max_mp);
            } else {
                htmltext = Some("31379-02.html".to_owned());
            }
        } else if event == "TELEPORT" {
            let list_name = format!("list{}", get_alliance_level(ctx, player));
            ctx.world_mut()
                .script_show_teleport_list(npc.object_id, player, &list_name);
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let alliance_level = get_alliance_level(ctx, player);
        match npc.id {
            ASHAS => {
                htmltext = if alliance_level < 0 {
                    "31377-friend.html"
                } else {
                    "31377-no.html"
                }
                .to_owned();
            }
            NARAN => {
                htmltext = if alliance_level < 0 {
                    "31378-friend.html"
                } else {
                    "31378-no.html"
                }
                .to_owned();
            }
            UDAN => {
                htmltext = if alliance_level < 0 {
                    if alliance_level > -3 {
                        "31379-01.html"
                    } else {
                        "31379-04.html"
                    }
                } else {
                    "31379-03.html"
                }
                .to_owned();
            }
            DIYABU => {
                htmltext = if alliance_level < 0 {
                    "31380-friend.html"
                } else {
                    "31380-no.html"
                }
                .to_owned();
            }
            HAGOS => {
                htmltext = if alliance_level < 0 {
                    if alliance_level == -1 {
                        "31381-01.html"
                    } else {
                        "31381-02.html"
                    }
                } else {
                    "31381-no.html"
                }
                .to_owned();
            }
            SHIKON => {
                htmltext = match alliance_level {
                    -1 | -2 => "31382-01.html",
                    -3 | -4 => "31382-02.html",
                    -5 => "31382-03.html",
                    _ => "31382-no.html",
                }
                .to_owned();
            }
            TERANU => {
                htmltext = match alliance_level {
                    -3..=-1 => "31383-01.html",
                    -4 => "31383-02.html",
                    -5 => "31383-03.html",
                    _ => "31383-no.html",
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, pages_to};

    const NPCS: &str = r#"
        <npc id="31377" type="Folk" level="70" name="Ashas"/>
        <npc id="31379" type="Folk" level="70" name="Udan"/>
        <npc id="31383" type="Folk" level="70" name="Teranu"/>"#;
    const ITEMS: &str = r#"<item id="7187" type="EtcItem" name="Seed"><set name="is_stackable" val="true"/></item>
        <item id="7221" type="EtcItem" name="Mark 1"/><item id="7225" type="EtcItem" name="Mark 5"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(VarkaSilenosSupport);
        let pages = Pages::of(&VarkaSilenosSupport)
            .page("31379-01.html", &["1", "7"])
            .plain(&[
                "31377-friend.html",
                "31377-no.html",
                "31379-02.html",
                "31379-03.html",
                "31379-04.html",
            ])
            .plain(&["31383-01.html", "31383-03.html", "31383-no.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ashas = t.spawn(ASHAS, 20);
        let udan = t.spawn(UDAN, 25);
        let teranu = t.spawn(TERANU, 30);
        (t, ashas, udan, teranu)
    }

    #[test]
    fn the_first_page_follows_the_mark() {
        let (mut t, ashas, udan, teranu) = setup();
        t.enter(1, 70, vec![]);
        t.enter(2, 70, vec![item(500, 7221, 1)]);
        t.enter(3, 70, vec![item(501, 7225, 1)]);
        let page = |t: &mut ScriptTest, player: i32, npc: ObjectId| {
            t.talk(player, npc);
            pages_to(&t.take(), player).pop().unwrap_or_default()
        };
        assert!(page(&mut t, 1, ashas).contains("31377-no.html"));
        assert!(page(&mut t, 2, ashas).contains("31377-friend.html"));
        assert!(page(&mut t, 1, udan).contains("31379-03.html"));
        assert!(page(&mut t, 2, udan).contains("31379-01.html"));
        assert!(page(&mut t, 3, udan).contains("31379-04.html"));
        assert!(page(&mut t, 2, teranu).contains("31383-01.html"));
        assert!(page(&mut t, 3, teranu).contains("31383-03.html"));
        assert!(page(&mut t, 1, teranu).contains("31383-no.html"));
    }

    #[test]
    fn a_buff_costs_seeds_and_without_them_the_buffer_says_so() {
        let (mut t, _, udan, _) = setup();
        t.enter(1, 70, vec![item(500, 7221, 1), item(501, SEED, 4)]);
        t.talk(1, udan);
        t.take();
        // Focus: 2 seeds.
        assert_eq!(t.event(1, "VarkaSilenosSupport", "1"), "");
        assert_eq!(t.count(1, SEED), 2);
        assert_eq!(t.event(1, "VarkaSilenosSupport", "1"), "");
        assert_eq!(t.count(1, SEED), 0);
        // No seeds left.
        assert!(t.event(1, "VarkaSilenosSupport", "7").contains("31379-02.html"));
    }
}
