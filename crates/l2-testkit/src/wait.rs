//! Polling helper for state that changes on another task.

use std::time::Duration;

/// Waits until `condition` is true (checked every 25 ms, for at most 5 seconds) and panics
/// otherwise. Use it for effects that happen on a connection task after a packet was sent, e.g.
/// the login server marking a game server as down.
pub async fn until(mut condition: impl FnMut() -> bool) {
    for _ in 0..200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in time");
}
