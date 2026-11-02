//! What the custom event scripts (Elpies, Rabbits, Race, Deathmatch, TvT, CtF) need from the
//! world (M9-C07).
// Java: entity/actor/instance/EventMonster.java
// Java: mechanics/script/Event.java

use l2_data::stats::AbnormalType;
use l2_protocol_game::server::{ChatType, CreatureSay, NpcHtmlMessage, SayBody, SenderField};
use l2_util::ObjectId;

use crate::effects::SkillFinishType;
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
}
