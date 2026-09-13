use anyhow::{
    Result,
    bail,
};
use rayon::prelude::*;
use serde::Serialize;
use serde::ser::SerializeStruct;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TournamentResult {
    pub game_count: u32,
    pub candidate_win_count: u32,
    pub decisive_game_count: u32,
    pub total_turn_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WinRateInterval {
    pub method: &'static str,
    pub confidence_level: f64,
    pub sample_count: u32,
    pub lower: f64,
    pub upper: f64,
}

impl Serialize for TournamentResult {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut result = serializer.serialize_struct("TournamentResult", 5)?;
        result.serialize_field("game_count", &self.game_count)?;
        result.serialize_field("candidate_win_count", &self.candidate_win_count)?;
        result.serialize_field("decisive_game_count", &self.decisive_game_count)?;
        result.serialize_field("total_turn_count", &self.total_turn_count)?;
        result.serialize_field("candidate_win_rate_interval", &self.calculate_candidate_win_rate_interval())?;
        result.end()
    }
}

impl TournamentResult {
    pub fn calculate_candidate_win_rate_interval(&self) -> Option<WinRateInterval> {
        // two-sided 95% normal critical value
        const Z: f64 = 1.959963984540054;
        if self.game_count == 0 || self.candidate_win_count > self.game_count {
            return None;
        }
        let sample_count = self.game_count as f64;
        let win_rate = self.calculate_candidate_win_rate();
        let adjustment = Z * Z / sample_count;
        let denominator = 1.0 + adjustment;
        let center = (win_rate + adjustment / 2.0) / denominator;
        let radius = Z * ((win_rate * (1.0 - win_rate) + adjustment / 4.0) / sample_count).sqrt() / denominator;
        Some(WinRateInterval {
            method: "wilson_binomial_approximation",
            confidence_level: 0.95,
            sample_count: self.game_count,
            lower: (center - radius).max(0.0),
            upper: (center + radius).min(1.0),
        })
    }

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

    #[test]
    fn win_rate_interval_matches_reference_values() {
        for (wins, lower, upper) in [(0, 0.0, 0.0369934982), (50, 0.4038315304, 0.5961684696), (100, 0.9630065018, 1.0)] {
            let result = TournamentResult {
                game_count: 100,
                candidate_win_count: wins,
                ..TournamentResult::empty()
            };
            let interval = result.calculate_candidate_win_rate_interval().unwrap();
            assert!((interval.lower - lower).abs() < 1e-9);
            assert!((interval.upper - upper).abs() < 1e-9);
            assert_eq!(interval.sample_count, 100);
        }
    }

    #[test]
    fn win_rate_interval_handles_empty_single_and_large_samples() {
        assert_eq!(TournamentResult::empty().calculate_candidate_win_rate_interval(), None);
        for game_count in [1, 10, 100, u32::MAX] {
            for wins in [0, game_count / 2, game_count] {
                let result = TournamentResult {
                    game_count,
                    candidate_win_count: wins,
                    ..TournamentResult::empty()
                };
                let interval = result.calculate_candidate_win_rate_interval().unwrap();
                assert!(interval.lower.is_finite() && interval.upper.is_finite());
                assert!(interval.lower >= 0.0 && interval.upper <= 1.0);
                assert!(interval.lower <= result.calculate_candidate_win_rate() + 1e-15);
                assert!(interval.upper >= result.calculate_candidate_win_rate() - 1e-15);
            }
        }
    }

    #[test]
    fn interval_includes_unfinished_games_and_is_computed_after_combining() {
        let first = TournamentResult {
            game_count: 10,
            candidate_win_count: 2,
            decisive_game_count: 4,
            total_turn_count: 100,
        };
        let combined = first.combine(first);
        let interval = combined.calculate_candidate_win_rate_interval().unwrap();
        assert_eq!(interval.sample_count, 20);
        assert_eq!(combined.calculate_candidate_win_rate(), 0.2);
        let smaller = first.calculate_candidate_win_rate_interval().unwrap();
        assert!(interval.upper - interval.lower < smaller.upper - smaller.lower);
        let json = serde_json::to_value(combined).unwrap();
        assert_eq!(json["candidate_win_rate_interval"], serde_json::to_value(interval).unwrap());
        assert!(serde_json::to_value(TournamentResult::empty()).unwrap()["candidate_win_rate_interval"].is_null());
    }

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
