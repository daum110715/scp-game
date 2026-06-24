mod components;
mod resources;
mod systems;

use std::cell::RefCell;
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
type AnimationFrameClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

use opengame_engine::color::Color;
use opengame_engine::ecs::{QuerySingle, World};
#[cfg(target_arch = "wasm32")]
use opengame_engine::input::{keys::KeyCode, InputManager};
use opengame_engine::math::Vec2;
#[cfg(target_arch = "wasm32")]
use opengame_engine::math::{Mat4, Vec3};
#[cfg(target_arch = "wasm32")]
use opengame_engine::renderer::{GlBackend, ShapeRenderer};
#[cfg(target_arch = "wasm32")]
use opengame_engine::time::Time;
use opengame_engine::transform::Transform2D;

use components::*;
use resources::*;

// ── Direct JS sound callback ──────────────────────────────────────────────────
#[cfg(target_arch = "wasm32")]
pub(crate) fn play_sound_js(name: &str) {
    let window = web_sys::window().unwrap();
    let fn_name = wasm_bindgen::JsValue::from_str("__playSound");
    if let Ok(func) = js_sys::Reflect::get(&window, &fn_name) {
        if let Ok(func) = func.dyn_into::<js_sys::Function>() {
            let arg = wasm_bindgen::JsValue::from_str(name);
            let _ = func.call1(&wasm_bindgen::JsValue::NULL, &arg);
        }
    }
}

/// No-op stub for native tests (no JS runtime available).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn play_sound_js(_name: &str) {}

// ── Constants ──────────────────────────────────────────────────────────────────
pub(crate) const PLAYER_SIZE: f32 = 32.0;
pub(crate) const PLAYER_SPEED: f32 = 320.0;
pub(crate) const JUMP_FORCE: f32 = 800.0;
pub(crate) const GRAVITY: f32 = 1800.0;
pub(crate) const GROUND_Y: f32 = 568.0;

pub(crate) const BULLET_SPEED: f32 = 650.0;
pub(crate) const ENEMY_BULLET_SPEED: f32 = 620.0;
pub(crate) const SHOOT_INTERVAL: f32 = 0.22;
pub(crate) const MAX_LIVES: i32 = 3;
pub(crate) const INVINCIBLE_TIME: f32 = 2.0;
pub(crate) const MAX_HEALTH: i32 = 100;
pub(crate) const MAX_ARMOR: i32 = 50;
pub(crate) const HIT_DAMAGE: i32 = 10;

// Ammo and reload (player)
pub(crate) const MAX_AMMO: i32 = 30;
pub(crate) const RELOAD_TIME: f32 = 1.5;

// Enemy ammo and reload
pub(crate) const ENEMY_MAX_AMMO: i32 = 10;
pub(crate) const ENEMY_RELOAD_TIME: f32 = 2.0;
pub(crate) const TANK_MAX_AMMO: i32 = 6;
pub(crate) const TANK_RELOAD_TIME: f32 = 2.5;

pub(crate) const MAX_PARTICLES: usize = 400;
pub(crate) const MAX_BULLETS: usize = 200;
pub(crate) const MAX_ENEMIES: usize = 25;

pub(crate) const WORLD_W: f32 = 800.0;
pub(crate) const WORLD_H: f32 = 600.0;
pub(crate) const LEVEL_W: f32 = 3200.0;

pub(crate) const CAM_DEAD_ZONE_X: f32 = 120.0;
pub(crate) const CAM_SMOOTH: f32 = 4.0;

// Enemy type speeds
pub(crate) const SCOUT_SPEED_MIN: f32 = 120.0;
pub(crate) const SCOUT_SPEED_MAX: f32 = 180.0;
pub(crate) const GRUNT_SPEED_MIN: f32 = 70.0;
pub(crate) const GRUNT_SPEED_MAX: f32 = 140.0;
pub(crate) const TANK_SPEED_MIN: f32 = 40.0;
pub(crate) const TANK_SPEED_MAX: f32 = 80.0;
pub(crate) const FLANKER_SPEED_MIN: f32 = 90.0;
pub(crate) const FLANKER_SPEED_MAX: f32 = 130.0;

// Enemy type HP
pub(crate) const SCOUT_HP: i32 = 1;
pub(crate) const GRUNT_HP: i32 = 2;
pub(crate) const TANK_HP: i32 = 4;
pub(crate) const FLANKER_HP: i32 = 2;

// Dodge
pub(crate) const DODGE_RANGE: f32 = 100.0;
pub(crate) const DODGE_COOLDOWN: f32 = 0.5;

// EVE mode — AI vs AI spectacle
pub(crate) const EVE_PLAYER_MAX_HEALTH: i32 = 200;
pub(crate) const EVE_PLAYER_MAX_ARMOR: i32 = 100;
pub(crate) const EVE_PLAYER_MAX_AMMO: i32 = 50;
pub(crate) const EVE_SPAWN_INTERVAL: f32 = 0.32;
pub(crate) const EVE_MAX_ENEMIES: usize = 30;

// ── Utility ────────────────────────────────────────────────────────────────────
pub(crate) fn rand() -> f32 {
    js_sys::Math::random() as f32
}

pub(crate) fn rand_range(min: f32, max: f32) -> f32 {
    min + rand() * (max - min)
}

pub(crate) fn particle_color(idx: u8) -> Color {
    match idx % 7 {
        0 => Color::new(1.0, 0.9, 0.2, 1.0),
        1 => Color::new(1.0, 0.5, 0.1, 1.0),
        2 => Color::new(1.0, 0.2, 0.1, 1.0),
        3 => Color::new(1.0, 0.6, 0.8, 1.0),
        4 => Color::new(0.9, 0.4, 1.0, 1.0),
        5 => Color::new(0.4, 0.8, 1.0, 1.0),
        _ => Color::new(1.0, 1.0, 1.0, 1.0),
    }
}

// ── Static game reference for WASM exports ─────────────────────────────────────
#[cfg(target_arch = "wasm32")]
thread_local! {
    static GAME_REF: RefCell<Option<Rc<RefCell<ScpGame>>>> = RefCell::new(None);
}

// ── Map Generation ────────────────────────────────────────────────────────────
fn generate_map() -> MapRes {
    let mut obstacles: Vec<Obstacle> = Vec::new();
    let mut x = 250.0; // skip player spawn area

    while x < LEVEL_W - 100.0 {
        // Gap between obstacles
        let gap = rand_range(80.0, 180.0);
        x += gap;

        if x >= LEVEL_W - 100.0 { break; }

        // Random dimensions
        let (w, h) = if rand() < 0.2 {
            // Tall obstacle
            (rand_range(30.0, 50.0), rand_range(80.0, 120.0))
        } else if rand() < 0.5 {
            // Square-ish
            let s = rand_range(30.0, 55.0);
            (s, s)
        } else {
            // Rectangle
            (rand_range(35.0, 75.0), rand_range(30.0, 80.0))
        };

        // Shape selection
        let shape = if rand() < 0.45 { ObstacleShape::Square } else { ObstacleShape::Rectangle };

        // White with slight variation
        let v = rand_range(0.75, 1.0);
        let color = (v, v, v);

        let y = GROUND_Y - h;

        obstacles.push(Obstacle { x, y, w, h, shape, color });
        x += w;
    }

    MapRes { obstacles }
}

// ── ECS World Setup ────────────────────────────────────────────────────────────
fn init_world(world: &mut World) {
    world.insert_resource(GameStateRes::default());
    world.insert_resource(ScoreRes::default());
    world.insert_resource(LivesRes::default());
    world.insert_resource(CameraRes::default());
    world.insert_resource(SpawnRes::default());
    world.insert_resource(InputState::default());
    world.insert_resource(ViewportRes::default());
    world.insert_resource(DifficultyRes::default());
    world.insert_resource(SettingsRes::default());
    world.insert_resource(KillFeedRes::default());
    world.insert_resource(GameModeRes::default());
    world.insert_resource(generate_map());

    // Check if EVE mode is active for boosted stats
    let is_eve = world.get_resource::<GameModeRes>()
        .map(|g| g.mode == GameMode::EVE)
        .unwrap_or(false);

    let (health, max_health, armor, max_armor, ammo, max_ammo) = if is_eve {
        (EVE_PLAYER_MAX_HEALTH, EVE_PLAYER_MAX_HEALTH, EVE_PLAYER_MAX_ARMOR, EVE_PLAYER_MAX_ARMOR,
         EVE_PLAYER_MAX_AMMO, EVE_PLAYER_MAX_AMMO)
    } else {
        (MAX_HEALTH, MAX_HEALTH, MAX_ARMOR, MAX_ARMOR, MAX_AMMO, MAX_AMMO)
    };

    // Player entity
    world.spawn()
        .with(Player {
            facing_right: true,
            invincible: 0.0,
            flash: 0.0,
            shoot_timer: 0.0,
            on_ground: true,
            sliding: false,
            slide_timer: 0.0,
            aim_angle: 0.0,
            ammo,
            max_ammo,
            reloading: false,
            reload_timer: 0.0,
            footstep_timer: 0.0,
            health,
            max_health,
            armor,
            max_armor,
        })
        .with(Transform2D::new(Vec2::new(100.0, GROUND_Y - PLAYER_SIZE)))
        .with(Velocity { x: 0.0, y: 0.0, gravity_scale: 1.0 })
        .build();
}

// ── Main Game Struct ───────────────────────────────────────────────────────────
#[cfg(target_arch = "wasm32")]
struct ScpGame {
    gl: GlBackend,
    shapes: ShapeRenderer,
    input: InputManager,
    time: Time,
    world: World,
}

#[cfg(target_arch = "wasm32")]
impl ScpGame {
    fn new() -> Result<Self, String> {
        opengame_engine::log::init();

        let gl = GlBackend::new("game-canvas")?;
        let shapes = ShapeRenderer::new(gl.gl())?;
        let input = InputManager::new()?;

        let window = web_sys::window().ok_or("No window")?;
        let performance = window.performance().ok_or("No performance")?;
        let time = Time::new(performance);

        let mut world = World::new();
        init_world(&mut world);

        Ok(Self {
            gl,
            shapes,
            input,
            time,
            world,
        })
    }

    fn reset_game(&mut self) {
        let high = self.world.get_resource::<ScoreRes>()
            .map(|s| s.high_score.max(s.score))
            .unwrap_or(0);
        let gs_res = self.world.get_resource::<GameStateRes>()
            .map(|gs| (gs.state, gs.can_start))
            .unwrap_or((GameState::Title, false));
        let (state, can_start) = gs_res;
        let game_mode = self.world.get_resource::<GameModeRes>()
            .map(|g| g.mode)
            .unwrap_or(GameMode::PVE);

        self.world.clear();
        init_world(&mut self.world);

        // Restore state, high score, and game mode that init_world() overwrote with defaults
        if let Some(gs) = self.world.get_resource_mut::<GameStateRes>() {
            gs.state = state;
            gs.can_start = can_start;
        }
        if let Some(score) = self.world.get_resource_mut::<ScoreRes>() {
            score.high_score = high;
        }
        if let Some(gm) = self.world.get_resource_mut::<GameModeRes>() {
            gm.mode = game_mode;
        }
    }

    // ── Input ──────────────────────────────────────────────────────────────────
    fn poll_input(&mut self) {
        let gs = self.world.get_resource::<GameStateRes>().unwrap().state;

        let mouse_pos = self.input.mouse_position();
        let escape_pressed = self.input.is_key_pressed(KeyCode::Escape);
        let input_state = InputState {
            left: self.input.is_key_down(KeyCode::KeyA),
            right: self.input.is_key_down(KeyCode::KeyD),
            jump_pressed: self.input.is_key_pressed(KeyCode::Space)
                || self.input.is_key_pressed(KeyCode::KeyW),
            slide_down: self.input.is_key_down(KeyCode::KeyS),
            shoot_down: self.input.is_key_down(KeyCode::KeyJ),
            start_pressed: self.input.is_key_pressed(KeyCode::Enter)
                || self.input.is_key_pressed(KeyCode::Space),
            mouse_pos,
            mouse_shoot: self.input.is_mouse_down(opengame_engine::input::keys::MouseButton::Left),
            reload_pressed: self.input.is_key_pressed(KeyCode::KeyR),
            escape_pressed,
            move_x: 0.0, // Keyboard input uses left/right booleans instead
        };
        self.world.insert_resource(input_state);

        match gs {
            GameState::Title => {
                let can_start = self.world.get_resource::<GameStateRes>().unwrap().can_start;
                if can_start && self.world.get_resource::<InputState>().unwrap().start_pressed {
                    self.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
                    self.reset_game();
                }
            }
            GameState::Playing => {
                if escape_pressed {
                    self.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Paused;
                }
            }
            GameState::Paused => {
                if escape_pressed {
                    self.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
                }
            }
            GameState::GameOver => {
                let timer = self.world.get_resource::<GameStateRes>().unwrap().game_over_timer;
                if timer > 1.5 && self.world.get_resource::<InputState>().unwrap().start_pressed {
                    self.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
                    self.reset_game();
                }
            }
        }
    }

    // ── Update ─────────────────────────────────────────────────────────────────
    fn update(&mut self, dt: f32) {
        // Update timers
        {
            let gs = self.world.get_resource_mut::<GameStateRes>().unwrap();
            match gs.state {
                GameState::Title => { gs.title_pulse += dt * 2.0; }
                GameState::GameOver => { gs.game_over_timer += dt; }
                GameState::Playing => {}
                GameState::Paused => {}
            }
        }

        let state = self.world.get_resource::<GameStateRes>().unwrap().state;
        if state == GameState::Playing {
            // In EVE mode, AI controls the player by writing synthetic input
            let is_eve = self.world.get_resource::<GameModeRes>()
                .map(|g| g.mode == GameMode::EVE)
                .unwrap_or(false);
            if is_eve {
                systems::player_ai_system(&mut self.world, dt);
            }

            // Game systems
            systems::player_move_system(&mut self.world, dt);
            systems::player_shoot_system(&mut self.world, dt);
            systems::enemy_spawn_system(&mut self.world, dt);
            systems::enemy_ai_system(&mut self.world, dt);
            systems::bullet_move_system(&mut self.world, dt);
            systems::particle_update_system(&mut self.world, dt);

            // Custom physics: gravity, integration, ground clamp, collision detection
            systems::physics_step(&mut self.world, dt);

            systems::camera_system(&mut self.world, dt);
            systems::kill_feed_system(&mut self.world, dt);
            systems::cleanup_system(&mut self.world);
        } else if state == GameState::Paused {
            // Keep particles alive for visual continuity
            systems::particle_update_system(&mut self.world, dt);
        } else if state == GameState::GameOver {
            systems::particle_update_system(&mut self.world, dt);
            systems::camera_system(&mut self.world, dt);
            systems::kill_feed_system(&mut self.world, dt);
        }
    }

    // ── Rendering ──────────────────────────────────────────────────────────────
    fn render(&mut self, _alpha: f32) {
        self.gl.resize();

        // Calculate viewport to fill entire canvas (no letterboxing)
        let canvas_w = self.gl.width() as f32;
        let canvas_h = self.gl.height() as f32;
        let canvas_aspect = canvas_w / canvas_h;

        // Keep vertical view fixed, extend horizontal view based on aspect ratio
        let visible_h = WORLD_H;
        let visible_w = visible_h * canvas_aspect;

        // Scale factor for mouse coordinate conversion
        let scale = canvas_h / visible_h;

        // Store viewport info for mouse coordinate conversion
        {
            let viewport = self.world.get_resource_mut::<ViewportRes>().unwrap();
            viewport.vp_x = 0;
            viewport.vp_y = 0;
            viewport.vp_w = canvas_w as i32;
            viewport.vp_h = canvas_h as i32;
            viewport.scale = scale;
            viewport.canvas_w = canvas_w;
            viewport.canvas_h = canvas_h;
        }

        // Clear entire canvas to background color
        self.gl.clear(0.04, 0.06, 0.18, 1.0);

        // Set viewport to fill entire canvas
        self.gl.set_viewport(0, 0, canvas_w as i32, canvas_h as i32);
        self.gl.enable_blend();

        let cam = self.world.get_resource::<CameraRes>().unwrap();
        let cam_x = cam.camera_x;
        let screen_shake = self.world.get_resource::<SettingsRes>().map(|s| s.screen_shake).unwrap_or(true);
        let shake_amount = if screen_shake { cam.shake_amount } else { 0.0 };
        let shake_x = if shake_amount > 0.0 { (rand() - 0.5) * shake_amount * 2.0 } else { 0.0 };
        let shake_y = if shake_amount > 0.0 { (rand() - 0.5) * shake_amount * 2.0 } else { 0.0 };

        // Calculate camera offset to center the view, clamped to level bounds
        let cam_center_x = cam_x + WORLD_W * 0.5;
        let half_visible = visible_w * 0.5;
        let mut view_left = cam_center_x - half_visible;
        let mut view_right = cam_center_x + half_visible;

        // Clamp viewport to level boundaries
        if view_left < 0.0 {
            view_left = 0.0;
            view_right = visible_w;
        } else if view_right > LEVEL_W {
            view_right = LEVEL_W;
            view_left = LEVEL_W - visible_w;
        }

        let projection = Mat4::orthographic_rh_gl(view_left, view_right, visible_h, 0.0, -1.0, 1.0);
        let view = Mat4::from_translation(Vec3::new(-shake_x, -shake_y, 0.0));
        let vp = projection * view;

        self.shapes.begin();

        // Stars - render across the visible area
        self.shapes.set_color(Color::new(0.6, 0.6, 0.8, 0.3));
        let cam_left = view_left;
        let cam_right = view_right;
        for i in 0..60 {
            let sx = (i as f32 * 137.5) % LEVEL_W;
            if sx >= cam_left - 5.0 && sx <= cam_right + 5.0 {
                let sy = (i as f32 * 73.1 + 20.0) % (GROUND_Y - 40.0);
                self.shapes.draw_rect(sx, sy, 2.0, 2.0);
            }
        }

        self.render_ground(cam_left, cam_right);
        self.render_obstacles(cam_left, cam_right);

        let gs = self.world.get_resource::<GameStateRes>().unwrap().state;
        match gs {
            GameState::Title => self.render_title(),
            GameState::Playing | GameState::Paused => {
                self.render_particles();
                self.render_bullets();
                self.render_enemies();
                self.render_player();
                if gs == GameState::Paused {
                    // Dim overlay — JS renders the pause menu on top
                    self.shapes.set_color(Color::new(0.0, 0.0, 0.0, 0.45));
                    self.shapes.draw_rect(0.0, 0.0, WORLD_W, WORLD_H);
                }
            }
            GameState::GameOver => {
                self.render_particles();
                self.render_bullets();
                self.render_enemies();
                self.render_game_over();
            }
        }

        self.shapes.flush(self.gl.gl(), &vp);
    }

    fn render_ground(&mut self, cam_left: f32, cam_right: f32) {
        self.shapes.set_color(Color::new(0.18, 0.20, 0.24, 1.0));
        self.shapes.draw_rect(0.0, GROUND_Y, LEVEL_W, WORLD_H - GROUND_Y);

        self.shapes.set_color(Color::new(0.30, 0.34, 0.40, 1.0));
        self.shapes.draw_rect(0.0, GROUND_Y, LEVEL_W, 3.0);

        self.shapes.set_color(Color::new(0.22, 0.25, 0.30, 1.0));
        let start = ((cam_left / 42.0).floor() * 42.0 + 10.0).max(0.0);
        let mut gx = start;
        while gx <= cam_right && gx <= LEVEL_W {
            self.shapes.draw_rect(gx, GROUND_Y + 4.0, 1.0, WORLD_H - GROUND_Y - 4.0);
            gx += 42.0;
        }
    }

    fn render_obstacles(&mut self, cam_left: f32, cam_right: f32) {
        let map = self.world.get_resource::<MapRes>().unwrap();
        for obs in &map.obstacles {
            // Skip obstacles outside the visible area
            if obs.x + obs.w < cam_left - 10.0 || obs.x > cam_right + 10.0 {
                continue;
            }

            let (r, g, b) = obs.color;

            match obs.shape {
                ObstacleShape::Square | ObstacleShape::Rectangle => {
                    // Faint white glow
                    let glow = 5.0;
                    self.shapes.set_color(Color::new(r, g, b, 0.10));
                    self.shapes.draw_rounded_rect(obs.x - glow, obs.y - glow, obs.w + glow * 2.0, obs.h + glow * 2.0, 5.0);

                    // Subtle bright top edge
                    self.shapes.set_color(Color::new(1.0, 1.0, 1.0, 0.4));
                    self.shapes.draw_rect(obs.x, obs.y, obs.w, 2.0);

                    // Main body
                    self.shapes.set_color(Color::new(r, g, b, 1.0));
                    self.shapes.draw_rounded_rect(obs.x, obs.y, obs.w, obs.h, 3.0);

                    // Dark bottom edge
                    self.shapes.set_color(Color::new(
                        (r - 0.15).max(0.0), (g - 0.15).max(0.0), (b - 0.15).max(0.0), 0.8,
                    ));
                    self.shapes.draw_rect(obs.x, obs.y + obs.h - 2.0, obs.w, 2.0);
                }
                ObstacleShape::Triangle => {
                    // Fallback: render as a standard rectangle
                    self.shapes.set_color(Color::new(r, g, b, 1.0));
                    self.shapes.draw_rounded_rect(obs.x, obs.y, obs.w, obs.h, 3.0);
                }
            }
        }
    }

    fn render_player(&mut self) {
        let query = QuerySingle::<Player>::new(&self.world);
        let data = match query {
            Some(q) => {
                match q.iter().next() {
                    Some((e, p)) => {
                        let pos = self.world.get_component::<Transform2D>(e)
                            .map(|t| (t.position.x, t.position.y))
                            .unwrap_or((100.0, GROUND_Y - PLAYER_SIZE));
                        (pos.0, pos.1, p.invincible, p.flash, p.sliding)
                    }
                    None => return,
                }
            }
            None => return,
        };
        let (px, py, invincible, flash, sliding) = data;

        if invincible > 0.0 && (flash * 0.5).sin() > 0.3 { return; }

        let t = self.time.elapsed();
        let breathe = (t * 3.0).sin() * 0.5 + 0.5;
        let glow_expand = 3.0 + breathe * 4.0;
        let glow_alpha = 0.08 + breathe * 0.10;

        // Player body (shorter when sliding)
        let body_h = if sliding { PLAYER_SIZE * 0.5 } else { PLAYER_SIZE };
        let body_y = if sliding { py + PLAYER_SIZE * 0.5 } else { py };

        self.shapes.set_color(Color::new(0.0, 0.95, 1.0, glow_alpha));
        self.shapes.draw_rounded_rect(px - glow_expand, body_y - glow_expand, PLAYER_SIZE + glow_expand * 2.0, body_h + glow_expand * 2.0, 6.0);

        self.shapes.set_color(Color::new(0.0, 0.95, 1.0, 1.0));
        self.shapes.draw_rounded_rect(px, body_y, PLAYER_SIZE, body_h, 4.0);
    }

    fn render_enemies(&mut self) {
        let t = self.time.elapsed();
        let query = QuerySingle::<Enemy>::new(&self.world);
        if let Some(q) = query {
            for (e, enemy) in q.iter() {
                if !enemy.alive { continue; }
                let (ex, ey) = self.world.get_component::<Transform2D>(e)
                    .map(|t| (t.position.x, t.position.y))
                    .unwrap_or((0.0, 0.0));
                let s = enemy.size;
                let phase = (ex * 0.05 + t * 2.8).sin() * 0.5 + 0.5;
                let glow_expand = 2.0 + phase * 3.0;
                let glow_alpha = 0.06 + phase * 0.08;

                // Color based on enemy type
                let (base_r, base_g, base_b) = match enemy.enemy_type {
                    EnemyType::Scout => (1.0, 0.85, 0.2),    // Yellow/gold
                    EnemyType::Grunt => (1.0, 0.45, 0.0),    // Orange
                    EnemyType::Tank => (0.8, 0.15, 0.15),    // Dark red
                    EnemyType::Flanker => (0.7, 0.2, 0.9),   // Purple
                };

                self.shapes.set_color(Color::new(base_r, base_g, base_b, glow_alpha));
                self.shapes.draw_rounded_rect(ex - glow_expand, ey - glow_expand, s + glow_expand * 2.0, s + glow_expand * 2.0, 5.0);

                let c = if enemy.flash > 0.0 {
                    Color::lerp(Color::new(base_r, base_g, base_b, 1.0), Color::WHITE, enemy.flash * 0.7)
                } else {
                    Color::new(base_r, base_g, base_b, 1.0)
                };
                self.shapes.set_color(c);
                self.shapes.draw_rounded_rect(ex, ey, s, s, 4.0);
            }
        }
    }

    fn render_bullets(&mut self) {
        let query = QuerySingle::<Bullet>::new(&self.world);
        if let Some(q) = query {
            for (_e, bullet) in q.iter() {
                if !bullet.alive { continue; }
                if bullet.is_player {
                    self.shapes.set_color(Color::new(0.3, 0.85, 1.0, 0.25));
                    self.shapes.draw_rounded_rect(bullet.x - 6.0, bullet.y - 5.0, 16.0, 10.0, 3.0);
                    self.shapes.set_color(Color::new(0.3, 0.9, 1.0, 1.0));
                    self.shapes.draw_rounded_rect(bullet.x - 3.0, bullet.y - 2.0, 10.0, 4.0, 2.0);
                    self.shapes.set_color(Color::new(0.8, 1.0, 1.0, 1.0));
                    self.shapes.draw_rounded_rect(bullet.x - 1.0, bullet.y - 1.0, 6.0, 2.0, 1.0);
                } else {
                    self.shapes.set_color(Color::new(1.0, 0.3, 0.2, 0.25));
                    self.shapes.draw_rounded_rect(bullet.x - 5.0, bullet.y - 4.0, 12.0, 8.0, 3.0);
                    self.shapes.set_color(Color::new(1.0, 0.35, 0.2, 1.0));
                    self.shapes.draw_rounded_rect(bullet.x - 2.0, bullet.y - 2.0, 7.0, 4.0, 2.0);
                    self.shapes.set_color(Color::new(1.0, 0.7, 0.5, 1.0));
                    self.shapes.draw_rounded_rect(bullet.x - 0.5, bullet.y - 1.0, 4.0, 2.0, 1.0);
                }
            }
        }
    }

    fn render_particles(&mut self) {
        let query = QuerySingle::<Particle>::new(&self.world);
        if let Some(q) = query {
            for (_e, p) in q.iter() {
                if p.life <= 0.0 { continue; }
                let t = p.life / p.max_life;
                let alpha = t * t;
                let size = p.size * (0.3 + t * 0.7);
                let c = particle_color(p.color_idx).with_alpha(alpha);
                self.shapes.set_color(c);
                let r = (size * 0.3).min(2.0);
                self.shapes.draw_rounded_rect(p.x - size * 0.5, p.y - size * 0.5, size, size, r);
            }
        }
    }

    fn render_title(&mut self) {
        let gs = self.world.get_resource::<GameStateRes>().unwrap();
        let pulse = (gs.title_pulse).sin() * 0.15 + 0.85;
        let high_score = self.world.get_resource::<ScoreRes>().unwrap().high_score;

        let cx = WORLD_W * 0.5;
        let ty = WORLD_H * 0.28;

        self.shapes.set_color(Color::new(0.3, 0.7, 1.0, pulse));
        self.shapes.draw_rect(cx - 100.0, ty, 30.0, 8.0);
        self.shapes.draw_rect(cx - 108.0, ty + 8.0, 8.0, 16.0);
        self.shapes.draw_rect(cx - 100.0, ty + 24.0, 30.0, 8.0);
        self.shapes.draw_rect(cx - 78.0, ty + 32.0, 8.0, 16.0);
        self.shapes.draw_rect(cx - 100.0, ty + 48.0, 30.0, 8.0);

        self.shapes.draw_rect(cx - 40.0, ty, 30.0, 8.0);
        self.shapes.draw_rect(cx - 48.0, ty + 8.0, 8.0, 40.0);
        self.shapes.draw_rect(cx - 40.0, ty + 48.0, 30.0, 8.0);

        self.shapes.draw_rect(cx + 10.0, ty, 30.0, 8.0);
        self.shapes.draw_rect(cx + 2.0, ty + 8.0, 8.0, 48.0);
        self.shapes.draw_rect(cx + 40.0, ty + 8.0, 8.0, 20.0);
        self.shapes.draw_rect(cx + 10.0, ty + 28.0, 30.0, 8.0);

        self.shapes.set_color(Color::new(1.0, 0.4, 0.2, pulse * 0.8));
        self.shapes.draw_rect(cx - 70.0, ty + 72.0, 140.0, 6.0);
        self.shapes.set_color(Color::new(1.0, 0.6, 0.3, pulse * 0.6));
        self.shapes.draw_rect(cx - 50.0, ty + 82.0, 100.0, 4.0);

        let title_pulse = self.world.get_resource::<GameStateRes>().unwrap().title_pulse;
        for i in 0..5 {
            let angle = title_pulse * 0.8 + i as f32 * std::f32::consts::TAU / 5.0;
            let radius = 80.0 + (title_pulse * 0.7 + i as f32).sin() * 15.0;
            let dx = cx + angle.cos() * radius;
            let dy = WORLD_H * 0.55 + angle.sin() * radius * 0.35;
            self.shapes.set_color(Color::new(1.0, 0.45, 0.0, 0.5 * pulse));
            self.shapes.draw_rect(dx - 8.0, dy - 8.0, 16.0, 16.0);
        }

        let blink = (title_pulse * 1.5).sin();
        if blink > -0.3 {
            self.shapes.set_color(Color::new(0.9, 0.9, 1.0, 0.5 + blink * 0.4));
            self.shapes.draw_rect(cx - 90.0, WORLD_H * 0.72, 180.0, 4.0);
        }

        self.shapes.set_color(Color::new(0.5, 0.5, 0.6, 0.5));
        self.shapes.draw_rect(cx - 70.0, WORLD_H * 0.80, 140.0, 2.0);
        self.shapes.draw_rect(cx - 70.0, WORLD_H * 0.80 + 18.0, 140.0, 2.0);
        self.shapes.draw_rect(cx - 70.0, WORLD_H * 0.80 + 36.0, 140.0, 2.0);

        if high_score > 0 {
            self.shapes.set_color(Color::new(1.0, 0.85, 0.3, 0.7));
            self.shapes.draw_rect(cx - 40.0, WORLD_H * 0.80 + 54.0, 80.0, 2.0);
        }
    }

    fn render_game_over(&mut self) {
        let gs = self.world.get_resource::<GameStateRes>().unwrap();
        let alpha = (gs.game_over_timer / 0.5).min(1.0);
        let score = self.world.get_resource::<ScoreRes>().unwrap().score;
        let high_score = self.world.get_resource::<ScoreRes>().unwrap().high_score;
        let cx = WORLD_W * 0.5;

        self.shapes.set_color(Color::new(0.0, 0.0, 0.0, 0.55 * alpha));
        self.shapes.draw_rect(0.0, 0.0, WORLD_W, WORLD_H);

        let center_y = WORLD_H * 0.35;

        self.shapes.set_color(Color::new(1.0, 0.2, 0.2, alpha * 0.95));
        self.shapes.draw_rounded_rect(cx - 90.0, center_y - 10.0, 180.0, 20.0, 5.0);
        self.shapes.set_color(Color::new(0.8, 0.1, 0.1, alpha * 0.7));
        self.shapes.draw_rounded_rect(cx - 70.0, center_y + 14.0, 140.0, 10.0, 4.0);

        let score_bars = (score / 50).min(40) as f32;
        self.shapes.set_color(Color::new(0.2, 0.2, 0.25, alpha * 0.8));
        self.shapes.draw_rounded_rect(cx - 60.0, center_y + 50.0, 120.0, 10.0, 4.0);
        self.shapes.set_color(Color::new(0.3, 0.9, 0.4, alpha * 0.9));
        if score_bars > 0.0 {
            self.shapes.draw_rounded_rect(cx - 58.0, center_y + 52.0, score_bars * 2.9, 6.0, 3.0);
        }

        if score >= high_score && score > 0 {
            self.shapes.set_color(Color::new(1.0, 0.85, 0.3, alpha * 0.9));
            self.shapes.draw_rect(cx - 30.0, center_y + 70.0, 60.0, 3.0);
        }

        let game_over_timer = self.world.get_resource::<GameStateRes>().unwrap().game_over_timer;
        if game_over_timer > 1.5 {
            let blink = (game_over_timer * 3.0).sin();
            if blink > 0.0 {
                self.shapes.set_color(Color::new(0.9, 0.9, 1.0, alpha * 0.6 * blink));
                self.shapes.draw_rect(cx - 60.0, center_y + 95.0, 120.0, 4.0);
            }
        }
    }
}

// ── WASM Exports ──────────────────────────────────────────────────────────────
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_score() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<ScoreRes>().map(|s| s.score).unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_lives() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<LivesRes>().map(|l| l.lives).unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_health() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.health))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_armor() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.armor))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_max_health() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.max_health))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_max_armor() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.max_armor))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_game_state() -> u8 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            match game.borrow().world.get_resource::<GameStateRes>() {
                Some(gs) => match gs.state {
                    GameState::Title => 0, GameState::Playing => 1, GameState::GameOver => 2,
                    GameState::Paused => 3,
                },
                None => 0,
            }
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_high_score() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<ScoreRes>().map(|s| s.high_score).unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
/// Dismiss the title popup and enable keyboard/gamepad start.
/// Called by JS when the popup is closed.
pub fn dismiss_popup() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(gs) = game.world.get_resource_mut::<GameStateRes>() {
                gs.can_start = true;
            }
        }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_game() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            let current = game.world.get_resource::<GameStateRes>().map(|gs| gs.state).unwrap_or(GameState::Title);
            if current == GameState::Title {
                let gs = game.world.get_resource_mut::<GameStateRes>().unwrap();
                gs.state = GameState::Playing;
                gs.can_start = true;
                drop(gs);
                game.reset_game();
            }
        }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn restart_game() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            let current = game.world.get_resource::<GameStateRes>().map(|gs| gs.state).unwrap_or(GameState::GameOver);
            if current == GameState::GameOver {
                game.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
                game.reset_game();
            }
        }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_ammo() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.ammo))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_max_ammo() -> i32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.max_ammo))
                .unwrap_or(0)
        } else { 0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_reloading() -> bool {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let game = game.borrow();
            let world = &game.world;
            QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.reloading))
                .unwrap_or(false)
        } else { false }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_eve_mode() -> bool {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<GameModeRes>()
                .map(|gm| gm.mode == GameMode::EVE)
                .unwrap_or(false)
        } else { false }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_eve_mode(enabled: bool) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            let new_mode = if enabled { GameMode::EVE } else { GameMode::PVE };
            let current = game.world.get_resource::<GameModeRes>()
                .map(|gm| gm.mode)
                .unwrap_or(GameMode::PVE);
            if current != new_mode {
                if let Some(gm) = game.world.get_resource_mut::<GameModeRes>() {
                    gm.mode = new_mode;
                }
                // Reset game to apply boosted/normal stats
                game.reset_game();
                game.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
            }
        }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_crosshair_x() -> f32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<InputState>()
                .map(|i| i.mouse_pos.x)
                .unwrap_or(0.0)
        } else { 0.0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_crosshair_y() -> f32 {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            game.borrow().world.get_resource::<InputState>()
                .map(|i| i.mouse_pos.y)
                .unwrap_or(0.0)
        } else { 0.0 }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn pause_game() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            let current = game.world.get_resource::<GameStateRes>().map(|gs| gs.state);
            if current == Some(GameState::Playing) {
                game.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Paused;
            }
        }
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn resume_game() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            let current = game.world.get_resource::<GameStateRes>().map(|gs| gs.state);
            if current == Some(GameState::Paused) {
                game.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
            }
        }
    })
}

/// Apply a single setting from JS. Key must match one of the known setting names.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_setting(key: &str, value: &str) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(s) = game.world.get_resource_mut::<SettingsRes>() {
                match key {
                    "master_volume" => { if let Ok(v) = value.parse::<f32>() { s.master_volume = v.clamp(0.0, 1.0); } }
                    "sfx_volume" => { if let Ok(v) = value.parse::<f32>() { s.sfx_volume = v.clamp(0.0, 1.0); } }
                    "music_volume" => { if let Ok(v) = value.parse::<f32>() { s.music_volume = v.clamp(0.0, 1.0); } }
                    "screen_shake" => { s.screen_shake = value == "true"; }
                    "show_fps" => { s.show_fps = value == "true"; }
                    "difficulty" => { if let Ok(v) = value.parse::<u8>() { s.difficulty = v.min(2); } }
                    "aim_assist" => { s.aim_assist = value == "true"; }
                    "crosshair_size" => { if let Ok(v) = value.parse::<f32>() { s.crosshair_size = v.clamp(0.5, 2.0); } }
                    "crosshair_r" => { if let Ok(v) = value.parse::<f32>() { s.crosshair_color[0] = v.clamp(0.0, 1.0); } }
                    "crosshair_g" => { if let Ok(v) = value.parse::<f32>() { s.crosshair_color[1] = v.clamp(0.0, 1.0); } }
                    "crosshair_b" => { if let Ok(v) = value.parse::<f32>() { s.crosshair_color[2] = v.clamp(0.0, 1.0); } }
                    "eve_mode" => {
                        let enabled = value == "true";
                        let new_mode = if enabled { GameMode::EVE } else { GameMode::PVE };
                        let current = game.world.get_resource::<GameModeRes>()
                            .map(|gm| gm.mode).unwrap_or(GameMode::PVE);
                        if current != new_mode {
                            if let Some(gm) = game.world.get_resource_mut::<GameModeRes>() {
                                gm.mode = new_mode;
                            }
                            game.reset_game();
                            game.world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
                        }
                    }
                    _ => {}
                }
            }
            // Apply difficulty presets to DifficultyRes
            if key == "difficulty" {
                if let Ok(d) = value.parse::<u8>() {
                    if let Some(diff) = game.world.get_resource_mut::<DifficultyRes>() {
                        match d {
                            0 => { diff.accuracy_mult = 0.7; diff.reaction_mult = 1.5; }
                            1 => { diff.accuracy_mult = 0.98; diff.reaction_mult = 0.5; }
                            2 => { diff.accuracy_mult = 1.0; diff.reaction_mult = 0.3; }
                            _ => {}
                        }
                    }
                }
            }
        }
    })
}

/// Drain new kill feed messages and return them as a pipe-separated string.
/// Returns empty string if no new messages. JS polls this each frame.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn drain_kill_feed() -> String {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(kill_feed) = game.world.get_resource_mut::<KillFeedRes>() {
                if kill_feed.new_entries.is_empty() { return String::new(); }
                let messages: Vec<String> = kill_feed.new_entries.drain(..).map(|e| e.message).collect();
                messages.join("|")
            } else { String::new() }
        } else { String::new() }
    })
}

// ── Mobile Touch Input ─────────────────────────────────────────────────────────

/// Set aim position from touch (screen coordinates). Called by JS on touch move.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_aim(x: f32, y: f32) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.mouse_pos = Vec2::new(x, y);
            }
        }
    })
}

/// Set touch shooting state. Called by JS when right-side screen is touched/released.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_shoot(shooting: bool) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.mouse_shoot = shooting;
            }
        }
    })
}

/// Set analog joystick movement. dx: -1.0 (left) to 1.0 (right).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_move(dx: f32) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.move_x = dx.clamp(-1.0, 1.0);
            }
        }
    })
}

/// Trigger a jump from mobile touch.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_jump(pressed: bool) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.jump_pressed = pressed;
            }
        }
    })
}

/// Trigger reload from mobile touch.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_reload() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.reload_pressed = true;
            }
        }
    })
}

/// Trigger slide from mobile touch.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_slide(sliding: bool) {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.slide_down = sliding;
            }
        }
    })
}

/// Trigger start/dismiss from mobile touch.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_touch_start() {
    GAME_REF.with(|g| {
        if let Some(ref game) = *g.borrow() {
            let mut game = game.borrow_mut();
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                input.start_pressed = true;
            }
        }
    })
}

// ── Entry Point ────────────────────────────────────────────────────────────────
#[cfg(all(target_arch = "wasm32", not(test)))]
#[wasm_bindgen(start)]
pub fn main() {
    let mut game = ScpGame::new().expect("Failed to create SCP Game");
    game.time.init();

    let game = Rc::new(RefCell::new(game));
    GAME_REF.with(|g| { *g.borrow_mut() = Some(game.clone()); });

    let f: AnimationFrameClosure = Rc::new(RefCell::new(None));
    let g = f.clone();
    let game_clone = game.clone();
    let mut last_time = 0.0_f64;

    *g.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
        let dt = if last_time == 0.0 { 1.0 / 60.0 } else { ((timestamp - last_time) / 1000.0).min(0.05) };
        last_time = timestamp;

        let mut game = game_clone.borrow_mut();
        game.time.update();
        game.input.update();
        game.poll_input();
        game.update(dt as f32);
        game.render(1.0);
        drop(game);

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

#[cfg(target_arch = "wasm32")]
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    web_sys::window().unwrap().request_animation_frame(f.as_ref().unchecked_ref()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═════════════════════════════════════════════════════════════════════════
    // ── Constants Tests ─────────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn constants_are_positive() {
        assert!(PLAYER_SIZE > 0.0);
        assert!(PLAYER_SPEED > 0.0);
        assert!(JUMP_FORCE > 0.0);
        assert!(GRAVITY > 0.0);
        assert!(GROUND_Y > 0.0);
        assert!(BULLET_SPEED > 0.0);
        assert!(ENEMY_BULLET_SPEED > 0.0);
        assert!(SHOOT_INTERVAL > 0.0);
        assert!(INVINCIBLE_TIME > 0.0);
        assert!(RELOAD_TIME > 0.0);
    }

    #[test]
    fn constants_world_dimensions() {
        assert!(WORLD_W > 0.0);
        assert!(WORLD_H > 0.0);
        assert!(LEVEL_W > WORLD_W, "Level should be wider than viewport");
    }

    #[test]
    fn constants_player_limits() {
        assert!(MAX_LIVES > 0);
        assert!(MAX_HEALTH > 0);
        assert!(MAX_ARMOR > 0);
        assert!(MAX_AMMO > 0);
        assert!(HIT_DAMAGE > 0);
        assert!(HIT_DAMAGE <= MAX_HEALTH, "Single hit shouldn't exceed max health");
    }

    #[test]
    fn constants_enemy_limits() {
        assert!(MAX_ENEMIES > 0);
        assert!(MAX_BULLETS > 0);
        assert!(MAX_PARTICLES > 0);
        assert!(ENEMY_MAX_AMMO > 0);
        assert!(TANK_MAX_AMMO > 0);
    }

    #[test]
    fn constants_camera() {
        assert!(CAM_DEAD_ZONE_X > 0.0);
        assert!(CAM_SMOOTH > 0.0);
    }

    #[test]
    fn constants_enemy_speeds_ordered() {
        // Scouts should be fastest, tanks slowest
        assert!(SCOUT_SPEED_MIN > TANK_SPEED_MIN);
        assert!(SCOUT_SPEED_MAX > TANK_SPEED_MAX);
        assert!(FLANKER_SPEED_MIN > GRUNT_SPEED_MIN);
    }

    #[test]
    fn constants_enemy_hp() {
        assert!(TANK_HP > GRUNT_HP, "Tanks should have more HP than grunts");
        assert!(GRUNT_HP >= SCOUT_HP, "Grunts should have at least as much HP as scouts");
        assert!(SCOUT_HP > 0);
    }

    #[test]
    fn constants_enemy_types_positive() {
        assert!(SCOUT_SPEED_MIN > 0.0);
        assert!(SCOUT_SPEED_MAX > SCOUT_SPEED_MIN);
        assert!(GRUNT_SPEED_MIN > 0.0);
        assert!(GRUNT_SPEED_MAX > GRUNT_SPEED_MIN);
        assert!(TANK_SPEED_MIN > 0.0);
        assert!(TANK_SPEED_MAX > TANK_SPEED_MIN);
        assert!(FLANKER_SPEED_MIN > 0.0);
        assert!(FLANKER_SPEED_MAX > FLANKER_SPEED_MIN);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Particle Color Tests ────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn particle_color_returns_valid_colors() {
        for idx in 0..7 {
            let c = particle_color(idx);
            assert!(c.r >= 0.0 && c.r <= 1.0, "Red out of range for idx={}: {}", idx, c.r);
            assert!(c.g >= 0.0 && c.g <= 1.0, "Green out of range for idx={}: {}", idx, c.g);
            assert!(c.b >= 0.0 && c.b <= 1.0, "Blue out of range for idx={}: {}", idx, c.b);
            assert!((c.a - 1.0).abs() < 0.01, "Alpha should be 1.0 for idx={}: {}", idx, c.a);
        }
    }

    #[test]
    fn particle_color_wraps_at_7() {
        // particle_color uses idx % 7, so idx 0 and 7 should give the same result
        let c0 = particle_color(0);
        let c7 = particle_color(7);
        assert!((c0.r - c7.r).abs() < 0.01);
        assert!((c0.g - c7.g).abs() < 0.01);
        assert!((c0.b - c7.b).abs() < 0.01);
    }

    #[test]
    fn particle_color_all_unique() {
        let mut colors = Vec::new();
        for idx in 0..7 {
            let c = particle_color(idx);
            colors.push((c.r, c.g, c.b));
        }
        // Check that all 7 colors are distinct
        for i in 0..7 {
            for j in (i + 1)..7 {
                let (r1, g1, b1) = colors[i];
                let (r2, g2, b2) = colors[j];
                assert!(
                    (r1 - r2).abs() > 0.01 || (g1 - g2).abs() > 0.01 || (b1 - b2).abs() > 0.01,
                    "Colors {} and {} should be different: ({},{},{}) vs ({},{},{})",
                    i, j, r1, g1, b1, r2, g2, b2
                );
            }
        }
    }

    #[test]
    fn particle_color_high_indices() {
        // Should work with large indices due to modulo
        let c = particle_color(100);
        assert!(c.r >= 0.0 && c.r <= 1.0);
        let c = particle_color(255);
        assert!(c.r >= 0.0 && c.r <= 1.0);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Init World Tests ────────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    fn setup_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(GameStateRes::default());
        world.insert_resource(ScoreRes::default());
        world.insert_resource(LivesRes::default());
        world.insert_resource(CameraRes::default());
        world.insert_resource(SpawnRes::default());
        world.insert_resource(InputState::default());
        world.insert_resource(ViewportRes::default());
        world.insert_resource(DifficultyRes::default());
        world.insert_resource(SettingsRes::default());
        world.insert_resource(KillFeedRes::default());
        world.insert_resource(GameModeRes::default());
        world.insert_resource(MapRes::default());
        world
    }

    fn spawn_test_player(world: &mut World) -> opengame_engine::ecs::Entity {
        world.spawn()
            .with(components::Player {
                facing_right: true, invincible: 0.0, flash: 0.0, shoot_timer: 0.0,
                on_ground: true, sliding: false, slide_timer: 0.0, aim_angle: 0.0,
                ammo: 30, max_ammo: 30, reloading: false, reload_timer: 0.0,
                footstep_timer: 0.0, health: 100, max_health: 100, armor: 50, max_armor: 50,
            })
            .with(Transform2D::new(Vec2::new(100.0, GROUND_Y - PLAYER_SIZE)))
            .with(components::Velocity { x: 0.0, y: 0.0, gravity_scale: 1.0 })
            .build()
    }

    #[test]
    fn world_setup_has_all_resources() {
        let world = setup_test_world();
        assert!(world.get_resource::<GameStateRes>().is_some());
        assert!(world.get_resource::<ScoreRes>().is_some());
        assert!(world.get_resource::<LivesRes>().is_some());
        assert!(world.get_resource::<CameraRes>().is_some());
        assert!(world.get_resource::<SpawnRes>().is_some());
        assert!(world.get_resource::<InputState>().is_some());
        assert!(world.get_resource::<ViewportRes>().is_some());
        assert!(world.get_resource::<DifficultyRes>().is_some());
        assert!(world.get_resource::<SettingsRes>().is_some());
        assert!(world.get_resource::<KillFeedRes>().is_some());
        assert!(world.get_resource::<GameModeRes>().is_some());
        assert!(world.get_resource::<MapRes>().is_some());
    }

    #[test]
    fn player_spawn_and_query() {
        let mut world = setup_test_world();
        let e = spawn_test_player(&mut world);

        let query = QuerySingle::<components::Player>::new(&world);
        assert!(query.is_some());
        let q = query.unwrap();
        assert_eq!(q.len(), 1);

        let p = q.get(e);
        assert!(p.is_some());
        assert_eq!(p.unwrap().health, 100);
    }
}
