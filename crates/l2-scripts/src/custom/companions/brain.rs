//! What a companion decides to do: pure rules on a picture of the fight, no world, no I/O.
//!
//! The script builds a [`View`] once per second, asks the companion's [`Behavior`] for an
//! [`Action`] and carries it out. Keeping the choice a plain function of the picture means the
//! roles can be tested move by move without a world, and nothing here can touch game state.
//!
//! There are two modes. In [`Mode::Follow`] nobody the party fights is near, and the companion
//! walks after its leader. In [`Mode::Combat`] somebody is, and the role decides:
//!
//! - the **tank** pulls the enemies that go for somebody else onto itself, then hits;
//! - the **healer** looks after the weakest ally and keeps close to the leader;
//! - a **damage dealer** hits what the leader hits, a ranged one backs off when it is pressed.

use l2_util::ObjectId;

/// An enemy this close to the leader (and engaged, see [`engaged`]) puts the party in combat.
pub const ENGAGE_RANGE: f64 = 1_100.0;
/// A companion that is farther from its leader than this walks after it again.
pub const FOLLOW_DISTANCE: f64 = 250.0;
/// Farther than this and a companion is moved next to its leader (stairs, teleports, a long run).
pub const CATCH_UP_DISTANCE: f64 = 2_000.0;
/// How often a tank pulls (milliseconds): the hate it adds lasts a while, so it need not repeat.
pub const TAUNT_COOLDOWN_MS: u64 = 3_000;
/// How often a healer casts (milliseconds): the heal skills have a reuse time of 3 seconds.
pub const HEAL_COOLDOWN_MS: u64 = 3_500;
/// An ally below this share of its hit points is healed.
pub const HEAL_BELOW: f32 = 0.75;
/// A healer with less MP than this share does not start a heal.
pub const MIN_HEALER_MP: f32 = 0.08;
/// A healer only heals allies this close (the cast range of the heal skills is 600).
pub const HEAL_RANGE: f64 = 550.0;
/// A ranged damage dealer that an enemy comes this close to, and is below [`KITE_BELOW`], backs off.
pub const KITE_DISTANCE: f64 = 180.0;
/// See [`KITE_DISTANCE`].
pub const KITE_BELOW: f32 = 0.5;

/// A place on the ground (the height does not matter for any rule here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn distance(self, other: Pos) -> f64 {
        f64::from(self.x - other.x).hypot(f64::from(self.y - other.y))
    }
}

/// A living friend: the leader, a companion (also the one that decides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    pub id: ObjectId,
    pub pos: Pos,
    /// Hit points as a share of the maximum (0 to 1).
    pub hp: f32,
}

/// A living monster near the party.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enemy {
    pub id: ObjectId,
    pub pos: Pos,
    pub hp: f32,
    /// Whom it is fighting.
    pub target: Option<ObjectId>,
}

/// The picture a companion decides on.
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    /// World time in milliseconds.
    pub now_ms: u64,
    /// The companion itself.
    pub me: Unit,
    /// Its MP as a share of the maximum.
    pub mp: f32,
    /// The player the companion follows.
    pub leader: Unit,
    /// What the leader has selected, if anything.
    pub leader_target: Option<ObjectId>,
    /// Everybody who may be healed or defended: the leader, the companions and `me`.
    pub allies: &'a [Unit],
    /// The monsters near the leader.
    pub enemies: &'a [Enemy],
}

/// What a companion does with this second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Walk after the leader.
    Follow,
    /// Stay where it is.
    Hold,
    /// Fight this monster.
    Attack(ObjectId),
    /// Make this monster fight the companion instead of its victim, and fight it.
    Taunt(ObjectId),
    /// Cast a heal on this ally.
    Heal(ObjectId),
}

/// Follow while the party is at peace, combat while it fights.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Follow,
    Combat,
}

/// What a companion remembers between two seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Memory {
    pub mode: Mode,
    pub last_taunt_ms: Option<u64>,
    pub last_heal_ms: Option<u64>,
}

/// A role: what it does in combat.
pub trait Behavior: Sync {
    fn decide(&self, view: &View<'_>, memory: &Memory) -> Action;
}

/// Has `cooldown_ms` passed since `last`?
fn ready(last: Option<u64>, now_ms: u64, cooldown_ms: u64) -> bool {
    last.is_none_or(|at| now_ms.saturating_sub(at) >= cooldown_ms)
}

/// The monsters the party is fighting: those near the leader that go for an ally, or that the
/// leader has selected. Everything else is left alone, so a companion never pulls a camp.
pub fn engaged<'a>(view: &'a View<'a>) -> impl Iterator<Item = &'a Enemy> + 'a {
    view.enemies.iter().filter(move |enemy| {
        enemy.pos.distance(view.leader.pos) <= ENGAGE_RANGE
            && (view.leader_target == Some(enemy.id)
                || enemy
                    .target
                    .is_some_and(|target| view.allies.iter().any(|ally| ally.id == target)))
    })
}

fn nearest<'a>(from: Pos, enemies: impl Iterator<Item = &'a Enemy>) -> Option<&'a Enemy> {
    enemies.min_by(|a, b| a.pos.distance(from).total_cmp(&b.pos.distance(from)))
}

/// The hit points of the ally an enemy fights (1 when it is nobody known).
fn victim_hp(view: &View<'_>, enemy: &Enemy) -> f32 {
    enemy
        .target
        .and_then(|target| view.allies.iter().find(|ally| ally.id == target))
        .map_or(1.0, |ally| ally.hp)
}

/// Runs one decision and updates what the companion remembers.
pub fn think(behavior: &dyn Behavior, view: &View<'_>, memory: &mut Memory) -> Action {
    let action = behavior.decide(view, memory);
    memory.mode = if engaged(view).next().is_some() {
        Mode::Combat
    } else {
        Mode::Follow
    };
    match action {
        Action::Taunt(_) => memory.last_taunt_ms = Some(view.now_ms),
        Action::Heal(_) => memory.last_heal_ms = Some(view.now_ms),
        Action::Follow | Action::Hold | Action::Attack(_) => {}
    }
    action
}

/// The tank: holds the enemies.
pub struct Tank;

impl Behavior for Tank {
    fn decide(&self, view: &View<'_>, memory: &Memory) -> Action {
        if engaged(view).next().is_none() {
            return Action::Follow;
        }
        // The enemy that hits the weakest of the others is the one to take over.
        let loose = engaged(view)
            .filter(|enemy| enemy.target.is_some_and(|target| target != view.me.id))
            .min_by(|a, b| victim_hp(view, a).total_cmp(&victim_hp(view, b)));
        if let Some(enemy) = loose
            && ready(memory.last_taunt_ms, view.now_ms, TAUNT_COOLDOWN_MS)
        {
            return Action::Taunt(enemy.id);
        }
        // Otherwise hit what is on the tank, then what the leader hit, then what is nearest.
        let on_me = engaged(view).filter(|enemy| enemy.target == Some(view.me.id));
        if let Some(enemy) = nearest(view.me.pos, on_me) {
            return Action::Attack(enemy.id);
        }
        if let Some(target) = view.leader_target
            && engaged(view).any(|enemy| enemy.id == target)
        {
            return Action::Attack(target);
        }
        nearest(view.me.pos, engaged(view)).map_or(Action::Follow, |enemy| Action::Attack(enemy.id))
    }
}

/// The healer: keeps the party alive and close.
pub struct Healer;

impl Behavior for Healer {
    fn decide(&self, view: &View<'_>, memory: &Memory) -> Action {
        if view.mp >= MIN_HEALER_MP && ready(memory.last_heal_ms, view.now_ms, HEAL_COOLDOWN_MS) {
            let weakest = view
                .allies
                .iter()
                .filter(|ally| ally.hp < HEAL_BELOW && ally.pos.distance(view.me.pos) <= HEAL_RANGE)
                .min_by(|a, b| a.hp.total_cmp(&b.hp));
            if let Some(ally) = weakest {
                return Action::Heal(ally.id);
            }
        }
        // Nothing to heal: stay behind the fighters, close to the leader.
        if engaged(view).next().is_some() && view.me.pos.distance(view.leader.pos) <= FOLLOW_DISTANCE * 1.5 {
            Action::Hold
        } else {
            Action::Follow
        }
    }
}

/// A damage dealer; `ranged` ones back off when an enemy presses them.
pub struct DamageDealer {
    pub ranged: bool,
}

impl DamageDealer {
    fn pressed(&self, view: &View<'_>) -> bool {
        self.ranged
            && view.me.hp < KITE_BELOW
            && engaged(view).any(|enemy| {
                enemy.target == Some(view.me.id) && enemy.pos.distance(view.me.pos) < KITE_DISTANCE
            })
    }
}

impl Behavior for DamageDealer {
    fn decide(&self, view: &View<'_>, _memory: &Memory) -> Action {
        if engaged(view).next().is_none() {
            return Action::Follow;
        }
        // Back to the leader, where the tank and the healer are.
        if self.pressed(view) {
            return Action::Follow;
        }
        // What the leader hits, else what attacks the leader, else what attacks the weakest ally,
        // else what is nearest.
        if let Some(target) = view.leader_target
            && engaged(view).any(|enemy| enemy.id == target)
        {
            return Action::Attack(target);
        }
        let on_leader = engaged(view).filter(|enemy| enemy.target == Some(view.leader.id));
        if let Some(enemy) = nearest(view.me.pos, on_leader) {
            return Action::Attack(enemy.id);
        }
        let weakest_victim = engaged(view).min_by(|a, b| victim_hp(view, a).total_cmp(&victim_hp(view, b)));
        weakest_victim.map_or(Action::Follow, |enemy| Action::Attack(enemy.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: ObjectId = ObjectId(10);
    const LEADER: ObjectId = ObjectId(1);
    const FRIEND: ObjectId = ObjectId(11);

    fn at(x: i32) -> Pos {
        Pos { x, y: 0 }
    }

    fn unit(id: ObjectId, x: i32, hp: f32) -> Unit {
        Unit { id, pos: at(x), hp }
    }

    fn enemy(id: i32, x: i32, target: Option<ObjectId>) -> Enemy {
        Enemy {
            id: ObjectId(id),
            pos: at(x),
            hp: 1.0,
            target,
        }
    }

    /// The picture: the leader at 0, `me` at 100, one friend at 150.
    struct Scene {
        allies: Vec<Unit>,
        enemies: Vec<Enemy>,
        leader_target: Option<ObjectId>,
        me_hp: f32,
        mp: f32,
        now_ms: u64,
    }

    impl Scene {
        fn new() -> Self {
            Self {
                allies: vec![unit(LEADER, 0, 1.0), unit(ME, 100, 1.0), unit(FRIEND, 150, 1.0)],
                enemies: Vec::new(),
                leader_target: None,
                me_hp: 1.0,
                mp: 1.0,
                now_ms: 100_000,
            }
        }

        fn view(&self) -> View<'_> {
            let mut me = unit(ME, 100, self.me_hp);
            if let Some(a) = self.allies.iter().find(|a| a.id == ME) {
                me.hp = a.hp.min(self.me_hp);
            }
            View {
                now_ms: self.now_ms,
                me,
                mp: self.mp,
                leader: unit(LEADER, 0, 1.0),
                leader_target: self.leader_target,
                allies: &self.allies,
                enemies: &self.enemies,
            }
        }

        fn hurt(mut self, id: ObjectId, hp: f32) -> Self {
            for ally in &mut self.allies {
                if ally.id == id {
                    ally.hp = hp;
                }
            }
            if id == ME {
                self.me_hp = hp;
            }
            self
        }
    }

    fn decide(behavior: &dyn Behavior, scene: &Scene, memory: &Memory) -> Action {
        behavior.decide(&scene.view(), memory)
    }

    // ---- engaged ----

    #[test]
    fn only_the_fight_of_the_party_counts() {
        let mut scene = Scene::new();
        scene.enemies = vec![
            enemy(100, 300, Some(LEADER)),       // hits the leader
            enemy(101, 300, Some(FRIEND)),       // hits a companion
            enemy(102, 300, None),               // minds its own business
            enemy(103, 300, Some(ObjectId(99))), // hits a stranger
            enemy(104, 5_000, Some(LEADER)),     // far from the leader
            enemy(105, 300, None),               // the leader selected it
        ];
        scene.leader_target = Some(ObjectId(105));
        let ids: Vec<i32> = engaged(&scene.view()).map(|e| e.id.0).collect();
        assert_eq!(ids, vec![100, 101, 105]);
    }

    // ---- follow ----

    #[test]
    fn at_peace_every_role_follows() {
        let scene = Scene::new();
        let memory = Memory::default();
        assert_eq!(decide(&Tank, &scene, &memory), Action::Follow);
        assert_eq!(decide(&Healer, &scene, &memory), Action::Follow);
        assert_eq!(
            decide(&DamageDealer { ranged: false }, &scene, &memory),
            Action::Follow
        );
        assert_eq!(
            decide(&DamageDealer { ranged: true }, &scene, &memory),
            Action::Follow
        );
    }

    #[test]
    fn a_monster_that_ignores_the_party_does_not_start_a_fight() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, None)];
        assert_eq!(decide(&Tank, &scene, &Memory::default()), Action::Follow);
        assert_eq!(
            decide(&DamageDealer { ranged: false }, &scene, &Memory::default()),
            Action::Follow
        );
    }

    // ---- tank ----

    #[test]
    fn the_tank_takes_the_enemy_that_hits_somebody_else() {
        let mut scene = Scene::new().hurt(FRIEND, 0.4);
        scene.enemies = vec![enemy(100, 300, Some(LEADER)), enemy(101, 320, Some(FRIEND))];
        // The one on the weaker ally first.
        assert_eq!(
            decide(&Tank, &scene, &Memory::default()),
            Action::Taunt(ObjectId(101))
        );
    }

    #[test]
    fn the_tank_hits_when_the_pull_is_cooling_down_and_never_taunts_what_is_on_it() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, Some(LEADER)), enemy(101, 250, Some(ME))];
        let cooling = Memory {
            last_taunt_ms: Some(scene.now_ms - 500),
            ..Memory::default()
        };
        // Cooling down: hit the one that is on the tank.
        assert_eq!(decide(&Tank, &scene, &cooling), Action::Attack(ObjectId(101)));
        // Ready again: the loose one is pulled.
        assert_eq!(
            decide(&Tank, &scene, &Memory::default()),
            Action::Taunt(ObjectId(100))
        );
        // Nothing loose: no taunt, just the fight.
        scene.enemies = vec![enemy(101, 250, Some(ME))];
        assert_eq!(
            decide(&Tank, &scene, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
    }

    #[test]
    fn the_tank_follows_the_leaders_target_when_nothing_is_on_it() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, None), enemy(101, 320, None)];
        scene.leader_target = Some(ObjectId(101));
        assert_eq!(
            decide(&Tank, &scene, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
    }

    #[test]
    fn the_tank_pulls_again_after_the_cooldown() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, Some(LEADER))];
        let mut memory = Memory::default();
        let first = think(&Tank, &scene.view(), &mut memory);
        assert_eq!(first, Action::Taunt(ObjectId(100)));
        assert_eq!(memory.last_taunt_ms, Some(scene.now_ms));
        assert_eq!(memory.mode, Mode::Combat);
        // One second later the enemy is still on the leader: the pull is cooling down, so the tank hits.
        scene.now_ms += 1_000;
        assert_eq!(
            think(&Tank, &scene.view(), &mut memory),
            Action::Attack(ObjectId(100))
        );
        scene.now_ms += TAUNT_COOLDOWN_MS;
        assert_eq!(
            think(&Tank, &scene.view(), &mut memory),
            Action::Taunt(ObjectId(100))
        );
    }

    // ---- healer ----

    #[test]
    fn the_healer_heals_the_weakest_ally_below_the_threshold() {
        let scene = Scene::new().hurt(LEADER, 0.7).hurt(FRIEND, 0.3);
        assert_eq!(decide(&Healer, &scene, &Memory::default()), Action::Heal(FRIEND));
        // Itself counts.
        let scene = Scene::new().hurt(ME, 0.2).hurt(LEADER, 0.6);
        assert_eq!(decide(&Healer, &scene, &Memory::default()), Action::Heal(ME));
    }

    #[test]
    fn the_healer_leaves_healthy_allies_alone_and_waits_for_mana_and_the_reuse_time() {
        let healthy = Scene::new().hurt(LEADER, 0.8);
        assert_eq!(decide(&Healer, &healthy, &Memory::default()), Action::Follow);

        let hurt = Scene::new().hurt(LEADER, 0.5);
        // Out of mana.
        let mut dry = Scene::new().hurt(LEADER, 0.5);
        dry.mp = 0.02;
        assert_eq!(decide(&Healer, &dry, &Memory::default()), Action::Follow);
        // Just cast.
        let cooling = Memory {
            last_heal_ms: Some(hurt.now_ms - 1_000),
            ..Memory::default()
        };
        assert_eq!(decide(&Healer, &hurt, &cooling), Action::Follow);
        // The reuse time is over.
        let ready_again = Memory {
            last_heal_ms: Some(hurt.now_ms - HEAL_COOLDOWN_MS),
            ..Memory::default()
        };
        assert_eq!(decide(&Healer, &hurt, &ready_again), Action::Heal(LEADER));
    }

    #[test]
    fn in_a_fight_the_healer_holds_near_the_leader_and_follows_when_far() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, Some(LEADER))];
        assert_eq!(decide(&Healer, &scene, &Memory::default()), Action::Hold);
        let mut far = Scene::new();
        far.allies[1].pos = at(2_000);
        far.enemies = vec![enemy(100, 300, Some(LEADER))];
        let mut view = far.view();
        view.me.pos = at(2_000);
        assert_eq!(Healer.decide(&view, &Memory::default()), Action::Follow);
    }

    // ---- damage dealers ----

    #[test]
    fn a_damage_dealer_assists_the_leader() {
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 300, Some(LEADER)), enemy(101, 500, Some(FRIEND))];
        scene.leader_target = Some(ObjectId(101));
        let melee = DamageDealer { ranged: false };
        assert_eq!(
            decide(&melee, &scene, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
        // Without a selection: what attacks the leader.
        scene.leader_target = None;
        assert_eq!(
            decide(&melee, &scene, &Memory::default()),
            Action::Attack(ObjectId(100))
        );
        // Nobody on the leader: the enemy on the weakest ally.
        let mut scene = Scene::new().hurt(FRIEND, 0.3);
        scene.enemies = vec![enemy(101, 500, Some(FRIEND)), enemy(102, 450, Some(ME))];
        assert_eq!(
            decide(&melee, &scene, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
    }

    #[test]
    fn a_ranged_damage_dealer_backs_off_only_when_pressed_and_hurt() {
        let archer = DamageDealer { ranged: true };
        let mut scene = Scene::new();
        scene.enemies = vec![enemy(100, 150, Some(ME)), enemy(101, 400, Some(LEADER))];
        scene.leader_target = Some(ObjectId(101));
        // Healthy: keeps shooting what the leader hits.
        assert_eq!(
            decide(&archer, &scene, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
        // Hurt and an enemy on it within reach: back to the leader.
        let hurt = Scene {
            enemies: scene.enemies.clone(),
            leader_target: scene.leader_target,
            ..Scene::new().hurt(ME, 0.3)
        };
        assert_eq!(decide(&archer, &hurt, &Memory::default()), Action::Follow);
        // A melee fighter never backs off.
        assert_eq!(
            decide(&DamageDealer { ranged: false }, &hurt, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
        // Hurt, but the enemy on it is far away: keeps shooting.
        let far = Scene {
            enemies: vec![enemy(100, 600, Some(ME)), enemy(101, 400, Some(LEADER))],
            leader_target: Some(ObjectId(101)),
            ..Scene::new().hurt(ME, 0.3)
        };
        assert_eq!(
            decide(&archer, &far, &Memory::default()),
            Action::Attack(ObjectId(101))
        );
    }

    // ---- the driver ----

    #[test]
    fn the_mode_follows_the_fight() {
        let mut memory = Memory::default();
        let mut scene = Scene::new();
        assert_eq!(think(&Tank, &scene.view(), &mut memory), Action::Follow);
        assert_eq!(memory.mode, Mode::Follow);
        scene.enemies = vec![enemy(100, 300, Some(LEADER))];
        think(&Tank, &scene.view(), &mut memory);
        assert_eq!(memory.mode, Mode::Combat);
        scene.enemies.clear();
        think(&Tank, &scene.view(), &mut memory);
        assert_eq!(memory.mode, Mode::Follow);
    }

    #[test]
    fn a_heal_is_remembered() {
        let scene = Scene::new().hurt(LEADER, 0.4);
        let mut memory = Memory::default();
        assert_eq!(think(&Healer, &scene.view(), &mut memory), Action::Heal(LEADER));
        assert_eq!(memory.last_heal_ms, Some(scene.now_ms));
        // Straight away again: no second cast.
        assert_eq!(think(&Healer, &scene.view(), &mut memory), Action::Follow);
    }
}
