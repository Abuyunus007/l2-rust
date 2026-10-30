//! Quest 998: Fallen Angel Select (Warehouse Chief Natools lets the player choose Dawn or Dusk).
// Java: dist/game/data/scripts/quests/Q00998_FallenAngelSelect/Q00998_FallenAngelSelect.java
//
// A custom quest (`setCustom(true)`): `startQuest` does nothing for it. It starts the Dawn or
// Dusk quest of the player by name and completes itself.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, Qs};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NATOOLS: i32 = 30894;

// Misc
const MIN_LEVEL: i32 = 38;

/// Java `Q00141_ShadowFoxPart3.class.getSimpleName()`.
const Q00141: &str = "Q00141_ShadowFoxPart3";
/// Java `Q00142_FallenAngelRequestOfDawn.class.getSimpleName()`.
const Q00142: &str = "Q00142_FallenAngelRequestOfDawn";
/// Java `Q00143_FallenAngelRequestOfDusk.class.getSimpleName()`.
const Q00143: &str = "Q00143_FallenAngelRequestOfDusk";

/// Java `Q00998_FallenAngelSelect`.
pub struct Q00998FallenAngelSelect;

/// Java `startQuest(name, player)`: the other script's state is made, its first page is
/// notified, and this quest is completed. Nothing happens when that script is not installed.
fn start_quest(ctx: &mut ScriptCtx<'_>, qs: Qs, name: &str, player: ObjectId) {
    if ctx.quest_state_of(name, player, true).is_some() {
        ctx.notify_event_of(name, "30894-01.html", None, Some(player));
        qs.set_state(ctx, COMPLETED);
    }
}

impl Script for Q00998FallenAngelSelect {
    fn name(&self) -> &'static str {
        "Q00998_FallenAngelSelect"
    }

    fn path(&self) -> &'static str {
        "quests/Q00998_FallenAngelSelect"
    }

    fn id(&self) -> i32 {
        998
    }

    fn is_custom(&self) -> bool {
        true
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NATOOLS]);
        s.add_talk_id(&[NATOOLS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "30894-01.html" | "30894-02.html" | "30894-03.html" => return Some(event.to_owned()),
            "dawn" => start_quest(ctx, qs, Q00142, player),
            "dusk" => start_quest(ctx, qs, Q00143, player),
            _ => {}
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        if !qs.is_started(ctx) {
            return Some(ctx.get_no_quest_msg(player));
        }

        // NOTE(java-quirk): Java calls `qs.isCompleted()` on the state of Q00141 without a null
        // check (it throws when the player has no state; `&&` is short-circuit at the level).
        // Here a missing state reads as not completed.
        let level_ok = ctx.player_level(player) >= MIN_LEVEL;
        let shadow_fox_done = ctx
            .player_quest_state(player, Q00141)
            .is_some_and(|s| s.is_completed());
        Some(
            if level_ok && shadow_fox_done {
                "30894-01.html"
            } else {
                "30894-00.html"
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{COMPLETED, CREATED, STARTED};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00998_FallenAngelSelect";

    const NPCS: &str = r#"
        <npc id="30894" type="Folk" level="60" name="Natools"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00141_ShadowFoxPart3`, as far as this quest reads it: the state.
    struct ShadowFoxPart3;

    impl Script for ShadowFoxPart3 {
        fn name(&self) -> &'static str {
            Q00141
        }
        fn path(&self) -> &'static str {
            "quests/Q00141_ShadowFoxPart3"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// Java `Q00142`/`Q00143`: scripts this quest starts, without hooks of their own.
    struct Dawn;

    impl Script for Dawn {
        fn name(&self) -> &'static str {
            Q00142
        }
        fn path(&self) -> &'static str {
            "quests/Q00142_FallenAngelRequestOfDawn"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Dusk;

    impl Script for Dusk {
        fn name(&self) -> &'static str {
            Q00143
        }
        fn path(&self) -> &'static str {
            "quests/Q00143_FallenAngelRequestOfDusk"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00998FallenAngelSelect);
        registry.add(ShadowFoxPart3);
        registry.add(Dawn);
        registry.add(Dusk);
        let pages = Pages::of(&Q00998FallenAngelSelect)
            .plain(&["30894-00.html", "30894-02.html", "30894-03.html"])
            .page(
                "30894-01.html",
                &["30894-02.html", "30894-03.html", "dawn", "dusk"],
            )
            .npcs(&[NATOOLS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let natools = t.spawn(NATOOLS, 20);
        (t, natools)
    }

    /// The state of `quest` of `player` (Java `player.getQuestState(name)`), read in a script.
    fn state_of(t: &mut ScriptTest, player: i32, quest: &str) -> Option<u8> {
        let mut state = None;
        t.world.run_as_script(NAME, |ctx| {
            state = ctx.player_quest_state(Oid(player), quest).map(|q| q.state());
        });
        state
    }

    /// Sets the state of `quest` for `player` (what the other quests do).
    fn set_state(t: &mut ScriptTest, player: i32, quest: &str, state: u8) {
        t.world.run_as_script(quest, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.set_state(ctx, state);
            }
        });
    }

    #[test]
    fn a_player_who_has_not_started_gets_the_no_quest_page() {
        let (mut t, natools) = setup();
        t.enter(1, 40, vec![]);
        assert!(!t.quest_page(1, natools).contains("30894-0"));
    }

    #[test]
    fn the_choice_needs_level_38_and_shadow_fox_3() {
        let (mut t, natools) = setup();
        t.enter(1, 37, vec![]);
        set_state(&mut t, 1, NAME, STARTED);
        t.take_writes();
        assert!(t.quest_page(1, natools).contains("30894-00.html"));

        // Level 38 is not enough without Shadow Fox - 3.
        t.enter(2, 38, vec![]);
        set_state(&mut t, 2, NAME, STARTED);
        assert!(t.quest_page(2, natools).contains("30894-00.html"));

        // Shadow Fox - 3 completed: the choice page.
        set_state(&mut t, 2, Q00141, COMPLETED);
        assert!(t.quest_page(2, natools).contains("30894-01.html"));
    }

    #[test]
    fn dawn_starts_the_dawn_quest_and_completes_this_one() {
        let (mut t, natools) = setup();
        t.enter(1, 40, vec![]);
        set_state(&mut t, 1, NAME, STARTED);
        set_state(&mut t, 1, Q00141, COMPLETED);
        t.take_writes();
        assert!(t.quest_page(1, natools).contains("30894-01.html"));

        // `startQuest(Q00142)`: its state is made, this one is completed. No page comes back.
        assert_eq!(t.event(1, NAME, "dawn"), "");
        assert_eq!(state_of(&mut t, 1, Q00142), Some(CREATED));
        assert_eq!(state_of(&mut t, 1, NAME), Some(COMPLETED));
        assert_eq!(t.take_writes(), [rows::update(NAME, "<state>", "Completed")]);
        assert_eq!(state_of(&mut t, 1, Q00143), None);
    }

    #[test]
    fn dusk_starts_the_dusk_quest() {
        let (mut t, natools) = setup();
        t.enter(1, 40, vec![]);
        set_state(&mut t, 1, NAME, STARTED);
        set_state(&mut t, 1, Q00141, COMPLETED);
        assert!(t.quest_page(1, natools).contains("30894-01.html"));

        assert_eq!(t.event(1, NAME, "dusk"), "");
        assert_eq!(state_of(&mut t, 1, Q00143), Some(CREATED));
        assert_eq!(state_of(&mut t, 1, NAME), Some(COMPLETED));
        assert_eq!(state_of(&mut t, 1, Q00142), None);
    }
}
