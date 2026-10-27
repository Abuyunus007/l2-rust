//! Java `OlympiadManager`: the waiting lists of the classed, non-classed and team matches.
// Java: mechanics/olympiad/OlympiadManager.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::game::CompetitionType;
use super::{NobleStats, OLYMPIAD_HTML_PATH};
use crate::party::PartyMessageType;
use crate::world::World;

/// The registration closes ten minutes before the competition ends.
const REGISTRATION_CLOSE_MS: i64 = 600_000;

impl World {
    fn olympiad_player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `getBaseClass()`.
    pub(crate) fn olympiad_base_class(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.base_class)
    }

    /// Java `isSubClassActive()` (`_classIndex > 0`).
    pub(crate) fn olympiad_subclass_active(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.data.profile.class_index > 0)
    }

    /// Java `isNoble()`.
    pub(crate) fn is_noble(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.data.profile.noble)
    }

    /// Java `Player.isInventoryUnder90(includeQuestInv)`.
    pub(crate) fn is_inventory_under_90(&self, id: ObjectId, include_quest: bool) -> bool {
        let Some(view) = self.stats(id) else {
            return false;
        };
        let size = if include_quest {
            view.non_quest_size() + self.quest_size(id)
        } else {
            view.non_quest_size()
        };
        f64::from(size) <= f64::from(view.inventory_limit()) * 0.9
    }

    /// `hasEnoughRegisteredClassed()`: the base classes whose list is long enough, `None` for
    /// none.
    pub(crate) fn has_enough_registered_classed(&self) -> Option<Vec<i32>> {
        let needed = usize::try_from(self.olympiad.settings.classed).unwrap_or(0);
        let list: Vec<i32> = self
            .olympiad
            .class_based
            .iter()
            .filter(|(_, ids)| ids.len() >= needed)
            .map(|(class, _)| *class)
            .collect();
        (!list.is_empty()).then_some(list)
    }

    /// `hasEnoughRegisteredNonClassed()`.
    pub(crate) fn has_enough_registered_non_classed(&self) -> bool {
        self.olympiad.non_class_based.len()
            >= usize::try_from(self.olympiad.settings.non_classed).unwrap_or(0)
    }

    /// `hasEnoughRegisteredTeams()`.
    pub(crate) fn has_enough_registered_teams(&self) -> bool {
        self.olympiad.teams_based.len() >= usize::try_from(self.olympiad.settings.teams).unwrap_or(0)
    }

    /// `clearRegistered()`.
    pub(crate) fn clear_registered(&mut self) {
        self.olympiad.non_class_based.clear();
        self.olympiad.class_based.clear();
        self.olympiad.teams_based.clear();
    }

    /// Java `OlympiadManager.isRegistered(noble)`.
    pub fn olympiad_is_registered(&self, id: ObjectId) -> bool {
        self.is_registered_shown(id, id, false)
    }

    /// `isRegistered(noble, player, showMessage)`.
    fn is_registered_shown(&self, noble: ObjectId, to: ObjectId, show: bool) -> bool {
        let obj = noble.0;
        // "party may be already dispersed"
        if self.olympiad.teams_based.iter().any(|team| team.contains(&obj)) {
            if show {
                self.send(
                    to,
                    &SystemMessage::new(
                        SystemMessageId::C1IsAlreadyRegisteredOnTheWaitingListForThe3Vs3ClassIrrelevantTeamMatch,
                    )
                    .with(SystemMessageParam::PlayerName(self.olympiad_player_name(noble))),
                );
            }
            return true;
        }
        if self.olympiad.non_class_based.contains(&obj) {
            if show {
                self.send(
                    to,
                    &SystemMessage::new(
                        SystemMessageId::C1IsAlreadyRegisteredOnTheWaitingListForTheClassIrrelevantIndividualMatch,
                    )
                    .with(SystemMessageParam::PlayerName(self.olympiad_player_name(noble))),
                );
            }
            return true;
        }
        let base = self.olympiad_base_class(noble);
        if self
            .olympiad
            .class_based
            .get(&base)
            .is_some_and(|list| list.contains(&obj))
        {
            if show {
                self.send(
                    to,
                    &SystemMessage::new(SystemMessageId::C1IsAlreadyRegisteredOnTheClassMatchWaitingList)
                        .with(SystemMessageParam::PlayerName(self.olympiad_player_name(noble))),
                );
            }
            return true;
        }
        false
    }

    /// Java `isRegisteredInComp(noble)`.
    pub fn is_registered_in_comp(&self, id: ObjectId) -> bool {
        self.is_registered_shown(id, id, false) || self.is_in_competition(id, id, false)
    }

    /// `isInCompetition(noble, player, showMessage)`.
    fn is_in_competition(&self, noble: ObjectId, to: ObjectId, show: bool) -> bool {
        if !self.olympiad.in_comp_period {
            return false;
        }
        for stadium in self.olympiad.stadiums().iter().rev() {
            let Some(game) = stadium.task.game.as_ref() else {
                continue;
            };
            if !game.contains_participant(noble.0) {
                continue;
            }
            if !show {
                return true;
            }
            let id = match game.kind {
                CompetitionType::Classed => SystemMessageId::C1IsAlreadyRegisteredOnTheClassMatchWaitingList,
                CompetitionType::NonClassed => {
                    SystemMessageId::C1IsAlreadyRegisteredOnTheWaitingListForTheClassIrrelevantIndividualMatch
                }
                CompetitionType::Teams => {
                    SystemMessageId::C1IsAlreadyRegisteredOnTheWaitingListForThe3Vs3ClassIrrelevantTeamMatch
                }
            };
            self.send(
                to,
                &SystemMessage::new(id)
                    .with(SystemMessageParam::PlayerName(self.olympiad_player_name(noble))),
            );
            return true;
        }
        false
    }

    /// Java `registerNoble(player, type)`.
    pub fn olympiad_register_noble(&mut self, player: ObjectId, kind: CompetitionType) -> bool {
        if !self.olympiad.in_comp_period {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheGrandOlympiadGamesAreNotCurrentlyInProgress),
            );
            return false;
        }
        if self.millis_to_comp_end() < REGISTRATION_CLOSE_MS {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ParticipationRequestsAreNoLongerBeingAccepted),
            );
            return false;
        }
        let char_id = player.0;
        if self.remaining_weekly_matches(char_id) < 1 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheMaximumMatchesYouCanParticipateIn1WeekIs70),
            );
            return false;
        }
        let too_many =
            SystemMessageId::TheTotalNumberOfMatchesThatCanBeEnteredIn1WeekIs60ClassIrrelevantIndividualMatches30SpecificMatchesAnd10TeamMatches;
        match kind {
            CompetitionType::Classed => {
                if !self.check_noble(player, player) {
                    return false;
                }
                if self.remaining_weekly_matches_classed(char_id) < 1 {
                    self.send(player, &SystemMessage::new(too_many));
                    return false;
                }
                let base = self.olympiad_base_class(player);
                self.olympiad.class_based.entry(base).or_default().push(char_id);
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouHaveBeenRegisteredForTheGrandOlympiadWaitingListForAClassSpecificMatch,
                    ),
                );
            }
            CompetitionType::NonClassed => {
                if !self.check_noble(player, player) {
                    return false;
                }
                if self.remaining_weekly_matches_non_classed(char_id) < 1 {
                    self.send(player, &SystemMessage::new(too_many));
                    return false;
                }
                self.olympiad.non_class_based.push(char_id);
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouAreCurrentlyRegisteredForA1v1ClassIrrelevantMatch,
                    ),
                );
            }
            CompetitionType::Teams => {
                let party = self.party_of(player);
                let members = party.map(|p| self.party_members(p)).unwrap_or_default();
                if party.is_none() || members.len() != 3 {
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::TheRequestCannotBeMadeBecauseTheRequirementsHaveNotBeenMetToParticipateInATeamMatchYouMustFirstFormA3MemberParty,
                        ),
                    );
                    return false;
                }
                if !self.is_party_leader(player) {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanRequestATeamMatch),
                    );
                    return false;
                }
                let mut team_points = 0;
                let mut team = Vec::with_capacity(members.len());
                for noble in &members {
                    if !self.check_noble(*noble, player) {
                        return false;
                    }
                    if self.remaining_weekly_matches_team(noble.0) < 1 {
                        self.send(player, &SystemMessage::new(too_many));
                        return false;
                    }
                    team.push(noble.0);
                    team_points += self.noble_points(noble.0);
                }
                if team_points < 10 {
                    // "TODO: replace with retail message"
                    self.send_message(player, "Your team must have at least 10 points in total.");
                    return false;
                }
                let message = SystemMessage::new(
                    SystemMessageId::YouAreCurrentlyRegisteredForA3Vs3ClassIrrelevantTeamMatch,
                );
                for member in &members {
                    self.send(*member, &message);
                }
                self.olympiad.teams_based.push(team);
            }
        }
        true
    }

    /// Java `unRegisterNoble(noble)`.
    pub fn olympiad_unregister_noble(&mut self, noble: ObjectId) -> bool {
        if !self.olympiad.in_comp_period {
            self.send(
                noble,
                &SystemMessage::new(SystemMessageId::TheGrandOlympiadGamesAreNotCurrentlyInProgress),
            );
            return false;
        }
        if !self.is_noble(noble) {
            self.send(
                noble,
                &SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsOnlyNoblesseCharactersCanParticipateInTheOlympiad,
                )
                .with(SystemMessageParam::Text(self.olympiad_player_name(noble))),
            );
            return false;
        }
        if !self.is_registered_shown(noble, noble, false) {
            self.send(
                noble,
                &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyRegisteredForTheGrandOlympiad),
            );
            return false;
        }
        if self.is_in_competition(noble, noble, false) {
            return false;
        }
        let obj = noble.0;
        let removed = SystemMessage::new(SystemMessageId::YouHaveBeenRemovedFromTheGrandOlympiadWaitingList);
        if let Some(index) = self.olympiad.non_class_based.iter().position(|id| *id == obj) {
            self.olympiad.non_class_based.remove(index);
            self.send(noble, &removed);
            return true;
        }
        let base = self.olympiad_base_class(noble);
        if let Some(list) = self.olympiad.class_based.get_mut(&base)
            && let Some(index) = list.iter().position(|id| *id == obj)
        {
            list.remove(index);
            self.send(noble, &removed);
            return true;
        }
        if let Some(index) = self
            .olympiad
            .teams_based
            .iter()
            .position(|team| team.contains(&obj))
        {
            let team = self.olympiad.teams_based.remove(index);
            self.announce_unreg_to_team(&team);
            return true;
        }
        false
    }

    /// `AnnounceUnregToTeam.run()` (Java runs it on the thread pool at once).
    fn announce_unreg_to_team(&self, team: &[i32]) {
        let message = SystemMessage::new(SystemMessageId::YouHaveBeenRemovedFromTheGrandOlympiadWaitingList);
        for member in team {
            let member = ObjectId(*member);
            if self.players.contains_key(&member) {
                self.send(member, &message);
            }
        }
    }

    /// Java `removeDisconnectedCompetitor(player)`.
    pub(crate) fn olympiad_remove_disconnected_competitor(&mut self, player: ObjectId) {
        let game_id = self.olympiad_game_id(player);
        if let Some(index) = usize::try_from(game_id).ok()
            && self
                .olympiad
                .stadiums()
                .get(index)
                .is_some_and(|s| s.task.is_game_started())
        {
            self.olympiad_handle_disconnect(index, player);
        }
        let obj = player.0;
        if let Some(index) = self.olympiad.non_class_based.iter().position(|id| *id == obj) {
            self.olympiad.non_class_based.remove(index);
            return;
        }
        let base = self.olympiad_base_class(player);
        if let Some(list) = self.olympiad.class_based.get_mut(&base)
            && let Some(index) = list.iter().position(|id| *id == obj)
        {
            list.remove(index);
            return;
        }
        if let Some(index) = self
            .olympiad
            .teams_based
            .iter()
            .position(|team| team.contains(&obj))
        {
            let team = self.olympiad.teams_based.remove(index);
            self.announce_unreg_to_team(&team);
        }
    }

    /// The `deleteMe` part: `if (isRegistered(this) || getOlympiadGameId() != -1)
    /// removeDisconnectedCompetitor(this)`.
    pub(crate) fn olympiad_on_logout(&mut self, id: ObjectId) {
        if self.olympiad_is_registered(id) || self.olympiad_game_id(id) != -1 {
            self.olympiad_remove_disconnected_competitor(id);
        }
    }

    /// `Logout` and `RequestRestart`: "Unregister from olympiad."
    pub(crate) fn olympiad_unregister_on_leave(&mut self, id: ObjectId) {
        if self.olympiad_is_registered(id) {
            self.olympiad_unregister_noble(id);
        }
    }

    /// Java `checkNoble(noble, player)`: the requirements; messages go to `player`.
    fn check_noble(&mut self, noble: ObjectId, player: ObjectId) -> bool {
        let name = self.olympiad_player_name(noble);
        if !self.is_noble(noble) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsOnlyNoblesseCharactersCanParticipateInTheOlympiad,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
        if self.olympiad_subclass_active(noble) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsASubclassCharacterCannotParticipateInTheOlympiad,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
        if self.is_cursed_weapon_equipped(noble) {
            let item = self
                .players
                .get(&noble)
                .map_or(0, |p| p.cursed_weapon_equipped_id);
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsTheOwnerOfS2CannotParticipateInTheOlympiad,
                )
                .with(SystemMessageParam::PlayerName(name))
                .with(SystemMessageParam::ItemName(item)),
            );
            return false;
        }
        if !self.is_inventory_under_90(noble, true) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1DoesNotMeetTheParticipationRequirementsYouCannotParticipateInTheOlympiadBecauseYourInventorySlotExceeds80,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
        // `isRegisteredOnEvent()`: the custom events are scripts (D-05).
        if self.is_registered_shown(noble, player, true) || self.is_in_competition(noble, player, true) {
            return false;
        }
        let char_id = noble.0;
        if !self.olympiad.nobles.contains_key(&char_id) {
            let stats = NobleStats {
                class_id: self.olympiad_base_class(noble),
                char_name: self.olympiad_player_name(noble),
                points: self.olympiad.settings.start_points,
                to_save: true,
                ..NobleStats::default()
            };
            self.olympiad.nobles.insert(char_id, stats);
        }
        if self.noble_points(char_id) <= 0 {
            let origin = self.players.get(&player).map_or(0, |p| p.talk.last_origin());
            let noble_origin = self.players.get(&noble).map_or(0, |p| p.talk.last_origin());
            if let Some(mut html) = self.html_from_file(
                player,
                origin,
                &format!("{OLYMPIAD_HTML_PATH}noble_nopoints1.htm"),
            ) {
                html.html = html.html.replace("%objectId%", &noble_origin.to_string());
                self.send_html(player, &html);
            }
            return false;
        }
        // `DUALBOX_CHECK_MAX_OLYMPIAD_PARTICIPANTS_PER_IP` (default 0, off): not ported.
        true
    }

    /// Java `getCountOpponents()`.
    pub fn olympiad_count_opponents(&self) -> usize {
        self.olympiad.non_class_based.len()
            + self.olympiad.class_based.len()
            + self.olympiad.teams_based.len()
    }

    /// `party.removePartyMember(player, PartyMessageType.EXPELLED)`.
    pub(crate) fn olympiad_expel_from_party(&mut self, id: ObjectId) {
        if let Some(party) = self.party_of(id) {
            self.remove_party_member(party, id, PartyMessageType::Expelled);
        }
    }
}
