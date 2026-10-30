//! Jinia Guild Hideout 4 instance zone, template 146.
// Java: dist/game/data/scripts/instances/JiniaGuildHideout4/JiniaGuildHideout4.java

use super::jinia_guild_hideout::{Condition, HideoutData, jinia_script};

const DATA: HideoutData = HideoutData {
    quest: "Q10287_StoryOfThoseLeft",
    condition: Condition::MemoState(1),
    template_id: 146,
};

/// Java `instances.JiniaGuildHideout4.JiniaGuildHideout4`.
pub struct JiniaGuildHideout4;

jinia_script!(JiniaGuildHideout4, "JiniaGuildHideout4", DATA);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rafforty_enters_the_quest_player_and_moves_the_quest_on() {
        super::super::jinia_guild_hideout::check_hideout(JiniaGuildHideout4, &DATA);
    }
}
