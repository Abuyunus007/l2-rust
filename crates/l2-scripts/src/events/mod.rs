//! `data/scripts/events`: the seasonal events.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].
//! `long_time_event` is Java's `LongTimeEvent` base class (no script of its own).

pub mod character_birthday;
pub mod freya_celebration;
pub mod gift_of_vitality;
pub mod heavy_medal;
pub mod l2_day;
pub mod long_time_event;
pub mod love_your_gatekeeper;
pub mod master_of_enchanting;
pub mod saving_santa;
pub mod the_valentine_event;
pub mod watermelon_ninja;

use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField};
use l2_world::{ScriptCtx, ScriptNpc, ScriptRegistry};

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(character_birthday::CharacterBirthday);
    r.add(freya_celebration::FreyaCelebration);
    r.add(gift_of_vitality::GiftOfVitality);
    r.add(heavy_medal::HeavyMedal);
    r.add(l2_day::L2Day);
    r.add(love_your_gatekeeper::LoveYourGatekeeper);
    r.add(master_of_enchanting::MasterOfEnchanting);
    r.add(saving_santa::SavingSanta);
    r.add(the_valentine_event::TheValentineEvent);
    r.add(watermelon_ninja::WatermelonNinja);
}

/// Java `npc.broadcastPacket(new CreatureSay(npc, chatType, npc.getName(), text))`: a `SAY2` line
/// of the NPC to the players that see it (the events do not use `NpcSay`).
pub(crate) fn broadcast_say_text(ctx: &ScriptCtx<'_>, npc: ScriptNpc, chat_type: ChatType, text: &str) {
    let packet = CreatureSay {
        sender_object_id: npc.object_id.0,
        chat_type,
        sender: SenderField::Name(ctx.world().npc_template_name(npc.object_id)),
        message_id: -1,
        body: SayBody::Text(text.to_owned()),
    };
    ctx.world().broadcast_packet(npc.object_id, &packet);
}

/// Java `npc.broadcastPacket(cs)` of `new CreatureSay(npc, chatType, npcStringId)` with the
/// `addStringParameter` calls: the sender is the NPC's name, the line is the NPC string.
pub(crate) fn broadcast_say_string(
    ctx: &ScriptCtx<'_>,
    npc: ScriptNpc,
    chat_type: ChatType,
    npc_string_id: i32,
    parameters: Vec<String>,
) {
    let packet = CreatureSay {
        sender_object_id: npc.object_id.0,
        chat_type,
        sender: SenderField::Name(ctx.world().npc_template_name(npc.object_id)),
        message_id: npc_string_id,
        body: SayBody::Parameters(parameters),
    };
    ctx.world().broadcast_packet(npc.object_id, &packet);
}

/// The hours and minutes of a reuse still to wait, as three scripts cut them:
/// `remainingTime = (reuse - now) / 1000; hours = (int) (remainingTime / 3600);
/// minutes = (int) ((remainingTime % 3600) / 60)`.
pub(crate) fn hours_and_minutes(remaining_ms: i64) -> (i32, i32) {
    let remaining_time = remaining_ms / 1000;
    (
        i32::try_from(remaining_time / 3600).unwrap_or(i32::MAX),
        i32::try_from((remaining_time % 3600) / 60).unwrap_or(0),
    )
}

/// A click on a link of the script's last page that keeps what the click sent: the page and the
/// names of the packets (`ScriptTest::event` throws the packets away).
#[cfg(test)]
pub(crate) fn click(
    t: &mut crate::testing::ScriptTest,
    player: i32,
    script: &str,
    event: &str,
) -> (String, Vec<String>) {
    t.bypass(player, &format!("Script {script} {event}"));
    let out = t.take();
    (
        crate::testing::pages_to(&out, player).pop().unwrap_or_default(),
        crate::testing::names_to(&out, player),
    )
}

#[cfg(test)]
mod tests {
    use super::hours_and_minutes;

    #[test]
    fn the_wait_is_cut_into_hours_and_minutes() {
        assert_eq!(hours_and_minutes(3_600_000 * 5 + 61_000), (5, 1));
        assert_eq!(hours_and_minutes(59_999), (0, 0));
    }
}
