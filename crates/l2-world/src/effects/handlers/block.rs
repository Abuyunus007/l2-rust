//! The blocks and limits an effect puts on a creature: actions, chat, abnormal slots, buff slots
//! and single skills.
// Java: dist/game/data/scripts/handlers/skill/effects/BlockAction.java
// Java: dist/game/data/scripts/handlers/skill/effects/BlockChat.java
// Java: dist/game/data/scripts/handlers/skill/effects/BlockAbnormalSlot.java
// Java: dist/game/data/scripts/handlers/skill/effects/EnlargeAbnormalSlot.java
// Java: dist/game/data/scripts/handlers/skill/effects/ResistSkill.java

use std::collections::BTreeSet;

use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;
use l2_data::stats::jnum::{java_split, parse_int};

use super::status_params::{int_or, required_str};
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// `BotReportTable.PARTY_ACTION_BLOCK_ID`.
const PARTY_ACTION_BLOCK_ID: i32 = -3;
/// `BotReportTable.CHAT_BLOCK_ID`.
const CHAT_BLOCK_ID: i32 = -5;

/// Java `BlockAction`: the bot report debuff. It blocks party actions and chat for as long as it
/// lasts.
// NOTE: Java starts a stored `PunishmentTask` (`PARTY_BAN`, `CHAT_BAN`, saved in the `punishments`
// table). The punishment system is not ported, so the bans are flags of the player's control
// state: they end with the effect and are not saved.
#[derive(Debug, Clone)]
pub(crate) struct BlockAction {
    blocked_actions: BTreeSet<i32>,
}

impl BlockAction {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let actions = required_str(template, "blockedActions")?;
        let mut blocked_actions = BTreeSet::new();
        for action in java_split(actions, ',') {
            let id = parse_int(action).ok_or_else(|| format!("For input string: \"{action}\""))?;
            blocked_actions.insert(id);
        }
        Ok(Box::new(BlockAction { blocked_actions }))
    }
}

impl EffectHandler for BlockAction {
    /// `effected != null && effected.isPlayer()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    /// `!_blockedActions.contains(id)`.
    fn check_condition(&self, value: i32) -> bool {
        !self.blocked_actions.contains(&value)
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if self.blocked_actions.contains(&PARTY_ACTION_BLOCK_ID)
            && let Some(c) = world.control_mut(cx.effected)
        {
            c.party_banned = false;
        }
        if self.blocked_actions.contains(&CHAT_BLOCK_ID)
            && let Some(c) = world.control_mut(cx.effected)
        {
            c.chat_banned = false;
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if self.blocked_actions.contains(&PARTY_ACTION_BLOCK_ID)
            && let Some(c) = world.control_mut(cx.effected)
        {
            c.party_banned = true;
        }
        if self.blocked_actions.contains(&CHAT_BLOCK_ID)
            && let Some(c) = world.control_mut(cx.effected)
        {
            c.chat_banned = true;
        }
    }
}

/// Java `BlockChat`: the chat ban of a bot report.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BlockChat;

impl BlockChat {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(BlockChat))
    }
}

impl EffectHandler for BlockChat {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::ChatBlock
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.chat_banned = false;
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(c) = world.control_mut(cx.effected) {
            c.chat_banned = true;
        }
    }
}

/// Java `BlockAbnormalSlot`: skills of these abnormal types do not land while it lasts.
#[derive(Debug, Clone)]
pub(crate) struct BlockAbnormalSlot {
    slots: BTreeSet<AbnormalType>,
}

impl BlockAbnormalSlot {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut slots = BTreeSet::new();
        if let Some(text) = template.param("slot").filter(|t| !t.is_empty()) {
            for slot in java_split(text, ';') {
                // `AbnormalType.getAbnormalType(slot)`: an unknown name is `NONE`.
                slots.insert(AbnormalType::from_name(slot).unwrap_or(AbnormalType::None));
            }
        }
        Ok(Box::new(BlockAbnormalSlot { slots }))
    }
}

impl EffectHandler for BlockAbnormalSlot {
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_blocked_abnormal_types(cx.effected, self.slots.iter().copied());
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.add_blocked_abnormal_types(cx.effected, self.slots.iter().copied());
    }
}

/// Java `EnlargeAbnormalSlot`: more buff slots for a player.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EnlargeAbnormalSlot {
    slots: i32,
}

impl EnlargeAbnormalSlot {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(EnlargeAbnormalSlot {
            slots: int_or(template, "slots", 0)?,
        }))
    }
}

impl EffectHandler for EnlargeAbnormalSlot {
    /// `effector != null && effected != null && effected.isPlayer()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // NOTE(java-quirk): `getMaxBuffCount()` includes the Divine Inspiration levels, which
        // `setMaxBuffCount` then stores into the base.
        let count = world.max_buff_count(cx.effected).wrapping_add(self.slots);
        world.set_max_buff_count(cx.effected, count);
    }

    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        let count = world.max_buff_count(cx.effected).wrapping_sub(self.slots).max(0);
        world.set_max_buff_count(cx.effected, count);
    }
}

/// Java `ResistSkill`: immunity to single skills (by id and level).
#[derive(Debug, Clone)]
pub(crate) struct ResistSkill {
    skills: Vec<(i32, i32)>,
}

impl ResistSkill {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut skills = Vec::new();
        for i in 1.. {
            let skill_id = int_or(template, &format!("skillId{i}"), 0)?;
            if skill_id == 0 {
                break;
            }
            let skill_level = int_or(template, &format!("skillLevel{i}"), 0)?;
            skills.push((skill_id, skill_level));
        }
        if skills.is_empty() {
            return Err("ResistSkill: Without parameters!".to_owned());
        }
        Ok(Box::new(ResistSkill { skills }))
    }
}

impl EffectHandler for ResistSkill {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        for &(id, level) in &self.skills {
            world.add_invul_against(cx.effected, id, level);
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        for &(id, _) in &self.skills {
            world.remove_invul_against(cx.effected, id);
        }
    }
}
