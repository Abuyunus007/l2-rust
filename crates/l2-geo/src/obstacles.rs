//! The hook for dynamic obstacles (doors and fences).
//!
//! Java: the `DoorData.getInstance().checkIfDoorsBetween(...)` and
//! `FenceData.getInstance().checkIfFenceBetween(...)` calls inside `GeoEngine`
//! (`canSeeTarget`, `getValidLocation`, `canMoveToTarget`).
//!
//! The doors and fences themselves do not exist in Rust yet. The engine asks a [`DynamicObstacles`]
//! implementation exactly where Java asks `DoorData` / `FenceData`, with the same values (Java's
//! `x, y, z, tx, ty, tz` are `from` and `to` here), so the later door / fence port only has to
//! implement this trait. [`NoObstacles`] answers "nothing in the way" and is the default.
//!
//! The trait is `Send + Sync` because the engine is shared between the world loop and worker
//! threads (D-14); an implementation that reads live door state must be a snapshot or handle that
//! is safe to read from several threads.

use crate::coords::Location;

/// What `GeoEngine` asks about doors and fences. All coordinates are **world** coordinates.
pub trait DynamicObstacles: Send + Sync {
    /// `DoorData.checkIfDoorsBetween(x, y, z, tx, ty, tz, instanceId, doubleFaceCheck)`: is a closed
    /// door on the segment? `double_face_check` is true for line of sight (`canSeeTarget`) and false
    /// for movement. Java evaluates this before the geodata is looked at.
    fn doors_between(&self, from: Location, to: Location, instance_id: i32, double_face_check: bool) -> bool;

    /// `FenceData.checkIfFenceBetween(x, y, z, tx, ty, tz, instanceId)`: is a fence on the segment?
    fn fence_between(&self, from: Location, to: Location, instance_id: i32) -> bool;
}

/// No doors, no fences. Equivalent to Java with empty `DoorData` / `FenceData`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoObstacles;

impl DynamicObstacles for NoObstacles {
    fn doors_between(
        &self,
        _from: Location,
        _to: Location,
        _instance_id: i32,
        _double_face_check: bool,
    ) -> bool {
        false
    }

    fn fence_between(&self, _from: Location, _to: Location, _instance_id: i32) -> bool {
        false
    }
}
