# Sorry! Simulator

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A web-based [Sorry!](https://en.wikipedia.org/wiki/Sorry!_(game)) board game — play it in your browser, watch bots play, or simulate thousands of games to test strategies. Powered by a Rust engine compiled to WebAssembly and rendered in 3D with three.js.

> **Status: in progress.** The Rust engine, WASM bridge, 3D board renderer, interactive local play, and replay with transport controls are all working. The polished play-mode UI, interactive rules page, batch simulator UI, and networked multiplayer server are planned but not yet built — see [Roadmap](#roadmap).

## Features

### Shipped

- **Rust core engine** — full Sorry! rules (Standard + a "Play Out" variant that establishes 1st→4th placement), deterministic seeded RNG, `GameHistory` captures every action for reproducible replay.
- **3D board renderer** — procedural three.js scene with classic square topology, animated pawn motion including hop arcs, slides, bumps, and 7-splits. LIGHT / DARK skin palettes on the same geometry; future texture-driven themes (Classic / Modern / Neon) slot into the same `BoardSkin` interface.
- **Interactive local play** (`/renderer-probe`) — click or keyboard-navigate your pawn and its highlighted legal destinations; the engine validates every action. Handles Sorry! bumps, 11-Swap, 7-Splits with a two-step on-board picker, and an 11 "may" Pass election. ARIA live region announces every turn.
- **Per-seat viewer** with animated camera rotation to that seat's edge view (500 ms ease).
- **Replay** (`/replay`) — runs a simulated game entirely client-side, steps forward / back / scrubs through every play with animation, auto-play, and a final placement readout. Pure TS reducer, no WASM calls mid-replay.
- **Play Out variant** — an alternate ruleset that continues after the first finisher so the game resolves full placement order.

### Roadmap

- **Interactive rules page** — a walkthrough that teaches Sorry! by letting the reader click through each card type on a live mini-board.
- **Polished play mode** — a top-level `/play` route with local hot-seat, vs-bot, and a proper HUD (hand / drawn-card panel, score display, game setup dialog).
- **Batch simulator UI** — run thousands of games with configurable strategies in a Web Worker, live progress, per-player stats, and per-game replay (mirrors the pattern in [skyjo](https://github.com/dantebarbieri/skyjo)).
- **Networked multiplayer** — `sorry-server` (axum + tokio + WebSocket) is scaffolded; lobby and live play UI are not yet wired up.
- **More strategies** — engine currently ships a `Random` strategy; Greedy / heuristic / MCTS are next.
- **Textured skins** — Classic Hasbro repaint, modern repaint, imagined themes like Neon.

## Architecture

```
sorry-core/     Rust library — engine, rules, strategies, GameHistory, Simulator
sorry-wasm/     wasm-bindgen wrapper exposing the engine to JavaScript
sorry-server/   axum + WebSocket host for networked multiplayer (scaffolded)
frontend/      SvelteKit + TypeScript + three.js (Vite)
```

All game logic lives in Rust. The frontend is a pure consumer — it reads state and histories from the WASM module and never computes legal moves or game state itself. The renderer animates transitions from one engine-committed state to the next; it does not invent geometry (the engine publishes `board_geometry()` with normalized coordinates and per-player adjacency).

Uses SvelteKit (not React) and pnpm. For prior-art on the crate layout, WASM bridge patterns, Web Worker simulation, and replay architecture, see the sibling [skyjo](https://github.com/dantebarbieri/skyjo) project.

## Getting Started

### Development

Prerequisites: [Rust](https://rustup.rs/), [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/), [Node.js ≥ 22](https://nodejs.org/), and [pnpm](https://pnpm.io/).

1. **Build the WASM module**

   ```bash
   cd sorry-wasm
   wasm-pack build --target web --out-dir ../frontend/src/lib/pkg
   ```

   The WASM glue lives inside the SvelteKit source tree (not `static/`) so Vite can resolve the generated JS as an ES module. The `pkg/` directory is gitignored.

2. **Install frontend dependencies**

   ```bash
   cd frontend
   pnpm install
   ```

3. **Start the dev server**

   ```bash
   pnpm dev
   ```

   Vite starts at [http://localhost:5173](http://localhost:5173). The live renderer probe is at [`/renderer-probe`](http://localhost:5173/renderer-probe) and replay is at [`/replay`](http://localhost:5173/replay).

To rebuild WASM after Rust changes, re-run `wasm-pack build` from `sorry-wasm/`.

### Lint & Test

```bash
# Rust
cd sorry-core && cargo clippy -- -D warnings
cd sorry-core && cargo test
cd sorry-wasm && cargo clippy -- -D warnings

# Frontend
cd frontend && pnpm check      # svelte-check / TypeScript
cd frontend && pnpm test       # vitest unit tests for the replay reducer + cursor
```

## License

MIT — see [LICENSE](LICENSE).

The 3D piece model is by 3P3D (Thingiverse #84406), used under CC-BY — see `frontend/static/models/ATTRIBUTION.txt`.

This project is not affiliated with Hasbro, Inc. "Sorry!" is a registered trademark of Hasbro.
