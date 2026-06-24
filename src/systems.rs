use opengame_engine::ecs::{World, QuerySingle};
use opengame_engine::math::Vec2;
use opengame_engine::transform::Transform2D;

use crate::components::*;
use crate::resources::*;
use crate::{rand, rand_range, PLAYER_SIZE, PLAYER_SPEED, JUMP_FORCE, BULLET_SPEED,
    ENEMY_BULLET_SPEED, SHOOT_INTERVAL, INVINCIBLE_TIME, GROUND_Y, WORLD_W, WORLD_H, LEVEL_W,
    MAX_BULLETS, MAX_ENEMIES, MAX_PARTICLES, CAM_DEAD_ZONE_X, CAM_SMOOTH, GRAVITY,
    SCOUT_SPEED_MIN, SCOUT_SPEED_MAX, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX,
    TANK_SPEED_MIN, TANK_SPEED_MAX, FLANKER_SPEED_MIN, FLANKER_SPEED_MAX,
    SCOUT_HP, GRUNT_HP, TANK_HP, FLANKER_HP, DODGE_RANGE, DODGE_COOLDOWN,
    RELOAD_TIME, ENEMY_MAX_AMMO, ENEMY_RELOAD_TIME, TANK_MAX_AMMO, TANK_RELOAD_TIME,
    EVE_SPAWN_INTERVAL, EVE_MAX_ENEMIES};

struct HitEvent { x: f32, y: f32, shake: f32, score: i32, hit_player: bool }

// ── AABB Helpers ─────────────────────────────────────────────────────────────

/// Test overlap between two axis-aligned rectangles defined by center (cx, cy) and half-size (hw, hh).
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn aabb_overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    (ax - bx).abs() < (aw + bw) * 0.5 && (ay - by).abs() < (ah + bh) * 0.5
}

// ── Custom Physics Step ──────────────────────────────────────────────────────

/// Replaces the engine's PhysicsSystem: applies gravity, integrates velocity,
/// clamps to ground, and runs AABB collision detection for game logic.
pub fn physics_step(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    // 1. Apply gravity and integrate velocity → position for all dynamic entities
    {
        let entities: Vec<_> = QuerySingle::<Velocity>::new(world)
            .map(|q| q.iter().map(|(e, _)| e).collect())
            .unwrap_or_default();
        for e in entities {
            let (vx, vy, _gs) = {
                match world.get_component::<Velocity>(e) {
                    Some(v) => (v.x, v.y + GRAVITY * v.gravity_scale * dt, v.gravity_scale),
                    None => continue,
                }
            };
            if let Some(v) = world.get_component_mut::<Velocity>(e) {
                v.y = vy;
            }
            if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                t.position.x += vx * dt;
                t.position.y += vy * dt;
            }
        }
    }

    // 2. Ground constraint: clamp player and enemies so they never sink below floor
    //    Position is top-left corner, so max_y = GROUND_Y - height
    {
        let entities: Vec<_> = QuerySingle::<Player>::new(world)
            .map(|q| q.iter().map(|(e, _)| e).collect())
            .unwrap_or_default();
        for e in entities {
            let max_y = GROUND_Y - PLAYER_SIZE;
            let pos_y = world.get_component::<Transform2D>(e)
                .map(|t| t.position.y).unwrap_or(0.0);
            let mut grounded = false;
            if pos_y >= max_y {
                grounded = true;
                if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                    t.position.y = max_y;
                }
                if let Some(v) = world.get_component_mut::<Velocity>(e) {
                    if v.y > 0.0 { v.y = 0.0; }
                }
            }
            if let Some(p) = world.get_component_mut::<Player>(e) {
                p.on_ground = grounded;
            }
        }
    }
    {
        let entities: Vec<_> = QuerySingle::<Enemy>::new(world)
            .map(|q| q.iter().map(|(e, en)| (e, en.size)).collect())
            .unwrap_or_default();
        for (e, size) in entities {
            let max_y = GROUND_Y - size;
            let pos_y = world.get_component::<Transform2D>(e)
                .map(|t| t.position.y).unwrap_or(0.0);
            let mut grounded = false;
            if pos_y >= max_y {
                grounded = true;
                if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                    t.position.y = max_y;
                }
                if let Some(v) = world.get_component_mut::<Velocity>(e) {
                    if v.y > 0.0 { v.y = 0.0; }
                }
            }
            if let Some(en) = world.get_component_mut::<Enemy>(e) {
                en.on_ground = grounded;
            }
        }
    }

    // 3. Obstacle collision resolution
    resolve_obstacle_collisions(world);

    // 4. Level bounds: clamp X for player and enemies
    {
        let entities: Vec<_> = QuerySingle::<Player>::new(world)
            .map(|q| q.iter().map(|(e, _)| e).collect())
            .unwrap_or_default();
        for e in entities {
            if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                t.position.x = t.position.x.clamp(0.0, LEVEL_W - PLAYER_SIZE);
            }
        }
    }

    // 5. AABB collision detection for game logic
    run_collision_detection(world);
}

// ── Obstacle Collision ───────────────────────────────────────────────────────

/// Resolves AABB overlaps between dynamic entities (player + enemies) and map obstacles.
/// Pushes entities out on the axis of least penetration.
fn resolve_obstacle_collisions(world: &mut World) {
    // Collect obstacle data into a local vec to avoid borrow conflicts
    let obstacles: Vec<(f32, f32, f32, f32)> = match world.get_resource::<MapRes>() {
        Some(m) => m.obstacles.iter().map(|o| (o.x, o.y, o.w, o.h)).collect(),
        None => return,
    };

    // Player collision
    {
        let entities: Vec<_> = QuerySingle::<Player>::new(world)
            .map(|q| q.iter().map(|(e, _)| e).collect())
            .unwrap_or_default();
        for e in entities {
            let (px, py) = world.get_component::<Transform2D>(e)
                .map(|t| (t.position.x, t.position.y)).unwrap_or((0.0, 0.0));
            let pw = PLAYER_SIZE;
            let ph = PLAYER_SIZE;

            for &(ox, oy, ow, oh) in &obstacles {
                if !aabb_overlap(
                    px + pw * 0.5, py + ph * 0.5, pw, ph,
                    ox + ow * 0.5, oy + oh * 0.5, ow, oh,
                ) {
                    continue;
                }

                // Calculate penetration on each axis
                let overlap_x = (pw + ow) * 0.5 - ((px + pw * 0.5) - (ox + ow * 0.5)).abs();
                let overlap_y = (ph + oh) * 0.5 - ((py + ph * 0.5) - (oy + oh * 0.5)).abs();

                if overlap_x < overlap_y {
                    // Resolve horizontally
                    if px + pw * 0.5 < ox + ow * 0.5 {
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.x = ox - pw;
                        }
                    } else {
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.x = ox + ow;
                        }
                    }
                    if let Some(v) = world.get_component_mut::<Velocity>(e) {
                        v.x = 0.0;
                    }
                } else {
                    // Resolve vertically
                    if py + ph * 0.5 < oy + oh * 0.5 {
                        // Landing on top
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.y = oy - ph;
                        }
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            if v.y > 0.0 { v.y = 0.0; }
                        }
                        if let Some(p) = world.get_component_mut::<Player>(e) {
                            p.on_ground = true;
                        }
                    } else {
                        // Hitting from below
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.y = oy + oh;
                        }
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            if v.y < 0.0 { v.y = 0.0; }
                        }
                    }
                }
            }
        }
    }

    // Enemy collision
    {
        let entities: Vec<_> = QuerySingle::<Enemy>::new(world)
            .map(|q| q.iter().filter(|(_, en)| en.alive).map(|(e, en)| (e, en.size)).collect())
            .unwrap_or_default();
        for (e, size) in entities {
            let (ex, ey) = world.get_component::<Transform2D>(e)
                .map(|t| (t.position.x, t.position.y)).unwrap_or((0.0, 0.0));

            for &(ox, oy, ow, oh) in &obstacles {
                if !aabb_overlap(
                    ex + size * 0.5, ey + size * 0.5, size, size,
                    ox + ow * 0.5, oy + oh * 0.5, ow, oh,
                ) {
                    continue;
                }

                let overlap_x = (size + ow) * 0.5 - ((ex + size * 0.5) - (ox + ow * 0.5)).abs();
                let overlap_y = (size + oh) * 0.5 - ((ey + size * 0.5) - (oy + oh * 0.5)).abs();

                if overlap_x < overlap_y {
                    // Side collision — check if enemy should climb
                    let climb_cooldown = world.get_component::<Enemy>(e)
                        .map(|en| en.climb_cooldown).unwrap_or(1.0);
                    let enemy_type = world.get_component::<Enemy>(e)
                        .map(|en| en.enemy_type).unwrap_or(EnemyType::Grunt);

                    let can_climb = climb_cooldown <= 0.0
                        && oh < 100.0
                        && enemy_type != EnemyType::Tank;

                    if can_climb {
                        // Climb: jump up and keep moving horizontally
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            v.y = -JUMP_FORCE * 0.85;
                        }
                        if let Some(en) = world.get_component_mut::<Enemy>(e) {
                            en.climb_cooldown = 1.0;
                        }
                    } else {
                        // Standard side resolution
                        if ex + size * 0.5 < ox + ow * 0.5 {
                            if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                                t.position.x = ox - size;
                            }
                        } else {
                            if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                                t.position.x = ox + ow;
                            }
                        }
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            v.x = 0.0;
                        }
                    }
                } else {
                    // Vertical resolution
                    if ey + size * 0.5 < oy + oh * 0.5 {
                        // Landing on top
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.y = oy - size;
                        }
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            if v.y > 0.0 { v.y = 0.0; }
                        }
                        if let Some(en) = world.get_component_mut::<Enemy>(e) {
                            en.on_ground = true;
                        }
                    } else {
                        // Hitting from below
                        if let Some(t) = world.get_component_mut::<Transform2D>(e) {
                            t.position.y = oy + oh;
                        }
                        if let Some(v) = world.get_component_mut::<Velocity>(e) {
                            if v.y < 0.0 { v.y = 0.0; }
                        }
                    }
                }
            }
        }
    }
}

// ── Collision Detection (AABB) ───────────────────────────────────────────────

fn run_collision_detection(world: &mut World) {
    let mut events: Vec<HitEvent> = Vec::new();

    // Collect entity data to avoid borrow conflicts
    let player_data: Vec<(opengame_engine::ecs::Entity, f32, f32, f32, bool)> = QuerySingle::<Player>::new(world)
        .map(|q| q.iter().map(|(e, p)| {
            let pos = world.get_component::<Transform2D>(e)
                .map(|t| (t.position.x, t.position.y)).unwrap_or((0.0, 0.0));
            (e, pos.0, pos.1, p.invincible, p.on_ground)
        }).collect())
        .unwrap_or_default();

    let enemy_data: Vec<(opengame_engine::ecs::Entity, f32, f32, f32, bool)> = QuerySingle::<Enemy>::new(world)
        .map(|q| q.iter().filter(|(_, en)| en.alive).map(|(e, en)| {
            let pos = world.get_component::<Transform2D>(e)
                .map(|t| (t.position.x, t.position.y)).unwrap_or((0.0, 0.0));
            (e, pos.0, pos.1, en.size, en.hp > 0)
        }).collect())
        .unwrap_or_default();

    let bullet_data: Vec<(opengame_engine::ecs::Entity, f32, f32, bool, bool)> = QuerySingle::<Bullet>::new(world)
        .map(|q| q.iter().filter(|(_, b)| b.alive).map(|(e, b)| {
            (e, b.x, b.y, b.is_player, b.alive)
        }).collect())
        .unwrap_or_default();

    // Player bullets vs enemies
    for &(bullet_e, bx, by, is_player, _) in &bullet_data {
        if !is_player { continue; }
        for &(enemy_e, ex, ey, esize, _) in &enemy_data {
            if aabb_overlap(bx, by, 10.0, 4.0, ex + esize * 0.5, ey + esize * 0.5, esize, esize) {
                // Mark bullet dead
                if let Some(b) = world.get_component_mut::<Bullet>(bullet_e) {
                    b.alive = false;
                }
                // Hit enemy
                if let Some(enemy) = world.get_component_mut::<Enemy>(enemy_e) {
                    if enemy.hp > 0 {
                        enemy.hp -= 1;
                        enemy.flash = 1.0;
                        if enemy.hp <= 0 {
                            enemy.alive = false;
                            events.push(HitEvent { x: ex, y: ey, shake: 6.0, score: 100, hit_player: false });
                        }
                    }
                }
                break; // bullet can only hit one enemy
            }
        }
    }

    // Enemy bullets vs player
    for &(bullet_e, bx, by, is_player, _) in &bullet_data {
        if is_player { continue; }
        for &(_player_e, px, py, invincible, _) in &player_data {
            if invincible > 0.0 { continue; }
            if aabb_overlap(bx, by, 6.0, 4.0, px + PLAYER_SIZE * 0.5, py + PLAYER_SIZE * 0.5, PLAYER_SIZE, PLAYER_SIZE) {
                if let Some(b) = world.get_component_mut::<Bullet>(bullet_e) {
                    b.alive = false;
                }
                events.push(HitEvent { x: px, y: py, shake: 14.0, score: 0, hit_player: true });
                break;
            }
        }
    }

    // Player vs enemies (contact damage)
    for &(_player_e, px, py, invincible, _) in &player_data {
        if invincible > 0.0 { continue; }
        for &(enemy_e, ex, ey, esize, _) in &enemy_data {
            if aabb_overlap(px + PLAYER_SIZE * 0.5, py + PLAYER_SIZE * 0.5, PLAYER_SIZE, PLAYER_SIZE,
                            ex + esize * 0.5, ey + esize * 0.5, esize, esize) {
                events.push(HitEvent { x: ex, y: ey, shake: 14.0, score: 0, hit_player: true });
                if let Some(e) = world.get_component_mut::<Enemy>(enemy_e) {
                    e.alive = false;
                }
                break; // one hit per frame
            }
        }
    }

    // Apply hit events
    for ev in events {
        let count = if ev.score > 0 { 22 } else { 15 };
        let power = if ev.score > 0 { 250.0 } else { 200.0 };
        spawn_explosion_particles(world, ev.x, ev.y, count, power);

        let cam = world.get_resource_mut::<CameraRes>().unwrap();
        cam.shake_amount = (cam.shake_amount + ev.shake).min(18.0);

        if ev.score > 0 {
            world.get_resource_mut::<ScoreRes>().unwrap().score += ev.score;

            // Add kill feed message
            let kill_feed = world.get_resource_mut::<KillFeedRes>().unwrap();
            kill_feed.new_entries.push(KillFeedEntry {
                message: "设施警卫已消灭一名混沌分裂者阿尔法级特工。".to_string(),
                timer: 4.0,
            });

            crate::play_sound_js("hit");
        }

        if ev.hit_player {
            crate::play_sound_js("hurt");

            // Armor absorbs damage first, remainder goes to health
            let mut remaining_damage = crate::HIT_DAMAGE;
            let entities: Vec<_> = QuerySingle::<Player>::new(world)
                .map(|q| q.iter().map(|(e, _)| e).collect())
                .unwrap_or_default();
            for e in &entities {
                if let Some(p) = world.get_component_mut::<Player>(*e) {
                    if p.armor > 0 {
                        let absorbed = p.armor.min(remaining_damage);
                        p.armor -= absorbed;
                        remaining_damage -= absorbed;
                    }
                    if remaining_damage > 0 {
                        p.health -= remaining_damage;
                    }
                }
            }

            // Check if player died
            let player_alive = QuerySingle::<Player>::new(world)
                .and_then(|q| q.iter().next().map(|(_, p)| p.health > 0))
                .unwrap_or(false);

            if !player_alive {
                let pos = QuerySingle::<Player>::new(world)
                    .and_then(|q| q.iter().next().map(|(e, _)| e))
                    .and_then(|e| world.get_component::<Transform2D>(e))
                    .map(|t| (t.position.x, t.position.y))
                    .unwrap_or((400.0, 300.0));
                spawn_explosion_particles(world, pos.0, pos.1, 45, 350.0);
                world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::GameOver;
                world.get_resource_mut::<GameStateRes>().unwrap().game_over_timer = 0.0;
            } else {
                for e in entities {
                    if let Some(p) = world.get_component_mut::<Player>(e) {
                        p.invincible = INVINCIBLE_TIME;
                        p.flash = 0.0;
                    }
                }
            }
        }
    }
}

// ── Player Movement ──────────────────────────────────────────────────────────

pub fn player_move_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    let input = world.get_resource::<InputState>().unwrap();
    let left = input.left;
    let right = input.right;
    let jump = input.jump_pressed;
    let slide = input.slide_down;
    let mouse_pos = input.mouse_pos;
    let reload_pressed = input.reload_pressed;
    let move_x = input.move_x;

    // Get camera and viewport info for coordinate conversion
    let camera_x = world.get_resource::<CameraRes>().unwrap().camera_x;
    let viewport = world.get_resource::<ViewportRes>().unwrap();
    let vp_x = viewport.vp_x as f32;
    let vp_y = viewport.vp_y as f32;
    let scale = viewport.scale;

    // Convert screen mouse to world coordinates (accounting for letterboxing)
    let world_mouse_x = (mouse_pos.x - vp_x) / scale + camera_x;
    let world_mouse_y = (mouse_pos.y - vp_y) / scale;

    let entities: Vec<_> = QuerySingle::<Player>::new(world)
        .map(|q| q.iter().map(|(e, _)| e).collect())
        .unwrap_or_default();

    for entity in entities {
        let (facing_right, on_ground, shoot_timer, sliding, slide_timer,
             ammo, max_ammo, reloading, reload_timer, footstep_timer) = {
            match world.get_component::<Player>(entity) {
                Some(p) => (p.facing_right, p.on_ground, p.shoot_timer, p.sliding, p.slide_timer,
                           p.ammo, p.max_ammo, p.reloading, p.reload_timer, p.footstep_timer),
                None => continue,
            }
        };

        let (px, py) = {
            match world.get_component::<Transform2D>(entity) {
                Some(t) => (t.position.x, t.position.y),
                None => continue,
            }
        };

        // Calculate aim angle from mouse position in world space
        let player_center_x = px + PLAYER_SIZE * 0.5;
        let player_center_y = py + PLAYER_SIZE * 0.5;
        let aim_angle = (world_mouse_y - player_center_y).atan2(world_mouse_x - player_center_x);

        // Determine facing direction from aim
        let new_facing = world_mouse_x >= player_center_x;

        // Sliding
        let mut is_sliding = sliding;
        let mut new_slide_timer = slide_timer - dt;

        if slide && on_ground && !sliding && new_slide_timer <= 0.0 {
            is_sliding = true;
            new_slide_timer = 0.4; // slide duration
        }

        if new_slide_timer <= 0.0 {
            is_sliding = false;
        }

        // Reload logic
        let mut new_reloading = reloading;
        let mut new_reload_timer = reload_timer - dt;
        let mut new_ammo = ammo;

        // Manual reload (R key)
        if reload_pressed && !reloading && ammo < max_ammo {
            new_reloading = true;
            new_reload_timer = RELOAD_TIME;
        }

        // Auto-reload when ammo is empty
        if ammo <= 0 && !reloading {
            new_reloading = true;
            new_reload_timer = RELOAD_TIME;
        }

        // Complete reload
        if new_reloading && new_reload_timer <= 0.0 {
            new_reloading = false;
            new_ammo = max_ammo;
            crate::play_sound_js("reload");
        }

        // Horizontal velocity
        let speed = if is_sliding { PLAYER_SPEED * 1.8 } else { PLAYER_SPEED };
        let mut vx: f32 = 0.0;
        if is_sliding {
            // Slide in facing direction
            vx = if facing_right { speed } else { -speed };
        } else if move_x.abs() > 0.01 {
            // Analog joystick input (mobile)
            vx = move_x * speed;
        } else {
            if left { vx = -speed; }
            if right { vx = speed; }
        }

        if let Some(v) = world.get_component_mut::<Velocity>(entity) {
            v.x = vx;
        }

        // Jump — set upward velocity directly
        if jump && on_ground && !is_sliding {
            if let Some(v) = world.get_component_mut::<Velocity>(entity) {
                v.y = -JUMP_FORCE;
            }
            crate::play_sound_js("jump");
        }

        // Footstep sound — play at intervals while moving on ground
        let is_moving = vx.abs() > 1.0 && on_ground && !is_sliding;
        let mut new_footstep_timer = footstep_timer - dt;
        if is_moving && new_footstep_timer <= 0.0 {
            crate::play_sound_js("footstep");
            new_footstep_timer = 0.3; // footstep interval
        }
        if !is_moving {
            new_footstep_timer = 0.0; // reset when stopped
        }

        // Update player state
        if let Some(p) = world.get_component_mut::<Player>(entity) {
            p.facing_right = new_facing;
            p.shoot_timer = (shoot_timer - dt).max(0.0);
            p.sliding = is_sliding;
            p.slide_timer = new_slide_timer;
            p.aim_angle = aim_angle;
            p.ammo = new_ammo;
            p.reloading = new_reloading;
            p.reload_timer = new_reload_timer;
            p.footstep_timer = new_footstep_timer;
            if jump && on_ground { p.on_ground = false; }
            if p.invincible > 0.0 {
                p.invincible -= dt;
                p.flash += dt * 15.0;
            }
        }
    }
}

// ── Player Shoot ─────────────────────────────────────────────────────────────

pub fn player_shoot_system(world: &mut World, _dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    let input = world.get_resource::<InputState>().unwrap();
    let shooting = input.shoot_down || input.mouse_shoot;
    if !shooting { return; }

    let player_entity = QuerySingle::<Player>::new(world)
        .and_then(|q| q.iter().next().map(|(e, _)| e));

    let entity = match player_entity {
        Some(e) => e,
        None => return,
    };

    let (can_shoot, aim_angle, ammo, reloading) = {
        match world.get_component::<Player>(entity) {
            Some(p) => (p.shoot_timer <= 0.0, p.aim_angle, p.ammo, p.reloading),
            None => return,
        }
    };

    // Can't shoot while reloading or if no ammo
    if !can_shoot || reloading || ammo <= 0 { return; }

    let (px, py) = {
        match world.get_component::<Transform2D>(entity) {
            Some(t) => (t.position.x, t.position.y),
            None => return,
        }
    };

    if let Some(p) = world.get_component_mut::<Player>(entity) {
        p.shoot_timer = SHOOT_INTERVAL;
        p.ammo -= 1;
    }

    let bullet_count = QuerySingle::<Bullet>::new(world)
        .map(|q| q.iter().filter(|(_, b)| b.is_player).count())
        .unwrap_or(0);
    if bullet_count >= MAX_BULLETS { return; }

    // Fire bullet toward aim direction
    let mut final_angle = aim_angle;

    // Aim assist: if crosshair is near an enemy's edge, nudge trajectory toward center
    let aim_assist = world.get_resource::<SettingsRes>().map(|s| s.aim_assist).unwrap_or(false);
    if aim_assist {
        let player_cx = px + PLAYER_SIZE * 0.5;
        let player_cy = py + PLAYER_SIZE * 0.5;
        let aim_range = 300.0; // how far to check along the aim line
        let aim_x = player_cx + aim_angle.cos() * aim_range;
        let aim_y = player_cy + aim_angle.sin() * aim_range;

        let mut best_enemy: Option<(f32, f32, f32, f32)> = None; // (center_x, center_y, size, dist_to_edge)
        let assist_inner = 15.0;
        let assist_outer = 20.0;

        if let Some(q) = QuerySingle::<Enemy>::new(world) {
            for (_e, enemy) in q.iter() {
                if !enemy.alive { continue; }
                if let Some(et) = world.get_component::<Transform2D>(_e) {
                    let ex = et.position.x;
                    let ey = et.position.y;
                    let es = enemy.size;

                    // Closest point on enemy AABB to the aim line point
                    let closest_x = aim_x.max(ex).min(ex + es);
                    let closest_y = aim_y.max(ey).min(ey + es);
                    let dx = aim_x - closest_x;
                    let dy = aim_y - closest_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    // Check if crosshair is in the assist ring (outside edge, within range)
                    if dist <= assist_outer {
                        // Distance from crosshair to enemy center
                        let to_center = ((aim_x - (ex + es * 0.5)).powi(2)
                            + (aim_y - (ey + es * 0.5)).powi(2)).sqrt();
                        match best_enemy {
                            Some((_, _, _, best_dist)) if dist >= best_dist => {}
                            _ => best_enemy = Some((ex + es * 0.5, ey + es * 0.5, es, dist)),
                        }
                    }
                }
            }
        }

        if let Some((ecx, ecy, _es, dist)) = best_enemy {
            // Blend factor: 5% at outer edge, 10% at inner edge
            let t = 0.05 + 0.05 * ((assist_outer - dist) / (assist_outer - assist_inner)).clamp(0.0, 1.0);
            let to_enemy = (ecy - player_cy).atan2(ecx - player_cx);
            final_angle = aim_angle + (to_enemy - aim_angle) * t;
        }
    }

    let bvx = final_angle.cos() * BULLET_SPEED;
    let bvy = final_angle.sin() * BULLET_SPEED;
    let bx = px + PLAYER_SIZE * 0.5 + final_angle.cos() * PLAYER_SIZE * 0.6;
    let by = py + PLAYER_SIZE * 0.5 + final_angle.sin() * PLAYER_SIZE * 0.6;

    world.spawn()
        .with(Bullet { x: bx, y: by, vx: bvx, vy: bvy, alive: true, is_player: true })
        .build();

    // Play gunshot sound directly via JS callback
    crate::play_sound_js("shoot");
}

// ── Player AI (EVE Mode) ─────────────────────────────────────────────────────

/// AI system that controls the player character in EVE mode.
/// Tactical style: stays at long range, aims carefully, then fires.
/// When swarmed by many enemies, enters a retreat mode — flees from the
/// enemy mass while laying down continuous fire at the nearest target.
pub fn player_ai_system(world: &mut World, _dt: f32) {
    // Collect player info
    let player_info = QuerySingle::<Player>::new(world)
        .and_then(|q| q.iter().next().map(|(e, _)| e))
        .and_then(|e| {
            let pos = world.get_component::<Transform2D>(e).map(|t| (t.position.x, t.position.y));
            let vel = world.get_component::<Velocity>(e).map(|v| (v.x, v.y));
            let p = world.get_component::<Player>(e);
            pos.zip(vel).zip(p).map(|(((px, py), (vx, vy)), p)| {
                (px, py, vx, vy, p.on_ground, p.shoot_timer, p.ammo, p.reloading, p.aim_angle)
            })
        });

    let (px, py, _pvx, _pvy, on_ground, shoot_timer, ammo, reloading, prev_aim) = match player_info {
        Some(info) => info,
        None => return,
    };

    // Collect enemy data
    let enemies: Vec<(f32, f32, f32, f32, f32, bool)> = QuerySingle::<Enemy>::new(world)
        .map(|q| q.iter()
            .filter(|(_, en)| en.alive)
            .map(|(e, en)| {
                let pos = world.get_component::<Transform2D>(e)
                    .map(|t| (t.position.x, t.position.y))
                    .unwrap_or((0.0, 0.0));
                let vel = world.get_component::<Velocity>(e)
                    .map(|v| (v.x, v.y))
                    .unwrap_or((0.0, 0.0));
                (pos.0, pos.1, vel.0, vel.1, en.size, en.hp > 0)
            })
            .collect())
        .unwrap_or_default();

    if enemies.is_empty() {
        // No enemies — stand still, don't shoot
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.left = false;
        input.right = false;
        input.jump_pressed = false;
        input.slide_down = false;
        input.shoot_down = false;
        input.mouse_shoot = false;
        input.reload_pressed = false;
        return;
    }

    // ── Find nearest enemy and count nearby threats ──
    let player_cx = px + PLAYER_SIZE * 0.5;
    let player_cy = py + PLAYER_SIZE * 0.5;

    let swarm_radius = 400.0;   // radius to count threats for swarm detection
    let swarm_threshold: usize = 4; // enemy count that triggers retreat mode

    let mut nearest_dist = f32::MAX;
    let mut nearest_idx = 0usize;
    let mut nearby_count: usize = 0;
    let mut sum_x = 0.0f32; // centroid accumulator for nearby enemies
    let mut sum_y = 0.0f32;

    for (i, &(ex, ey, _, _, size, _)) in enemies.iter().enumerate() {
        let ecx = ex + size * 0.5;
        let ecy = ey + size * 0.5;
        let dx = ecx - player_cx;
        let dy = ecy - player_cy;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < nearest_dist {
            nearest_dist = dist;
            nearest_idx = i;
        }
        if dist < swarm_radius {
            nearby_count += 1;
            sum_x += ecx;
            sum_y += ecy;
        }
    }

    let is_swarmed = nearby_count >= swarm_threshold;

    let (ex, ey, evx, evy, esize, _) = enemies[nearest_idx];
    let enemy_cx = ex + esize * 0.5;
    let enemy_cy = ey + esize * 0.5;
    let dx = enemy_cx - player_cx;

    // Predictive aiming — lead the target
    let pred_time = nearest_dist / BULLET_SPEED;
    let aim_x = enemy_cx + evx * pred_time * 0.8;
    let aim_y = enemy_cy + evy * pred_time * 0.8;

    // Calculate aim angle to target
    let desired_aim = (aim_y - player_cy).atan2(aim_x - player_cx);

    // Check aim alignment — how close current aim is to desired aim
    let aim_diff = (desired_aim - prev_aim).abs();
    let aim_diff = if aim_diff > std::f32::consts::PI {
        std::f32::consts::TAU - aim_diff
    } else {
        aim_diff
    };

    // Wider aim tolerance when swarmed — fire more freely while retreating
    let aim_tolerance = if is_swarmed { 0.35 } else { 0.15 };
    let aim_aligned = aim_diff < aim_tolerance;

    // ── Tactical Movement ──
    let danger_range = 250.0;
    let max_range = 600.0;

    let too_close = nearest_dist < danger_range;
    let in_optimal = nearest_dist >= danger_range && nearest_dist <= max_range;
    let too_far = nearest_dist > max_range;

    let (mut move_left, mut move_right) = (false, false);
    let should_jump;

    if is_swarmed {
        // ── Swarm Retreat Mode ──
        // Compute retreat direction: flee from the centroid of nearby enemies.
        // If the centroid is roughly at the player, pick a fallback direction
        // toward the nearest level edge (more defensible position).
        let centroid_dx = (sum_x / nearby_count as f32) - player_cx;
        let centroid_dy = (sum_y / nearby_count as f32) - player_cy;
        let centroid_dist = (centroid_dx * centroid_dx + centroid_dy * centroid_dy).sqrt();

        let retreat_dir_x = if centroid_dist > 10.0 {
            // Flee from the enemy mass
            -centroid_dx.signum()
        } else {
            // Enemies surround us evenly — retreat toward nearest level edge
            let dist_to_left = px;
            let dist_to_right = LEVEL_W - px;
            if dist_to_left < dist_to_right { -1.0 } else { 1.0 }
        };

        // Move hard in the retreat direction, ignoring range checks
        move_left = retreat_dir_x < 0.0;
        move_right = retreat_dir_x > 0.0;

        // Aggressive jumping to escape the swarm:
        // jump if grounded and any enemy is close, or if an enemy is above,
        // or randomly to be unpredictable. Always jump at level edges when
        // enemies are close — try to leap over them.
        let enemy_above = enemy_cy < player_cy - 30.0;
        let close_threat = nearest_dist < swarm_radius * 0.6;
        let at_edge = px < 80.0 || px > LEVEL_W - 80.0;
        should_jump = on_ground
            && (close_threat || enemy_above || (at_edge && nearest_dist < 300.0) || rand() < 0.08);
    } else {
        // ── Normal Tactical Movement ──
        if too_close {
            // Full retreat — move away from nearest enemy
            move_left = dx > 0.0;
            move_right = dx < 0.0;
        } else if in_optimal {
            // At good range — strafe to dodge, but keep facing enemy
            if rand() < 0.4 {
                move_left = rand() < 0.5;
                move_right = !move_left;
            }
        } else if too_far {
            // Slowly approach to get into optimal range
            move_left = dx < -30.0;
            move_right = dx > 30.0;
        }

        // Jump: to dodge close enemies or when blocked
        let enemy_above = enemy_cy < player_cy - 40.0;
        should_jump = on_ground && (too_close || enemy_above || rand() < 0.01);
    }

    // ── Shooting: fire when aim is aligned (or spray when swarmed and close) ──
    let can_shoot = shoot_timer <= 0.0 && ammo > 0 && !reloading;
    let in_range = nearest_dist < max_range;
    let should_shoot = if is_swarmed {
        // When swarmed, fire at any target in range — survival overrides precision
        can_shoot && in_range && (aim_aligned || nearest_dist < danger_range)
    } else {
        can_shoot && in_range && aim_aligned
    };

    // Reload when ammo is low
    let should_reload = ammo <= 8 && !reloading && ammo > 0;

    // Write synthetic input
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.left = move_left;
    input.right = move_right;
    input.jump_pressed = should_jump;
    input.slide_down = false;
    input.shoot_down = should_shoot;
    input.mouse_shoot = false;
    input.reload_pressed = should_reload;
}

// ── Enemy Spawn ──────────────────────────────────────────────────────────────

pub fn enemy_spawn_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    let is_eve = world.get_resource::<GameModeRes>()
        .map(|g| g.mode == GameMode::EVE)
        .unwrap_or(false);

    // Update difficulty
    {
        let diff = world.get_resource_mut::<DifficultyRes>().unwrap();
        diff.elapsed_time += dt;
        // In EVE mode, ramp up difficulty much faster
        let time_div = if is_eve { 5.0 } else { 15.0 };
        diff.level = (diff.elapsed_time / time_div).min(10.0) as i32;
        diff.accuracy_mult = (0.6 + diff.level as f32 * 0.09).min(1.5);
        diff.reaction_mult = (1.2 - diff.level as f32 * 0.07).max(0.5);
    }

    {
        let spawn = world.get_resource_mut::<SpawnRes>().unwrap();
        spawn.difficulty_timer += dt;
        if spawn.difficulty_timer > 10.0 {
            spawn.difficulty_timer = 0.0;
            spawn.spawn_interval = (spawn.spawn_interval * 0.88).max(0.4);
        }
        // Override spawn interval in EVE mode
        if is_eve && spawn.spawn_interval > EVE_SPAWN_INTERVAL {
            spawn.spawn_interval = EVE_SPAWN_INTERVAL;
        }
        spawn.spawn_timer -= dt;
    }

    let should_spawn = world.get_resource::<SpawnRes>().unwrap().spawn_timer <= 0.0;
    if !should_spawn { return; }

    // Always reset the spawn timer when it triggers
    {
        let spawn = world.get_resource_mut::<SpawnRes>().unwrap();
        spawn.spawn_timer = spawn.spawn_interval;
    }

    let max_enemies = if is_eve { EVE_MAX_ENEMIES } else { MAX_ENEMIES };
    // In EVE mode, spawn multiple enemies per tick to fill the battlefield faster
    let spawn_batch = if is_eve { 2 } else { 1 };

    for _ in 0..spawn_batch {
    let enemy_count = QuerySingle::<Enemy>::new(world)
        .map(|q| q.len())
        .unwrap_or(0);
    if enemy_count >= max_enemies { break; }
    {
        let _camera_x = world.get_resource::<CameraRes>().unwrap().camera_x;
        let diff_level = world.get_resource::<DifficultyRes>().unwrap().level;

        // Select enemy type based on difficulty
        let (enemy_type, hp, speed_min, speed_max, shoot_min, shoot_max) = {
            let roll = rand();
            if is_eve {
                // EVE mode: all enemy types from the start
                if roll < 0.25 {
                    (EnemyType::Scout, SCOUT_HP, SCOUT_SPEED_MIN, SCOUT_SPEED_MAX, 0.1, 0.4)
                } else if roll < 0.45 {
                    (EnemyType::Flanker, FLANKER_HP, FLANKER_SPEED_MIN, FLANKER_SPEED_MAX, 0.1, 0.3)
                } else if roll < 0.6 {
                    (EnemyType::Tank, TANK_HP, TANK_SPEED_MIN, TANK_SPEED_MAX, 0.05, 0.2)
                } else {
                    (EnemyType::Grunt, GRUNT_HP, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX, 0.1, 0.3)
                }
            } else if diff_level < 3 {
                // Early game: only grunts
                (EnemyType::Grunt, GRUNT_HP, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX, 0.1, 0.3)
            } else if diff_level < 5 {
                // Mid game: grunts and scouts
                if roll < 0.3 {
                    (EnemyType::Scout, SCOUT_HP, SCOUT_SPEED_MIN, SCOUT_SPEED_MAX, 0.1, 0.4)
                } else {
                    (EnemyType::Grunt, GRUNT_HP, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX, 0.1, 0.3)
                }
            } else if diff_level < 7 {
                // Late mid: grunts, scouts, flankers
                if roll < 0.25 {
                    (EnemyType::Scout, SCOUT_HP, SCOUT_SPEED_MIN, SCOUT_SPEED_MAX, 0.1, 0.4)
                } else if roll < 0.5 {
                    (EnemyType::Flanker, FLANKER_HP, FLANKER_SPEED_MIN, FLANKER_SPEED_MAX, 0.1, 0.3)
                } else {
                    (EnemyType::Grunt, GRUNT_HP, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX, 0.1, 0.3)
                }
            } else {
                // Hard: all types
                if roll < 0.2 {
                    (EnemyType::Scout, SCOUT_HP, SCOUT_SPEED_MIN, SCOUT_SPEED_MAX, 0.1, 0.4)
                } else if roll < 0.4 {
                    (EnemyType::Flanker, FLANKER_HP, FLANKER_SPEED_MIN, FLANKER_SPEED_MAX, 0.1, 0.3)
                } else if roll < 0.55 {
                    (EnemyType::Tank, TANK_HP, TANK_SPEED_MIN, TANK_SPEED_MAX, 0.05, 0.2)
                } else {
                    (EnemyType::Grunt, GRUNT_HP, GRUNT_SPEED_MIN, GRUNT_SPEED_MAX, 0.1, 0.3)
                }
            }
        };

        let size = match enemy_type {
            EnemyType::Tank => rand_range(36.0, 44.0),
            EnemyType::Scout => rand_range(20.0, 26.0),
            _ => rand_range(24.0, 34.0),
        };

        // Spawn direction — more varied in EVE mode
        let (spawn_x, spawn_y, initial_vx, initial_vy) = if is_eve {
            let roll = rand();
            if roll < 0.20 {
                // Drop from above — various positions and angles
                let sx = rand_range(40.0, LEVEL_W - 40.0);
                let vx = rand_range(-80.0, 80.0);
                let vy = rand_range(50.0, 120.0);
                (sx, -size - rand_range(30.0, 150.0), vx, vy)
            } else if roll < 0.35 {
                // High arc from left — lobbed in with parabolic trajectory
                let sx = rand_range(-size - 40.0, 100.0);
                (sx, -size - rand_range(60.0, 200.0),
                 rand_range(speed_min * 0.8, speed_max * 1.2), rand_range(30.0, 80.0))
            } else if roll < 0.50 {
                // High arc from right — lobbed in with parabolic trajectory
                let sx = rand_range(LEVEL_W - 100.0, LEVEL_W + size + 40.0);
                (sx, -size - rand_range(60.0, 200.0),
                 -rand_range(speed_min * 0.8, speed_max * 1.2), rand_range(30.0, 80.0))
            } else if roll < 0.65 {
                // Ground rush from left
                (-size - rand_range(10.0, 60.0), GROUND_Y - size,
                 rand_range(speed_min, speed_max * 1.3), 0.0)
            } else if roll < 0.80 {
                // Ground rush from right
                (LEVEL_W + rand_range(10.0, 60.0), GROUND_Y - size,
                 -rand_range(speed_min, speed_max * 1.3), 0.0)
            } else if roll < 0.90 {
                // Airdrop at center — parachuting in
                let sx = rand_range(LEVEL_W * 0.3, LEVEL_W * 0.7);
                (sx, -size - rand_range(100.0, 300.0),
                 rand_range(-40.0, 40.0), rand_range(60.0, 100.0))
            } else {
                // Side dive — from top-corner with diagonal momentum
                let from_right = rand() > 0.5;
                let sx = if from_right { LEVEL_W + rand_range(10.0, 30.0) } else { -size - rand_range(10.0, 30.0) };
                let vx = if from_right { -rand_range(speed_min, speed_max) } else { rand_range(speed_min, speed_max) };
                (sx, rand_range(20.0, 150.0), vx, rand_range(40.0, 80.0))
            }
        } else {
            // Normal PVE spawning
            let drop_in = rand() < 0.15;
            if drop_in {
                let sx = rand_range(40.0, LEVEL_W - 40.0);
                (sx, -size - rand_range(20.0, 80.0), rand_range(-30.0, 30.0), rand_range(40.0, 80.0))
            } else {
                let from_right = rand() > 0.5;
                if from_right {
                    (LEVEL_W + rand_range(10.0, 40.0), GROUND_Y - size,
                     -rand_range(speed_min, speed_max), 0.0)
                } else {
                    (-size - rand_range(10.0, 40.0), GROUND_Y - size,
                     rand_range(speed_min, speed_max), 0.0)
                }
            }
        };

        // Set ammo based on enemy type
        let (max_ammo, reload_time) = match enemy_type {
            EnemyType::Tank => (TANK_MAX_AMMO, TANK_RELOAD_TIME),
            _ => (ENEMY_MAX_AMMO, ENEMY_RELOAD_TIME),
        };

        // Enemy starts on ground if spawned near ground level
        let starts_grounded = spawn_y >= GROUND_Y - size - 5.0;

        world.spawn()
            .with(Enemy {
                hp,
                max_hp: hp,
                alive: true,
                on_ground: starts_grounded,
                shoot_timer: rand_range(shoot_min, shoot_max),
                ai_timer: rand_range(0.3, 0.8),
                flash: 0.0,
                size,
                enemy_type,
                ai_state: AIState::Chase,
                strafe_dir: if rand() > 0.5 { 1.0 } else { -1.0 },
                burst_count: 0,
                dodge_timer: 0.0,
                state_timer: 0.0,
                ammo: max_ammo,
                max_ammo,
                reloading: false,
                reload_timer: reload_time,
                climb_cooldown: 0.0,
            })
            .with(Transform2D::new(Vec2::new(spawn_x, spawn_y)))
            .with(Velocity { x: initial_vx, y: initial_vy, gravity_scale: 1.0 })
            .build();
    }
    } // end spawn batch loop
}

// ── Enemy AI ─────────────────────────────────────────────────────────────────

pub fn enemy_ai_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    // Get player info
    let player_info = QuerySingle::<Player>::new(world)
        .and_then(|q| q.iter().next().map(|(e, _)| e))
        .and_then(|e| {
            let pos = world.get_component::<Transform2D>(e).map(|t| (t.position.x, t.position.y));
            let vel = world.get_component::<Velocity>(e).map(|v| (v.x, v.y));
            pos.zip(vel).map(|((px, py), (vx, vy))| (px, py, vx, vy))
        });

    let (player_x, player_y, player_vx, player_vy) = match player_info {
        Some(info) => info,
        None => return,
    };

    // Get difficulty settings
    let (accuracy_mult, reaction_mult) = {
        let diff = world.get_resource::<DifficultyRes>().unwrap();
        (diff.accuracy_mult, diff.reaction_mult)
    };

    struct ShootEvent { x: f32, y: f32, vx: f32, vy: f32 }
    let mut shoot_events: Vec<ShootEvent> = Vec::new();

    // Collect player bullet positions for dodge detection
    let player_bullets: Vec<(f32, f32, f32, f32)> = QuerySingle::<Bullet>::new(world)
        .map(|q| q.iter()
            .filter(|(_, b)| b.alive && b.is_player)
            .map(|(_, b)| (b.x, b.y, b.vx, b.vy))
            .collect())
        .unwrap_or_default();

    let entities: Vec<_> = QuerySingle::<Enemy>::new(world)
        .map(|q| q.iter().filter(|(_, e)| e.alive).map(|(e, _)| e).collect())
        .unwrap_or_default();

    for entity in entities {
        let (mut ai_timer, shoot_timer, size, on_ground, flash, enemy_type, mut ai_state,
             mut strafe_dir, mut burst_count, mut dodge_timer, mut state_timer, hp, max_hp,
             ammo, reloading, reload_timer, mut climb_cooldown) = {
            match world.get_component::<Enemy>(entity) {
                Some(e) => (e.ai_timer, e.shoot_timer, e.size, e.on_ground, e.flash,
                           e.enemy_type, e.ai_state, e.strafe_dir, e.burst_count,
                           e.dodge_timer, e.state_timer, e.hp, e.max_hp,
                           e.ammo, e.reloading, e.reload_timer, e.climb_cooldown),
                None => continue,
            }
        };

        let (ex, ey) = {
            match world.get_component::<Transform2D>(entity) {
                Some(t) => (t.position.x, t.position.y),
                None => continue,
            }
        };

        let dx = player_x - ex;
        let dy = player_y - ey;
        let dist = (dx * dx + dy * dy).sqrt();

        // Update timers
        ai_timer -= dt;
        dodge_timer = (dodge_timer - dt).max(0.0);
        climb_cooldown = (climb_cooldown - dt).max(0.0);
        state_timer += dt;

        // ── Bullet Dodging (Scout, Flanker, Grunt with low chance) ──
        if dodge_timer <= 0.0 {
            let dodge_chance = match enemy_type {
                EnemyType::Scout => 0.4,
                EnemyType::Flanker => 0.25,
                EnemyType::Grunt => 0.1,
                EnemyType::Tank => 0.0,
            };

            for (bx, by, bvx, bvy) in &player_bullets {
                let bullet_dx = bx - ex - size * 0.5;
                let bullet_dy = by - ey - size * 0.5;
                let bullet_dist = (bullet_dx * bullet_dx + bullet_dy * bullet_dy).sqrt();

                // Check if bullet is heading toward this enemy
                let dot = bullet_dx * bvx + bullet_dy * bvy;
                if bullet_dist < DODGE_RANGE && dot < 0.0 && on_ground && rand() < dodge_chance {
                    // Dodge by jumping
                    if let Some(v) = world.get_component_mut::<Velocity>(entity) {
                        v.y = -crate::JUMP_FORCE * 0.8;
                    }
                    dodge_timer = DODGE_COOLDOWN;
                    break;
                }
            }
        }

        // ── State Machine ──
        let hp_ratio = hp as f32 / max_hp as f32;

        // State transitions based on enemy type
        match enemy_type {
            EnemyType::Scout => {
                match ai_state {
                    AIState::Chase => {
                        if dist < 200.0 {
                            ai_state = AIState::Attack;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Attack => {
                        if state_timer > 0.5 || dist > 250.0 {
                            ai_state = AIState::Flee;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Flee => {
                        if state_timer > 1.5 || dist > 300.0 {
                            ai_state = AIState::Chase;
                            state_timer = 0.0;
                        }
                    }
                    _ => { ai_state = AIState::Chase; }
                }
            }
            EnemyType::Grunt => {
                match ai_state {
                    AIState::Chase => {
                        if dist < 180.0 {
                            ai_state = AIState::Attack;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Attack => {
                        if dist > 250.0 {
                            ai_state = AIState::Chase;
                            state_timer = 0.0;
                        } else if hp_ratio < 0.5 {
                            ai_state = AIState::Flee;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Flee => {
                        if state_timer > 2.0 || dist > 350.0 {
                            ai_state = AIState::Chase;
                            state_timer = 0.0;
                        }
                    }
                    _ => { ai_state = AIState::Chase; }
                }
            }
            EnemyType::Tank => {
                // Tanks never flee, always chase or attack
                match ai_state {
                    AIState::Chase => {
                        if dist < 200.0 {
                            ai_state = AIState::Attack;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Attack => {
                        if dist > 300.0 {
                            ai_state = AIState::Chase;
                            state_timer = 0.0;
                        }
                    }
                    _ => { ai_state = AIState::Chase; }
                }
            }
            EnemyType::Flanker => {
                match ai_state {
                    AIState::Chase => {
                        if dist < 150.0 {
                            ai_state = AIState::Flank;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Flank => {
                        // Check if we're behind the player
                        let behind_player = (dx > 0.0 && player_vx > 0.0) || (dx < 0.0 && player_vx < 0.0);
                        if behind_player || state_timer > 2.0 {
                            ai_state = AIState::Attack;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Attack => {
                        if state_timer > 1.0 {
                            ai_state = AIState::Flee;
                            state_timer = 0.0;
                        }
                    }
                    AIState::Flee => {
                        if state_timer > 1.5 || dist > 300.0 {
                            ai_state = AIState::Chase;
                            state_timer = 0.0;
                        }
                    }
                }
            }
        }

        // ── Movement Based on State ──
        if ai_timer <= 0.0 {
            ai_timer = rand_range(0.3, 0.8) * reaction_mult;

            let (target_vx, should_jump) = match (enemy_type, ai_state) {
                // Scout: fast approach, quick retreat
                (EnemyType::Scout, AIState::Chase) => {
                    let speed = rand_range(SCOUT_SPEED_MIN, SCOUT_SPEED_MAX);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }
                (EnemyType::Scout, AIState::Attack) => {
                    // Strafe while attacking
                    strafe_dir *= -1.0;
                    (strafe_dir * rand_range(80.0, 120.0), false)
                }
                (EnemyType::Scout, AIState::Flee) => {
                    let speed = rand_range(SCOUT_SPEED_MIN, SCOUT_SPEED_MAX) * 1.2;
                    (if dx > 0.0 { -speed } else { speed }, false)
                }

                // Grunt: steady approach, strafe when attacking
                (EnemyType::Grunt, AIState::Chase) => {
                    let speed = rand_range(GRUNT_SPEED_MIN, GRUNT_SPEED_MAX);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }
                (EnemyType::Grunt, AIState::Attack) => {
                    // Strafe with occasional direction changes
                    if rand() < 0.3 { strafe_dir *= -1.0; }
                    (strafe_dir * rand_range(60.0, 100.0), player_y < ey - 50.0 && rand() < 0.2)
                }
                (EnemyType::Grunt, AIState::Flee) => {
                    let speed = rand_range(GRUNT_SPEED_MIN, GRUNT_SPEED_MAX);
                    (if dx > 0.0 { -speed } else { speed }, false)
                }

                // Tank: slow, relentless advance
                (EnemyType::Tank, AIState::Chase) => {
                    let speed = rand_range(TANK_SPEED_MIN, TANK_SPEED_MAX);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }
                (EnemyType::Tank, AIState::Attack) => {
                    // Keep advancing slowly
                    let speed = rand_range(TANK_SPEED_MIN * 0.5, TANK_SPEED_MAX * 0.5);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }

                // Flanker: try to get behind player
                (EnemyType::Flanker, AIState::Chase) => {
                    let speed = rand_range(FLANKER_SPEED_MIN, FLANKER_SPEED_MAX);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }
                (EnemyType::Flanker, AIState::Flank) => {
                    // Move past the player to get behind
                    let speed = rand_range(FLANKER_SPEED_MIN, FLANKER_SPEED_MAX) * 1.3;
                    let dir = if dx > 0.0 { 1.0 } else { -1.0 };
                    (dir * speed, player_y < ey - 40.0 && rand() < 0.5)
                }
                (EnemyType::Flanker, AIState::Attack) => {
                    // Quick retreat after attacking
                    let speed = rand_range(FLANKER_SPEED_MIN, FLANKER_SPEED_MAX);
                    (if dx > 0.0 { -speed } else { speed }, false)
                }
                (EnemyType::Flanker, AIState::Flee) => {
                    let speed = rand_range(FLANKER_SPEED_MIN, FLANKER_SPEED_MAX);
                    (if dx > 0.0 { -speed } else { speed }, false)
                }

                // Default fallback
                _ => {
                    let speed = rand_range(70.0, 140.0);
                    (if dx > 0.0 { speed } else { -speed }, false)
                }
            };

            if let Some(v) = world.get_component_mut::<Velocity>(entity) {
                v.x = target_vx;
                if should_jump && on_ground {
                    v.y = -crate::JUMP_FORCE;
                }
            }
        }

        // ── Reload Logic ──
        let mut new_reloading = reloading;
        let mut new_reload_timer = reload_timer - dt;
        let mut new_ammo = ammo;

        // Auto-reload when ammo is empty
        if new_ammo <= 0 && !new_reloading {
            new_reloading = true;
            new_reload_timer = match enemy_type {
                EnemyType::Tank => TANK_RELOAD_TIME,
                _ => ENEMY_RELOAD_TIME,
            };
        }

        // Complete reload
        if new_reloading && new_reload_timer <= 0.0 {
            new_reloading = false;
            new_ammo = match enemy_type {
                EnemyType::Tank => TANK_MAX_AMMO,
                _ => ENEMY_MAX_AMMO,
            };
        }

        // ── Shooting ──
        let mut new_shoot_timer = shoot_timer - dt;

        // Determine if enemy should shoot based on state — all types fire aggressively
        let should_shoot = match enemy_type {
            EnemyType::Tank => dist < 600.0,
            _ => dist < 450.0,
        };

        // Can only shoot if not reloading and has ammo
        if new_shoot_timer <= 0.0 && should_shoot && !new_reloading && new_ammo > 0 {
            // Set shoot timer based on enemy type
            new_shoot_timer = match enemy_type {
                EnemyType::Scout => rand_range(0.3, 0.7),
                EnemyType::Grunt => rand_range(0.25, 0.6),
                EnemyType::Tank => rand_range(0.15, 0.4),
                EnemyType::Flanker => rand_range(0.3, 0.65),
            };

            // Predictive aiming
            let pred_time = dist / ENEMY_BULLET_SPEED;
            let pred_x = player_x + PLAYER_SIZE * 0.5 + player_vx * pred_time * accuracy_mult;
            let pred_y = player_y + PLAYER_SIZE * 0.5 + player_vy * pred_time * accuracy_mult;

            let aim_dx = pred_x - (ex + size * 0.5);
            let aim_dy = pred_y - (ey + size * 0.5);
            let aim_dist = (aim_dx * aim_dx + aim_dy * aim_dy).sqrt();

            let (bvx, bvy) = if aim_dist > 1.0 {
                (aim_dx / aim_dist * ENEMY_BULLET_SPEED, aim_dy / aim_dist * ENEMY_BULLET_SPEED)
            } else { (-ENEMY_BULLET_SPEED, 0.0) };

            // Tank burst fire
            if enemy_type == EnemyType::Tank && burst_count > 0 {
                burst_count -= 1;
                new_shoot_timer = 0.08; // Rapid follow-up shots
            } else if enemy_type == EnemyType::Tank && rand() < 0.75 {
                burst_count = 4; // Start a burst
                new_shoot_timer = 0.05;
            }

            new_ammo -= 1;

            shoot_events.push(ShootEvent {
                x: ex + size * 0.5,
                y: ey + size * 0.5,
                vx: bvx,
                vy: bvy
            });
        }

        // Update enemy component
        if let Some(e) = world.get_component_mut::<Enemy>(entity) {
            e.ai_timer = ai_timer;
            e.shoot_timer = new_shoot_timer;
            e.flash = (flash - dt * 5.0).max(0.0);
            e.ai_state = ai_state;
            e.strafe_dir = strafe_dir;
            e.burst_count = burst_count;
            e.dodge_timer = dodge_timer;
            e.state_timer = state_timer;
            e.climb_cooldown = climb_cooldown;
            e.ammo = new_ammo;
            e.reloading = new_reloading;
            e.reload_timer = new_reload_timer;
        }

        // Cleanup off-screen enemies
        let cam_x = world.get_resource::<CameraRes>().unwrap().camera_x;
        if ex < cam_x - 200.0 || ex > cam_x + WORLD_W + 200.0 {
            if let Some(e) = world.get_component_mut::<Enemy>(entity) {
                e.alive = false;
            }
        }

        // Clamp to level bounds
        if let Some(t) = world.get_component_mut::<Transform2D>(entity) {
            t.position.x = t.position.x.clamp(0.0, LEVEL_W - size);
        }
    }

    // Spawn enemy bullets
    let bullet_count = QuerySingle::<Bullet>::new(world)
        .map(|q| q.iter().filter(|(_, b)| !b.is_player).count())
        .unwrap_or(0);

    let mut spawned = 0;
    for ev in shoot_events {
        if bullet_count + spawned >= MAX_BULLETS { break; }
        world.spawn()
            .with(Bullet { x: ev.x, y: ev.y, vx: ev.vx, vy: ev.vy, alive: true, is_player: false })
            .build();
        spawned += 1;
        crate::play_sound_js("enemy_shoot");
    }
}

// ── Bullet Movement ──────────────────────────────────────────────────────────

pub fn bullet_move_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    // Collect obstacle data for bullet collision
    let obstacles: Vec<(f32, f32, f32, f32)> = world.get_resource::<MapRes>()
        .map(|m| m.obstacles.iter().map(|o| (o.x, o.y, o.w, o.h)).collect())
        .unwrap_or_default();

    let entities: Vec<_> = QuerySingle::<Bullet>::new(world)
        .map(|q| q.iter().map(|(e, _)| e).collect())
        .unwrap_or_default();

    // Collect spark events to avoid borrow conflicts
    struct SparkEvent { x: f32, y: f32, vx: f32, vy: f32, floor_hit: bool }
    let mut sparks: Vec<SparkEvent> = Vec::new();

    for entity in entities {
        let (alive, vx, vy, is_player) = {
            match world.get_component::<Bullet>(entity) {
                Some(b) => (b.alive, b.vx, b.vy, b.is_player),
                None => continue,
            }
        };
        if !alive { continue; }

        let (new_x, new_y) = {
            match world.get_component::<Bullet>(entity) {
                Some(b) => (b.x + vx * dt, b.y + vy * dt),
                None => continue,
            }
        };

        if let Some(b) = world.get_component_mut::<Bullet>(entity) {
            b.x = new_x;
            b.y = new_y;

            // Floor hit
            if new_y >= GROUND_Y {
                b.alive = false;
                sparks.push(SparkEvent { x: new_x, y: GROUND_Y, vx, vy, floor_hit: true });
            }
            // Out of bounds (no sparks for off-screen deaths)
            else if new_x < -10.0 || new_x > LEVEL_W + 10.0 || new_y < -10.0 {
                b.alive = false;
            }

            // Obstacle hit
            if b.alive {
                for &(ox, oy, ow, oh) in &obstacles {
                    if new_x >= ox && new_x <= ox + ow && new_y >= oy && new_y <= oy + oh {
                        b.alive = false;
                        sparks.push(SparkEvent { x: new_x, y: new_y, vx, vy, floor_hit: false });
                        break;
                    }
                }
            }
        }
    }

    // Spawn spark particles for each impact
    for spark in sparks {
        spawn_spark_particles(world, spark.x, spark.y, spark.vx, spark.vy, spark.floor_hit);
    }
}

// ── Particle Update ──────────────────────────────────────────────────────────

pub fn particle_update_system(world: &mut World, dt: f32) {
    let entities: Vec<_> = QuerySingle::<Particle>::new(world)
        .map(|q| q.iter().map(|(e, _)| e).collect())
        .unwrap_or_default();

    for entity in entities {
        if let Some(p) = world.get_component_mut::<Particle>(entity) {
            if p.life <= 0.0 { continue; }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= 0.98;
            p.vy *= 0.98;
            p.life -= dt;
        }
    }
}

// ── Camera ───────────────────────────────────────────────────────────────────

pub fn camera_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing && gs.state != GameState::GameOver { return; }

    let target_x = QuerySingle::<Player>::new(world)
        .and_then(|q| q.iter().next().map(|(e, _)| e))
        .and_then(|e| world.get_component::<Transform2D>(e))
        .map(|t| t.position.x + PLAYER_SIZE * 0.5);

    let target_x = match target_x {
        Some(x) => x,
        None => return,
    };

    // Calculate visible width based on viewport aspect ratio
    let canvas_aspect = world.get_resource::<ViewportRes>()
        .map(|v| v.canvas_w / v.canvas_h)
        .unwrap_or(WORLD_W / WORLD_H);
    let visible_w = WORLD_H * canvas_aspect;

    let cam = world.get_resource_mut::<CameraRes>().unwrap();

    if cam.shake_amount > 0.0 {
        cam.shake_amount = (cam.shake_amount - 6.0 * dt).max(0.0);
    }

    let cam_center = cam.camera_x + WORLD_W * 0.5;
    let diff = target_x - cam_center;

    let desired_x = if diff > CAM_DEAD_ZONE_X {
        target_x - CAM_DEAD_ZONE_X - WORLD_W * 0.5
    } else if diff < -CAM_DEAD_ZONE_X {
        target_x + CAM_DEAD_ZONE_X - WORLD_W * 0.5
    } else {
        cam.camera_x
    };

    cam.camera_x += (desired_x - cam.camera_x) * CAM_SMOOTH * dt;

    // Clamp camera so viewport never extends beyond level boundaries
    let half_visible = visible_w * 0.5;
    let half_world = WORLD_W * 0.5;
    let min_cam = half_visible - half_world;
    let max_cam = LEVEL_W - half_visible - half_world;
    cam.camera_x = cam.camera_x.clamp(min_cam.max(0.0), max_cam.min(LEVEL_W - WORLD_W));
    cam.camera_y = 0.0;
}

// ── Cleanup ──────────────────────────────────────────────────────────────────

pub fn cleanup_system(world: &mut World) {
    let mut to_despawn: Vec<opengame_engine::ecs::Entity> = Vec::new();

    if let Some(q) = QuerySingle::<Bullet>::new(world) {
        for (e, b) in q.iter() { if !b.alive { to_despawn.push(e); } }
    }
    if let Some(q) = QuerySingle::<Enemy>::new(world) {
        for (e, en) in q.iter() { if !en.alive { to_despawn.push(e); } }
    }
    if let Some(q) = QuerySingle::<Particle>::new(world) {
        for (e, p) in q.iter() { if p.life <= 0.0 { to_despawn.push(e); } }
    }

    for e in to_despawn {
        world.despawn(e);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Spawn directional spark particles for bullet impacts on floors and obstacles.
fn spawn_spark_particles(world: &mut World, x: f32, y: f32, vx: f32, vy: f32, floor_hit: bool) {
    let current = QuerySingle::<Particle>::new(world)
        .map(|q| q.len())
        .unwrap_or(0);

    let count: usize = if floor_hit { 6 } else { 5 };

    for i in 0..count {
        if current + i >= MAX_PARTICLES { break; }

        // Base direction: reflect off the surface
        let (base_angle, speed_min, speed_max) = if floor_hit {
            // Floor: mostly upward with spread
            (std::f32::consts::PI * 1.5 + rand_range(-0.8, 0.8), 80.0, 200.0)
        } else {
            // Obstacle: reflect roughly opposite to bullet direction
            let incoming = vy.atan2(vx) + std::f32::consts::PI;
            (incoming + rand_range(-0.6, 0.6), 60.0, 160.0)
        };

        let speed = rand_range(speed_min, speed_max);
        let life = rand_range(0.15, 0.35);

        // Spark colors: warm tones (yellow-orange-white)
        let color_idx = if floor_hit { 0 } else { (i % 3) as u8 };

        world.spawn()
            .with(Particle {
                x, y,
                vx: base_angle.cos() * speed,
                vy: base_angle.sin() * speed,
                life,
                max_life: life,
                size: rand_range(1.5, 3.5),
                color_idx,
            })
            .build();
    }
}

fn spawn_explosion_particles(world: &mut World, x: f32, y: f32, count: usize, power: f32) {
    let current = QuerySingle::<Particle>::new(world)
        .map(|q| q.len())
        .unwrap_or(0);

    for i in 0..count {
        if current + i >= MAX_PARTICLES { break; }
        let angle = rand() * std::f32::consts::TAU;
        let speed = rand_range(60.0, power);
        let life = rand_range(0.3, 0.9);
        world.spawn()
            .with(Particle {
                x, y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                life,
                max_life: life,
                size: rand_range(3.0, 7.0),
                color_idx: (i % 7) as u8,
            })
            .build();
    }
}

// ── Kill Feed Update ────────────────────────────────────────────────────────

pub fn kill_feed_system(world: &mut World, dt: f32) {
    if let Some(kill_feed) = world.get_resource_mut::<KillFeedRes>() {
        // Update timers
        for entry in &mut kill_feed.entries {
            entry.timer -= dt;
        }

        // Remove expired entries
        kill_feed.entries.retain(|entry| entry.timer > 0.0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ── Tests ─────────────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use opengame_engine::ecs::World;
    use opengame_engine::math::Vec2;
    use opengame_engine::transform::Transform2D;

    // ── Helper: create a World with all required resources ──────────────────

    fn setup_world() -> World {
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

    fn spawn_player(world: &mut World, x: f32, y: f32) -> opengame_engine::ecs::Entity {
        world.spawn()
            .with(Player {
                facing_right: true,
                invincible: 0.0,
                flash: 0.0,
                shoot_timer: 0.0,
                on_ground: false,
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
            })
            .with(Transform2D::new(Vec2::new(x, y)))
            .with(Velocity { x: 0.0, y: 0.0, gravity_scale: 1.0 })
            .build()
    }

    fn spawn_enemy(world: &mut World, x: f32, y: f32, enemy_type: EnemyType) -> opengame_engine::ecs::Entity {
        let (hp, size) = match enemy_type {
            EnemyType::Scout => (1, 24.0),
            EnemyType::Grunt => (2, 30.0),
            EnemyType::Tank => (4, 40.0),
            EnemyType::Flanker => (2, 28.0),
        };
        world.spawn()
            .with(Enemy {
                hp, max_hp: hp, alive: true, on_ground: false,
                shoot_timer: 1.0, ai_timer: 0.5, flash: 0.0, size,
                enemy_type, ai_state: AIState::Chase, strafe_dir: 1.0,
                burst_count: 0, dodge_timer: 0.0, state_timer: 0.0,
                ammo: 10, max_ammo: 10, reloading: false, reload_timer: 2.0,
                climb_cooldown: 0.0,
            })
            .with(Transform2D::new(Vec2::new(x, y)))
            .with(Velocity { x: 0.0, y: 0.0, gravity_scale: 1.0 })
            .build()
    }

    fn spawn_bullet(world: &mut World, x: f32, y: f32, vx: f32, vy: f32, is_player: bool) -> opengame_engine::ecs::Entity {
        world.spawn()
            .with(Bullet { x, y, vx, vy, alive: true, is_player })
            .build()
    }

    fn spawn_particle(world: &mut World, x: f32, y: f32, life: f32) -> opengame_engine::ecs::Entity {
        world.spawn()
            .with(Particle { x, y, vx: 0.0, vy: 0.0, life, max_life: life, size: 4.0, color_idx: 0 })
            .build()
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── AABB Overlap Tests ──────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn aabb_overlap_identical_rects() {
        assert!(aabb_overlap(100.0, 100.0, 32.0, 32.0, 100.0, 100.0, 32.0, 32.0));
    }

    #[test]
    fn aabb_overlap_partial_horizontal() {
        // Two 32x32 rects centered at (100,100) and (120,100) — overlap by 12px
        assert!(aabb_overlap(100.0, 100.0, 32.0, 32.0, 120.0, 100.0, 32.0, 32.0));
    }

    #[test]
    fn aabb_overlap_partial_vertical() {
        assert!(aabb_overlap(100.0, 100.0, 32.0, 32.0, 100.0, 120.0, 32.0, 32.0));
    }

    #[test]
    fn aabb_overlap_touching_edges_no_overlap() {
        // Two 32x32 rects: center at (100,100) and (132,100) — edges touch but don't overlap
        // distance = 32, half-widths sum = 32, condition is < not <=
        assert!(!aabb_overlap(100.0, 100.0, 32.0, 32.0, 132.0, 100.0, 32.0, 32.0));
    }

    #[test]
    fn aabb_overlap_separated_horizontal() {
        assert!(!aabb_overlap(0.0, 0.0, 10.0, 10.0, 100.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn aabb_overlap_separated_vertical() {
        assert!(!aabb_overlap(0.0, 0.0, 10.0, 10.0, 0.0, 100.0, 10.0, 10.0));
    }

    #[test]
    fn aabb_overlap_different_sizes() {
        // Small rect (10x10 at 50,50) inside big rect (100x100 at 50,50)
        assert!(aabb_overlap(50.0, 50.0, 10.0, 10.0, 50.0, 50.0, 100.0, 100.0));
    }

    #[test]
    fn aabb_overlap_corner_touch() {
        // Two rects that only touch at a corner — should not overlap
        // rect1: center (50,50), size 20x20 → extends to (60,60)
        // rect2: center (60,60), size 20x20 → extends from (50,50)
        // distance_x = 10, half_w_sum = 20 → 10 < 20 → true on x
        // distance_y = 10, half_h_sum = 20 → 10 < 20 → true on y
        assert!(aabb_overlap(50.0, 50.0, 20.0, 20.0, 60.0, 60.0, 20.0, 20.0));
    }

    #[test]
    fn aabb_overlap_zero_size() {
        // Zero-size rect should not overlap with anything (distance < 0 is impossible)
        assert!(!aabb_overlap(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn aabb_overlap_negative_coords() {
        assert!(aabb_overlap(-50.0, -50.0, 32.0, 32.0, -50.0, -50.0, 32.0, 32.0));
        assert!(!aabb_overlap(-100.0, -100.0, 10.0, 10.0, 100.0, 100.0, 10.0, 10.0));
    }

    #[test]
    fn aabb_overlap_bullet_vs_enemy() {
        // Simulates a bullet (10x4) hitting an enemy (30x30)
        // Bullet at (115, 100), enemy centered at (130, 100)
        assert!(aabb_overlap(115.0, 100.0, 10.0, 4.0, 130.0, 100.0, 30.0, 30.0));
    }

    #[test]
    fn aabb_overlap_bullet_near_miss() {
        // Bullet passes just above the enemy
        assert!(!aabb_overlap(115.0, 60.0, 10.0, 4.0, 130.0, 100.0, 30.0, 30.0));
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Particle Update System Tests ────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn particle_update_moves_position() {
        let mut world = setup_world();
        let e = spawn_particle(&mut world, 100.0, 200.0, 1.0);

        // Set velocity
        if let Some(p) = world.get_component_mut::<Particle>(e) {
            p.vx = 50.0;
            p.vy = -100.0;
        }

        particle_update_system(&mut world, 0.1);

        let p = world.get_component::<Particle>(e).unwrap();
        assert!((p.x - 105.0).abs() < 0.01, "Expected x≈105.0, got {}", p.x);
        assert!((p.y - 190.0).abs() < 0.01, "Expected y≈190.0, got {}", p.y);
    }

    #[test]
    fn particle_update_decays_velocity() {
        let mut world = setup_world();
        let e = spawn_particle(&mut world, 0.0, 0.0, 1.0);

        if let Some(p) = world.get_component_mut::<Particle>(e) {
            p.vx = 100.0;
            p.vy = 200.0;
        }

        particle_update_system(&mut world, 0.016);

        let p = world.get_component::<Particle>(e).unwrap();
        // Velocity should be multiplied by 0.98 each frame
        assert!(p.vx < 100.0, "vx should decay: {}", p.vx);
        assert!(p.vy < 200.0, "vy should decay: {}", p.vy);
        assert!(p.vx > 90.0, "vx shouldn't decay too much: {}", p.vx);
    }

    #[test]
    fn particle_update_decreases_life() {
        let mut world = setup_world();
        let e = spawn_particle(&mut world, 0.0, 0.0, 1.0);

        particle_update_system(&mut world, 0.1);

        let p = world.get_component::<Particle>(e).unwrap();
        assert!((p.life - 0.9).abs() < 0.01, "Expected life≈0.9, got {}", p.life);
    }

    #[test]
    fn particle_update_skips_dead_particles() {
        let mut world = setup_world();
        let e = spawn_particle(&mut world, 100.0, 100.0, 0.0);

        // Life is already 0, particle should not be updated
        if let Some(p) = world.get_component_mut::<Particle>(e) {
            p.vx = 500.0;
            p.vy = 500.0;
        }

        particle_update_system(&mut world, 0.1);

        let p = world.get_component::<Particle>(e).unwrap();
        assert!((p.x - 100.0).abs() < 0.01, "Dead particle should not move");
        assert!((p.y - 100.0).abs() < 0.01, "Dead particle should not move");
    }

    #[test]
    fn particle_update_multiple_particles() {
        let mut world = setup_world();
        let e1 = spawn_particle(&mut world, 0.0, 0.0, 1.0);
        let e2 = spawn_particle(&mut world, 50.0, 50.0, 0.5);

        if let Some(p) = world.get_component_mut::<Particle>(e1) { p.vx = 10.0; }
        if let Some(p) = world.get_component_mut::<Particle>(e2) { p.vy = -20.0; }

        particle_update_system(&mut world, 0.1);

        let p1 = world.get_component::<Particle>(e1).unwrap();
        let p2 = world.get_component::<Particle>(e2).unwrap();
        assert!(p1.x > 0.0, "Particle 1 should have moved");
        assert!(p2.y < 50.0, "Particle 2 should have moved up");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Cleanup System Tests ────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn cleanup_removes_dead_bullets() {
        let mut world = setup_world();
        let alive_bullet = spawn_bullet(&mut world, 0.0, 0.0, 100.0, 0.0, true);
        let dead_bullet = spawn_bullet(&mut world, 0.0, 0.0, 100.0, 0.0, true);

        if let Some(b) = world.get_component_mut::<Bullet>(dead_bullet) {
            b.alive = false;
        }

        cleanup_system(&mut world);

        assert!(world.get_component::<Bullet>(alive_bullet).is_some(), "Alive bullet should remain");
        assert!(world.get_component::<Bullet>(dead_bullet).is_none(), "Dead bullet should be removed");
    }

    #[test]
    fn cleanup_removes_dead_enemies() {
        let mut world = setup_world();
        let alive_enemy = spawn_enemy(&mut world, 100.0, 500.0, EnemyType::Grunt);
        let dead_enemy = spawn_enemy(&mut world, 200.0, 500.0, EnemyType::Scout);

        if let Some(e) = world.get_component_mut::<Enemy>(dead_enemy) {
            e.alive = false;
        }

        cleanup_system(&mut world);

        assert!(world.get_component::<Enemy>(alive_enemy).is_some(), "Alive enemy should remain");
        assert!(world.get_component::<Enemy>(dead_enemy).is_none(), "Dead enemy should be removed");
    }

    #[test]
    fn cleanup_removes_expired_particles() {
        let mut world = setup_world();
        let alive_particle = spawn_particle(&mut world, 0.0, 0.0, 1.0);
        let expired_particle = spawn_particle(&mut world, 0.0, 0.0, 0.0);

        cleanup_system(&mut world);

        assert!(world.get_component::<Particle>(alive_particle).is_some(), "Alive particle should remain");
        assert!(world.get_component::<Particle>(expired_particle).is_none(), "Expired particle should be removed");
    }

    #[test]
    fn cleanup_preserves_alive_entities() {
        let mut world = setup_world();
        let player = spawn_player(&mut world, 100.0, 536.0);
        let enemy = spawn_enemy(&mut world, 300.0, 536.0, EnemyType::Grunt);
        let bullet = spawn_bullet(&mut world, 150.0, 536.0, 100.0, 0.0, true);
        let particle = spawn_particle(&mut world, 150.0, 536.0, 0.5);

        cleanup_system(&mut world);

        assert!(world.get_component::<Player>(player).is_some());
        assert!(world.get_component::<Enemy>(enemy).is_some());
        assert!(world.get_component::<Bullet>(bullet).is_some());
        assert!(world.get_component::<Particle>(particle).is_some());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Kill Feed System Tests ──────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn kill_feed_decreases_timers() {
        let mut world = setup_world();
        {
            let kf = world.get_resource_mut::<KillFeedRes>().unwrap();
            kf.entries.push(KillFeedEntry { message: "Test".to_string(), timer: 4.0 });
        }

        kill_feed_system(&mut world, 1.0);

        let kf = world.get_resource::<KillFeedRes>().unwrap();
        assert_eq!(kf.entries.len(), 1);
        assert!((kf.entries[0].timer - 3.0).abs() < 0.01);
    }

    #[test]
    fn kill_feed_removes_expired() {
        let mut world = setup_world();
        {
            let kf = world.get_resource_mut::<KillFeedRes>().unwrap();
            kf.entries.push(KillFeedEntry { message: "Old".to_string(), timer: 0.5 });
            kf.entries.push(KillFeedEntry { message: "New".to_string(), timer: 3.0 });
        }

        kill_feed_system(&mut world, 1.0);

        let kf = world.get_resource::<KillFeedRes>().unwrap();
        assert_eq!(kf.entries.len(), 1);
        assert_eq!(kf.entries[0].message, "New");
    }

    #[test]
    fn kill_feed_empty() {
        let mut world = setup_world();
        kill_feed_system(&mut world, 1.0);
        let kf = world.get_resource::<KillFeedRes>().unwrap();
        assert!(kf.entries.is_empty());
    }

    #[test]
    fn kill_feed_multiple_expired() {
        let mut world = setup_world();
        {
            let kf = world.get_resource_mut::<KillFeedRes>().unwrap();
            kf.entries.push(KillFeedEntry { message: "A".to_string(), timer: 0.1 });
            kf.entries.push(KillFeedEntry { message: "B".to_string(), timer: 0.2 });
            kf.entries.push(KillFeedEntry { message: "C".to_string(), timer: 5.0 });
        }

        kill_feed_system(&mut world, 0.5);

        let kf = world.get_resource::<KillFeedRes>().unwrap();
        assert_eq!(kf.entries.len(), 1);
        assert_eq!(kf.entries[0].message, "C");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Camera System Tests ─────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn camera_follows_player() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        spawn_player(&mut world, 400.0, 536.0);

        // Run camera system multiple times to let it converge
        for _ in 0..60 {
            camera_system(&mut world, 1.0 / 60.0);
        }

        let cam = world.get_resource::<CameraRes>().unwrap();
        // Camera should have moved toward the player
        assert!(cam.camera_x > -10.0, "Camera x should be near zero for centered player: {}", cam.camera_x);
    }

    #[test]
    fn camera_does_not_move_when_not_playing() {
        let mut world = setup_world();
        // Default state is Title
        spawn_player(&mut world, 400.0, 536.0);

        camera_system(&mut world, 1.0 / 60.0);

        let cam = world.get_resource::<CameraRes>().unwrap();
        assert!((cam.camera_x - 0.0).abs() < 0.01, "Camera should not move in Title state");
    }

    #[test]
    fn camera_shake_decays() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        spawn_player(&mut world, 400.0, 536.0);
        world.get_resource_mut::<CameraRes>().unwrap().shake_amount = 10.0;

        camera_system(&mut world, 0.1);

        let cam = world.get_resource::<CameraRes>().unwrap();
        assert!(cam.shake_amount < 10.0, "Shake should decay: {}", cam.shake_amount);
    }

    #[test]
    fn camera_shake_clamps_to_zero() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        spawn_player(&mut world, 400.0, 536.0);
        world.get_resource_mut::<CameraRes>().unwrap().shake_amount = 0.5;

        camera_system(&mut world, 1.0);

        let cam = world.get_resource::<CameraRes>().unwrap();
        assert!(cam.shake_amount >= 0.0, "Shake should not go negative: {}", cam.shake_amount);
    }

    #[test]
    fn camera_works_during_game_over() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::GameOver;
        spawn_player(&mut world, 400.0, 536.0);

        // Should not panic
        camera_system(&mut world, 1.0 / 60.0);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Physics Step Tests ──────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn physics_applies_gravity() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_player(&mut world, 100.0, 0.0); // Start high up

        // Set zero initial velocity
        if let Some(v) = world.get_component_mut::<Velocity>(e) {
            v.y = 0.0;
        }

        physics_step(&mut world, 0.1);

        let v = world.get_component::<Velocity>(e).unwrap();
        assert!(v.y > 0.0, "Gravity should cause downward velocity: {}", v.y);
    }

    #[test]
    fn physics_ground_clamp() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        // Spawn player below ground (should be clamped up)
        let e = spawn_player(&mut world, 100.0, 600.0);

        if let Some(v) = world.get_component_mut::<Velocity>(e) {
            v.y = 100.0; // Moving down
        }

        physics_step(&mut world, 0.1);

        let t = world.get_component::<Transform2D>(e).unwrap();
        let max_y = crate::GROUND_Y - crate::PLAYER_SIZE;
        assert!(t.position.y <= max_y + 0.1, "Player should be clamped to ground: {} > {}", t.position.y, max_y);
    }

    #[test]
    fn physics_ground_sets_on_ground() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_player(&mut world, 100.0, 536.0);

        physics_step(&mut world, 0.016);

        let p = world.get_component::<Player>(e).unwrap();
        assert!(p.on_ground, "Player at ground level should be on_ground");
    }

    #[test]
    fn physics_clamps_to_level_bounds() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_player(&mut world, -10.0, 536.0);

        physics_step(&mut world, 0.016);

        let t = world.get_component::<Transform2D>(e).unwrap();
        assert!(t.position.x >= 0.0, "Player should not go below level start: {}", t.position.x);
    }

    #[test]
    fn physics_clamps_to_level_end() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_player(&mut world, crate::LEVEL_W + 100.0, 536.0);

        physics_step(&mut world, 0.016);

        let t = world.get_component::<Transform2D>(e).unwrap();
        let max_x = crate::LEVEL_W - crate::PLAYER_SIZE;
        assert!(t.position.x <= max_x + 0.1, "Player should not exceed level end: {} > {}", t.position.x, max_x);
    }

    #[test]
    fn physics_does_nothing_when_not_playing() {
        let mut world = setup_world();
        // Default state is Title
        let e = spawn_player(&mut world, 100.0, 0.0);
        if let Some(v) = world.get_component_mut::<Velocity>(e) { v.y = 500.0; }

        physics_step(&mut world, 0.1);

        let v = world.get_component::<Velocity>(e).unwrap();
        assert!((v.y - 500.0).abs() < 0.01, "Physics should not run in Title state");
    }

    #[test]
    fn physics_enemy_ground_clamp() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_enemy(&mut world, 200.0, 600.0, EnemyType::Grunt);

        physics_step(&mut world, 0.016);

        let en = world.get_component::<Enemy>(e).unwrap();
        assert!(en.on_ground, "Enemy at ground level should be on_ground");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Bullet Move System Tests ────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn bullet_moves_horizontally() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_bullet(&mut world, 100.0, 300.0, 650.0, 0.0, true);

        bullet_move_system(&mut world, 0.1);

        let b = world.get_component::<Bullet>(e).unwrap();
        assert!((b.x - 165.0).abs() < 0.01, "Bullet should move right: {}", b.x);
        assert!((b.y - 300.0).abs() < 0.01, "Bullet height should not change: {}", b.y);
    }

    // Note: bullet_dies_on_floor_hit is not tested natively because floor hits
    // trigger spark particle spawning which calls rand() (requires JS runtime).

    #[test]
    fn bullet_dies_out_of_bounds() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_bullet(&mut world, -5.0, 300.0, -200.0, 0.0, true);

        bullet_move_system(&mut world, 0.1);

        let b = world.get_component::<Bullet>(e).unwrap();
        assert!(!b.alive, "Bullet should die when out of bounds");
    }

    #[test]
    fn bullet_stays_alive_in_bounds() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_bullet(&mut world, 400.0, 300.0, 100.0, 0.0, true);

        bullet_move_system(&mut world, 0.016);

        let b = world.get_component::<Bullet>(e).unwrap();
        assert!(b.alive, "Bullet in bounds should stay alive");
    }

    #[test]
    fn bullet_does_nothing_when_not_playing() {
        let mut world = setup_world();
        // Default state is Title
        let e = spawn_bullet(&mut world, 100.0, 300.0, 650.0, 0.0, true);

        bullet_move_system(&mut world, 0.1);

        let b = world.get_component::<Bullet>(e).unwrap();
        assert!((b.x - 100.0).abs() < 0.01, "Bullet should not move in Title state");
    }

    #[test]
    fn bullet_dies_above_screen() {
        let mut world = setup_world();
        world.get_resource_mut::<GameStateRes>().unwrap().state = GameState::Playing;
        let e = spawn_bullet(&mut world, 400.0, -5.0, 0.0, -200.0, true);

        bullet_move_system(&mut world, 0.1);

        let b = world.get_component::<Bullet>(e).unwrap();
        assert!(!b.alive, "Bullet should die when above screen");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ── Game State Tests ────────────────────────────────────────────────────
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn game_state_transitions() {
        assert_ne!(GameState::Title, GameState::Playing);
        assert_ne!(GameState::Playing, GameState::GameOver);
        assert_ne!(GameState::Paused, GameState::Playing);
    }

    #[test]
    fn game_state_clone_copy() {
        let s = GameState::Playing;
        let s2 = s;
        assert_eq!(s, s2);
    }
}
