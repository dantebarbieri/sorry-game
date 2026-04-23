# Planned strategies

These strategies are planned for the Sorry! simulator. Only **Random** is
currently implemented in `sorry-core`; the others are descriptions to
guide implementation and testing. The `/rules` page surfaces these in the
Strategies section, and the `/simulate` page will offer them as options
once they land.

## Random — **shipped**

Picks a legal move uniformly at random. Trivial baseline; useful as a
null hypothesis when comparing other strategies.

## Greedy — **planned**

Always tries to minimize the distance from each pawn to Home. When
offered a choice of moves, picks the one that brings the average (or
best) pawn closest to its home space. Ties broken by preferring moves
that stay on safe spaces.

## Not Sorry — **planned**

Prioritizes getting pawns out of Start — any card that can play a pawn
out (1, 2, Sorry!) is always used for that purpose when possible.
Otherwise falls back to **Greedy** behavior.

## Survivor — **planned**

Plays defensively, targeting whichever opponent is closest to winning.
Priority order per turn:

1. **Attack the leader.** If any move bumps the leading opponent — the
   one with the smallest average distance-to-home — prefer it. When an
   11-swap is available and would send a leader pawn backwards, take the
   swap.
2. **Rush to safety.** Move any pawn into a Safety zone when possible.
3. **Go home.** If a pawn can reach Home this turn, do so.
4. **Greedy fallback.** Otherwise play as **Greedy**.

Strategic reversal: given a 4 or cascade of 10s, willingly send the
leader backwards instead of advancing own pawns.

## Reverse — **planned**

Keeps one primary pawn on the board near its own `start_exit`, hoping to
catch a 4 (15-space forward-equivalent) or a chain of 10s (backward 1
each) to reach Home quickly. Secondary pawns stay in Start unless
forced. Counts discard pile to estimate probability of a 4 or 10 before
committing the primary pawn further.

## Teleporter — **planned**

Always takes an 11-swap when it is legal, regardless of whether the
trade benefits the player. When choosing a swap target, picks the
opponent whose pawn is furthest around the track relative to this
player's start exit — maximizing own board progress at the expense of
any reasoning about opponent state.

## Sidekick — **planned**

Attacks whichever opponent most recently lost a pawn (had one bumped to
Start). Effectively "gangs up" on the victim. When no recent victim
exists, plays as **Greedy**.

## Notes for implementation

- All strategies receive a `StrategyView` with all public knowledge
  (hand, pawn positions for all players, discard contents, deck size).
- Non-determinism should flow through the provided `rng`, not
  `rand::thread_rng()`, so simulator batches stay reproducible.
- Strategies must respect the `Rules::can_split` and `Rules::hand_size`
  contracts — a strategy that assumes `hand_size == 1` breaks in
  multi-card variants.
