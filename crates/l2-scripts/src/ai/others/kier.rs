//! Kier, the Ice Merchant's helper: a first-talk NPC script reading two quests.
// Java: dist/game/data/scripts/ai/others/Kier/Kier.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KIER: i32 = 32022;
/// Java `Q00115_TheOtherSideOfTruth.class.getSimpleName()`.
const Q00115_THE_OTHER_SIDE_OF_TRUTH: &str = "Q00115_TheOtherSideOfTruth";
/// Java `Q10283_RequestOfIceMerchant.class.getSimpleName()`.
const Q10283_REQUEST_OF_ICE_MERCHANT: &str = "Q10283_RequestOfIceMerchant";

/// Java `ai.others.Kier.Kier`.
pub struct Kier;

impl Script for Kier {
    fn name(&self) -> &'static str {
        "Kier"
    }

    fn path(&self) -> &'static str {
        "ai/others/Kier"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[KIER]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        match ctx.player_quest_state(player, Q00115_THE_OTHER_SIDE_OF_TRUTH) {
            None => htmltext = Some("32022-02.html"),
            Some(qs115) => {
                if !qs115.is_completed() {
                    htmltext = Some("32022-01.html");
                }
            }
        }

        if let Some(qs10283) = ctx.player_quest_state(player, Q10283_REQUEST_OF_ICE_MERCHANT) {
            if qs10283.is_memo_state(2) {
                htmltext = Some("32022-03.html");
            } else if qs10283.is_completed() {
                htmltext = Some("32022-04.html");
            }
        }

        htmltext.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, pages_to, player_data};

    const NPCS: &str = r#"<npc id="32022" type="Folk" level="70" name="Kier"/>"#;
    const DIR: &str = "data/scripts/ai/others/Kier";

    /// A stand-in quest of a given name.
    struct Quest(&'static str, i32);

    impl Script for Quest {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/stand-in"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn page_for(rows: Vec<(String, Option<String>)>) -> String {
        let mut registry = ScriptRegistry::new();
        registry.add(Kier);
        registry.add(Quest(Q00115_THE_OTHER_SIDE_OF_TRUTH, 115));
        registry.add(Quest(Q10283_REQUEST_OF_ICE_MERCHANT, 10283));
        let pages: Vec<(String, String)> = ["01", "02", "03", "04"]
            .iter()
            .map(|n| {
                (
                    format!("{DIR}/32022-{n}.html"),
                    format!("<html><body>p{n}</body></html>"),
                )
            })
            .collect();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let kier = t.spawn(KIER, 20);
        let mut data = player_data(1, 75, vec![]);
        data.profile.quests = QuestLoad {
            states: rows,
            variables: vec![],
        };
        t.enter_with(data);
        t.take();
        t.talk(1, kier);
        pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    #[test]
    fn the_page_follows_the_two_quests() {
        // No quest 115 at all.
        assert!(page_for(vec![]).contains("p02"));
        // Quest 115 started.
        assert!(
            page_for(vec![(
                Q00115_THE_OTHER_SIDE_OF_TRUTH.to_owned(),
                Some("Started".to_owned())
            )])
            .contains("p01")
        );
        // Quest 115 completed and nothing of 10283: Java leaves the page null, so the NPC's own
        // window opens (no script page).
        assert!(
            !page_for(vec![(
                Q00115_THE_OTHER_SIDE_OF_TRUTH.to_owned(),
                Some("Completed".to_owned())
            )])
            .contains("p0")
        );
        // Quest 10283 completed wins over the first answer.
        let rows = vec![
            (
                Q00115_THE_OTHER_SIDE_OF_TRUTH.to_owned(),
                Some("Started".to_owned()),
            ),
            (
                Q10283_REQUEST_OF_ICE_MERCHANT.to_owned(),
                Some("Completed".to_owned()),
            ),
        ];
        assert!(page_for(rows).contains("p04"));
    }
}
