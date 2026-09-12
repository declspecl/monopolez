use anyhow::{
    Result,
    bail,
};
use rayon::prelude::*;
use serde::Serialize;

use crate::game::engine::turn::{
    GameOutcome,
    play_game,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::configurable::ConfigurableStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct TournamentConfig {
    pub game_count: u32,
    pub max_turn_count: u32,
    pub seed: u64,
    pub player_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct TournamentResult {
    pub game_count: u32,
    pub candidate_win_count: u32,
    pub decisive_game_count: u32,
    pub total_turn_count: u64,
}

impl TournamentResult {
    pub const fn empty() -> Self {
        Self {
            game_count: 0,
            candidate_win_count: 0,
            decisive_game_count: 0,
            total_turn_count: 0,
        }
    }

    pub const fn combine(
        self,
        other: Self,
    ) -> Self {
        Self {
            game_count: self.game_count + other.game_count,
            candidate_win_count: self.candidate_win_count + other.candidate_win_count,
            decisive_game_count: self.decisive_game_count + other.decisive_game_count,
            total_turn_count: self.total_turn_count + other.total_turn_count,
        }
    }

    pub fn calculate_candidate_win_rate(&self) -> f64 {
        if self.game_count == 0 {
            return 0.0;
        }

        self.candidate_win_count as f64 / self.game_count as f64
    }

    pub fn calculate_decisive_game_ratio(&self) -> f64 {
        if self.game_count == 0 {
            return 0.0;
        }

        self.decisive_game_count as f64 / self.game_count as f64
    }

    pub fn calculate_average_turn_count(&self) -> f64 {
        if self.game_count == 0 {
            return 0.0;
        }

        self.total_turn_count as f64 / self.game_count as f64
    }
}

pub fn run_tournament(
    ruleset: &Ruleset,
    candidate: ConfigurableStrategy,
    baseline: ConfigurableStrategy,
    config: &TournamentConfig,
) -> Result<TournamentResult> {
    run_pool_tournament(ruleset, candidate, &[baseline], config)
}

pub fn run_pool_tournament(
    ruleset: &Ruleset,
    candidate: ConfigurableStrategy,
    opponent_pool: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> Result<TournamentResult> {
    if opponent_pool.is_empty() {
        bail!("opponent pool must not be empty");
    }

    match config.player_count {
        2 => Ok(play_tournament::<2>(ruleset, candidate, opponent_pool, config)),
        3 => Ok(play_tournament::<3>(ruleset, candidate, opponent_pool, config)),
        4 => Ok(play_tournament::<4>(ruleset, candidate, opponent_pool, config)),
        5 => Ok(play_tournament::<5>(ruleset, candidate, opponent_pool, config)),
        6 => Ok(play_tournament::<6>(ruleset, candidate, opponent_pool, config)),
        7 => Ok(play_tournament::<7>(ruleset, candidate, opponent_pool, config)),
        8 => Ok(play_tournament::<8>(ruleset, candidate, opponent_pool, config)),
        player_count => bail!("player count {player_count} is outside the supported range of 2 to 8"),
    }
}

fn play_tournament<const PLAYER_COUNT: usize>(
    ruleset: &Ruleset,
    candidate: ConfigurableStrategy,
    opponent_pool: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> TournamentResult {
    (0..config.game_count)
        .into_par_iter()
        .map(|game_index| {
            let candidate_seat = game_index as usize % PLAYER_COUNT;
            let mut strategies: [ConfigurableStrategy; PLAYER_COUNT] = core::array::from_fn(|seat| {
                if seat == candidate_seat {
                    return candidate;
                }

                opponent_pool[(game_index as usize / PLAYER_COUNT + seat) % opponent_pool.len()]
            });
            let mut game_state = GameState::<PLAYER_COUNT>::create_starting_state(ruleset, config.seed.wrapping_add(game_index as u64));

            let game_summary = play_game(&mut game_state, ruleset, &mut strategies, config.max_turn_count);
            let candidate_win_count = match game_summary.outcome {
                GameOutcome::Winner(player_id) if player_id as usize == candidate_seat => 1,
                _ => 0,
            };

            TournamentResult {
                game_count: 1,
                candidate_win_count,
                decisive_game_count: u32::from(matches!(game_summary.outcome, GameOutcome::Winner(_))),
                total_turn_count: game_summary.turn_count as u64,
            }
        })
        .reduce(TournamentResult::empty, TournamentResult::combine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;

    fn create_config(game_count: u32) -> TournamentConfig {
        TournamentConfig {
            game_count,
            max_turn_count: 2_000,
            seed: 11,
            player_count: 4,
        }
    }

    #[test]
    fn counts_every_game_once() {
        let strategy = ConfigurableStrategy::new();
        let result = run_tournament(&DEX_RULESET, strategy, strategy, &create_config(64)).expect("4 players should be supported");

        assert_eq!(result.game_count, 64);
        assert!(result.candidate_win_count <= result.decisive_game_count);
    }

    #[test]
    fn gives_identical_strategies_a_fair_share_of_wins() {
        let strategy = ConfigurableStrategy::new();
        let result = run_tournament(&DEX_RULESET, strategy, strategy, &create_config(400)).expect("4 players should be supported");
        let win_rate = result.calculate_candidate_win_rate();

        assert!(win_rate > 0.15 && win_rate < 0.35, "mirror matches should land near 25%, got {win_rate}");
    }
}
