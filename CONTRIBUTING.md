# Contributing to SCP Shooter

Thanks for your interest in contributing! This guide will help you get started.

## Getting Started

1. **Fork** the repository
2. **Clone** your fork locally
3. **Install dependencies** (see [README.md](README.md#prerequisites))
4. **Create a branch** for your change: `git checkout -b my-feature`
5. **Make your changes** and test them
6. **Submit a pull request**

## Development Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/scp-game.git
cd scp-game

# Install the WASM target
rustup target add wasm32-unknown-unknown

# Install Trunk (if not already installed)
cargo install trunk

# Start the dev server
trunk serve
```

The game will be available at `http://localhost:8080` with hot-reload enabled.

## Code Style

- **Formatting**: Run `cargo fmt --all` before committing
- **Linting**: Run `cargo clippy --all-targets -- -D warnings` to check for warnings
- **Naming**: Follow Rust conventions — `snake_case` for functions/variables, `PascalCase` for types
- **Comments**: Write comments for non-obvious logic; don't comment the obvious

## Making Changes

### Game Logic

Most game logic lives in `src/systems.rs`. If you're adding a new mechanic:

1. Add any new components to `src/components.rs`
2. Add any new resources to `src/resources.rs`
3. Implement the system in `src/systems.rs`
4. Register the system in `src/lib.rs` (in the game loop)

### UI / Visuals

The UI is built with vanilla HTML/CSS/JS in `index.html`. The game rendering uses `ShapeRenderer` (rect-based drawing) in Rust.

- CSS styles are in the `<style>` block (lines ~7–203)
- JavaScript logic is in the `<script>` block (lines ~220–893)
- The WASM-JS bridge uses exported functions (`get_score`, `get_lives`, `get_game_state`, etc.)

### Audio

All audio is procedural — synthesized with Web Audio API in the `SoundEngine` class in `index.html`. No audio files are used. If you want to add a new sound effect, add a method to `SoundEngine`.

## Pull Request Guidelines

- **One change per PR** — keep pull requests focused on a single feature or fix
- **Describe what you changed** and why in the PR description
- **Test your changes** — make sure the game runs and your change works as expected
- **Run the linter** — `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
- **Keep commits clean** — write clear commit messages; squash if needed

## Reporting Issues

Open an issue on GitHub with:

- A clear title and description
- Steps to reproduce (if it's a bug)
- Expected vs. actual behavior
- Browser and OS information

## Licensing

By contributing, you agree that your contributions will be licensed under the project's dual-license model:

- **Code contributions** — MIT License
- **SCP Foundation content** — CC BY-SA 3.0

If your contribution includes SCP Foundation universe content (entity descriptions, lore, etc.), it must comply with the [SCP Foundation licensing guidelines](http://scp-wiki.wikidot.com/licensing-guide).

## Questions?

Open an issue or start a discussion on GitHub. We're happy to help.
