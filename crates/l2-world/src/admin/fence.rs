//! The fence admin commands: `//addfence <width> <length> <height>` (a closed fence where the GM
//! stands), `//setfencestate <objectId> <state>`, `//removefence <objectId>`, `//listfence [page]`
//! (the fences in `fences.htm`, 10 a page) and `//gofence <objectId>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminFence.java
//
// # Left out, with the system that brings it
// - The instance of a fence: a fence has no instance in the world, so `//addfence` makes it in
//   the common world and `//gofence` teleports to instance 0.
// - A runtime fence has no name (Java's `null`): the name is shown as `null`, and the list shows
//   the object id for it (`fence.getName() == null ? fence.getId() : fence.getName()`).

use l2_data::FenceState;
use l2_data::fence::FenceTemplates;
use l2_util::ObjectId;

use super::html::build_page;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminFence.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_addfence", admin_addfence),
        ("admin_setfencestate", admin_setfencestate),
        ("admin_removefence", admin_removefence),
        ("admin_listfence", admin_listfence),
        ("admin_gofence", admin_gofence),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `FenceState.values().length`: four states, indices 0 to 3.
const STATE_COUNT: i32 = 4;

/// Java `Enum.toString()` of a fence state (the enum name).
fn state_name(state: FenceState) -> &'static str {
    match state {
        FenceState::Hidden => "HIDDEN",
        FenceState::Opened => "OPENED",
        FenceState::Closed => "CLOSED",
        FenceState::ClosedHidden => "CLOSED_HIDDEN",
    }
}

/// `FenceState.values()[ordinal]`: `HIDDEN(0)`, `OPENED(1)`, `CLOSED(2)`, `CLOSED_HIDDEN(0)` in the
/// Java enum, whose ordinals are 0 to 3 in that order.
fn state_of_ordinal(ordinal: i32) -> FenceState {
    match ordinal {
        0 => FenceState::Hidden,
        1 => FenceState::Opened,
        2 => FenceState::Closed,
        _ => FenceState::ClosedHidden,
    }
}

/// The fence's name for a message: a runtime fence has none (Java `null`).
fn fence_name_text(name: &str) -> &str {
    if name.is_empty() { "null" } else { name }
}

/// Java `admin_addfence <width> <length> <height>`.
fn admin_addfence(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let values = (|| -> Option<(i32, i32, i32)> {
        let width = parse_int(st.next_token()?).ok()?;
        let length = parse_int(st.next_token()?).ok()?;
        let height = parse_int(st.next_token()?).ok()?;
        Some((width, length, height))
    })();
    let Some((width, length, height)) = values else {
        world.send_sys_message(gm, "Format must be: //addfence <width> <length> <height>");
        return Ok(());
    };
    if width < 1 || length < 1 {
        world.send_sys_message(gm, "Width and length values must be positive numbers.");
        return Ok(());
    }
    if !(1..=3).contains(&height) {
        world.send_sys_message(gm, "The range for height can only be 1-3.");
        return Ok(());
    }
    let (x, y, z) = world.position_of(gm).unwrap_or((0, 0, 0));
    // `FenceData.spawnFence` makes the fence from its numbers: one template of the fence list.
    let xml = format!(
        "<list><fence name=\"\" x=\"{x}\" y=\"{y}\" z=\"{z}\" width=\"{width}\" length=\"{length}\" height=\"{height}\" state=\"CLOSED\"/></list>"
    );
    let templates = FenceTemplates::from_xml(&xml).map_err(|e| JavaException(e.to_string()))?;
    let ids = world
        .npc_world
        .ids
        .clone()
        .ok_or_else(|| JavaException("java.lang.IllegalStateException: no object ids".to_owned()))?;
    world
        .spawn_fences(&templates, &ids)
        .map_err(|e| JavaException(e.to_string()))?;
    world.send_sys_message(gm, "Fence added successfully.");
    Ok(())
}

/// Java `admin_setfencestate <objectId> <fenceState>`.
fn admin_setfencestate(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let values = (|| -> Option<(i32, i32)> {
        let object = parse_int(st.next_token()?).ok()?;
        let ordinal = parse_int(st.next_token()?).ok()?;
        Some((object, ordinal))
    })();
    let Some((object, ordinal)) = values else {
        world.send_sys_message(
            gm,
            "Format mustr be: //setfencestate <fenceObjectId> <fenceState>",
        );
        return Ok(());
    };
    if !(0..STATE_COUNT).contains(&ordinal) {
        world.send_sys_message(
            gm,
            &format!(
                "Specified FenceType is out of range. Only 0-{} are permitted.",
                STATE_COUNT - 1
            ),
        );
        return Ok(());
    }
    let id = ObjectId(object);
    let Some(fence) = world.fence(id).map(|f| f.name.clone()) else {
        world.send_sys_message(gm, "Target is not a fence.");
        return Ok(());
    };
    let state = state_of_ordinal(ordinal);
    world.set_fence_state(id, state);
    world.send_sys_message(
        gm,
        &format!(
            "Fence {}[{}]'s state has been changed to {}",
            fence_name_text(&fence),
            object,
            state_name(state)
        ),
    );
    Ok(())
}

/// Java `admin_removefence <objectId>`: the fence goes, then the list (page 0) is shown.
fn admin_removefence(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let object = st.next_token().and_then(|v| parse_int(v).ok());
    match object {
        Some(object) => {
            if world.fence(ObjectId(object)).is_some() {
                world.remove_fence(ObjectId(object));
                world.send_sys_message(gm, "Fence removed successfully.");
            } else {
                world.send_sys_message(gm, "Target is not a fence.");
            }
        }
        None => world.send_sys_message(gm, "Invalid object ID or target was not found."),
    }
    send_html(world, gm, 0);
    Ok(())
}

/// Java `admin_listfence [page]`: a page that is not a number is an exception.
fn admin_listfence(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let page = match st.next_token() {
        Some(value) => parse_int(value)?,
        None => 0,
    };
    send_html(world, gm, page);
    Ok(())
}

/// Java `admin_gofence <objectId>`: the GM goes to the object's place (any object).
fn admin_gofence(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let object = st.next_token().and_then(|v| parse_int(v).ok());
    let Some(object) = object else {
        world.send_sys_message(gm, "Invalid object ID or target was not found.");
        return Ok(());
    };
    let id = ObjectId(object);
    let place = world
        .fence(id)
        .map(|f| (f.x, f.y, f.z))
        .or_else(|| world.position_of(id));
    if let Some((x, y, z)) = place {
        // `teleToLocation(loc)`: no random offset, the object's heading (a fence has none) and
        // instance 0 (see the module note).
        world.tele_to_location(gm, TeleportTarget::at(x, y, z));
    }
    Ok(())
}

/// Java `AdminFence.sendHtml(activeChar, page)`: `fences.htm` with the fences of the page.
fn send_html(world: &mut World, gm: ObjectId, page: i32) {
    let mut fences: Vec<(i32, String)> = world.fences().map(|f| (f.object_id.0, f.name.clone())).collect();
    fences.sort_by_key(|(id, _)| *id);
    let result = build_page(
        &fences,
        10,
        "bypass -h admin_listfence",
        page,
        |(id, name), sb| {
            let shown = if name.is_empty() {
                id.to_string()
            } else {
                name.clone()
            };
            sb.push_str("<tr><td>");
            sb.push_str(&shown);
            sb.push_str("</td><td>");
            sb.push_str(&format!(
            "<button value=\"Go\" action=\"bypass -h admin_gofence {id}\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
            sb.push_str("</td><td>");
            sb.push_str(&format!(
            "<button value=\"Hide\" action=\"bypass -h admin_setfencestate {id} 0\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
            sb.push_str("</td><td>");
            sb.push_str(&format!(
            "<button value=\"Off\" action=\"bypass -h admin_setfencestate {id} 1\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
            sb.push_str("</td><td>");
            sb.push_str(&format!(
            "<button value=\"On\" action=\"bypass -h admin_setfencestate {id} 2\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
            sb.push_str("</td><td>");
            sb.push_str(&format!(
            "<button value=\"X\" action=\"bypass -h admin_removefence {id}\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        ));
            sb.push_str("</td></tr>");
        },
    );
    let mut html = world.admin_html_file(gm, "data/html/admin/fences.htm");
    html.replace("%pages%", &result.pager_html());
    html.replace("%fences%", &result.body);
    world.send_admin_html(gm, &html);
}
