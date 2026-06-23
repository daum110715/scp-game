# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-23

### Added

- Initial release of SCP Shooter
- Side-scrolling 2D shooter gameplay with ECS architecture
- Player movement (walk, jump), shooting, and health system
- Enemy spawning with basic AI behavior
- Custom physics: gravity, ground collision, AABB hit detection
- Camera system with dead-zone following and screen shake
- Particle effects for hits and deaths
- Procedural audio engine (Web Audio API) — gunshots, footsteps, jumps, hits, reload
- HTML/CSS UI overlay: start screen, HUD, pause screen, settings panel, game over screen
- Settings system with localStorage persistence (audio, display, gameplay, crosshair, controls)
- Kill feed with timed notifications
- WASM-JS bridge for game state export
- Built on OpenGame Engine (ECS, rendering, input, math)
- Dual license: MIT (code) + CC BY-SA 3.0 (SCP Foundation content)
