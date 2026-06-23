use crate::components::{GameState, ObstacleShape};
use opengame_engine::math::Vec2;

/// Snapshot of input state, populated once per frame.
pub struct InputState {
    pub left: bool,
    pub right: bool,
    pub jump_pressed: bool,
    pub slide_down: bool,
    pub shoot_down: bool,
    pub start_pressed: bool,
    pub mouse_pos: Vec2,
    pub mouse_shoot: bool,
    pub reload_pressed: bool,
    pub escape_pressed: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            left: false,
            right: false,
            jump_pressed: false,
            slide_down: false,
            shoot_down: false,
            start_pressed: false,
            mouse_pos: Vec2::ZERO,
            mouse_shoot: false,
            reload_pressed: false,
            escape_pressed: false,
        }
    }
}

/// Current game state and transition timers.
pub struct GameStateRes {
    pub state: GameState,
    pub title_pulse: f32,
    pub game_over_timer: f32,
}

impl Default for GameStateRes {
    fn default() -> Self {
        Self {
            state: GameState::Title,
            title_pulse: 0.0,
            game_over_timer: 0.0,
        }
    }
}

/// Score tracking.
pub struct ScoreRes {
    pub score: i32,
    pub high_score: i32,
}

impl Default for ScoreRes {
    fn default() -> Self {
        Self {
            score: 0,
            high_score: 0,
        }
    }
}

/// Player lives.
pub struct LivesRes {
    pub lives: i32,
}

impl Default for LivesRes {
    fn default() -> Self {
        Self {
            lives: crate::MAX_LIVES,
        }
    }
}

/// Camera state.
pub struct CameraRes {
    pub camera_x: f32,
    pub camera_y: f32,
    pub shake_amount: f32,
}

impl Default for CameraRes {
    fn default() -> Self {
        Self {
            camera_x: 0.0,
            camera_y: 0.0,
            shake_amount: 0.0,
        }
    }
}

/// Enemy spawn timing and difficulty.
pub struct SpawnRes {
    pub spawn_timer: f32,
    pub spawn_interval: f32,
    pub difficulty_timer: f32,
}

impl Default for SpawnRes {
    fn default() -> Self {
        Self {
            spawn_timer: 1.5,
            spawn_interval: 2.0,
            difficulty_timer: 0.0,
        }
    }
}

/// Viewport mapping for responsive scaling.
/// Stores the actual GL viewport and the game-world-to-screen mapping.
pub struct ViewportRes {
    /// GL viewport position (for letterboxing)
    pub vp_x: i32,
    pub vp_y: i32,
    pub vp_w: i32,
    pub vp_h: i32,
    /// Scale factor: world units -> pixels within the viewport
    pub scale: f32,
    /// Canvas pixel dimensions
    pub canvas_w: f32,
    pub canvas_h: f32,
}

impl Default for ViewportRes {
    fn default() -> Self {
        Self {
            vp_x: 0,
            vp_y: 0,
            vp_w: 800,
            vp_h: 600,
            scale: 1.0,
            canvas_w: 800.0,
            canvas_h: 600.0,
        }
    }
}

/// Difficulty scaling for enemy AI.
pub struct DifficultyRes {
    pub elapsed_time: f32,
    pub level: i32,
    pub accuracy_mult: f32,
    pub reaction_mult: f32,
}

impl Default for DifficultyRes {
    fn default() -> Self {
        Self {
            elapsed_time: 0.0,
            level: 0,
            accuracy_mult: 0.98,
            reaction_mult: 0.5,
        }
    }
}

/// Kill feed message with timer.
pub struct KillFeedEntry {
    pub message: String,
    pub timer: f32,
}

/// Kill feed system resource.
pub struct KillFeedRes {
    pub entries: Vec<KillFeedEntry>,
    pub new_entries: Vec<KillFeedEntry>,
}

impl Default for KillFeedRes {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            new_entries: Vec::new(),
        }
    }
}

/// All user-configurable settings.
pub struct SettingsRes {
    // Audio
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    // Display
    pub screen_shake: bool,
    pub show_fps: bool,
    // Gameplay
    pub difficulty: u8, // 0=easy, 1=normal, 2=hard
    pub aim_assist: bool,
    // Crosshair
    pub crosshair_color: [f32; 3], // RGB 0.0-1.0
    pub crosshair_size: f32,       // 0.5-2.0 scale
}

impl Default for SettingsRes {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            sfx_volume: 0.8,
            music_volume: 0.6,
            screen_shake: true,
            show_fps: false,
            difficulty: 1, // normal
            aim_assist: false,
            crosshair_color: [0.0, 1.0, 0.53], // green
            crosshair_size: 1.0,
        }
    }
}

/// A static map obstacle.
pub struct Obstacle {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub shape: ObstacleShape,
    pub color: (f32, f32, f32),
}

/// Map state — holds all generated obstacles.
pub struct MapRes {
    pub obstacles: Vec<Obstacle>,
}

impl Default for MapRes {
    fn default() -> Self {
        Self { obstacles: Vec::new() }
    }
}
