//! Batch game runner for Sorry!.
//!
//! `Simulator` is parameterized by factory closures so every game gets a
//! fresh `Box<dyn Rules>` / `Box<dyn Strategy>` set — trait objects aren't
//! clonable. Per-game seeds are derived from a single `base_seed`, which
//! means the whole batch is reproducible from one `SimulatorConfig`.

use serde::{Deserialize, Serialize};

use crate::board::PlayerId;
use crate::error::Result;
use crate::game::Game;
use crate::history::GameHistory;
use crate::rules::Rules;
use crate::strategy::Strategy;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub num_games: usize,
    pub base_seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub winners: Vec<PlayerId>,
    pub num_turns: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub num_games: usize,
    pub num_players: usize,
    pub wins_per_player: Vec<usize>,
    pub win_rate_per_player: Vec<f64>,
    pub avg_turns_per_game: f64,
    pub min_turns: usize,
    pub max_turns: usize,
    pub truncated_count: usize,
}

pub struct Simulator {
    config: SimulatorConfig,
    rules_factory: Box<dyn Fn() -> Box<dyn Rules>>,
    strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>>,
}

impl Simulator {
    pub fn new(
        config: SimulatorConfig,
        rules_factory: Box<dyn Fn() -> Box<dyn Rules>>,
        strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>>,
    ) -> Self {
        Self {
            config,
            rules_factory,
            strategy_factories,
        }
    }

    /// Run all games and return every `GameHistory` alongside aggregate stats.
    pub fn run(&self) -> Result<(Vec<GameHistory>, AggregateStats)> {
        let num_players = self.strategy_factories.len();
        let mut histories = Vec::with_capacity(self.config.num_games);
        let mut per_game = Vec::with_capacity(self.config.num_games);

        for i in 0..self.config.num_games {
            let history = self.play_one(i)?;
            per_game.push(GameStats::from_history(&history));
            histories.push(history);
        }

        let aggregate = compute_aggregate(&per_game, num_players);
        Ok((histories, aggregate))
    }

    /// Run all games without retaining histories — useful for large batches
    /// where the caller only needs win rates.
    pub fn run_stats_only(&self) -> Result<AggregateStats> {
        let num_players = self.strategy_factories.len();
        let mut per_game = Vec::with_capacity(self.config.num_games);

        for i in 0..self.config.num_games {
            let history = self.play_one(i)?;
            per_game.push(GameStats::from_history(&history));
        }

        Ok(compute_aggregate(&per_game, num_players))
    }

    fn play_one(&self, index: usize) -> Result<GameHistory> {
        let seed = self.config.base_seed.wrapping_add(index as u64);
        let rules = (self.rules_factory)();
        let strategies: Vec<Box<dyn Strategy>> =
            self.strategy_factories.iter().map(|f| f()).collect();
        let game = Game::new(rules, strategies, seed)?;
        game.play()
    }
}

impl GameStats {
    pub fn from_history(history: &GameHistory) -> Self {
        Self {
            winners: history.winners.clone(),
            num_turns: history.turns.len(),
            truncated: history.truncated,
        }
    }
}

fn compute_aggregate(stats: &[GameStats], num_players: usize) -> AggregateStats {
    let num_games = stats.len();
    let mut wins_per_player = vec![0usize; num_players];
    let mut truncated_count = 0usize;
    let mut total_turns: u64 = 0;
    let mut min_turns = usize::MAX;
    let mut max_turns = 0usize;

    for game in stats {
        for winner in &game.winners {
            let idx = winner.0 as usize;
            if idx < num_players {
                wins_per_player[idx] += 1;
            }
        }
        if game.truncated {
            truncated_count += 1;
        }
        total_turns += game.num_turns as u64;
        min_turns = min_turns.min(game.num_turns);
        max_turns = max_turns.max(game.num_turns);
    }

    let denom = num_games.max(1) as f64;
    let win_rate_per_player = wins_per_player
        .iter()
        .map(|&w| w as f64 / denom)
        .collect();
    let avg_turns_per_game = total_turns as f64 / denom;

    AggregateStats {
        num_games,
        num_players,
        wins_per_player,
        win_rate_per_player,
        avg_turns_per_game,
        min_turns: if num_games == 0 { 0 } else { min_turns },
        max_turns,
        truncated_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::StandardRules;
    use crate::strategies::RandomStrategy;

    fn make_simulator(num_games: usize, base_seed: u64, num_players: usize) -> Simulator {
        let strategy_factories: Vec<Box<dyn Fn() -> Box<dyn Strategy>>> = (0..num_players)
            .map(|_| {
                Box::new(|| Box::new(RandomStrategy) as Box<dyn Strategy>)
                    as Box<dyn Fn() -> Box<dyn Strategy>>
            })
            .collect();
        Simulator::new(
            SimulatorConfig {
                num_games,
                base_seed,
            },
            Box::new(|| Box::new(StandardRules::new()) as Box<dyn Rules>),
            strategy_factories,
        )
    }

    #[test]
    fn identical_config_produces_identical_histories() {
        let sim_a = make_simulator(4, 123, 2);
        let sim_b = make_simulator(4, 123, 2);
        let (histories_a, stats_a) = sim_a.run().expect("run a");
        let (histories_b, stats_b) = sim_b.run().expect("run b");
        assert_eq!(histories_a, histories_b);
        assert_eq!(stats_a.wins_per_player, stats_b.wins_per_player);
    }

    #[test]
    fn run_stats_only_matches_run_aggregate() {
        let sim = make_simulator(5, 7, 3);
        let (_, stats_full) = sim.run().expect("run");
        let stats_only = make_simulator(5, 7, 3)
            .run_stats_only()
            .expect("run_stats_only");
        assert_eq!(stats_full.wins_per_player, stats_only.wins_per_player);
        assert_eq!(
            stats_full.truncated_count,
            stats_only.truncated_count,
        );
        assert_eq!(stats_full.min_turns, stats_only.min_turns);
        assert_eq!(stats_full.max_turns, stats_only.max_turns);
        assert!(
            (stats_full.avg_turns_per_game - stats_only.avg_turns_per_game).abs() < 1e-9
        );
    }

    #[test]
    fn every_game_has_at_least_one_winner_or_is_truncated() {
        let sim = make_simulator(3, 42, 2);
        let (histories, stats) = sim.run().expect("run");
        let total_winner_slots: usize = stats.wins_per_player.iter().sum();
        let non_truncated = histories.iter().filter(|h| !h.truncated).count();
        // Sorry!'s resolve_winners may return multiple tied winners, so use >=.
        assert!(total_winner_slots >= non_truncated);
        assert_eq!(stats.num_games, histories.len());
        assert_eq!(stats.num_players, 2);
    }
}
