//! The Black Judge: lifts the death penalty for adena.
// Java: dist/game/data/scripts/ai/others/BlackJudge/BlackJudge.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BLACK_JUDGE: i32 = 30981;

// Misc
const COSTS: [i64; 6] = [3600, 8640, 25200, 50400, 86400, 144000];
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `ai.others.BlackJudge.BlackJudge`.
pub struct BlackJudge;

impl Script for BlackJudge {
    fn name(&self) -> &'static str {
        "BlackJudge"
    }

    fn path(&self) -> &'static str {
        "ai/others/BlackJudge"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACK_JUDGE]);
        s.add_talk_id(&[BLACK_JUDGE]);
        s.add_first_talk_id(&[BLACK_JUDGE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        let expertise = ctx.expertise_level(player);
        // `getExpertiseLevel()` can be above 5 (skill 239 goes to 6 for S84): capped like Java.
        let level = if expertise < 5 { expertise } else { 5 };
        match event {
            "remove_info" => {
                htmltext = Some(format!("30981-0{}.html", level + 1));
            }
            "remove_dp" => {
                if ctx.death_penalty_buff_level(player) > 0 {
                    let cost = COSTS[usize::try_from(level).unwrap_or(0).min(5)];
                    if ctx.adena_of(player) >= cost {
                        ctx.take_items(player, ADENA_ID, cost);
                        // NOTE(java-quirk): only the field changes; the death penalty skill stays
                        // until the next relog.
                        let now = ctx.death_penalty_buff_level(player);
                        ctx.set_death_penalty_buff_level(player, now - 1);
                        ctx.send_system_message(player, SystemMessageId::YourDeathPenaltyHasBeenLifted);
                        ctx.send_etc_status_update(player);
                    } else {
                        htmltext = Some("30981-07.html".to_owned());
                    }
                } else {
                    htmltext = Some("30981-08.html".to_owned());
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to, player_data};

    const NPCS: &str = r#"<npc id="30981" type="Folk" level="70" name="Black Judge"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/others/BlackJudge";

    fn setup(adena: i64, penalty: i32) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(BlackJudge);
        let main = r#"<html><body>main
<a action="bypass -h Script BlackJudge remove_info">i</a>
<a action="bypass -h Script BlackJudge remove_dp">d</a></body></html>"#;
        let mut pages = vec![(format!("{DIR}/30981.html"), main.to_owned())];
        for n in 1..=8 {
            pages.push((
                format!("{DIR}/30981-0{n}.html"),
                format!("<html><body>page0{n}</body></html>"),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(BLACK_JUDGE, 20);
        let mut data = player_data(1, 40, vec![item(900, ADENA_ID, adena)]);
        data.profile.death_penalty_level = penalty;
        t.enter_with(data);
        t.talk(1, npc);
        t.take();
        t
    }

    #[test]
    fn the_info_page_follows_the_expertise() {
        let mut t = setup(0, 0);
        t.bypass(1, "Script BlackJudge remove_info");
        // No expertise skill: level 0 -> page 01.
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page01"));
    }

    #[test]
    fn lifting_costs_adena_and_one_level() {
        let mut t = setup(5000, 2);
        t.bypass(1, "Script BlackJudge remove_dp");
        assert_eq!(t.count(1, ADENA_ID), 5000 - COSTS[0]);
        assert_eq!(
            t.world
                .player(ObjectId(1))
                .unwrap()
                .data
                .profile
                .death_penalty_level,
            1
        );
        let out = names_to(&t.take(), 1);
        assert!(out.contains(&"SYSTEM_MESSAGE".to_owned()));
        assert!(out.contains(&"ETC_STATUS_UPDATE".to_owned()));
    }

    #[test]
    fn too_little_adena_and_no_penalty_have_their_pages() {
        let mut t = setup(100, 1);
        t.bypass(1, "Script BlackJudge remove_dp");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page07"));
        assert_eq!(
            t.world
                .player(ObjectId(1))
                .unwrap()
                .data
                .profile
                .death_penalty_level,
            1
        );

        let mut t = setup(9999, 0);
        t.bypass(1, "Script BlackJudge remove_dp");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page08"));
        assert_eq!(t.count(1, ADENA_ID), 9999);
    }
}
