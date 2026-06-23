# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SCP Shooter is a side-scrolling 2D shooter game built with Rust targeting WebAssembly. It runs entirely in the browser using WebGL 2.0 for rendering. The game uses the OpenGame Engine (`opengame-engine`) as its foundation, which provides ECS, rendering, input handling, and math utilities.

## Build & Development Commands

```bash
# Development server with hot-reload (serves on http://0.0.0.0:8080)
trunk serve

# Build WASM bundle (development)
trunk build

# Build optimized WASM bundle (release, uses wasm-opt)
trunk build --release

# Run engine tests (must target native, not wasm)
cargo test -p opengame-engine --target x86_64-unknown-linux-gnu

# Format code
cargo fmt --all

# Lint (format check + clippy)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Quick compile check
cargo check --all-targets
```

## Architecture

### Module Structure

- **`src/lib.rs`** — Game entry point (`main()` via `#[wasm_bindgen(start)]`). Contains the `ScpGame` struct that owns the renderer, input manager, time, and ECS `World`. The game loop runs via `requestAnimationFrame`. Also exports WASM functions (`get_score`, `get_lives`, `get_game_state`, `start_game`, `restart_game`) called from JavaScript in `index.html`.

- **`src/components.rs`** — ECS component definitions: `Player`, `Enemy`, `Bullet`, `Particle`, `Velocity`, `GameState`.

- **`src/resources.rs`** — ECS resources (global singletons): `InputState`, `GameStateRes`, `ScoreRes`, `LivesRes`, `CameraRes`, `SpawnRes`.

- **`src/systems.rs`** — All game logic systems: `player_move_system`, `player_shoot_system`, `enemy_spawn_system`, `enemy_ai_system`, `bullet_move_system`, `particle_update_system`, `camera_system`, `cleanup_system`, and `physics_step` (custom gravity + AABB collision).

- **`index.html`** — HTML shell with CSS UI overlays (start screen, HUD, game over). JavaScript polls WASM exports every 100ms to render UI state. Click events on the overlay trigger `start_game()` / `restart_game()`.

### Key Design Decisions

1. **Custom physics, not engine physics**: The game implements its own `physics_step()` with gravity integration, ground clamping, and AABB collision detection rather than using the engine's `PhysicsSystem`. Bullets and particles use manual position updates (not `Transform2D` + `Velocity`) to avoid physics overhead.

2. **ECS query pattern**: Uses `QuerySingle::<Component>::new(&world)` to iterate entities with a specific component. To avoid borrow conflicts, systems collect entity data into `Vec`s first, then mutate components in separate loops.

3. **WASM-JS bridge**: Game state is exposed to JavaScript via `thread_local! { static GAME_REF }` holding an `Rc<RefCell<ScpGame>>`. The JS overlay reads game state and renders HTML UI; the Rust side handles all game logic and canvas rendering.

4. **Rendering**: All rendering uses `ShapeRenderer` (rect-based drawing). No sprites or textures — everything is drawn with colored rectangles. The camera uses an orthographic projection with dead-zone following and screen shake.

### Constants

Game constants are defined at the top of `lib.rs`: `PLAYER_SIZE`, `PLAYER_SPEED`, `JUMP_FORCE`, `GRAVITY`, `GROUND_Y`, `BULLET_SPEED`, `WORLD_W/H`, `LEVEL_W`, etc.

### Game States

`GameState` enum: `Title` → `Playing` → `GameOver`. State transitions happen in `poll_input()` (title/keyboard) and `run_collision_detection()` (death → game over).

## Engine Dependency

The `opengame-engine` crate lives at `../crates/engine` (workspace member). Key modules used:

- `opengame_engine::ecs` — `World`, `QuerySingle`, entity builder pattern
- `opengame_engine::renderer` — `GlBackend`, `ShapeRenderer`
- `opengame_engine::input` — `InputManager`, `KeyCode`
- `opengame_engine::math` — `Vec2`, `Vec3`, `Mat4` (re-exports `glam`)
- `opengame_engine::transform` — `Transform2D`
- `opengame_engine::time` — `Time`
- `opengame_engine::color` — `Color`

## Toolchain

- Rust stable with `wasm32-unknown-unknown` target
- Trunk (WASM bundler) — installed at `/home/dev/.local/bin/trunk`
- `wasm-bindgen` for JS interop
- Release builds use `opt-level = "z"`, LTO, and symbol stripping for minimal bundle size
