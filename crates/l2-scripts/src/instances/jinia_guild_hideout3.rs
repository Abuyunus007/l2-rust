//! Jinia Guild Hideout 3 instance zone, template 145.
// Java: dist/game/data/scripts/instances/JiniaGuildHideout3/JiniaGuildHideout3.java

use super::jinia_guild_hideout::{Condition, HideoutData, jinia_script};

const DATA: HideoutData = HideoutData {
    quest: "Q10286_ReunionWithSirra",
    condition: Condition::MemoState(1),
    template_id: 145,
};

/// Java `instances.JiniaGuildHideout3.JiniaGuildHideout3`.
pub struct JiniaGuildHideout3;

jinia_script!(JiniaGuildHideout3, "JiniaGuildHideout3", DATA);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rafforty_enters_the_quest_player_and_moves_the_quest_on() {
        super::super::jinia_guild_hideout::check_hideout(JiniaGuildHideout3, &DATA);
    }
}
