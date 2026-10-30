//! Jinia Guild Hideout 1 instance zone, template 140.
// Java: dist/game/data/scripts/instances/JiniaGuildHideout1/JiniaGuildHideout1.java

use super::jinia_guild_hideout::{Condition, HideoutData, jinia_script};

const DATA: HideoutData = HideoutData {
    quest: "Q10284_AcquisitionOfDivineSword",
    condition: Condition::Cond(1),
    template_id: 140,
};

/// Java `instances.JiniaGuildHideout1.JiniaGuildHideout1`.
pub struct JiniaGuildHideout1;

jinia_script!(JiniaGuildHideout1, "JiniaGuildHideout1", DATA);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rafforty_enters_the_quest_player_and_moves_the_quest_on() {
        super::super::jinia_guild_hideout::check_hideout(JiniaGuildHideout1, &DATA);
    }
}
