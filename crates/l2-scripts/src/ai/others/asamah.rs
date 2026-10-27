//! Asamah, the Elrokian hunter of the Primeval Isle: a first-talk NPC script.
// Java: dist/game/data/scripts/ai/others/Asamah/Asamah.java
//
// The reference port of an NPC script for the batch workers (`docs/design/scripts.md`): it reads
// another quest's state by name, as Java's `player.getQuestState(Q00111_....class.getSimpleName())`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASAMAH: i32 = 32115;
/// Java `Q00111_ElrokianHuntersProof.class.getSimpleName()`.
const ELROKIAN_HUNTERS_PROOF: &str = "Q00111_ElrokianHuntersProof";

/// Java `ai.others.Asamah.Asamah`.
pub struct Asamah;

impl Script for Asamah {
    fn name(&self) -> &'static str {
        "Asamah"
    }

    fn path(&self) -> &'static str {
        "ai/others/Asamah"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ASAMAH]);
    }

    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        if event == "32115-03.htm" || event == "32115-04.htm" {
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let completed = ctx
            .player_quest_state(player, ELROKIAN_HUNTERS_PROOF)
            .is_some_and(|qs| qs.is_completed());
        Some(if completed { "32115-01.htm" } else { "32115-02.htm" }.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to, player_data};

    const NPCS: &str = r#"<npc id="32115" type="Folk" level="70" name="Asamah"/>"#;
    const DIR: &str = "data/scripts/ai/others/Asamah";

    /// Java `Q00111_ElrokianHuntersProof` as far as Asamah needs it: a quest of that name.
    struct ElrokianHuntersProof;

    impl Script for ElrokianHuntersProof {
        fn name(&self) -> &'static str {
            ELROKIAN_HUNTERS_PROOF
        }
        fn path(&self) -> &'static str {
            "quests/Q00111_ElrokianHuntersProof"
        }
        fn id(&self) -> i32 {
            111
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Asamah);
        registry.add(ElrokianHuntersProof);
        let link = |page: &str| format!(r#"<a action="bypass Script Asamah {page}">x</a>"#);
        let pages = [
            (
                format!("{DIR}/32115-01.htm"),
                format!(
                    "<html><body>32115-01 %objectId%{}</body></html>",
                    link("32115-03.htm")
                ),
            ),
            (
                format!("{DIR}/32115-02.htm"),
                format!("<html><body>32115-02{}</body></html>", link("32115-03.htm")),
            ),
            (
                format!("{DIR}/32115-03.htm"),
                format!("<html><body>32115-03{}</body></html>", link("32115-04.htm")),
            ),
            (
                format!("{DIR}/32115-04.htm"),
                "<html><body>32115-04</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let asamah = t.spawn(ASAMAH, 20);
        (t, asamah)
    }

    #[test]
    fn the_first_talk_depends_on_the_elrokian_quest() {
        let (mut t, asamah) = setup();
        t.enter(1, 75, vec![]);
        t.take();
        t.talk(1, asamah);
        let out = t.take();
        // A script (id -1) answers in a plain NPC window.
        assert!(names_to(&out, 1).contains(&"NPC_HTML_MESSAGE".to_owned()));
        assert!(pages_to(&out, 1).pop().unwrap().contains("32115-02"));

        // A character whose `character_quests` rows say the quest is completed.
        let mut data = player_data(2, 75, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(ELROKIAN_HUNTERS_PROOF.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        t.take();
        t.talk(2, asamah);
        let page = pages_to(&t.take(), 2).pop().unwrap();
        assert!(page.contains("32115-01"));
        // `showHtmlFile` puts the NPC's object id in.
        assert!(page.contains(&asamah.0.to_string()));
    }

    #[test]
    fn the_script_bypass_pages_go_on_from_the_first_page() {
        let (mut t, asamah) = setup();
        t.enter(1, 75, vec![]);
        t.talk(1, asamah);
        t.take();
        t.bypass(1, "Script Asamah 32115-03.htm");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32115-03"));
        t.bypass(1, "Script Asamah 32115-04.htm");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32115-04"));
        // A link no page offered is dropped by the html action check.
        t.bypass(1, "Script Asamah 32115-05.htm");
        assert!(pages_to(&t.take(), 1).is_empty());
    }
}
