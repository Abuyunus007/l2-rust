//! One bot: the whole way in through the login server and the game server, then a simple loop.
//!
//! The way in is the real client's: login (GameGuard, RSA `RequestAuthLogin`), server list,
//! `RequestServerLogin`, game server handshake, `AuthLogin`, create the character when the
//! account has none, select it, `EnterWorld`.
//!
//! The loop (about two actions a second): walk to a random spot near the start, say something,
//! click and attack a monster, rarely restart to the character screen and come back. After each
//! action the bot reads what the server sends for a moment (that is what counts as packets per
//! second). The bots only know what the server tells them: a "monster" is an NPC the server
//! announced as attackable (`NPC_INFO`). In a town there are none, so the combat share becomes a
//! walk until one shows up.
//!
//! The fake clients panic on a protocol surprise. The runner catches that per bot task.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::RequestRestart;
use l2_protocol_game::server::{CharSelected, KeyPacket, NpcInfo};
use l2_testkit::fake_game_client::{FakeGameClient, Recv, character_create, packet_name};
use l2_testkit::fake_login_client::FakeClient as LoginClient;

use crate::stats::Stats;

/// A protocol revision the game server accepts by default (same as the e2e tests).
pub const PROTOCOL: i32 = 273;
/// Chat type "all" (`Say2.ALL`).
const CHAT_ALL: i32 = 0;
/// How far from the start position a bot walks.
const WALK_RADIUS: i32 = 300;
/// Monsters a bot remembers.
const MAX_MONSTERS: usize = 32;
/// How long a restart waits for the character screen.
const RESTART_WAIT: Duration = Duration::from_secs(10);
/// How long the logout at the end waits for the server to close the connection.
const LOGOUT_WAIT: Duration = Duration::from_secs(3);

/// What one bot needs.
#[derive(Debug, Clone)]
pub struct BotContext {
    pub login_addr: SocketAddr,
    pub account: String,
    pub password: String,
    pub stats: Arc<Stats>,
    /// The bot stops playing at this time.
    pub deadline: Instant,
    /// Seed of this bot's random choices.
    pub seed: u64,
}

/// How a bot ended without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotEnd {
    /// It played until the deadline and logged out.
    Finished,
    /// The server closed the connection while it played (already counted in the stats).
    Disconnected,
}

/// SplitMix64: a tiny seeded generator, so a run with the same seed makes the same choices.
#[derive(Debug, Clone)]
pub struct BotRng(u64);

impl BotRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` must not be 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// A number in `-radius..=radius`.
    pub fn offset(&mut self, radius: i32) -> i32 {
        let span = u64::from(radius.unsigned_abs()) * 2 + 1;
        i32::try_from(self.below(span)).unwrap_or(0) - radius.abs()
    }
}

/// The seed of bot `index` for a run seeded with `seed`.
pub fn bot_seed(seed: u64, index: u32) -> u64 {
    BotRng::new(seed ^ (u64::from(index) << 32 | 0x5EED)).next_u64()
}

/// What a bot does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Walk,
    Chat,
    Combat,
    Restart,
    Idle,
}

/// The loop's dice: 58 % walk, 12 % chat, 22 % combat, 0.5 % restart, the rest idles. About two
/// actions a second, so a restart comes every few minutes per bot.
pub fn pick_action(rng: &mut BotRng) -> Action {
    match rng.below(1000) {
        0..580 => Action::Walk,
        580..700 => Action::Chat,
        700..920 => Action::Combat,
        920..925 => Action::Restart,
        _ => Action::Idle,
    }
}

/// An attackable NPC the server announced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Monster {
    object_id: i32,
}

/// What a bot learned from the packets it read.
#[derive(Debug, Default)]
struct Seen {
    monsters: Vec<Monster>,
    died: bool,
    teleported: bool,
}

impl Seen {
    fn observe(&mut self, body: &[u8]) {
        match packet_name(body).as_str() {
            "NPC_INFO" => {
                let Ok(npc) = NpcInfo::decode(&mut PacketReader::new(&body[1..])) else {
                    return;
                };
                if npc.attackable
                    && self.monsters.len() < MAX_MONSTERS
                    && !self.monsters.iter().any(|m| m.object_id == npc.object_id)
                {
                    self.monsters.push(Monster { object_id: npc.object_id });
                }
            }
            "DIE" => self.died = true,
            "TELEPORT_TO_LOCATION" => self.teleported = true,
            _ => {}
        }
    }
}

/// How a read ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    Open,
    Closed,
}

/// Reads and counts what the server sends until `until`.
async fn drain(game: &mut FakeGameClient, stats: &Stats, seen: &mut Seen, until: Instant) -> Read {
    loop {
        let now = Instant::now();
        if now >= until {
            return Read::Open;
        }
        match game.recv_within(until - now).await {
            Recv::Packet(body) => {
                stats.packets_rx.fetch_add(1, Ordering::Relaxed);
                game.clear_log();
                seen.observe(&body);
            }
            Recv::Closed => return Read::Closed,
            Recv::Silent => return Read::Open,
        }
    }
}

/// Reads until a packet called `name` arrives. `false`: closed or too slow.
async fn wait_for(game: &mut FakeGameClient, stats: &Stats, name: &str, within: Duration) -> bool {
    let until = Instant::now() + within;
    loop {
        let now = Instant::now();
        if now >= until {
            return false;
        }
        match game.recv_within(until - now).await {
            Recv::Packet(body) => {
                stats.packets_rx.fetch_add(1, Ordering::Relaxed);
                game.clear_log();
                if packet_name(&body) == name {
                    return true;
                }
            }
            Recv::Closed | Recv::Silent => return false,
        }
    }
}

/// `CharacterSelect` for the first character and `EnterWorld`. Returns the start position.
async fn enter(game: &mut FakeGameClient, stats: &Stats) -> (i32, i32, i32) {
    let started = Instant::now();
    let selected: CharSelected = game.select_character(0).await;
    game.enter_world().await;
    game.clear_log();
    stats.record_enter(started.elapsed());
    stats.world_entered();
    (selected.x, selected.y, selected.z)
}

/// Runs one bot to the end of the run.
pub async fn run_bot(ctx: BotContext) -> Result<BotEnd, String> {
    let stats = &*ctx.stats;
    let mut rng = BotRng::new(ctx.seed);

    // The login server.
    let started = Instant::now();
    let mut login = LoginClient::connect(ctx.login_addr).await;
    stats.record_connect(started.elapsed());
    let ok = login.login_ok(&ctx.account, &ctx.password).await;
    let list = login.server_list(&ok).await;
    let server = list
        .servers
        .iter()
        .find(|s| s.up)
        .ok_or("the login server lists no game server that is up")?;
    let play = login.play_ok(&ok, server.server_id).await;
    stats.record_login(started.elapsed());

    // The game server. 0.0.0.0 in the list means "the host you reached me on".
    let [a, b, c, d] = server.ip;
    let ip = Ipv4Addr::new(a, b, c, d);
    let port = u16::try_from(server.port).map_err(|_| format!("bad game server port {}", server.port))?;
    let game_addr = if ip.is_unspecified() {
        SocketAddr::new(ctx.login_addr.ip(), port)
    } else {
        SocketAddr::from((ip, port))
    };
    let started = Instant::now();
    let mut game = FakeGameClient::connect(game_addr).await;
    let key = game.handshake_any(PROTOCOL).await;
    if key.result != KeyPacket::RESULT_OK {
        return Err(format!("the game server refused protocol {PROTOCOL}"));
    }
    game.auth(&ctx.account, &ok, &play).await;
    let characters = game.char_selection_info().await;
    stats.record_game(started.elapsed());
    // The real client keeps the login connection open a while; the game server does not need it.
    drop(login);

    if characters.characters.is_empty() {
        // Class 0 is a human fighter, a starting class in every datapack.
        game.create_character(&character_create(&ctx.account, 0))
            .await
            .map_err(|reason| format!("character creation failed (reason {reason})"))?;
        game.goto_lobby().await;
    }
    let home = enter(&mut game, stats).await;
    let mut pos = home;
    let mut seen = Seen::default();

    let mut end = BotEnd::Finished;
    'play: while Instant::now() < ctx.deadline {
        match pick_action(&mut rng) {
            Action::Walk => {
                let target = (
                    home.0 + rng.offset(WALK_RADIUS),
                    home.1 + rng.offset(WALK_RADIUS),
                    home.2,
                );
                game.move_to(target, pos).await;
                pos = target;
                stats.packets_tx.fetch_add(1, Ordering::Relaxed);
            }
            Action::Chat => {
                game.say(CHAT_ALL, &format!("hello {}", rng.below(1000)), None).await;
                stats.packets_tx.fetch_add(1, Ordering::Relaxed);
            }
            Action::Combat => {
                if seen.monsters.is_empty() {
                    let target = (
                        home.0 + rng.offset(WALK_RADIUS),
                        home.1 + rng.offset(WALK_RADIUS),
                        home.2,
                    );
                    game.move_to(target, pos).await;
                    pos = target;
                    stats.packets_tx.fetch_add(1, Ordering::Relaxed);
                } else {
                    let pick = seen.monsters[usize::try_from(rng.below(seen.monsters.len() as u64)).unwrap_or(0)];
                    // The first click targets, the attack request after it hits.
                    game.click(pick.object_id, pos).await;
                    game.attack(pick.object_id, pos).await;
                    stats.packets_tx.fetch_add(2, Ordering::Relaxed);
                }
            }
            Action::Restart => {
                game.send_packet(&RequestRestart).await;
                stats.packets_tx.fetch_add(1, Ordering::Relaxed);
                stats.world_left();
                if !wait_for(&mut game, stats, "CHARACTER_SELECTION_INFO", RESTART_WAIT).await {
                    stats.disconnects.fetch_add(1, Ordering::Relaxed);
                    end = BotEnd::Disconnected;
                    break 'play;
                }
                stats.restarts.fetch_add(1, Ordering::Relaxed);
                pos = enter(&mut game, stats).await;
                seen = Seen::default();
            }
            Action::Idle => {}
        }

        let listen = Duration::from_millis(300 + rng.below(600));
        let until = (Instant::now() + listen).min(ctx.deadline.max(Instant::now()));
        if drain(&mut game, stats, &mut seen, until).await == Read::Closed {
            stats.world_left();
            stats.disconnects.fetch_add(1, Ordering::Relaxed);
            end = BotEnd::Disconnected;
            break 'play;
        }
        if seen.teleported {
            seen.teleported = false;
            game.appearing().await;
            stats.packets_tx.fetch_add(1, Ordering::Relaxed);
        }
        if seen.died {
            seen.died = false;
            // "To the village" (type 0); the death window would otherwise keep the bot dead.
            game.restart_point(0).await;
            stats.packets_tx.fetch_add(1, Ordering::Relaxed);
            pos = home;
        }
    }

    if end == BotEnd::Finished {
        game.send_packet(&l2_protocol_game::client::Logout).await;
        stats.packets_tx.fetch_add(1, Ordering::Relaxed);
        stats.world_left();
        // Give the server a moment to close the connection (its answer is not interesting).
        let _ = wait_for(&mut game, stats, "\u{0}never", LOGOUT_WAIT).await;
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = BotRng::new(7);
        let mut b = BotRng::new(7);
        for _ in 0..100 {
            let x = a.below(10);
            assert_eq!(x, b.below(10));
            assert!(x < 10);
        }
        let mut r = BotRng::new(1);
        for _ in 0..1000 {
            assert!((-300..=300).contains(&r.offset(300)));
        }
        assert_ne!(BotRng::new(1).next_u64(), BotRng::new(2).next_u64());
    }

    #[test]
    fn bots_get_different_seeds() {
        assert_ne!(bot_seed(1, 0), bot_seed(1, 1));
        assert_eq!(bot_seed(1, 5), bot_seed(1, 5));
    }

    #[test]
    fn the_dice_mix_all_actions() {
        let mut rng = BotRng::new(3);
        let mut counts = [0u32; 5];
        for _ in 0..20_000 {
            counts[match pick_action(&mut rng) {
                Action::Walk => 0,
                Action::Chat => 1,
                Action::Combat => 2,
                Action::Restart => 3,
                Action::Idle => 4,
            }] += 1;
        }
        assert!(counts.iter().all(|&c| c > 0), "{counts:?}");
        assert!(counts[0] > counts[2] && counts[2] > counts[1] && counts[1] > counts[3]);
    }

    #[test]
    fn seen_remembers_death_and_teleports() {
        let mut seen = Seen::default();
        // DIE has opcode 0x00 in H5; the name lookup is the same one the real packets use.
        seen.observe(&[l2_protocol_game::ServerOpcode::Die.id()]);
        assert!(seen.died);
        assert!(!seen.teleported);
        seen.observe(&[l2_protocol_game::ServerOpcode::TeleportToLocation.id()]);
        assert!(seen.teleported);
    }
}
