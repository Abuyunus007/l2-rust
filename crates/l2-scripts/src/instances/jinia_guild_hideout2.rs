//! Jinia Guild Hideout 2 instance zone, template 141.
// Java: dist/game/data/scripts/instances/JiniaGuildHideout2/JiniaGuildHideout2.java

use super::jinia_guild_hideout::{Condition, HideoutData, jinia_script};

const DATA: HideoutData = HideoutData {
    quest: "Q10285_MeetingSirra",
    condition: Condition::MemoState(1),
    template_id: 141,
};

/// Java `instances.JiniaGuildHideout2.JiniaGuildHideout2`.
pub struct JiniaGuildHideout2;

jinia_script!(JiniaGuildHideout2, "JiniaGuildHideout2", DATA);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rafforty_enters_the_quest_player_and_moves_the_quest_on() {
        super::super::jinia_guild_hideout::check_hideout(JiniaGuildHideout2, &DATA);
    }
}
