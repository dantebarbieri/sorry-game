# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Online player and simulator for the **Sorry!** board game. A Rust engine compiled to WebAssembly drives game logic; a **SvelteKit** frontend renders the board, plays games locally or online, and visualizes simulations. The project mirrors the structure of `~/Programming/skyjo` — reference that repo for prior art on crate layout, WASM bridge patterns, Web Worker simulation, and replay architecture, but note that this project uses **SvelteKit** (not React) and **pnpm**.

Goals:
- **Play** — local hot-seat, networked multiplayer against other humans, or vs. computer strategies.
- **Simulate** — run batches of games with configurable AI strategies, deterministic seeded replay, and aggregate statistics.
- **Teach** — an interactive rules landing page that walks new players through the game.
- **Flex** — first-class support for **alternate rulesets** (non-standard board shapes, multi-card hands with player choice, etc.). Rules variants are the product, not an afterthought.

## Build Order

Build components **in this order**, and do not scaffold downstream layers before the upstream one is usable:

1. **`sorry-core`** — Rust library (engine, rules, strategies, simulator). Stable trait surfaces, serializable game history.
2. **`sorry-wasm`** — wasm-bindgen wrapper over `sorry-core`. Narrow JSON-string API.
3. **`frontend`** — SvelteKit app consuming the WASM module. Includes interactive rules landing page and play/simulate views.
4. **`sorry-server`** — Rust server (axum + ws) for networked multiplayer. Hosted on the user's local server.

## Architecture

### Separation of Concerns

```
sorry-core/     Rust library — engine, rules, strategies, simulator, GameHistory
sorry-wasm/     wasm-bindgen wrapper — JSON-string API surface for the frontend
sorry-server/   axum server — lobbies, rooms, WebSocket game sessions
frontend/       SvelteKit + TypeScript — play UI, simulator UI, interactive rules
```

### Rust Core (`sorry-core`)

The core is designed around **trait-based extensibility** so rule variants and computer strategies are hot-swappable without touching game logic:

- **`Rules` trait** — Abstracts every aspect that varies between rulesets: board shape/topology, deck composition, hand size, how many cards are drawn/played per turn, slide behavior, start/home/safety-zone layout, Sorry! card behavior, win conditions, tiebreaks. New variants are added by implementing this trait, **not** by adding conditionals in game logic.
- **`Board` abstraction** — The board is a graph of spaces, not a hardcoded ring. Spaces are addressed by an opaque `SpaceId`; `Rules` provides adjacency, slide destinations, and per-player start/home/safety mappings. This is what allows non-standard board shapes.
- **`Strategy` trait** — Defines computer player decision-making. Receives a `StrategyView` with all public knowledge: own hand (if hand size > 1), all pawns' positions, discard pile contents (for card counting), deck size, opponent cumulative state. Must choose which card to play (when hand size > 1) and what legal move to make with it.
- **`Game` struct** — Orchestrates turns and scoring. Parameterized by a `Rules` implementation. Records a full `GameHistory` of every action for deterministic replay.
- **`GameHistory`** — Complete record of a game: initial deck order, all player actions per turn. Sufficient to reconstruct and replay any state. Serializable (serde) for passing to the frontend.
- **`Simulator`** — Runs batches of games with given strategies/rules, collects aggregate statistics (win rates, average game length, pawns-home distributions, etc.).

### WASM Bridge (`sorry-wasm`)

Follow the skyjo pattern: a **thin** wasm-bindgen layer exposing simulation, replay, and interactive-play primitives to JS via JSON strings. Keep the surface narrow — no complex bindings, no `serde-wasm-bindgen`. Expected exports roughly parallel skyjo's:

- `simulate(config_json)` / `simulate_with_histories(config_json)` — batch simulation.
- `simulate_one(config_json)` / `simulate_one_with_history(config_json)` — single game; used by a Web Worker for incremental progress.
- `get_available_strategies()` / `get_available_rules()` — introspection for UI dropdowns.
- Interactive-play entrypoints that validate a proposed action against a given state and return the next state + legal-moves set. All game state transitions go through WASM — the frontend never computes game state itself.

Strategy and rule names are mapped to concrete types via match statements in `sorry-wasm/src/lib.rs`. The WASM module builds to `frontend/src/lib/pkg/` via `wasm-pack build --target web --out-dir ../frontend/src/lib/pkg` — inside the source tree so Vite can resolve the generated JS as an ES module (imports of JS files from `static/` are disallowed by Vite). The `pkg/` directory is gitignored.

### Frontend (`frontend`)

- **SvelteKit + TypeScript**, package manager is **pnpm** (never npm).
- **Interactive rules landing page** (`src/routes/rules/+page.svelte`) — the main entry point for new users. Walks through setup, turn structure, each card's behavior, slides, Sorry!, and win conditions with an interactive mini-board the reader can click through. Designed so someone who has never played Sorry! can learn by doing.
- **Play mode** (`src/routes/play/+page.svelte`) — local hot-seat + vs. bots + networked. Click-based interaction for humans; bot turns automated via a store/effect that calls into WASM.
- **Simulate mode** (`src/routes/simulate/+page.svelte`) — batch simulation UI with live progress, pause/resume/stop, per-player stats table, game list, and per-game replay.
- **Async simulation via Web Worker** — simulations run in a dedicated worker (`src/lib/worker.ts`) that loads WASM independently, runs games one at a time, accumulates stats incrementally, and posts progress updates every ~50ms.
- **Replay** — step through `GameHistory` turn-by-turn. Board state is reconstructed in TypeScript from the history; the frontend never invents state.
- **Networked play** — talks to `sorry-server` over WebSocket. Authoritative game state lives on the server (which also runs `sorry-core`); clients send actions and receive state patches.
- **Component layout** — Svelte components in `src/lib/components/`, stores in `src/lib/stores/`, pure TS helpers in `src/lib/`. Routes are file-based under `src/routes/`.

### Server (`sorry-server`)

Axum + tokio + tower-http. Mirrors skyjo-server's pattern: WebSocket-based rooms, `dashmap` for in-memory room state, `uuid` for room/player IDs, served on the user's local network. Depends directly on `sorry-core` so server-side validation uses the exact same engine as the client's WASM.

## Build Commands

```bash
# Rust core
cd sorry-core && cargo build
cd sorry-core && cargo test
cd sorry-core && cargo test <test_name>        # single test

# WASM (outputs to frontend/src/lib/pkg/)
cd sorry-wasm && wasm-pack build --target web --out-dir ../frontend/src/lib/pkg

# Frontend (pnpm only — never npm)
cd frontend && pnpm install
cd frontend && pnpm dev                         # SvelteKit dev server
cd frontend && pnpm build                       # production build
cd frontend && pnpm preview                     # preview prod build

# Server
cd sorry-server && cargo run

# Lint
cd sorry-core && cargo clippy -- -D warnings
cd sorry-wasm && cargo clippy -- -D warnings
cd sorry-server && cargo clippy -- -D warnings
cd frontend && pnpm lint
cd frontend && pnpm check                       # svelte-check
```

## Key Design Principles

- **Trait objects for hot-swappability** — `Rules` and `Strategy` are trait objects (`Box<dyn Rules>`, `Box<dyn Strategy>`). Rule variants and strategies are selected at runtime by name.
- **No hardcoded board** — board topology lives in `Rules`, not in constants. Alternate board shapes must not require changes to `Game` or `Strategy` internals.
- **Flexible hand size** — the engine handles hand size ≥ 1 as a first-class parameter. For standard Sorry! (hand size = 1, play immediately), the player-choice step is a no-op; variants with larger hands expose the choice through `Strategy::choose_card`.
- **Deterministic replay** — all randomness flows through a seedable RNG. `GameHistory` + seed reproduces any game exactly.
- **Frontend is purely a consumer** — all game logic lives in Rust (core + WASM + server). The frontend reads state and renders; it does not compute legal moves or apply rules.
- **Narrow serialization boundary** — the WASM↔JS and server↔client boundaries pass JSON-serialized structs. Keep the interface small.
- **Server and client share the engine** — `sorry-server` depends on `sorry-core` directly, so the authoritative validator is byte-for-byte the same logic as the client-side WASM.

## Reference Project

`~/Programming/skyjo` is the template for this project's layout, WASM bridge style, Web Worker simulation pattern, replay architecture, and server design. **Consult it before inventing a new pattern.** Known deliberate differences:

- Frontend framework: **SvelteKit** (skyjo uses React + Vite).
- Package manager: **pnpm** exclusively (skyjo also uses pnpm).
- WASM output path: `frontend/src/lib/pkg/` (inside the Svelte source tree so Vite can import the glue JS as an ES module), not `frontend/pkg/` or `frontend/static/pkg/`.
- Game domain: Sorry! has a graph-shaped board and pawn-based movement rather than a grid of cards, so `Board`/`Rules` carry more structure than skyjo's grid dimensions.

## Planned strategies

Only **Random** is implemented in `sorry-core` today. These are the full specs for the other strategies — keep this section authoritative so `/rules` can stay brief. All strategies receive a `StrategyView` with all public knowledge (hand, pawn positions for all players, discard contents, deck size) and must route non-determinism through the provided `rng`.

- **Random** *(shipped)* — picks a legal move uniformly at random. Baseline.

- **Greedy** *(planned)* — always minimizes distance-to-home. Given multiple moves, picks the one that brings the average (or best) pawn closest to its home space. Ties broken by preferring safety-zone spaces.

- **Not Sorry** *(planned)* — maximizes pawns out of Start. Any move that starts a pawn (1, 2, Sorry!) is taken whenever possible; otherwise plays as **Greedy**.

- **Survivor** *(planned)* — attacks the current leader (smallest average distance-to-home among opponents) regardless of cost to its own position. Decision order:
  1. **Bump the leader directly** with an Advance/Retreat that lands on a leader pawn. The 4 and 10 are self-cards — they cannot reverse an opponent. Their only anti-leader use is when a leader pawn sits exactly 4 behind (for a 4) or 1 behind (for a 10's backward option) one of Survivor's own pawns on the track, so the backwards move bumps it.
  2. **Sorry! the leader** from Start whenever possible.
  3. **11-swap with a leader pawn** whenever legal, regardless of whether the swap benefits Survivor's own position.
  4. **Fallback**: play as **Greedy**.

- **Reverse** *(planned)* — holds one primary pawn near its `start_exit`, hoping for a 4 (15-space forward-equivalent) or a chain of 10s to reach Home quickly. Secondary pawns stay in Start unless forced. Counts discard to estimate remaining 4s / 10s before committing the primary pawn forward.

- **Teleporter** *(planned)* — **always** takes an 11-swap when legal, even if the swap moves its own pawn backwards along the track. When multiple swap targets exist, picks the one that maximizes its own distance around the board relative to its `start_exit`. Falls back to **Greedy** when no swap is available.

- **Sidekick** *(planned)* — piles on whichever opponent most recently lost a pawn (had one bumped to Start). When no recent victim exists, plays as **Greedy**.
