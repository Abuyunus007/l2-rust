#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! The world holds the geodata and ignores `PathFound` until the movement code exists
//! (M3-GEO-03).

use std::sync::Arc;
use std::time::Duration;

use l2_geo::{GeoEngine, GeoEngineOptions, GeoSet, PathFailure, PathResult};
use l2_world::*;

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

#[test]
fn the_world_keeps_the_geo_engine_and_ignores_path_results() {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    assert!(world.geo().is_none());
    let engine = Arc::new(GeoEngine::new(GeoSet::new(), GeoEngineOptions::default()));
    world.set_geo(Arc::clone(&engine));
    assert!(Arc::ptr_eq(world.geo().unwrap(), &engine));

    let command = WorldCommand::PathFound {
        requester: 7,
        request_id: 3,
        result: PathResult {
            requester: 7,
            id: 3,
            path: Err(PathFailure::NoPath),
            elapsed: Duration::ZERO,
        },
    };
    assert_eq!(format!("{command:?}"), "PathFound(7, 3, no path)");
    world.handle(command);
    world.tick();
}
