//! What the custom event scripts (Elpies, Rabbits, Race, Deathmatch, TvT, CtF) need from the
//! world (M9-C07).
// Java: entity/actor/instance/EventMonster.java
// Java: mechanics/script/Event.java

use l2_data::stats::AbnormalType;
use l2_protocol_game::server::{ChatType, CreatureSay, NpcHtmlMessage, SayBody, SenderField};
use l2_util::ObjectId;

use crate::effects::SkillFinishType;
use l2_protocol_game::server::PartyDistributionType;
use crate::teleport::TeleportTarget;
use crate::zones::ZoneId;
use crate::world::World;

/// The two flags of Java `EventMonster` (`drop_on_ground`, `block_skill_attack`), both `false`
/// until a script sets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventMonsterFlags {
    /// `eventDropOnGround()`.
    pub drop_on_ground: bool,
    /// `eventSkillAttackBlocked()`.
    pub block_skill_attack: bool,
}

impl World {
    /// Java `EventMonster.eventSetDropOnGround(value)`.
    pub fn event_monster_set_drop_on_ground(&mut self, npc: ObjectId, value: bool) {
        self.custom_scripts.event_monsters.entry(npc).or_default().drop_on_ground = value;
    }

    /// Java `EventMonster.eventSetBlockOffensiveSkills(value)`.
    pub fn event_monster_set_block_offensive_skills(&mut self, npc: ObjectId, value: bool) {
        self.custom_scripts
            .event_monsters
            .entry(npc)
            .or_default()
            .block_skill_attack = value;
    }

    /// Java `((reference instanceof EventMonster) && eventDropOnGround())` of `ItemManager`.
    pub fn event_monster_drops_on_ground(&self, npc: ObjectId) -> bool {
        self.npcs.contains_key(&npc)
            && self
                .custom_scripts
                .event_monsters
                .get(&npc)
                .is_some_and(|f| f.drop_on_ground)
    }

    /// Java `((target instanceof EventMonster) && eventSkillAttackBlocked())` of `Player`.
    pub fn event_monster_blocks_skills(&self, npc: ObjectId) -> bool {
        self.npcs.contains_key(&npc)
            && self
                .custom_scripts
                .event_monsters
                .get(&npc)
                .is_some_and(|f| f.block_skill_attack)
    }

    /// Java `player.sendPacket(new CreatureSay(sender, chatType, name, text))` (a plain text
    /// line). `sender` `None` is Java's `null` object (id 0).
    pub fn send_creature_say_text(
        &self,
        to: ObjectId,
        sender: Option<ObjectId>,
        chat_type: ChatType,
        sender_name: &str,
        text: &str,
    ) {
        self.send(
            to,
            &CreatureSay {
                sender_object_id: sender.map_or(0, |s| s.0),
                chat_type,
                sender: SenderField::Name(sender_name.to_owned()),
                message_id: -1,
                body: SayBody::Text(text.to_owned()),
            },
        );
    }

    /// Java `player.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, AbnormalType.SPEED_UP)`.
    pub fn stop_speed_up_effects(&mut self, player: ObjectId) {
        self.stop_skill_effects_by_abnormal(player, SkillFinishType::Removed, AbnormalType::SpeedUp);
    }

    /// Java `player.stopSkillEffects(SkillFinishType.REMOVED, skillId)`.
    pub fn stop_skill_effects_removed(&mut self, player: ObjectId, skill_id: i32) {
        self.stop_skill_effects(player, SkillFinishType::Removed, skill_id);
    }

    /// Java `player.sendPacket(new NpcHtmlMessage())` with `setHtml(text)`: a page without an NPC.
    pub fn send_page_text(&mut self, player: ObjectId, html: &str) {
        if let Some(message) = NpcHtmlMessage::new(0, html) {
            self.send_html(player, &message);
        }
    }

    /// Java `SkillData.getInstance().getSkill(id, level)`: the name of the skill, `None` when the
    /// skill table has no such skill.
    pub fn skill_name_at(&self, skill_id: i32, level: i32) -> Option<String> {
        self.data
            .stat_data()
            .and_then(|d| d.skills.get(skill_id, level))
            .map(|s| s.name.clone())
    }

    /// Java `Player.setTeam(team)` for the event teams (0 none, 1 blue, 2 red): the team colour
    /// ring, sent to everybody who sees the player.
    pub fn set_player_team(&mut self, player: ObjectId, team: u8) {
        self.set_team(player, team);
    }

    /// `RequestRestartPoint`: "Custom event resurrection management". A player on an event whose
    /// `ON_CREATURE_DEATH` listener belongs to an `Event` script has that script resurrect it
    /// (`notifyEvent("ResurrectPlayer", null, player)`). `true` when one did.
    pub(crate) fn custom_event_resurrect(&mut self, player: ObjectId) -> bool {
        let scripts = self.object_listener_scripts(player, crate::script::EventKind::CreatureDeath);
        for script in scripts {
            if self.scripts.script(script).is_some_and(|s| s.is_event()) {
                self.script_notify_event(script, "ResurrectPlayer", None, Some(player));
                return true;
            }
        }
        false
    }

    /// Java `Creature.fullRestore()` of a player: HP, MP and CP to the maximum.
    pub fn player_full_restore(&mut self, player: ObjectId) {
        let Some((hp, mp, cp)) = self
            .stats(player)
            .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp()), f64::from(v.max_cp())))
        else {
            return;
        };
        self.set_current_hp(player, hp, true);
        self.set_current_mp(player, mp, true);
        self.set_current_cp(player, cp, true);
    }

    /// Java `creature.broadcastSocialAction(id)`.
    pub fn broadcast_social_action(&mut self, creature: ObjectId, action_id: i32) {
        self.broadcast_creature_packet(
            creature,
            &l2_protocol_game::server::SocialAction {
                object_id: creature.0,
                action_id,
            },
        );
    }

    /// Java `skill.getHitTime()` and `skill.getReuseDelay()` of a skill ((0, 0) for an unknown one).
    pub fn skill_hit_and_reuse(&self, skill_id: i32, level: i32) -> (i32, i32) {
        self.data
            .stat_data()
            .and_then(|d| d.skills.get(skill_id, level))
            .map_or((0, 0), |s| (s.hit_time, s.reuse_delay))
    }

    /// Java `player.setIsPendingRevive(value)`.
    pub fn player_set_pending_revive(&mut self, player: ObjectId, value: bool) {
        self.set_pending_revive(player, value);
    }

    /// Java `player.leaveParty()`.
    pub fn player_leave_party(&mut self, player: ObjectId) {
        self.leave_party(player);
    }

    /// The event block that makes the parties of one team: `new Party(participant,
    /// FINDERS_KEEPERS)` for the first of every `party_member_count`, the others join it, and
    /// a team larger than one party is put in a command channel. Does nothing for one player.
    pub fn event_make_team_parties(&mut self, team: &[ObjectId], party_member_count: usize) {
        if team.len() <= 1 {
            return;
        }
        let mut channel = None;
        let mut last_party = None;
        let mut counter = 0;
        for &participant in team {
            counter += 1;
            if counter == 1 {
                let party = self.create_party(participant, PartyDistributionType::FindersKeepers);
                last_party = Some(party);
                self.join_party(participant, party);
                if team.len() > party_member_count {
                    match channel {
                        None => channel = self.create_command_channel(participant),
                        Some(channel) => self.command_channel_add_party(channel, party),
                    }
                }
            } else if let Some(party) = last_party {
                self.join_party(participant, party);
            }

            if counter == party_member_count {
                counter = 0;
            }
        }
    }

    /// Java `Creature.teleToLocation(x, y, z, instanceId, randomOffset)` to a point, into an
    /// instance, with the random offset the events use (50).
    pub fn event_teleport(&mut self, player: ObjectId, at: (i32, i32, i32), instance_id: i32, offset: i32) {
        self.tele_to_location(
            player,
            TeleportTarget::at(at.0, at.1, at.2)
                .with_instance(instance_id)
                .with_random_offset(offset),
        );
    }

    /// Java `InstanceWorld.ejectPlayer(player)`: a player inside the instance goes out, to the
    /// exit point or the town.
    pub fn instance_eject_player(&mut self, instance_id: i32, player: ObjectId) {
        if self.players.contains_key(&player) && self.instance_id_of(player) == instance_id {
            let exit = self.instances.instances.get(&instance_id).and_then(|i| i.exit_location);
            self.set_instance_id(player, 0);
            self.teleport_out_of_instance(player, exit);
        }
    }

    /// The pair of lines the events do to a participant and its summon: `setInvul`,
    /// `setImmobilized`, `disableAllSkills` (or the opposite).
    pub fn event_set_player_disabled(&mut self, player: ObjectId, disabled: bool) {
        let mut targets = vec![player];
        if let Some(summon) = self.summon_of(player) {
            targets.push(summon);
        }
        for target in targets {
            self.set_invul(target, disabled);
            self.set_immobilized(target, disabled);
            if disabled {
                self.disable_all_skills(target);
            } else {
                self.enable_all_skills(target);
            }
        }
    }

    /// The registration checks of the PvP events (TvT, Deathmatch, CtF) that come after the
    /// list-size check, each with the lines Java sends. `None` when the player may register.
    pub fn event_registration_refusal(&self, player: ObjectId) -> Option<Vec<&'static str>> {
        if self.stats(player).is_some_and(|v| v.is_flying_mounted()) {
            return Some(vec!["You cannot register on the event while flying."]);
        }
        if self.is_transformed(player) {
            return Some(vec!["You cannot register on the event while on a transformed state."]);
        }
        let under_80 = self
            .stats(player)
            .is_some_and(|view| f64::from(view.non_quest_size()) <= f64::from(view.inventory_limit()) * 0.8);
        if !under_80 {
            return Some(vec![
                "There are too many items in your inventory.",
                "Try removing some items.",
            ]);
        }
        if self.players.get(&player).is_some_and(|p| p.weight_penalty != 0) {
            return Some(vec![
                "Your invetory weight has exceeded the normal limit.",
                "Try removing some items.",
            ]);
        }
        if self.is_cursed_weapon_equipped(player) || self.players.get(&player).is_some_and(|p| p.data.karma > 0) {
            return Some(vec!["People with bad reputation can't register."]);
        }
        if self.is_in_duel(player) {
            return Some(vec!["You cannot register while on a duel."]);
        }
        if self.is_in_olympiad_mode(player) || self.olympiad_is_registered(player) {
            return Some(vec!["You cannot participate while registered on the Olympiad."]);
        }
        if self.instance_id_of(player) > 0 {
            return Some(vec!["You cannot register while in an instance."]);
        }
        if self.is_in_siege(player) || self.is_inside_zone(player, ZoneId::Siege) {
            return Some(vec!["You cannot register while on a siege."]);
        }
        if self.is_fishing(player) {
            return Some(vec!["You cannot register while fishing."]);
        }
        None
    }
}
