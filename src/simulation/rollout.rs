use anyhow::{
    Result,
    bail,
    ensure,
};
use rayon::prelude::*;
use serde::Serialize;
use std::time::Instant;

use super::paired::PairedResult;
use super::tournament::{
    TournamentConfig,
    TournamentResult,
    play_trial,
};
use crate::game::engine::turn::{
    GameOutcome,
    play_game,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::strategy::rollout::RolloutConfig;
use crate::game::strategy::rollout_player::{
    RolloutStats,
    RolloutStrategy,
};

#[derive(Debug, Serialize)]
pub struct RolloutBenchmark {
    pub paired: PairedResult,
    pub stats: RolloutStats,
    pub rollout_elapsed_seconds: f64,
    pub baseline_elapsed_seconds: f64,
}

pub fn benchmark(
    rules: &Ruleset,
    baseline: ConfigurableStrategy,
    opponents: &[ConfigurableStrategy],
    config: &TournamentConfig,
    rollout: RolloutConfig,
) -> Result<RolloutBenchmark> {
    ensure!(!opponents.is_empty(), "rollout benchmark requires opponents");
    ensure!(
        config.game_count > 0 && config.max_turn_count > 0 && rollout.sample_count > 0 && rollout.max_turn_count > 0,
        "rollout benchmark requires positive game, sample, and turn counts"
    );
    match config.player_count {
        2 => Ok(run::<2>(rules, baseline, opponents, config, rollout)),
        3 => Ok(run::<3>(rules, baseline, opponents, config, rollout)),
        4 => Ok(run::<4>(rules, baseline, opponents, config, rollout)),
        5 => Ok(run::<5>(rules, baseline, opponents, config, rollout)),
        6 => Ok(run::<6>(rules, baseline, opponents, config, rollout)),
        7 => Ok(run::<7>(rules, baseline, opponents, config, rollout)),
        8 => Ok(run::<8>(rules, baseline, opponents, config, rollout)),
        _ => bail!("rollout benchmark requires 2 to 8 players"),
    }
}

fn run<const N: usize>(
    rules: &Ruleset,
    baseline: ConfigurableStrategy,
    opponents: &[ConfigurableStrategy],
    config: &TournamentConfig,
    rollout: RolloutConfig,
) -> RolloutBenchmark {
    let start = Instant::now();
    let candidates: Vec<_> = (0..config.game_count)
        .into_par_iter()
        .map(|index| {
            let candidate_seat = index as usize % N;
            let mut strategies = core::array::from_fn(|seat| {
                if seat == candidate_seat {
                    RolloutStrategy::new(baseline, Some(rollout))
                } else {
                    RolloutStrategy::new(opponents[(index as usize / N + seat) % opponents.len()], None)
                }
            });
            let mut state = GameState::<N>::create_starting_state(rules, config.seed.wrapping_add(index as u64));
            let summary = play_game(&mut state, rules, &mut strategies, config.max_turn_count);
            (
                TournamentResult {
                    game_count: 1,
                    candidate_win_count: u32::from(matches!(summary.outcome, GameOutcome::Winner(player) if player as usize == candidate_seat)),
                    decisive_game_count: u32::from(matches!(summary.outcome, GameOutcome::Winner(_))),
                    total_turn_count: summary.turn_count as u64,
                },
                strategies[candidate_seat].stats,
            )
        })
        .collect();
    let rollout_elapsed_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let baselines: Vec<_> = (0..config.game_count).into_par_iter().map(|index| play_trial::<N>(rules, baseline, opponents, config, index)).collect();
    let baseline_elapsed_seconds = start.elapsed().as_secs_f64();
    let mut paired = PairedResult::empty();
    let mut stats = RolloutStats::default();
    for ((candidate, cost), baseline) in candidates.into_iter().zip(baselines) {
        let candidate_won = candidate.candidate_win_count == 1;
        let baseline_won = baseline.candidate_win_count == 1;
        paired = paired.combine(PairedResult {
            candidate,
            baseline,
            both_win_count: u32::from(candidate_won && baseline_won),
            neither_win_count: u32::from(!candidate_won && !baseline_won),
            candidate_only_win_count: u32::from(candidate_won && !baseline_won),
            baseline_only_win_count: u32::from(!candidate_won && baseline_won),
        });
        stats = stats.combine(cost);
    }
    RolloutBenchmark {
        paired,
        stats,
        rollout_elapsed_seconds,
        baseline_elapsed_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;

    #[test]
    fn benchmark_is_deterministic_and_baseline_uses_the_same_trials() {
        let baseline = ConfigurableStrategy::new();
        let config = TournamentConfig {
            game_count: 4,
            max_turn_count: 100,
            seed: 7,
            player_count: 2,
        };
        let rollout = RolloutConfig {
            decisions: Default::default(),
            seed: 19,
            sample_count: 2,
            max_turn_count: 1,
        };
        let run = || benchmark(&DEX_RULESET, baseline, &[baseline], &config, rollout).unwrap();
        let serial = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(run);
        let parallel = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(run);
        assert_eq!(serial.paired, parallel.paired);
        assert_eq!(serial.stats, parallel.stats);
        assert_eq!(
            serial.paired.baseline,
            super::super::tournament::run_pool_tournament(&DEX_RULESET, baseline, &[baseline], &config).unwrap()
        );
        assert!(serial.stats.decisions > 0);
        assert_eq!(serial.stats.failures, 0);
        assert!(benchmark(&DEX_RULESET, baseline, &[], &config, rollout).is_err());
    }
}
