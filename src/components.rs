/// Game state enum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameState {
    Title,
    Playing,
    Paused,
    GameOver,
}

/// Velocity component — replaces engine RigidBody for custom physics.
pub struct Velocity {
    pub x: f32,
    pub y: f32,
    pub gravity_scale: f32,
}

/// Player component — position synced via Transform2D.
pub struct Player {
    pub facing_right: bool,
    pub invincible: f32,
    pub flash: f32,
    pub shoot_timer: f32,
    pub on_ground: bool,
    pub sliding: bool,
    pub slide_timer: f32,
    pub aim_angle: f32,
    pub ammo: i32,
    pub max_ammo: i32,
    pub reloading: bool,
    pub reload_timer: f32,
    pub footstep_timer: f32,
    pub health: i32,
    pub max_health: i32,
    pub armor: i32,
    pub max_armor: i32,
}

/// Enemy type determines base behavior and stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnemyType {
    Scout,
    Grunt,
    Tank,
    Flanker,
}

/// AI state machine for enemy behavior.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AIState {
    Chase,
    Attack,
    Flee,
    Flank,
}

/// Enemy component — position synced via Transform2D.
pub struct Enemy {
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
    pub on_ground: bool,
    pub shoot_timer: f32,
    pub ai_timer: f32,
    pub flash: f32,
    pub size: f32,
    pub enemy_type: EnemyType,
    pub ai_state: AIState,
    pub strafe_dir: f32,
    pub burst_count: i32,
    pub dodge_timer: f32,
    pub state_timer: f32,
    pub ammo: i32,
    pub max_ammo: i32,
    pub reloading: bool,
    pub reload_timer: f32,
    pub climb_cooldown: f32,
}

/// Shape type for map obstacles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObstacleShape {
    Square,
    Rectangle,
    Triangle,
}

/// Bullet component — NOT in physics system, positions updated manually.
pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub alive: bool,
    pub is_player: bool,
}

/// Particle component — purely visual, no physics.
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color_idx: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── GameState Tests ─────────────────────────────────────────────────────

    #[test]
    fn game_state_variants_are_distinct() {
        assert_ne!(GameState::Title, GameState::Playing);
        assert_ne!(GameState::Playing, GameState::Paused);
        assert_ne!(GameState::Paused, GameState::GameOver);
        assert_ne!(GameState::GameOver, GameState::Title);
    }

    #[test]
    fn game_state_clone_copy() {
        let states = [GameState::Title, GameState::Playing, GameState::Paused, GameState::GameOver];
        for s in states {
            let copied = s;
            assert_eq!(s, copied);
        }
    }

    #[test]
    fn game_state_partial_eq() {
        assert_eq!(GameState::Title, GameState::Title);
        assert_eq!(GameState::Playing, GameState::Playing);
        assert_eq!(GameState::Paused, GameState::Paused);
        assert_eq!(GameState::GameOver, GameState::GameOver);
    }

    // ── Velocity Tests ──────────────────────────────────────────────────────

    #[test]
    fn velocity_fields() {
        let v = Velocity { x: 10.0, y: -20.0, gravity_scale: 1.0 };
        assert_eq!(v.x, 10.0);
        assert_eq!(v.y, -20.0);
        assert_eq!(v.gravity_scale, 1.0);
    }

    #[test]
    fn velocity_zero_gravity() {
        let v = Velocity { x: 0.0, y: 0.0, gravity_scale: 0.0 };
        assert_eq!(v.gravity_scale, 0.0);
    }

    // ── Player Tests ────────────────────────────────────────────────────────

    #[test]
    fn player_default_state() {
        let p = Player {
            facing_right: true,
            invincible: 0.0,
            flash: 0.0,
            shoot_timer: 0.0,
            on_ground: true,
            sliding: false,
            slide_timer: 0.0,
            aim_angle: 0.0,
            ammo: 30,
            max_ammo: 30,
            reloading: false,
            reload_timer: 0.0,
            footstep_timer: 0.0,
            health: 100,
            max_health: 100,
            armor: 50,
            max_armor: 50,
        };
        assert!(p.facing_right);
        assert!(p.on_ground);
        assert!(!p.sliding);
        assert!(!p.reloading);
        assert_eq!(p.ammo, 30);
        assert_eq!(p.health, 100);
        assert_eq!(p.armor, 50);
    }

    #[test]
    fn player_can_be_damaged() {
        let mut p = Player {
            facing_right: true, invincible: 0.0, flash: 0.0, shoot_timer: 0.0,
            on_ground: true, sliding: false, slide_timer: 0.0, aim_angle: 0.0,
            ammo: 30, max_ammo: 30, reloading: false, reload_timer: 0.0,
            footstep_timer: 0.0, health: 100, max_health: 100, armor: 50, max_armor: 50,
        };
        p.health -= 10;
        assert_eq!(p.health, 90);
    }

    // ── EnemyType Tests ─────────────────────────────────────────────────────

    #[test]
    fn enemy_type_variants() {
        let types = [EnemyType::Scout, EnemyType::Grunt, EnemyType::Tank, EnemyType::Flanker];
        for (i, t) in types.iter().enumerate() {
            for (j, other) in types.iter().enumerate() {
                if i == j {
                    assert_eq!(t, other);
                } else {
                    assert_ne!(t, other);
                }
            }
        }
    }

    #[test]
    fn enemy_type_clone_copy() {
        let t = EnemyType::Tank;
        let t2 = t;
        assert_eq!(t, t2);
    }

    // ── AIState Tests ───────────────────────────────────────────────────────

    #[test]
    fn ai_state_variants() {
        let states = [AIState::Chase, AIState::Attack, AIState::Flee, AIState::Flank];
        assert_eq!(states.len(), 4);
        assert_ne!(AIState::Chase, AIState::Attack);
        assert_ne!(AIState::Flee, AIState::Flank);
    }

    // ── Enemy Tests ─────────────────────────────────────────────────────────

    #[test]
    fn enemy_creation() {
        let e = Enemy {
            hp: 2, max_hp: 2, alive: true, on_ground: false,
            shoot_timer: 1.0, ai_timer: 0.5, flash: 0.0, size: 30.0,
            enemy_type: EnemyType::Grunt, ai_state: AIState::Chase,
            strafe_dir: 1.0, burst_count: 0, dodge_timer: 0.0,
            state_timer: 0.0, ammo: 10, max_ammo: 10,
            reloading: false, reload_timer: 2.0, climb_cooldown: 0.0,
        };
        assert!(e.alive);
        assert_eq!(e.hp, 2);
        assert_eq!(e.enemy_type, EnemyType::Grunt);
    }

    // ── Bullet Tests ────────────────────────────────────────────────────────

    #[test]
    fn bullet_creation() {
        let b = Bullet { x: 100.0, y: 200.0, vx: 650.0, vy: 0.0, alive: true, is_player: true };
        assert!(b.alive);
        assert!(b.is_player);
        assert_eq!(b.x, 100.0);
    }

    #[test]
    fn bullet_enemy_bullet() {
        let b = Bullet { x: 0.0, y: 0.0, vx: -620.0, vy: 0.0, alive: true, is_player: false };
        assert!(!b.is_player);
    }

    // ── Particle Tests ──────────────────────────────────────────────────────

    #[test]
    fn particle_creation() {
        let p = Particle { x: 50.0, y: 50.0, vx: 100.0, vy: -50.0, life: 0.5, max_life: 1.0, size: 4.0, color_idx: 3 };
        assert_eq!(p.color_idx, 3);
        assert!(p.life > 0.0);
    }

    #[test]
    fn particle_life_ratio() {
        let p = Particle { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, life: 0.5, max_life: 1.0, size: 4.0, color_idx: 0 };
        let ratio = p.life / p.max_life;
        assert!((ratio - 0.5).abs() < 0.01);
    }

    // ── ObstacleShape Tests ─────────────────────────────────────────────────

    #[test]
    fn obstacle_shape_variants() {
        assert_ne!(ObstacleShape::Square, ObstacleShape::Rectangle);
        assert_ne!(ObstacleShape::Rectangle, ObstacleShape::Triangle);
        assert_eq!(ObstacleShape::Square, ObstacleShape::Square);
    }
}
