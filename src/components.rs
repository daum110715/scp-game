/// Game state enum.
#[derive(Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy, PartialEq)]
pub enum EnemyType {
    Scout,
    Grunt,
    Tank,
    Flanker,
}

/// AI state machine for enemy behavior.
#[derive(Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy, PartialEq)]
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
