//! The GM petition system: a player's `RequestPetition`, the queue GMs answer from, the
//! consultation chat and the end of it.
// Java: managers/PetitionManager.java, mechanics/petition/{Petition,PetitionState,PetitionType}.java
// Java: network/clientpackets/{RequestPetition,RequestPetitionCancel,RequestPetitionFeedback}.java
// Java: network/serverpackets/PetitionVotePacket.java
// Java: dist/game/data/scripts/handlers/chat/channels/ChatPetition.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPetition.java
//
// A petition keeps the object ids of the petitioner and of the answering GM, and their names as
// they were when each took part: Java holds the `Player` objects, which still have a name when
// they are offline, and the pages show it (greyed out).
//
// NOTE(java-quirk): Java keeps the queue in a plain `HashMap` keyed by the petition's object id,
// so the order of the lists is the hash order. The queue is ordered by id here, which is the
// order the petitions came in.
//
// Left out: `petition_feedback` rows are written only through the persistence task (the
// feedback packet), see [`PersistCommand::PetitionFeedback`].

use std::collections::BTreeMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, PetitionVotePacket, SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::admin::html::AdminHtml;
use crate::outbox::PersistCommand;
use crate::world::{World, unix_ms_now};

/// Java `PetitionState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetitionState {
    Pending,
    ResponderCancel,
    ResponderMissing,
    ResponderReject,
    ResponderComplete,
    PetitionerCancel,
    PetitionerMissing,
    InProcess,
    Completed,
}

/// Java `PetitionType`: `toString().replace('_', ' ')` is what the pages show.
const PETITION_TYPES: [&str; 9] = [
    "IMMOBILITY",
    "RECOVERY RELATED",
    "BUG REPORT",
    "QUEST RELATED",
    "BAD USER",
    "SUGGESTIONS",
    "GAME TIP",
    "OPERATION RELATED",
    "OTHER",
];

/// Java `Petition`.
#[derive(Debug, Clone)]
pub(crate) struct Petition {
    id: i32,
    submit_time_ms: i64,
    /// `PetitionType.getTypeAsString()`.
    kind: &'static str,
    state: PetitionState,
    content: String,
    log: Vec<CreatureSay>,
    petitioner: ObjectId,
    petitioner_name: String,
    responder: Option<(ObjectId, String)>,
}

/// Java `PetitionManager`'s two maps.
#[derive(Default)]
pub(crate) struct PetitionWorld {
    pending: BTreeMap<i32, Petition>,
    completed: BTreeMap<i32, Petition>,
    /// Java `Player._lastPetitionGmName`: the GM who last answered each petitioner.
    last_gm: BTreeMap<ObjectId, String>,
}

/// The input of the petition packets, read by the session (`ClientInput::Petition`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetitionInput {
    /// `RequestPetition`.
    Submit { content: String, kind: i32 },
    /// `RequestPetitionCancel`.
    Cancel,
    /// `RequestPetitionFeedback`.
    Feedback { rate: i32, message: String },
}

/// Java `DATE_FORMAT`: `yyyy-MM-dd HH:mm` in the machine's zone (D-48).
fn format_date(epoch_ms: i64) -> String {
    let local = epoch_ms.saturating_add(l2_util::time::local_offset_millis(epoch_ms));
    let minutes = local.div_euclid(60_000);
    let days = minutes.div_euclid(1440);
    let rest = minutes.rem_euclid(1440);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rest / 60, rest % 60)
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl World {
    fn petition_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    fn petition_online(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id)
    }

    /// The `Player` object of Java is gone when the player leaves, and with it the name of the
    /// GM who last answered (`_lastPetitionGmName`).
    pub(crate) fn petition_forget_player(&mut self, id: ObjectId) {
        self.admin.ctl.petitions.last_gm.remove(&id);
    }

    /// Java `Petition.endPetitionConsultation(endState)`.
    fn end_petition_consultation(&mut self, id: i32, state: PetitionState) -> bool {
        let Some(petition) = self.admin.ctl.petitions.pending.get_mut(&id) else {
            // Java sets the state and moves the object regardless; a petition that is not in
            // the queue has nothing to move.
            return false;
        };
        petition.state = state;
        let (petitioner, petitioner_name) = (petition.petitioner, petition.petitioner_name.clone());
        let responder = petition.responder.clone();

        if let Some((responder, _)) = responder
            && self.petition_online(responder)
        {
            if state == PetitionState::ResponderReject {
                self.send_message(petitioner, "Your petition was rejected. Please try again later.");
            } else {
                // Ending petition consultation with <Player>.
                self.send(
                    responder,
                    &SystemMessage::new(SystemMessageId::EndingPetitionConsultationWithC1)
                        .with(SystemMessageParam::Text(petitioner_name)),
                );
                if state == PetitionState::PetitionerCancel {
                    // Receipt No. <ID> petition cancelled.
                    self.send(
                        responder,
                        &SystemMessage::new(SystemMessageId::ReceiptNoS1PetitionCancelled)
                            .with(SystemMessageParam::IntNumber(id)),
                    );
                }
            }
        }

        // End petition consultation and inform them, if they are still online. And if
        // petitioner is online, enable Evaluation button.
        if self.petition_online(petitioner) {
            self.send(
                petitioner,
                &SystemMessage::new(
                    SystemMessageId::ThisEndsTheGmPetitionConsultationNpleaseGiveUsFeedbackOnThePetitionService,
                ),
            );
            self.send(petitioner, &PetitionVotePacket);
        }

        let petitions = &mut self.admin.ctl.petitions;
        let removed = petitions.pending.remove(&id);
        if let Some(petition) = removed {
            petitions.completed.insert(id, petition);
            true
        } else {
            false
        }
    }

    /// Java `PetitionManager.submitPetition(petitioner, text, type)`.
    pub(crate) fn submit_petition(&mut self, petitioner: ObjectId, text: &str, kind: i32) -> Option<i32> {
        let id = self.allocate_item_id()?.0;
        // `PetitionType.values()[petitionType - 1]`: out of range is an exception in Java.
        let kind = usize::try_from(kind.wrapping_sub(1))
            .ok()
            .and_then(|index| PETITION_TYPES.get(index))
            .copied()?;
        let petitioner_name = self.petition_name(petitioner);
        self.admin.ctl.petitions.pending.insert(
            id,
            Petition {
                id,
                submit_time_ms: unix_ms_now(),
                kind,
                state: PetitionState::Pending,
                content: text.to_owned(),
                log: Vec::new(),
                petitioner,
                petitioner_name: petitioner_name.clone(),
                responder: None,
            },
        );
        // Notify all GMs that a new petition has been submitted.
        self.broadcast_to_gms(&CreatureSay {
            sender_object_id: petitioner.0,
            chat_type: ChatType::HeroVoice,
            sender: SenderField::Name("Petition System".to_owned()),
            message_id: -1,
            body: SayBody::Text(format!("{petitioner_name} has submitted a new petition.")),
        });
        Some(id)
    }

    /// Java `PetitionManager.acceptPetition(admin, petitionId)`.
    pub(crate) fn accept_petition(&mut self, admin: ObjectId, id: i32) -> bool {
        let admin_name = self.petition_name(admin);
        let Some(petition) = self.admin.ctl.petitions.pending.get_mut(&id) else {
            return false;
        };
        if petition.responder.is_some() {
            return false;
        }
        // `setResponder`, `setState(IN_PROCESS)`.
        petition.responder = Some((admin, admin_name.clone()));
        petition.state = PetitionState::InProcess;
        let (petitioner, petitioner_name) = (petition.petitioner, petition.petitioner_name.clone());

        // Petition application accepted. (Send to Petitioner).
        self.petition_send_petitioner(
            id,
            &SystemMessage::new(SystemMessageId::PetitionApplicationAccepted),
        );
        // Petition application accepted. Reciept No. is <ID>.
        self.petition_send_responder(
            id,
            &SystemMessage::new(SystemMessageId::YourPetitionApplicationHasBeenAcceptedNReceiptNoIsS1)
                .with(SystemMessageParam::IntNumber(id)),
        );
        // Petition consultation with <Player> underway.
        self.petition_send_responder(
            id,
            &SystemMessage::new(SystemMessageId::StartingPetitionConsultationWithC1)
                .with(SystemMessageParam::Text(petitioner_name)),
        );
        // Set responder name on petitioner instance.
        self.admin.ctl.petitions.last_gm.insert(petitioner, admin_name);
        true
    }

    /// Java `Petition.sendPetitionerPacket`: only an online petitioner hears it.
    fn petition_send_petitioner(&self, id: i32, packet: &impl l2_net::Encode) {
        if let Some(petition) = self.admin.ctl.petitions.pending.get(&id)
            && self.petition_online(petition.petitioner)
        {
            self.send(petition.petitioner, packet);
        }
    }

    /// Java `Petition.sendResponderPacket`: a responder who is gone ends the consultation.
    fn petition_send_responder(&mut self, id: i32, packet: &impl l2_net::Encode) {
        let responder = self
            .admin
            .ctl
            .petitions
            .pending
            .get(&id)
            .and_then(|p| p.responder.as_ref().map(|(responder, _)| *responder));
        match responder {
            Some(responder) if self.petition_online(responder) => {
                self.send(responder, packet);
            }
            _ => {
                self.end_petition_consultation(id, PetitionState::ResponderMissing);
            }
        }
    }

    /// Java `PetitionManager.cancelActivePetition(player)`.
    fn cancel_active_petition(&mut self, player: ObjectId) -> bool {
        let found = self.admin.ctl.petitions.pending.values().find_map(|p| {
            if p.petitioner == player {
                Some((p.id, PetitionState::PetitionerCancel))
            } else if p.responder.as_ref().is_some_and(|(r, _)| *r == player) {
                Some((p.id, PetitionState::ResponderCancel))
            } else {
                None
            }
        });
        match found {
            Some((id, state)) => self.end_petition_consultation(id, state),
            None => false,
        }
    }

    /// Java `PetitionManager.checkPetitionMessages(petitioner)`: the chat of the petition, which
    /// the petitioner missed while it was away.
    pub(crate) fn check_petition_messages(&self, petitioner: ObjectId) -> Vec<CreatureSay> {
        self.admin
            .ctl
            .petitions
            .pending
            .values()
            .find(|p| p.petitioner == petitioner)
            .map(|p| p.log.clone())
            .unwrap_or_default()
    }

    /// Java `PetitionManager.endActivePetition(player)`: a GM completes its consultation.
    fn end_active_petition(&mut self, player: ObjectId) -> bool {
        if !self.is_gm(player) {
            return false;
        }
        let found = self
            .admin
            .ctl
            .petitions
            .pending
            .values()
            .find(|p| p.responder.as_ref().is_some_and(|(r, _)| *r == player))
            .map(|p| p.id);
        match found {
            Some(id) => self.end_petition_consultation(id, PetitionState::Completed),
            None => false,
        }
    }

    /// Java `PetitionManager.getPlayerTotalPetitionCount(player)`: pending and completed.
    fn player_total_petition_count(&self, player: ObjectId) -> i32 {
        let petitions = &self.admin.ctl.petitions;
        let count = petitions
            .pending
            .values()
            .chain(petitions.completed.values())
            .filter(|p| p.petitioner == player)
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `PetitionManager.isPetitionInProcess()`.
    pub(crate) fn is_any_petition_in_process(&self) -> bool {
        self.admin
            .ctl
            .petitions
            .pending
            .values()
            .any(|p| p.state == PetitionState::InProcess)
    }

    /// Java `PetitionManager.isPetitionInProcess(petitionId)`.
    pub(crate) fn is_petition_in_process(&self, id: i32) -> bool {
        self.admin
            .ctl
            .petitions
            .pending
            .get(&id)
            .is_some_and(|p| p.state == PetitionState::InProcess)
    }

    /// Java `PetitionManager.isPlayerInConsultation(player)`.
    pub(crate) fn is_player_in_consultation(&self, player: ObjectId) -> bool {
        self.admin.ctl.petitions.pending.values().any(|p| {
            p.state == PetitionState::InProcess
                && (p.petitioner == player || p.responder.as_ref().is_some_and(|(r, _)| *r == player))
        })
    }

    /// Java `PetitionManager.isPlayerPetitionPending(petitioner)`.
    fn is_player_petition_pending(&self, petitioner: ObjectId) -> bool {
        self.admin
            .ctl
            .petitions
            .pending
            .values()
            .any(|p| p.petitioner == petitioner)
    }

    /// Java `PetitionManager.rejectPetition(admin, petitionId)`.
    pub(crate) fn reject_petition(&mut self, admin: ObjectId, id: i32) -> bool {
        let admin_name = self.petition_name(admin);
        let Some(petition) = self.admin.ctl.petitions.pending.get_mut(&id) else {
            return false;
        };
        if petition.responder.is_some() {
            return false;
        }
        petition.responder = Some((admin, admin_name));
        self.end_petition_consultation(id, PetitionState::ResponderReject)
    }

    /// Java `PetitionManager.sendActivePetitionMessage(player, messageText)`.
    fn send_active_petition_message(&mut self, player: ObjectId, text: &str) -> bool {
        let found = self.admin.ctl.petitions.pending.values().find_map(|p| {
            if p.petitioner == player {
                Some((p.id, ChatType::PetitionPlayer))
            } else if p.responder.as_ref().is_some_and(|(r, _)| *r == player) {
                Some((p.id, ChatType::PetitionGm))
            } else {
                None
            }
        });
        let Some((id, chat_type)) = found else {
            return false;
        };
        let say = CreatureSay {
            sender_object_id: player.0,
            chat_type,
            sender: SenderField::Name(self.petition_name(player)),
            message_id: -1,
            body: SayBody::Text(text.to_owned()),
        };
        if let Some(petition) = self.admin.ctl.petitions.pending.get_mut(&id) {
            petition.log.push(say.clone());
        }
        self.petition_send_responder(id, &say);
        self.petition_send_petitioner(id, &say);
        true
    }

    /// Java `PetitionManager.clearPendingPetitions()`.
    pub(crate) fn clear_pending_petitions(&mut self) {
        let count = self.admin.ctl.petitions.pending.len();
        self.admin.ctl.petitions.pending.clear();
        tracing::info!("PetitionManager: Pending petition queue cleared. {count} petitions removed.");
    }

    /// Java `PetitionManager.getPendingPetitionCount()`.
    fn pending_petition_count(&self) -> i32 {
        i32::try_from(self.admin.ctl.petitions.pending.len()).unwrap_or(i32::MAX)
    }

    /// Java `PetitionManager.sendPendingPetitionList(player)`.
    pub(crate) fn send_pending_petition_list(&mut self, player: ObjectId) {
        let pending: Vec<Petition> = self.admin.ctl.petitions.pending.values().cloned().collect();
        let mut html = String::with_capacity(600 + pending.len() * 300);
        html.push_str("<html><body><center><table width=270><tr><td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center>Petition Menu</center></td><td width=45><button value=\"Back\" action=\"bypass admin_admin7\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br><table width=\"270\"><tr><td><table width=\"270\"><tr><td><button value=\"Reset\" action=\"bypass -h admin_reset_petitions\" width=\"80\" height=\"21\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td align=right><button value=\"Refresh\" action=\"bypass -h admin_view_petitions\" width=\"80\" height=\"21\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br></td></tr>");
        if pending.is_empty() {
            html.push_str("<tr><td>There are no currently pending petitions.</td></tr>");
        } else {
            html.push_str("<tr><td><font color=\"LEVEL\">Current Petitions:</font><br></td></tr>");
        }
        let mut color = true;
        let mut count = 0;
        for petition in &pending {
            html.push_str(&format!(
                "<tr><td width=\"270\"><table width=\"270\" cellpadding=\"2\" bgcolor={}><tr><td width=\"130\">{}",
                if color { "131210" } else { "444444" },
                format_date(petition.submit_time_ms)
            ));
            html.push_str(&format!(
                "</td><td width=\"140\" align=right><font color=\"{}\">{}</font></td></tr>",
                if self.petition_online(petition.petitioner) {
                    "00FF00"
                } else {
                    "999999"
                },
                petition.petitioner_name
            ));
            html.push_str("<tr><td width=\"130\">");
            if petition.state != PetitionState::InProcess {
                html.push_str(&format!(
                    "<table width=\"130\" cellpadding=\"2\"><tr><td><button value=\"View\" action=\"bypass -h admin_view_petition {0}\" width=\"50\" height=\"21\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td><button value=\"Reject\" action=\"bypass -h admin_reject_petition {0}\" width=\"50\" height=\"21\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table>",
                    petition.id
                ));
            } else if let Some((responder, name)) = &petition.responder {
                html.push_str(&format!(
                    "<font color=\"{}\">{name}</font>",
                    if self.petition_online(*responder) {
                        "00FF00"
                    } else {
                        "999999"
                    }
                ));
            }
            html.push_str(&format!(
                "</td>{0}<td width=\"140\" align=right>{0}</td></tr></table></td></tr>",
                petition.kind
            ));
            color = !color;
            count += 1;
            if count > 10 {
                html.push_str(
                    "<tr><td><font color=\"LEVEL\">There is more pending petition...</font><br></td></tr>",
                );
                break;
            }
        }
        html.push_str("</table></center></body></html>");
        // `new NpcHtmlMessage()` + `setHtml(...)`.
        let page = AdminHtml::from_html(&html);
        self.send_admin_html(player, &page);
    }

    /// Java `PetitionManager.viewPetition(player, petitionId)`.
    pub(crate) fn view_petition(&mut self, player: ObjectId, id: i32) {
        if !self.is_gm(player) {
            return;
        }
        let Some(petition) = self.admin.ctl.petitions.pending.get(&id).cloned() else {
            return;
        };
        let mut html = self.admin_html_file(player, "data/html/admin/petition.htm");
        html.replace("%petition%", &petition.id.to_string())
            .replace("%time%", &format_date(petition.submit_time_ms))
            .replace("%type%", petition.kind)
            .replace("%petitioner%", &petition.petitioner_name)
            .replace(
                "%online%",
                if self.petition_online(petition.petitioner) {
                    "00FF00"
                } else {
                    "999999"
                },
            )
            .replace("%text%", &petition.content);
        self.send_admin_html(player, &html);
    }

    /// The petition packets of the client.
    pub(crate) fn on_petition_input(&mut self, player: ObjectId, input: PetitionInput) {
        if !self.players.contains_key(&player) {
            return;
        }
        match input {
            PetitionInput::Submit { content, kind } => self.request_petition(player, &content, kind),
            PetitionInput::Cancel => self.request_petition_cancel(player),
            PetitionInput::Feedback { rate, message } => {
                self.request_petition_feedback(player, rate, message)
            }
        }
    }

    /// Java `RequestPetition.runImpl`.
    fn request_petition(&mut self, player: ObjectId, content: &str, kind: i32) {
        if kind <= 0 || kind >= 10 {
            return;
        }
        if !self.is_gm_online(false) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ThereAreNoGmsCurrentlyVisibleInThePublicListAsTheyMayBePerformingOtherFunctionsAtTheMoment,
                ),
            );
            return;
        }
        let settings = self.admin.ctl.settings.clone();
        if !settings.petitioning_allowed {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheGameClientEncounteredAnErrorAndWasUnableToConnectToThePetitionServer,
                ),
            );
            return;
        }
        if self.is_player_petition_pending(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouMayOnlySubmitOnePetitionActiveAtATime),
            );
            return;
        }
        if self.pending_petition_count() == settings.max_petitions_pending {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ThePetitionSystemIsCurrentlyUnavailablePleaseTryAgainLater,
                ),
            );
            return;
        }
        let total = self.player_total_petition_count(player) + 1;
        if total > settings.max_petitions_per_player {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::WeHaveReceivedS1PetitionsFromYouTodayAndThatIsTheMaximumThatYouCanSubmitInOneDayYouCannotSubmitAnyMorePetitions,
                )
                .with(SystemMessageParam::IntNumber(total)),
            );
            return;
        }
        if content.encode_utf16().count() > 255 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::PetitionsCannotExceed255Characters),
            );
            return;
        }
        let Some(id) = self.submit_petition(player, content, kind) else {
            return;
        };
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YourPetitionApplicationHasBeenAcceptedNReceiptNoIsS1)
                .with(SystemMessageParam::IntNumber(id)),
        );
        self.send(
            player,
            &SystemMessage::new(
                SystemMessageId::YouHaveSubmittedS1PetitionSNYouMaySubmitS2MorePetitionSToday,
            )
            .with(SystemMessageParam::IntNumber(total))
            .with(SystemMessageParam::IntNumber(
                settings.max_petitions_per_player - total,
            )),
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::ThereAreS1PetitionsCurrentlyOnTheWaitingList)
                .with(SystemMessageParam::IntNumber(self.pending_petition_count())),
        );
    }

    /// Java `RequestPetitionCancel.runImpl`.
    fn request_petition_cancel(&mut self, player: ObjectId) {
        if self.is_player_in_consultation(player) {
            if self.is_gm(player) {
                self.end_active_petition(player);
            } else {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YourPetitionIsBeingProcessed),
                );
            }
        } else if self.is_player_petition_pending(player) {
            if self.cancel_active_petition(player) {
                let remaining = self.admin.ctl.settings.max_petitions_per_player
                    - self.player_total_petition_count(player);
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::ThePetitionWasCanceledYouMaySubmitS1MorePetitionSToday,
                    )
                    .with(SystemMessageParam::Text(remaining.to_string())),
                );
                // Notify all GMs that the player's pending petition has been cancelled.
                let name = self.petition_name(player);
                self.broadcast_to_gms(&CreatureSay {
                    sender_object_id: player.0,
                    chat_type: ChatType::HeroVoice,
                    sender: SenderField::Name("Petition System".to_owned()),
                    message_id: -1,
                    body: SayBody::Text(format!("{name} has canceled a pending petition.")),
                });
            } else {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::FailedToCancelPetitionPleaseTryAgainLater),
                );
            }
        } else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveNotSubmittedAPetition),
            );
        }
    }

    /// Java `RequestPetitionFeedback.runImpl`: the row goes to the persistence task.
    fn request_petition_feedback(&mut self, player: ObjectId, rate: i32, message: String) {
        let Some(gm_name) = self.admin.ctl.petitions.last_gm.get(&player).cloned() else {
            return;
        };
        if !(0..=4).contains(&rate) {
            return;
        }
        let name = self.petition_name(player);
        self.outbox.persist(PersistCommand::PetitionFeedback {
            character: name,
            gm: gm_name,
            rate,
            message,
            time: unix_ms_now(),
        });
    }

    /// Java `ChatPetition.onChat`.
    pub(crate) fn chat_petition(&mut self, id: ObjectId, chat_type: ChatType, text: &str) {
        if self.channel_refuses(id, chat_type, true) {
            return;
        }
        if !self.is_player_in_consultation(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreCurrentlyNotInAPetitionChat),
            );
            return;
        }
        self.send_active_petition_message(id, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_types_read_like_the_pages() {
        assert_eq!(PETITION_TYPES[0], "IMMOBILITY");
        assert_eq!(PETITION_TYPES[8], "OTHER");
        assert_eq!(PETITION_TYPES.len(), 9);
    }

    #[test]
    fn dates_are_year_month_day_hour_minute() {
        let text = format_date(0);
        // Whatever the zone of the machine, the shape is fixed.
        assert_eq!(text.len(), 16);
        assert_eq!(&text[4..5], "-");
        assert_eq!(&text[10..11], " ");
    }
}
