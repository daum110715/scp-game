use crate::components::{GameState, ObstacleShape};
use opengame_engine::math::Vec2;

/// Game mode — controls player agency and difficulty scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameMode {
    /// Normal player-controlled gameplay.
    PVE,
    /// AI vs AI spectacle — player is AI-controlled, boosted stats, faster spawns.
    EVE,
}

/// Current game mode resource.
pub struct GameModeRes {
    pub mode: GameMode,
}

impl Default for GameModeRes {
    fn default() -> Self {
        Self { mode: GameMode::PVE }
    }
}

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
    /// Analog movement from mobile joystick (-1.0 = full left, 1.0 = full right).
    /// When non-zero, overrides left/right booleans.
    pub move_x: f32,
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
            move_x: 0.0,
        }
    }
}

/// Current game state and transition timers.
pub struct GameStateRes {
    pub state: GameState,
    pub title_pulse: f32,
    pub game_over_timer: f32,
    /// When false, keyboard input cannot start the game from Title (popup must be dismissed first).
    pub can_start: bool,
}

impl Default for GameStateRes {
    fn default() -> Self {
        Self {
            state: GameState::Title,
            title_pulse: 0.0,
            game_over_timer: 0.0,
            can_start: false,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── GameMode Tests ──────────────────────────────────────────────────────

    #[test]
    fn game_mode_variants() {
        assert_ne!(GameMode::PVE, GameMode::EVE);
        assert_eq!(GameMode::PVE, GameMode::PVE);
    }

    #[test]
    fn game_mode_res_default() {
        let gm = GameModeRes::default();
        assert_eq!(gm.mode, GameMode::PVE);
    }

    // ── InputState Tests ────────────────────────────────────────────────────

    #[test]
    fn input_state_default() {
        let input = InputState::default();
        assert!(!input.left);
        assert!(!input.right);
        assert!(!input.jump_pressed);
        assert!(!input.slide_down);
        assert!(!input.shoot_down);
        assert!(!input.start_pressed);
        assert!(!input.mouse_shoot);
        assert!(!input.reload_pressed);
        assert!(!input.escape_pressed);
        assert_eq!(input.mouse_pos, Vec2::ZERO);
        assert!((input.move_x - 0.0).abs() < 0.01);
    }

    #[test]
    fn input_state_modification() {
        let mut input = InputState::default();
        input.left = true;
        input.mouse_shoot = true;
        assert!(input.left);
        assert!(input.mouse_shoot);
    }

    // ── GameStateRes Tests ──────────────────────────────────────────────────

    #[test]
    fn game_state_res_default() {
        let gs = GameStateRes::default();
        assert_eq!(gs.state, GameState::Title);
        assert!((gs.title_pulse - 0.0).abs() < 0.01);
        assert!((gs.game_over_timer - 0.0).abs() < 0.01);
        assert!(!gs.can_start);
    }

    #[test]
    fn game_state_res_transitions() {
        let mut gs = GameStateRes::default();

        gs.state = GameState::Playing;
        assert_eq!(gs.state, GameState::Playing);

        gs.state = GameState::Paused;
        assert_eq!(gs.state, GameState::Paused);

        gs.state = GameState::Playing;
        assert_eq!(gs.state, GameState::Playing);

        gs.state = GameState::GameOver;
        assert_eq!(gs.state, GameState::GameOver);
    }

    // ── ScoreRes Tests ──────────────────────────────────────────────────────

    #[test]
    fn score_res_default() {
        let s = ScoreRes::default();
        assert_eq!(s.score, 0);
        assert_eq!(s.high_score, 0);
    }

    #[test]
    fn score_res_tracks_high_score() {
        let mut s = ScoreRes::default();
        s.score = 500;
        s.high_score = s.score.max(s.high_score);
        assert_eq!(s.high_score, 500);

        s.score = 200;
        s.high_score = s.score.max(s.high_score);
        assert_eq!(s.high_score, 500, "High score should not decrease");

        s.score = 1000;
        s.high_score = s.score.max(s.high_score);
        assert_eq!(s.high_score, 1000);
    }

    // ── LivesRes Tests ──────────────────────────────────────────────────────

    #[test]
    fn lives_res_default() {
        let l = LivesRes::default();
        assert_eq!(l.lives, crate::MAX_LIVES);
    }

    // ── CameraRes Tests ─────────────────────────────────────────────────────

    #[test]
    fn camera_res_default() {
        let c = CameraRes::default();
        assert!((c.camera_x - 0.0).abs() < 0.01);
        assert!((c.camera_y - 0.0).abs() < 0.01);
        assert!((c.shake_amount - 0.0).abs() < 0.01);
    }

    // ── SpawnRes Tests ──────────────────────────────────────────────────────

    #[test]
    fn spawn_res_default() {
        let s = SpawnRes::default();
        assert!((s.spawn_timer - 1.5).abs() < 0.01);
        assert!((s.spawn_interval - 2.0).abs() < 0.01);
        assert!((s.difficulty_timer - 0.0).abs() < 0.01);
    }

    // ── ViewportRes Tests ───────────────────────────────────────────────────

    #[test]
    fn viewport_res_default() {
        let v = ViewportRes::default();
        assert_eq!(v.vp_x, 0);
        assert_eq!(v.vp_y, 0);
        assert_eq!(v.vp_w, 800);
        assert_eq!(v.vp_h, 600);
        assert!((v.scale - 1.0).abs() < 0.01);
        assert!((v.canvas_w - 800.0).abs() < 0.01);
        assert!((v.canvas_h - 600.0).abs() < 0.01);
    }

    // ── DifficultyRes Tests ─────────────────────────────────────────────────

    #[test]
    fn difficulty_res_default() {
        let d = DifficultyRes::default();
        assert!((d.elapsed_time - 0.0).abs() < 0.01);
        assert_eq!(d.level, 0);
        assert!((d.accuracy_mult - 0.98).abs() < 0.01);
        assert!((d.reaction_mult - 0.5).abs() < 0.01);
    }

    // ── KillFeedRes Tests ───────────────────────────────────────────────────

    #[test]
    fn kill_feed_res_default() {
        let kf = KillFeedRes::default();
        assert!(kf.entries.is_empty());
        assert!(kf.new_entries.is_empty());
    }

    #[test]
    fn kill_feed_entry() {
        let entry = KillFeedEntry { message: "Test kill".to_string(), timer: 4.0 };
        assert_eq!(entry.message, "Test kill");
        assert!((entry.timer - 4.0).abs() < 0.01);
    }

    // ── SettingsRes Tests ───────────────────────────────────────────────────

    #[test]
    fn settings_res_default() {
        let s = SettingsRes::default();
        assert!((s.master_volume - 0.8).abs() < 0.01);
        assert!((s.sfx_volume - 0.8).abs() < 0.01);
        assert!((s.music_volume - 0.6).abs() < 0.01);
        assert!(s.screen_shake);
        assert!(!s.show_fps);
        assert_eq!(s.difficulty, 1);
        assert!(!s.aim_assist);
        assert!((s.crosshair_color[0] - 0.0).abs() < 0.01);
        assert!((s.crosshair_color[1] - 1.0).abs() < 0.01);
        assert!((s.crosshair_color[2] - 0.53).abs() < 0.01);
        assert!((s.crosshair_size - 1.0).abs() < 0.01);
    }

    // ── MapRes Tests ────────────────────────────────────────────────────────

    #[test]
    fn map_res_default() {
        let m = MapRes::default();
        assert!(m.obstacles.is_empty());
    }

    #[test]
    fn obstacle_creation() {
        let obs = Obstacle {
            x: 100.0, y: 500.0, w: 40.0, h: 68.0,
            shape: ObstacleShape::Rectangle,
            color: (0.9, 0.9, 0.9),
        };
        assert!((obs.x - 100.0).abs() < 0.01);
        assert_eq!(obs.shape, ObstacleShape::Rectangle);
    }
}
