# SCP Shooter

A side-scrolling 2D shooter game built with Rust and WebAssembly, set in the SCP Foundation universe. Runs entirely in the browser using WebGL 2.0 — no plugins, no downloads, just open and play.

## Overview

You are a Foundation operative navigating a facility overrun by anomalous entities. Shoot your way through waves of SCPs, survive as long as you can, and rack up your score.

This project is built on the [OpenGame Engine](https://github.com/opengame-engine/opengame-engine) and compiled to WebAssembly via [Trunk](https://trunkrs.dev/), delivering a native-quality game experience in the browser.

## Features

- **Side-scrolling 2D shooter** with smooth camera following and screen shake
- **ECS architecture** using OpenGame Engine's entity-component-system
- **Custom physics** — gravity, AABB collision detection, ground clamping
- **Procedural audio** — all sounds synthesized in real-time via Web Audio API (no audio files)
- **Responsive UI** — HUD, settings panel, kill feed, pause screen, game over screen
- **Configurable settings** — audio, display, gameplay, crosshair, and controls
- **Runs in any modern browser** — no installation required

## Play

Open `index.html` in a browser with a local server, or build and serve the WASM bundle:

```bash
# Quick start — development server with hot-reload
trunk serve

# Then open http://localhost:8080
```

## Building from Source

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev/#getting-started): `cargo install trunk`

### Build Commands

```bash
# Development server with hot-reload
trunk serve

# Build WASM bundle (development)
trunk build

# Build optimized WASM bundle (release, uses wasm-opt)
trunk build --release

# Run engine tests (native only)
cargo test -p opengame-engine --target x86_64-unknown-linux-gnu

# Format code
cargo fmt --all

# Lint (format check + clippy)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Quick compile check
cargo check --all-targets
```

## Controls

| Action     | Key              |
|------------|------------------|
| Move Left  | A / Left Arrow   |
| Move Right | D / Right Arrow  |
| Jump       | W / Up Arrow / Space |
| Shoot      | Left Click       |
| Pause      | Escape           |

## Architecture

```
scp-game/
├── Cargo.toml          # Rust package manifest
├── Trunk.toml          # Trunk WASM bundler config
├── index.html          # HTML shell with CSS UI + JavaScript (WASM bridge, audio engine)
├── src/
│   ├── lib.rs          # Game entry point, ScpGame struct, WASM exports, game loop
│   ├── components.rs   # ECS components (Player, Enemy, Bullet, Particle, Velocity)
│   ├── resources.rs    # ECS resources (InputState, GameState, Score, Lives, Camera, Spawn)
│   └── systems.rs      # Game logic systems (movement, shooting, AI, physics, camera)
└── dist/               # Build output (generated)
```

### Key Design Decisions

- **Custom physics over engine physics**: The game implements its own `physics_step()` with gravity integration and AABB collision rather than using the engine's built-in `PhysicsSystem`, giving fine-grained control over gameplay feel.
- **Rect-based rendering**: All visuals are drawn with colored rectangles via `ShapeRenderer` — no sprites or textures. This keeps the build small and the rendering fast.
- **WASM-JS bridge**: Game state is exposed to JavaScript via `thread_local!` + `Rc<RefCell<ScpGame>>`. The JS overlay reads state and renders HTML UI; Rust handles all game logic and canvas rendering.
- **Procedural audio**: All sound effects are synthesized at runtime using Web Audio API oscillators and noise buffers — zero external audio assets.

## License

This project uses a **dual-license** model:

### Code — MIT License

All source code (Rust, JavaScript, HTML, CSS) is licensed under the **MIT License**. See [LICENSE-MIT](LICENSE-MIT) for the full text.

You are free to use, modify, and distribute the code for any purpose, including commercial projects.

### SCP Foundation Content — CC BY-SA 3.0

All SCP Foundation universe setting content — including SCP entity names, numbers, descriptions, Foundation lore, terminology, and narrative elements — is licensed under the **Creative Commons Attribution-ShareAlike 3.0 Unported (CC BY-SA 3.0)** license. See [LICENSE-CC-BY-SA-3.0](LICENSE-CC-BY-SA-3.0) for the full text.

This means you may share and adapt the SCP-themed content, even commercially, as long as you provide appropriate attribution and distribute derivative works under the same license.

The SCP Foundation universe is a collaborative fiction project. For more information, visit [scp-wiki.wikidot.com](http://scp-wiki.wikidot.com/).

### Summary

| Component                | License    |
|--------------------------|------------|
| Source code              | MIT        |
| SCP Foundation content   | CC BY-SA 3.0 |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute.

## Acknowledgments

- [SCP Foundation](http://scp-wiki.wikidot.com/) — the collaborative fiction universe this game is based on
- [OpenGame Engine](https://github.com/opengame-engine/opengame-engine) — the ECS + rendering engine powering the game
- [Trunk](https://trunkrs.dev/) — the WASM bundler for building and serving
- [glam](https://github.com/bitshifter/glam-rs) — the math library used for vectors and matrices
