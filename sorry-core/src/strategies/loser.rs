use rand::RngCore;
use rand::seq::SliceRandom;

use crate::board::PlayerId;
use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::{is_bumping, simulated_distance};

/// "Loser" — deliberately plays to lose. Priority is fixed:
///
/// 1. **Bump avoidance**. Never knocks an opponent pawn back to Start if
///    any non-bumping legal move exists. This ranks above progress
///    minimization — Loser will advance forward rather than bump.
///
/// 2. **Self-harm score**. Among non-bumping survivors (or the full
///    legal set if every move bumps), maximize
///    `own_total_distance − opponents_total_distance` after applying the
///    move. This single rule captures: retreats beat advances; backward-1
///    (from a 10) beats forward-10; SwapEleven (self-sabotage) beats a
///    plain Advance-11 when the swap moves own pawn backward and pushes
///    an opponent pawn closer to their home; keeping pawns in Start
///    beats releasing them; and Sorry starts-of-pawn are avoided.
///
/// 3. **Tiebreak**. `Pass` wins all ties when present; else random among
///    top-scoring candidates.
///
/// Loser must still return a legal move — the engine rejects foreign
/// moves — so this is "pick the worst legal move", not "refuse".
#[derive(Debug, Default, Clone, Copy)]
pub struct LoserStrategy;

impl Strategy for LoserStrategy {
    fn name(&self) -> &str {
        "Loser"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Deliberately plays to lose — avoids bumps and maximizes own distance-to-home.".to_string(),
            complexity: Complexity::Medium,
        }
    }

    fn choose_move(
        &self,
        view: &StrategyView,
        rules: &dyn Rules,
        _card: Card,
        legal: &[Move],
        rng: &mut dyn RngCore,
    ) -> Move {
        if legal.is_empty() {
            return Move::Pass;
        }
        if let Some(pass) = legal.iter().find(|m| matches!(m, Move::Pass)) {
            return pass.clone();
        }
        let me = view.my_player;

        // Tier 1: bump-avoidance is primary.
        let non_bumping: Vec<&Move> =
            legal.iter().filter(|m| !is_bumping(m, view, me)).collect();
        let candidates: Vec<&Move> = if !non_bumping.is_empty() {
            non_bumping
        } else {
            legal.iter().collect()
        };

        // Tier 2: score by self-harm. Higher is worse (= better for Loser).
        let opponents: Vec<PlayerId> = (0..view.num_players)
            .map(|i| PlayerId(i as u8))
            .filter(|p| *p != me)
            .collect();
        let scored: Vec<(i64, &Move)> = candidates
            .iter()
            .map(|m| {
                let own = simulated_distance(view, rules, me, m, me) as i64;
                let opp: i64 = opponents
                    .iter()
                    .map(|p| simulated_distance(view, rules, me, m, *p) as i64)
                    .sum();
                (own - opp, *m)
            })
            .collect();
        let best = scored.iter().map(|(s, _)| *s).max().unwrap();
        let tops: Vec<&Move> = scored
            .into_iter()
            .filter_map(|(s, m)| (s == best).then_some(m))
            .collect();
        tops.choose(rng).cloned().cloned().unwrap_or(Move::Pass)
    }
}
