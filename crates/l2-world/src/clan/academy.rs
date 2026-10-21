//! The clan academy: the graduation of a member who completes the 2nd class transfer.
// Java: entity/actor/Player.java (setPlayerClass: the `_lvlJoinedAcademy` block)
// Java: config/FeatureConfig.java (`CompleteAcademyMinPoints`, `CompleteAcademyMaxPoints`)
// Java: data/xml/CategoryData.java (`THIRD_CLASS_GROUP`)
//
// # What is where
// - Joining the academy (level 40 or lower, no 2nd class, `lvl_joined_academy`, power grade 9):
//   `handlers.rs` (`RequestJoinPledge`, `RequestAnswerJoinPledge`), M6-CLAN-01.
// - The sponsor (`RequestPledgeSetAcademyMaster`): `handlers.rs`, M6-CLAN-01.
// - Graduation: here. The class change itself (`Player.setPlayerClass`, a quest reward or a GM
//   command) is not ported yet; whatever ports it calls [`World::academy_graduation`] first, the
//   way Java runs this block before it sets the new class template.

use l2_data::ClassId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::server::SystemMessageParam;
use l2_protocol_game::server::pledge::PledgeShowMemberListDelete;
use l2_util::ObjectId;

use crate::world::World;

/// The academy circlet every graduate gets (Java `addItem(REWARD, 8181, 1, ...)`).
const ACADEMY_CIRCLET_ID: i32 = 8181;

/// Java `CategoryData.isInCategory(CategoryType.THIRD_CLASS_GROUP, classId)`: the classes of
/// the 2nd class transfer (`categoryData.xml` lists 2, 3, 5, 6, 8, 9, 12 ... and the Kamael
/// 127 to 130 and 135), which are the classes two steps below a root class.
pub fn is_third_class_group(class: ClassId) -> bool {
    class.class_level() == 2
}

/// The clan reputation a graduate brings: the earlier he joined the more (Java: up to level 16
/// the maximum, from level 39 the minimum, 20 points less for every level in between).
pub fn academy_reputation(lvl_joined_academy: i32, min_points: i32, max_points: i32) -> i32 {
    if lvl_joined_academy <= 16 {
        max_points
    } else if lvl_joined_academy >= 39 {
        min_points
    } else {
        max_points.wrapping_sub((lvl_joined_academy - 16).wrapping_mul(20))
    }
}

impl World {
    /// The first block of Java `Player.setPlayerClass(id)`: an academy member who changes to a
    /// class of the 2nd transfer graduates. The clan gets reputation, the member leaves the clan
    /// without a penalty and gets the academy circlet. Returns whether the player graduated.
    pub fn academy_graduation(&mut self, player: ObjectId, new_class_id: i32) -> bool {
        let state = self.player_clan_state(player);
        let in_group = u16::try_from(new_class_id)
            .map(ClassId)
            .is_ok_and(is_third_class_group);
        if state.lvl_joined_academy == 0 || !in_group {
            return false;
        }
        let clan_id = state.clan_id;
        if self.clan(clan_id).is_none() {
            return false;
        }
        let settings = &self.clans.settings;
        let points = academy_reputation(
            state.lvl_joined_academy,
            settings.join_academy_min_rep_score,
            settings.join_academy_max_rep_score,
        );
        self.add_reputation_score(clan_id, points);

        if let Some(s) = self.player_clan_state_mut(player) {
            s.lvl_joined_academy = 0;
        }

        // Oust pledge member from the academy, cuz he has finished his 2nd class transfer.
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::ClanMemberS1HasBeenExpelled)
                .with(SystemMessageParam::PlayerName(name.clone())),
        );
        self.broadcast_to_clan(clan_id, &PledgeShowMemberListDelete { player_name: name });
        self.remove_clan_member(clan_id, player, 0, true);
        self.send(
            player,
            &SystemMessage::new(
                SystemMessageId::CongratulationsYouWillNowGraduateFromTheClanAcademyAndLeaveYourCurrentClanAsAGraduateOfTheAcademyYouCanImmediatelyJoinAClanAsARegularMemberWithoutBeingSubjectToAnyPenalties,
            ),
        );

        // Receive graduation gift: give academy circlet.
        if let Some(added) = self.inventory_add_item_by_id(player, ACADEMY_CIRCLET_ID, 1) {
            self.update_item_in_db(player, added.item.object_id);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reputation_falls_by_twenty_per_level_between_16_and_39() {
        // Java defaults: `CompleteAcademyMinPoints = 190`, `CompleteAcademyMaxPoints = 650`.
        assert_eq!(academy_reputation(1, 190, 650), 650);
        assert_eq!(academy_reputation(16, 190, 650), 650);
        assert_eq!(academy_reputation(17, 190, 650), 630);
        assert_eq!(academy_reputation(30, 190, 650), 370);
        assert_eq!(academy_reputation(38, 190, 650), 210);
        assert_eq!(academy_reputation(39, 190, 650), 190);
        assert_eq!(academy_reputation(40, 190, 650), 190);
    }

    /// The ids of `THIRD_CLASS_GROUP` in the reference datapack; skipped when it is not there.
    #[test]
    fn the_third_class_group_matches_category_data() {
        let path = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game\data\categoryData.xml");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let start = text.find("<category name=\"THIRD_CLASS_GROUP\">").unwrap();
        let block = &text[start..start + text[start..].find("</category>").unwrap()];
        let listed: Vec<u16> = block
            .split("<id>")
            .skip(1)
            .map(|part| part.split("</id>").next().unwrap().trim().parse().unwrap())
            .collect();
        let computed: Vec<u16> = (0..=146)
            .filter(|&id| ClassId(id).is_player_class() && is_third_class_group(ClassId(id)))
            .collect();
        assert_eq!(computed, listed);
    }

    #[test]
    fn the_third_class_group_is_the_2nd_class_transfer() {
        // `categoryData.xml`: Gladiator (2), Warlord (3), Bishop (16), Berserker (127).
        for id in [2, 3, 5, 16, 57, 127, 130, 135] {
            assert!(is_third_class_group(ClassId(id)), "{id}");
        }
        // Fighter (0), Warrior (1), Duelist (88), Soldier (123).
        for id in [0, 1, 88, 123, 125] {
            assert!(!is_third_class_group(ClassId(id)), "{id}");
        }
    }
}
