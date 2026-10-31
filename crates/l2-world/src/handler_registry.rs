//! Audit of the Java handler folders (S-HAND-01): every class under
//! `dist/game/data/scripts/handlers/{actions,bypass,chat,items,punishments}` is either ported or
//! left with its reason. The tests check the lists against the Java folders (the class and word
//! lists in `java-vectors/out/java_facts/handlers.json`, written by `codegen java-facts`) and the
//! "ported" bypass and item words against the real registries.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::bypass::BypassHandlers;
    use crate::community_board::Board;
    use crate::inventory::handlers::ItemHandlerKind;
    use l2_testkit::java_facts::handlers;

    /// Java classes of `actions/click` that are left, with the reason (none: `DecoyClick` and
    /// `TrapClick` are `World::decoy_trap_click`, S-HAND-04).
    const CLICK_LEFT: &[(&str, &str)] = &[];

    /// `bypass/npc` classes that are left. The ported ones are found by their command words
    /// (S-HAND-03 ported `Loto` and `NpcViewMod`).
    const NPC_LEFT: &[(&str, &str)] = &[("Rebirth", "custom feature (M9)")];

    /// `chat/commands/user` classes that are left (none: `Unstuck` and `MyBirthday` are ported).
    const USER_LEFT: &[(&str, &str)] = &[];

    /// `chat/commands/user` classes ported by S-HAND-02 with the command id they answer.
    const USER_PORTED: &[(&str, i32)] = &[
        ("Unstuck", crate::script_handlers::unstuck::UNSTUCK_COMMAND),
        ("MyBirthday", crate::script_handlers::unstuck::MY_BIRTHDAY_COMMAND),
    ];

    /// `chat/commands/voiced` classes that are ported (`Wedding`, `ExperienceGain`); the others
    /// are registered by `MasterHandler` only when their config switch is on (all off by default),
    /// so without the switch the text is plain chat, as it is here.
    const VOICED_PORTED: &[&str] = &["Wedding", "ExperienceGain"];

    /// `items` classes that are left (none: S-HAND-02 ported `CharmOfCourage`, `RollingDice` and
    /// `EventItem`).
    const ITEMS_LEFT: &[(&str, &str)] = &[];

    #[test]
    fn every_reason_is_written() {
        for (name, reason) in CLICK_LEFT
            .iter()
            .chain(NPC_LEFT)
            .chain(USER_LEFT)
            .chain(ITEMS_LEFT)
        {
            assert!(!name.is_empty() && reason.len() > 10, "{name}");
        }
    }

    /// The bypass classes whose words are all registered are ported; the rest must be exactly
    /// `NPC_LEFT`.
    #[test]
    fn npc_bypass_handlers_are_ported_or_listed() {
        let registry = BypassHandlers::default();
        let mut left = BTreeSet::new();
        for (class, words) in &handlers().bypass_npc {
            if words.is_empty() || !words.iter().all(|w| registry.get(w).is_some()) {
                left.insert(class.clone());
            }
        }
        // `ChatLink` registers `Chat`; `ScriptLink` registers `Script`; `Rift` and `Festival` and the
        // warehouse and olympiad handlers register their words: all found above. Classes whose words
        // are registered by another name in Rust are accepted here.
        let expected: BTreeSet<String> = NPC_LEFT.iter().map(|(n, _)| (*n).to_owned()).collect();
        let unexpected: Vec<_> = left.difference(&expected).collect();
        let ported: Vec<_> = expected.difference(&left).collect();
        assert!(
            unexpected.is_empty() && ported.is_empty(),
            "not registered and not listed: {unexpected:?}; listed but registered: {ported:?}"
        );
    }

    #[test]
    fn community_board_words_are_ported() {
        for (class, words) in &handlers().communityboard {
            for word in words {
                assert!(Board::find(word).is_some(), "{class}: {word}");
            }
        }
    }

    #[test]
    fn item_handlers_are_ported_or_listed() {
        let left: BTreeSet<_> = ITEMS_LEFT.iter().map(|(n, _)| *n).collect();
        for class in handlers().items {
            let ported = ItemHandlerKind::from_name(&class).is_some()
                || crate::inventory::handlers::shots::ShotHandler::from_name(&class).is_some();
            assert_eq!(!ported, left.contains(class.as_str()), "{class}");
        }
    }

    #[test]
    fn user_voiced_and_click_lists_match_the_java_folders() {
        let facts = handlers();
        for (name, _) in USER_LEFT {
            assert!(facts.user.contains_key(*name), "{name}");
        }
        for (name, command) in USER_PORTED {
            let ids = facts.user.get(*name).unwrap_or_else(|| panic!("{name}"));
            assert!(ids.contains(command), "{name} answers command {command}");
        }
        for name in VOICED_PORTED {
            assert!(facts.voiced.iter().any(|v| v == name), "{name}");
        }
        assert_eq!(facts.click.len(), 10);
        for (name, _) in CLICK_LEFT {
            assert!(facts.click.iter().any(|c| c == name), "{name}");
        }
        assert_eq!(facts.shiftclick.len(), 6);
        assert_eq!(facts.punishments.len(), 3);
    }
}
