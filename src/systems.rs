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
    RELOAD_TIME, ENEMY_MAX_AMMO, ENEMY_RELOAD_TIME, TANK_MAX_AMMO, TANK_RELOAD_TIME};

struct HitEvent { x: f32, y: f32, shake: f32, score: i32, hit_player: bool }

// ── AABB Helpers ─────────────────────────────────────────────────────────────

/// Test overlap between two axis-aligned rectangles defined by center (cx, cy) and half-size (hw, hh).
fn aabb_overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
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

// ── Enemy Spawn ──────────────────────────────────────────────────────────────

pub fn enemy_spawn_system(world: &mut World, dt: f32) {
    let gs = world.get_resource::<GameStateRes>().unwrap();
    if gs.state != GameState::Playing { return; }

    // Update difficulty
    {
        let diff = world.get_resource_mut::<DifficultyRes>().unwrap();
        diff.elapsed_time += dt;
        diff.level = (diff.elapsed_time / 15.0).min(10.0) as i32;
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
        spawn.spawn_timer -= dt;
    }

    let should_spawn = world.get_resource::<SpawnRes>().unwrap().spawn_timer <= 0.0;
    if !should_spawn { return; }

    // Always reset the spawn timer when it triggers
    {
        let spawn = world.get_resource_mut::<SpawnRes>().unwrap();
        spawn.spawn_timer = spawn.spawn_interval;
    }

    let enemy_count = QuerySingle::<Enemy>::new(world)
        .map(|q| q.len())
        .unwrap_or(0);

    if enemy_count < MAX_ENEMIES {
        let _camera_x = world.get_resource::<CameraRes>().unwrap().camera_x;
        let diff_level = world.get_resource::<DifficultyRes>().unwrap().level;

        // Select enemy type based on difficulty
        let (enemy_type, hp, speed_min, speed_max, shoot_min, shoot_max) = {
            let roll = rand();
            if diff_level < 3 {
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

        // Spawn from map boundaries or drop from the sky
        let drop_in = rand() < 0.15; // 15% chance to drop from above

        let (spawn_x, spawn_y, initial_vx, initial_vy) = if drop_in {
            // Drop in from above at a random X across the level
            let sx = rand_range(40.0, LEVEL_W - 40.0);
            (sx, -size - rand_range(20.0, 80.0), rand_range(-30.0, 30.0), rand_range(40.0, 80.0))
        } else {
            // Enter from the left or right map boundary
            let from_right = rand() > 0.5;
            if from_right {
                (LEVEL_W + rand_range(10.0, 40.0), GROUND_Y - size,
                 -rand_range(speed_min, speed_max), 0.0)
            } else {
                (-size - rand_range(10.0, 40.0), GROUND_Y - size,
                 rand_range(speed_min, speed_max), 0.0)
            }
        };

        // Set ammo based on enemy type
        let (max_ammo, reload_time) = match enemy_type {
            EnemyType::Tank => (TANK_MAX_AMMO, TANK_RELOAD_TIME),
            _ => (ENEMY_MAX_AMMO, ENEMY_RELOAD_TIME),
        };

        world.spawn()
            .with(Enemy {
                hp,
                max_hp: hp,
                alive: true,
                on_ground: !drop_in,
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
