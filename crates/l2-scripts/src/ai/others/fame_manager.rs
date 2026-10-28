//! Rapidus and Scipio, the fame managers: fame for PK points or clan reputation.
// Java: dist/game/data/scripts/ai/others/FameManager/FameManager.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const FAME_MANAGER: [i32; 2] = [
    36479, // Rapidus
    36480, // Scipio
];

// Misc
const MIN_LEVEL: i32 = 40;
const DECREASE_COST: i32 = 5000;
const REPUTATION_COST: i32 = 1000;
const MIN_CLAN_LEVEL: i32 = 5;
const CLASS_LEVEL: i32 = 2;

/// Java `ai.others.FameManager.FameManager`.
pub struct FameManager;

impl Script for FameManager {
    fn name(&self) -> &'static str {
        "FameManager"
    }

    fn path(&self) -> &'static str {
        "ai/others/FameManager"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&FAME_MANAGER);
        s.add_talk_id(&FAME_MANAGER);
        s.add_first_talk_id(&FAME_MANAGER);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "36479.html" | "36479-02.html" | "36479-07.html" | "36480.html" | "36480-02.html"
            | "36480-07.html" => {
                htmltext = Some(event.to_owned());
            }
            "decreasePk" => {
                let (player, npc) = (player?, npc?);
                if ctx.pk_kills_of(player) > 0 {
                    if ctx.fame_of(player) >= DECREASE_COST
                        && ctx.player_level(player) >= MIN_LEVEL
                        && ctx.class_level_of(player) >= CLASS_LEVEL
                    {
                        let fame = ctx.fame_of(player);
                        ctx.set_fame(player, fame - DECREASE_COST);
                        let pk = ctx.pk_kills_of(player);
                        ctx.set_pk_kills(player, pk - 1);
                        ctx.update_user_info(player);
                        htmltext = Some(format!("{}-06.html", npc.id));
                    } else {
                        htmltext = Some(format!("{}-01.html", npc.id));
                    }
                } else {
                    htmltext = Some(format!("{}-05.html", npc.id));
                }
            }
            "clanRep" => {
                let (player, npc) = (player?, npc?);
                if ctx
                    .clan_level_of(player)
                    .is_some_and(|level| level >= MIN_CLAN_LEVEL)
                {
                    if ctx.fame_of(player) >= REPUTATION_COST
                        && ctx.player_level(player) >= MIN_LEVEL
                        && ctx.class_level_of(player) >= CLASS_LEVEL
                    {
                        let fame = ctx.fame_of(player);
                        ctx.set_fame(player, fame - REPUTATION_COST);
                        ctx.add_clan_reputation_score(player, 50);
                        ctx.update_user_info(player);
                        ctx.send_system_message(player, SystemMessageId::YouHaveAcquired50ClanFamePoints);
                        htmltext = Some(format!("{}-04.html", npc.id));
                    } else {
                        htmltext = Some(format!("{}-01.html", npc.id));
                    }
                } else {
                    htmltext = Some(format!("{}-03.html", npc.id));
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(
            if ctx.fame_of(player) > 0
                && ctx.player_level(player) >= MIN_LEVEL
                && ctx.class_level_of(player) >= CLASS_LEVEL
            {
                format!("{}.html", npc.id)
            } else {
                format!("{}-01.html", npc.id)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to, player_data};

    const NPCS: &str = r#"<npc id="36479" type="Folk" level="70" name="Rapidus"/>"#;
    const DIR: &str = "data/scripts/ai/others/FameManager";

    /// Class id 90 (Phoenix Knight) is a third class (`class_level` 2).
    const THIRD_CLASS: i32 = 90;

    fn setup(level: i32, class_id: i32, fame: i32, pk: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(FameManager);
        let main = r#"<html><body>main
<a action="bypass -h Script FameManager decreasePk">d</a>
<a action="bypass -h Script FameManager clanRep">c</a>
<a action="bypass Script FameManager 36479-02.html">x</a></body></html>"#;
        let mut pages = vec![(format!("{DIR}/36479.html"), main.to_owned())];
        for n in 1..=7 {
            pages.push((
                format!("{DIR}/36479-0{n}.html"),
                format!("<html><body>page0{n}</body></html>"),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let npc = t.spawn(36479, 20);
        let mut data = player_data(1, level, vec![]);
        data.class_id = class_id;
        data.pk_kills = pk;
        data.profile.fame = fame;
        t.enter_with(data);
        t.talk(1, npc);
        t.take();
        (t, npc)
    }

    fn profile(t: &ScriptTest) -> (i32, i32) {
        let p = t.world.player(ObjectId(1)).unwrap();
        (p.data.profile.fame, p.data.pk_kills)
    }

    #[test]
    fn the_first_page_needs_fame_level_and_a_third_class() {
        let (mut t, npc) = setup(60, THIRD_CLASS, 100, 0);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("main"));
        let (mut t, npc) = setup(60, THIRD_CLASS, 0, 0);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page01"));
        let (mut t, npc) = setup(39, THIRD_CLASS, 100, 0);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page01"));
    }

    #[test]
    fn a_static_page_is_a_link() {
        let (mut t, _) = setup(60, THIRD_CLASS, 100, 0);
        t.bypass(1, "Script FameManager 36479-02.html");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page02"));
    }

    #[test]
    fn decrease_pk_takes_fame_and_a_kill() {
        let (mut t, _) = setup(60, THIRD_CLASS, 6000, 3);
        t.bypass(1, "Script FameManager decreasePk");
        assert_eq!(profile(&t), (1000, 2));
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page06"));
    }

    #[test]
    fn decrease_pk_refusals() {
        let (mut t, _) = setup(60, THIRD_CLASS, 6000, 0);
        t.bypass(1, "Script FameManager decreasePk");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page05"));
        let (mut t, _) = setup(60, THIRD_CLASS, 4999, 3);
        t.bypass(1, "Script FameManager decreasePk");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page01"));
        assert_eq!(profile(&t), (4999, 3));
    }

    #[test]
    fn clan_reputation_needs_a_clan_of_level_five() {
        // No clan at all: page 03.
        let (mut t, _) = setup(60, THIRD_CLASS, 6000, 0);
        t.bypass(1, "Script FameManager clanRep");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page03"));
        assert_eq!(profile(&t).0, 6000);
    }
}
