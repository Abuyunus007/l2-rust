//! The words of the admin panel that do nothing in Java: `//set`, `//set_mod` and `//gmon`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAdmin.java (the `admin_set`
// branch, which catches `admin_set_mod` as well, and `admin_gmon`: both are `// nothing`)
//
// The other words of `AdminAdmin` that are not here, and why:
// - `gmliston`, `gmlistoff`, `silence`, `diet`, `tradeoff`: each sets a flag of the GM (the GM
//   list, the message refusal, the diet mode, the trade refusal) that the world does not keep yet.
// - `setconfig` and `config_server`: they change the rates and other config values at runtime,
//   which the settings of the world do not allow.
// - `checkolympiad`, `saveolymp`, `endolympiad` are `olympiad.rs` (M8-ADMIN-07); `sethero` and
//   `givehero` are `olympiad/hero.rs` (M8-HERO-01).

use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// The words the panel registers as no-ops.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_set", admin_noop),
        ("admin_set_mod", admin_noop),
        ("admin_gmon", admin_noop),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminAdmin.onCommand`: the branch is `// nothing`, so no message and no effect.
fn admin_noop(_world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    Ok(())
}
