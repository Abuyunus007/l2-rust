//! Runs a [`World`] on its own thread: drain commands, tick every 50 ms (D-10, D-12).

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use tracing::{info, warn};

use crate::command::WorldCommand;
use crate::world::{TICK, World};

/// Commands handled per tick at most, so a flood can't starve the tick. The rest waits.
pub const MAX_COMMANDS_PER_TICK: usize = 10_000;

/// Starts the world thread. It ends after `WorldCommand::Shutdown`, or when every sender is
/// dropped (then it stores all players first). Returns the world for inspection.
pub fn spawn(world: World, commands: Receiver<WorldCommand>) -> std::io::Result<JoinHandle<World>> {
    thread::Builder::new().name("world".into()).spawn(move || {
        let mut world = world;
        run(&mut world, &commands);
        world
    })
}

fn run(world: &mut World, commands: &Receiver<WorldCommand>) {
    let mut next_tick = Instant::now() + TICK;
    loop {
        let mut handled = 0usize;
        loop {
            let now = Instant::now();
            if now >= next_tick {
                break;
            }
            if handled >= MAX_COMMANDS_PER_TICK {
                warn!("more than {MAX_COMMANDS_PER_TICK} commands in one tick; the rest waits");
                thread::sleep(next_tick - now);
                break;
            }
            match commands.recv_timeout(next_tick - now) {
                Ok(command) => {
                    world.handle(command);
                    handled += 1;
                    if world.is_stopped() {
                        info!("World thread stopped.");
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    warn!("World command channel closed; shutting the world down.");
                    world.handle(WorldCommand::Shutdown {
                        done: Box::new(|| {}),
                    });
                    return;
                }
            }
        }

        let started = Instant::now();
        world.tick();
        let took = started.elapsed();
        if took > TICK {
            warn!(
                "World tick {} took {} ms (budget {} ms).",
                world.tick_count(),
                took.as_millis(),
                TICK.as_millis()
            );
        }
        next_tick += TICK;
        let now = Instant::now();
        if next_tick < now {
            // Behind schedule: start the next tick right away, but don't try to catch up.
            next_tick = now;
        }
    }
}
