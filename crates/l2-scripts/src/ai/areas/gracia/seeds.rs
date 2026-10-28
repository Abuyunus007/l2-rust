//! What the scripts of Gracia read from Java's seed managers.
// Java: gameserver/managers/SeedOfInfinityManager.java, SeedOfDestructionManager.java
//
// The managers keep their state in `global_variables` (`SoI_stage`, ...), which the scripts read
// through the context (D-54: re-read and in memory; written only with `CycleDatabaseWrites`).

use l2_world::ScriptCtx;

/// Java `SeedOfInfinityManager.getCurrentStage()`.
pub fn soi_current_stage(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoI_stage", 1)
}

/// Java `SeedOfDestructionManager.getInstance().getSoDState()`: 1 until a state is stored.
pub fn sod_state(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoDState", 1)
}

/// Java `SeedOfInfinityManager.getOpenedTime()`, in milliseconds (`now` is `System.currentTimeMillis()`).
pub fn soi_opened_time(ctx: &ScriptCtx<'_>, now: i64) -> i64 {
    if soi_current_stage(ctx) != 3 {
        return 0;
    }
    (ctx.global_variables().get_long("SoI_opened", 0) * 1000) - now
}
