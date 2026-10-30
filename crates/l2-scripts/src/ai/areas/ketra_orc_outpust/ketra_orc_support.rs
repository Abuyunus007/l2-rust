//! The NPCs of the Ketra Orc Outpost: what they say and sell follows the character's Mark of
//! Ketra's Alliance.
// Java: dist/game/data/scripts/ai/areas/KetraOrcOutpust/KetraOrcSupport/KetraOrcSupport.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

/// Java `BuffsData`: a skill (level 1) and the horns it costs.
struct BuffsData {
    skill: i32,
    cost: i64,
}

// NPCs
const KADUN: i32 = 31370; // Hierarch
const WAHKAN: i32 = 31371; // Messenger
const ASEFA: i32 = 31372; // Soul Guide
const ATAN: i32 = 31373; // Grocer
const JAFF: i32 = 31374; // Warehouse Keeper
const JUMARA: i32 = 31375; // Trader
const KURFA: i32 = 31376; // Gate Keeper
// Items
const HORN: i32 = 7186;
const KETRA_MARKS: [i32; 5] = [
    7211, // Mark of Ketra's Alliance - Level 1
    7212, // Mark of Ketra's Alliance - Level 2
    7213, // Mark of Ketra's Alliance - Level 3
    7214, // Mark of Ketra's Alliance - Level 4
    7215, // Mark of Ketra's Alliance - Level 5
];
// Misc: `BUFF` by the key 1..=8.
const BUFF: [BuffsData; 8] = [
    BuffsData { skill: 4359, cost: 2 }, // Focus: Requires 2 Buffalo Horns
    BuffsData { skill: 4360, cost: 2 }, // Death Whisper: Requires 2 Buffalo Horns
    BuffsData { skill: 4345, cost: 3 }, // Might: Requires 3 Buffalo Horns
    BuffsData { skill: 4355, cost: 3 }, // Acumen: Requires 3 Buffalo Horns
    BuffsData { skill: 4352, cost: 3 }, // Berserker: Requires 3 Buffalo Horns
    BuffsData { skill: 4354, cost: 3 }, // Vampiric Rage: Requires 3 Buffalo Horns
    BuffsData { skill: 4356, cost: 6 }, // Empower: Requires 6 Buffalo Horns
    BuffsData { skill: 4357, cost: 6 }, // Haste: Requires 6 Buffalo Horns
];

/// Java `ai.areas.KetraOrcOutpust.KetraOrcSupport.KetraOrcSupport`.
pub struct KetraOrcSupport;

/// Java `getAllianceLevel(player)`: the level of the mark the player carries, 0 for none.
fn get_alliance_level(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    for (i, mark) in KETRA_MARKS.iter().enumerate() {
        if ctx.has_quest_items(player, *mark) {
            return i32::try_from(i).unwrap_or(0) + 1;
        }
    }
    0
}

impl Script for KetraOrcSupport {
    fn name(&self) -> &'static str {
        "KetraOrcSupport"
    }

    fn path(&self) -> &'static str {
        "ai/areas/KetraOrcOutpust/KetraOrcSupport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[KADUN, WAHKAN, ASEFA, ATAN, JAFF, JUMARA, KURFA]);
        s.add_talk_id(&[ASEFA, KURFA, JAFF]);
        s.add_start_npc(&[KURFA, JAFF]);
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
            if ctx.get_quest_items_count(player, HORN) >= buff.cost {
                ctx.take_items(player, HORN, buff.cost);
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
                htmltext = Some("31372-02.html".to_owned());
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
            KADUN => {
                htmltext = if alliance_level > 0 {
                    "31370-friend.html"
                } else {
                    "31370-no.html"
                }
                .to_owned();
            }
            WAHKAN => {
                htmltext = if alliance_level > 0 {
                    "31371-friend.html"
                } else {
                    "31371-no.html"
                }
                .to_owned();
            }
            ASEFA => {
                htmltext = if alliance_level > 0 {
                    if alliance_level < 3 {
                        "31372-01.html"
                    } else {
                        "31372-04.html"
                    }
                } else {
                    "31372-03.html"
                }
                .to_owned();
            }
            ATAN => {
                htmltext = if alliance_level > 0 {
                    "31373-friend.html"
                } else {
                    "31373-no.html"
                }
                .to_owned();
            }
            JAFF => {
                htmltext = if alliance_level > 0 {
                    if alliance_level == 1 {
                        "31374-01.html"
                    } else {
                        "31374-02.html"
                    }
                } else {
                    "31374-no.html"
                }
                .to_owned();
            }
            JUMARA => {
                htmltext = match alliance_level {
                    1 | 2 => "31375-01.html",
                    3 | 4 => "31375-02.html",
                    5 => "31375-03.html",
                    _ => "31375-no.html",
                }
                .to_owned();
            }
            KURFA => {
                htmltext = match alliance_level {
                    1..=3 => "31376-01.html",
                    4 => "31376-02.html",
                    5 => "31376-03.html",
                    _ => "31376-no.html",
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
        <npc id="31370" type="Folk" level="70" name="Kadun"/>
        <npc id="31372" type="Folk" level="70" name="Asefa"/>
        <npc id="31376" type="Folk" level="70" name="Kurfa"/>"#;
    const ITEMS: &str = r#"<item id="7186" type="EtcItem" name="Horn"><set name="is_stackable" val="true"/></item>
        <item id="7211" type="EtcItem" name="Mark 1"/><item id="7215" type="EtcItem" name="Mark 5"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(KetraOrcSupport);
        let pages = Pages::of(&KetraOrcSupport)
            .page("31372-01.html", &["1", "7"])
            .plain(&[
                "31370-friend.html",
                "31370-no.html",
                "31372-02.html",
                "31372-03.html",
                "31372-04.html",
            ])
            .plain(&["31376-01.html", "31376-03.html", "31376-no.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kadun = t.spawn(KADUN, 20);
        let asefa = t.spawn(ASEFA, 25);
        let kurfa = t.spawn(KURFA, 30);
        (t, kadun, asefa, kurfa)
    }

    #[test]
    fn the_first_page_follows_the_mark() {
        let (mut t, kadun, asefa, kurfa) = setup();
        t.enter(1, 70, vec![]);
        t.enter(2, 70, vec![item(500, 7211, 1)]);
        t.enter(3, 70, vec![item(501, 7215, 1)]);
        let page = |t: &mut ScriptTest, player: i32, npc: ObjectId| {
            t.talk(player, npc);
            pages_to(&t.take(), player).pop().unwrap_or_default()
        };
        assert!(page(&mut t, 1, kadun).contains("31370-no.html"));
        assert!(page(&mut t, 2, kadun).contains("31370-friend.html"));
        assert!(page(&mut t, 1, asefa).contains("31372-03.html"));
        assert!(page(&mut t, 2, asefa).contains("31372-01.html"));
        assert!(page(&mut t, 3, asefa).contains("31372-04.html"));
        assert!(page(&mut t, 2, kurfa).contains("31376-01.html"));
        assert!(page(&mut t, 3, kurfa).contains("31376-03.html"));
        assert!(page(&mut t, 1, kurfa).contains("31376-no.html"));
    }

    #[test]
    fn a_buff_costs_horns_and_without_them_the_soul_guide_says_so() {
        let (mut t, _, asefa, _) = setup();
        t.enter(1, 70, vec![item(500, 7211, 1), item(501, HORN, 4)]);
        t.talk(1, asefa);
        t.take();
        assert_eq!(t.event(1, "KetraOrcSupport", "1"), "");
        assert_eq!(t.count(1, HORN), 2);
        assert_eq!(t.event(1, "KetraOrcSupport", "1"), "");
        assert_eq!(t.count(1, HORN), 0);
        assert!(t.event(1, "KetraOrcSupport", "7").contains("31372-02.html"));
    }
}
