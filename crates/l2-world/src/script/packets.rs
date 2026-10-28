//! The quest journal and the tutorial window: `QuestList`, the quest states read at
//! `CharacterSelect`, and the client requests of the journal and the tutorial.
// Java: network/serverpackets/QuestList.java
// Java: network/clientpackets/{RequestQuestList,RequestQuestAbort,RequestTutorialLinkHtml,
//       RequestTutorialPassCmdToServer,RequestTutorialQuestionMark,RequestTutorialClientEvent}.java
// Java: network/serverpackets/{NpcQuestHtmlMessage,TutorialShowHtml,TutorialCloseHtml}.java
//       (runImpl: the html action cache of their scope)
// Java: mechanics/script/Quest.java (playerEnter)

use l2_protocol_game::server::enterworld::{QuestList, QuestListEntry};
use l2_protocol_game::server::{NpcQuestHtmlMessage, TutorialCloseHtml, TutorialShowHtml};
use l2_util::ObjectId;
use tracing::{debug, warn};

use super::ctx::ScriptCtx;
use super::state::{COMPLETED_STATE_FLAGS_VAR, QuestState, state_id};
use crate::dialog::{HtmlActionScope, build_html_action_cache};
use crate::world::World;

/// The name of the tutorial quest the tutorial packets talk to.
pub const TUTORIAL_QUEST: &str = "Q00255_Tutorial";

/// The client packets of the journal and the tutorial, as the world gets them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestInput {
    /// Java `RequestQuestList`.
    List,
    /// Java `RequestQuestAbort`.
    Abort { quest_id: i32 },
    /// Java `RequestTutorialLinkHtml`.
    TutorialLinkHtml { bypass: String },
    /// Java `RequestTutorialPassCmdToServer`.
    TutorialPassCmd { bypass: String },
    /// Java `RequestTutorialQuestionMark`.
    TutorialQuestionMark { number: i32 },
    /// Java `RequestTutorialClientEvent`.
    TutorialClientEvent { event_id: i32 },
    /// Java `EndScenePlayer` (S-API-03).
    EndScene { movie_id: i32 },
}

impl World {
    /// Java `new QuestList(player)`: the started journal quests in the order of the player's
    /// quest map; each with its skipped-step flags when they are positive, else its condition.
    pub fn quest_list(&self, player: ObjectId) -> QuestList {
        let mut list = QuestList::default();
        let Some(quests) = self.scripts.players.get(&player) else {
            return list;
        };
        for script in self.all_active_quests(player) {
            let quest_id = self.scripts.quest_id_of(script);
            let status = match quests.get(self.scripts.name_of(script)) {
                None => 0,
                Some(qs) => {
                    let states = qs.get_int(COMPLETED_STATE_FLAGS_VAR);
                    // NOTE(java-quirk): the flags `setCond` writes have the sign bit set, so they
                    // are never `> 0` and the condition goes out instead.
                    if states > 0 { states } else { qs.cond() }
                }
            };
            list.quests.push(QuestListEntry { quest_id, status });
        }
        list
    }

    /// `player.sendPacket(new QuestList(player))`.
    pub(crate) fn send_quest_list(&self, player: ObjectId) {
        if self.players.contains_key(&player) {
            self.send(player, &self.quest_list(player));
        }
    }

    /// `player.sendPacket(NpcQuestHtmlMessage)`: the `NPC_QUEST_HTML` action cache, then the page.
    pub(crate) fn send_quest_html(&mut self, to: ObjectId, message: &NpcQuestHtmlMessage) -> bool {
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(HtmlActionScope::NpcQuestHtml);
            build_html_action_cache(
                &mut player.talk,
                HtmlActionScope::NpcQuestHtml,
                message.npc_obj_id,
                &message.html,
            );
        }
        self.send(to, message)
    }

    /// `player.sendPacket(new TutorialShowHtml(html))`: the `TUTORIAL_HTML` action cache, then
    /// the window.
    pub fn send_tutorial_html(&mut self, to: ObjectId, html: &str) -> bool {
        let packet = TutorialShowHtml::new(html);
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(HtmlActionScope::TutorialHtml);
            build_html_action_cache(&mut player.talk, HtmlActionScope::TutorialHtml, 0, &packet.html);
        }
        self.send(to, &packet)
    }

    /// `player.sendPacket(TutorialCloseHtml.STATIC_PACKET)`: its `runImpl` clears the tutorial
    /// actions.
    pub fn send_tutorial_close_html(&mut self, to: ObjectId) -> bool {
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(HtmlActionScope::TutorialHtml);
        }
        self.send(to, &TutorialCloseHtml)
    }

    /// Java `Quest.playerEnter(player)`: the quest states and variables the session read at
    /// `CharacterSelect` (D-15) become the player's quest states.
    pub(crate) fn script_player_enter(&mut self, id: ObjectId) {
        let Some(load) = self
            .players
            .get_mut(&id)
            .map(|p| std::mem::take(&mut p.data.profile.quests))
        else {
            return;
        };
        let mut quests = self.scripts.players.remove(&id).unwrap_or_default();
        let mut aborted = false;
        for (name, value) in &load.states {
            let Some(script) = self.scripts.by_name(name) else {
                // NOTE(D-39): Java deletes the rows of an unknown quest with
                // `AutoDeleteInvalidQuestData`. This server does not have every script yet, so an
                // unknown name is a quest the Java game server owns: its rows are never deleted.
                debug!("Unknown quest {name} for {id}");
                continue;
            };
            let Some(value) = value else {
                // NOTE(java-quirk): `State.getStateId(null)` throws; Java logs "could not insert
                // char quest" and reads nothing more.
                warn!("could not insert char quest: {name} of {id} has no state");
                aborted = true;
                break;
            };
            quests.insert(QuestState::new(self.scripts.name_of(script), state_id(value)));
        }
        if !aborted {
            for (name, var, value) in &load.variables {
                let Some(script) = self.scripts.by_name(name) else {
                    continue;
                };
                let quest_name = self.scripts.name_of(script);
                // `quest.getQuestState(player, true)`: a variable without a state row makes a
                // created state.
                if !quests.contains(quest_name) {
                    quests.insert(QuestState::new(quest_name, super::CREATED));
                }
                if let Some(qs) = quests.get_mut(quest_name) {
                    qs.set_internal(var, value.as_deref());
                }
            }
        }
        // NOTE(java-quirk): Java ends with `player.processScriptEvent(name, "enter")` for every
        // script; with no NPC in reach at login it never reaches a script, so it is left out.
        self.scripts.players.insert(id, quests);
    }

    /// The journal and tutorial packets.
    pub(crate) fn on_quest_input(&mut self, id: ObjectId, input: QuestInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            QuestInput::List => self.send_quest_list(id),
            QuestInput::Abort { quest_id } => self.on_request_quest_abort(id, quest_id),
            QuestInput::TutorialLinkHtml { bypass } => {
                if bypass.eq_ignore_ascii_case("close") {
                    self.send_tutorial_close_html(id);
                }
                self.tutorial_bypass(id, &bypass, "TL:");
            }
            QuestInput::TutorialPassCmd { bypass } => {
                self.tutorial_bypass(id, &bypass, "TC:");
                // "Notify scripts." (S-API-03)
                self.script_on_player_bypass(id, &bypass);
            }
            QuestInput::TutorialQuestionMark { number } => {
                self.tutorial_event(id, &format!("QM:{number}"));
                // "Notify scripts." (S-API-03)
                self.script_on_tutorial_mark(id, number);
            }
            QuestInput::TutorialClientEvent { event_id } => {
                self.tutorial_event(id, &format!("CE:{event_id}"));
            }
            QuestInput::EndScene { movie_id } => self.on_end_scene_player(id, movie_id),
        }
    }

    /// Java `RequestQuestAbort.runImpl`.
    fn on_request_quest_abort(&mut self, id: ObjectId, quest_id: i32) {
        let Some(script) = self.scripts.by_quest_id(quest_id) else {
            return;
        };
        let name = self.scripts.name_of(script);
        if !self.scripts.players.get(&id).is_some_and(|q| q.contains(name)) {
            return;
        }
        let qs = super::Qs { player: id, script };
        self.with_script(script, |_, ctx: &mut ScriptCtx<'_>| {
            qs.exit_quest(ctx, true, false)
        });
        self.send_quest_list(id);
    }

    /// `RequestTutorialLinkHtml` / `RequestTutorialPassCmdToServer`: a bypass handler of the
    /// command, else the tutorial quest's `<prefix><bypass>` event.
    fn tutorial_bypass(&mut self, id: ObjectId, bypass: &str, prefix: &str) {
        match self.talk.bypass_handlers.get(bypass) {
            Some(handler) => {
                handler(self, id, bypass, None);
            }
            None => self.tutorial_event(id, &format!("{prefix}{bypass}")),
        }
    }

    /// `player.getQuestState("Q00255_Tutorial")` -> `notifyEvent(event, null, player)`.
    fn tutorial_event(&mut self, id: ObjectId, event: &str) {
        let Some(script) = self.scripts.by_name(TUTORIAL_QUEST) else {
            return;
        };
        if self
            .scripts
            .players
            .get(&id)
            .is_some_and(|q| q.contains(TUTORIAL_QUEST))
        {
            self.script_notify_event(script, event, None, Some(id));
        }
    }
}
