//! Audit of the Java handler folders (S-HAND-01): every class under
//! `dist/game/data/scripts/handlers/{actions,bypass,chat,items,punishments}` is either ported or
//! left with its reason. The tests check the lists against the Java folders (when the reference
//! folder exists) and the "ported" bypass and item words against the real registries.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use crate::bypass::BypassHandlers;
    use crate::community_board::Board;
    use crate::inventory::handlers::ItemHandlerKind;

    const ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/handlers"
    );

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

    fn root() -> Option<&'static Path> {
        let path = Path::new(ROOT);
        if path.is_dir() {
            Some(path)
        } else {
            eprintln!("skipped: the reference folder {ROOT} is missing");
            None
        }
    }

    fn java_classes(dir: &Path) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(path) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "java")
                    && let Some(stem) = p.file_stem()
                {
                    found.insert(stem.to_string_lossy().into_owned());
                }
            }
        }
        found
    }

    /// The quoted words of the first `{ ... }` block after `marker` in a Java file.
    fn words_after(source: &str, marker: &str) -> Vec<String> {
        let Some(start) = source.find(marker) else {
            return Vec::new();
        };
        let rest = &source[start..];
        let Some(end) = rest.find("};") else {
            return Vec::new();
        };
        rest[..end]
            .split('"')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, w)| w.to_owned())
            .collect()
    }

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
        let Some(root) = root() else { return };
        let handlers = BypassHandlers::default();
        let dir = root.join("bypass").join("npc");
        let mut left = BTreeSet::new();
        for class in java_classes(&dir) {
            let source = std::fs::read_to_string(dir.join(format!("{class}.java"))).unwrap();
            let words = words_after(&source, "COMMANDS");
            if words.is_empty() || !words.iter().all(|w| handlers.get(w).is_some()) {
                left.insert(class);
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
        let Some(root) = root() else { return };
        let dir = root.join("bypass").join("communityboard");
        for class in java_classes(&dir) {
            let source = std::fs::read_to_string(dir.join(format!("{class}.java"))).unwrap();
            let words = words_after(&source, "COMMAND");
            for word in words {
                assert!(Board::find(&word).is_some(), "{class}: {word}");
            }
        }
    }

    #[test]
    fn item_handlers_are_ported_or_listed() {
        let Some(root) = root() else { return };
        let left: BTreeSet<_> = ITEMS_LEFT.iter().map(|(n, _)| *n).collect();
        for class in java_classes(&root.join("items")) {
            let ported = ItemHandlerKind::from_name(&class).is_some()
                || crate::inventory::handlers::shots::ShotHandler::from_name(&class).is_some();
            assert_eq!(!ported, left.contains(class.as_str()), "{class}");
        }
    }

    #[test]
    fn user_voiced_and_click_lists_match_the_java_folders() {
        let Some(root) = root() else { return };
        let user_dir = root.join("chat").join("commands").join("user");
        let user = java_classes(&user_dir);
        for (name, _) in USER_LEFT {
            assert!(user.contains(*name), "{name}");
        }
        for (name, command) in USER_PORTED {
            let source = std::fs::read_to_string(user_dir.join(format!("{name}.java"))).unwrap();
            assert!(
                source.lines().any(|l| l.trim() == command.to_string()),
                "{name} answers command {command}"
            );
        }
        let voiced = java_classes(&root.join("chat").join("commands").join("voiced"));
        for name in VOICED_PORTED {
            assert!(voiced.contains(*name), "{name}");
        }
        let click = java_classes(&root.join("actions").join("click"));
        assert_eq!(click.len(), 10);
        for (name, _) in CLICK_LEFT {
            assert!(click.contains(*name), "{name}");
        }
        assert_eq!(java_classes(&root.join("actions").join("shiftclick")).len(), 6);
        assert_eq!(java_classes(&root.join("punishments")).len(), 3);
    }
}
