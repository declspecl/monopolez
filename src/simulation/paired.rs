use anyhow::{
    Result,
    bail,
    ensure,
};
use rayon::prelude::*;
use serde::Serialize;

use super::tournament::{
    TournamentConfig,
    TournamentResult,
    play_trial,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::strategy::configurable::ConfigurableStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PairedResult {
    pub candidate: TournamentResult,
    pub baseline: TournamentResult,
    pub both_win_count: u32,
    pub neither_win_count: u32,
    pub candidate_only_win_count: u32,
    pub baseline_only_win_count: u32,
}

impl PairedResult {
    pub(super) fn empty() -> Self {
        Self {
            candidate: TournamentResult::empty(),
            baseline: TournamentResult::empty(),
            both_win_count: 0,
            neither_win_count: 0,
            candidate_only_win_count: 0,
            baseline_only_win_count: 0,
        }
    }

    pub(super) fn combine(
        self,
        other: Self,
    ) -> Self {
        Self {
            candidate: self.candidate.combine(other.candidate),
            baseline: self.baseline.combine(other.baseline),
            both_win_count: self.both_win_count + other.both_win_count,
            neither_win_count: self.neither_win_count + other.neither_win_count,
            candidate_only_win_count: self.candidate_only_win_count + other.candidate_only_win_count,
            baseline_only_win_count: self.baseline_only_win_count + other.baseline_only_win_count,
        }
    }

    pub fn win_rate_difference(&self) -> Option<f64> {
        (self.candidate.game_count > 0).then(|| (self.candidate_only_win_count as f64 - self.baseline_only_win_count as f64) / self.candidate.game_count as f64)
    }
}

pub fn compare_policies(
    rules: &Ruleset,
    candidate: ConfigurableStrategy,
    baseline: ConfigurableStrategy,
    opponents: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> Result<PairedResult> {
    ensure!(!opponents.is_empty(), "opponent pool must not be empty");
    match config.player_count {
        2 => Ok(run::<2>(rules, candidate, baseline, opponents, config)),
        3 => Ok(run::<3>(rules, candidate, baseline, opponents, config)),
        4 => Ok(run::<4>(rules, candidate, baseline, opponents, config)),
        5 => Ok(run::<5>(rules, candidate, baseline, opponents, config)),
        6 => Ok(run::<6>(rules, candidate, baseline, opponents, config)),
        7 => Ok(run::<7>(rules, candidate, baseline, opponents, config)),
        8 => Ok(run::<8>(rules, candidate, baseline, opponents, config)),
        _ => bail!("paired comparison requires 2 to 8 players"),
    }
}

fn run<const N: usize>(
    rules: &Ruleset,
    candidate: ConfigurableStrategy,
    baseline: ConfigurableStrategy,
    opponents: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> PairedResult {
    (0..config.game_count)
        .into_par_iter()
        .map(|index| {
            let candidate = play_trial::<N>(rules, candidate, opponents, config, index);
            let baseline = play_trial::<N>(rules, baseline, opponents, config, index);
            let candidate_won = candidate.candidate_win_count == 1;
            let baseline_won = baseline.candidate_win_count == 1;
            PairedResult {
                candidate,
                baseline,
                both_win_count: u32::from(candidate_won && baseline_won),
                neither_win_count: u32::from(!candidate_won && !baseline_won),
                candidate_only_win_count: u32::from(candidate_won && !baseline_won),
                baseline_only_win_count: u32::from(!candidate_won && baseline_won),
            }
        })
        .reduce(PairedResult::empty, PairedResult::combine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::data::{
        BASELINE_STRATEGY,
        DEX_OPTIMAL_STRATEGY,
    };
    use crate::simulation::tournament::run_pool_tournament;

    #[test]
    fn paired_marginals_match_standalone_tournaments_for_all_player_counts() {
        let pool = [BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY];
        for player_count in 2..=8 {
            let config = TournamentConfig {
                game_count: 24,
                max_turn_count: 500,
                seed: u64::MAX - 10,
                player_count,
            };
            let result = compare_policies(&DEX_RULESET, DEX_OPTIMAL_STRATEGY, BASELINE_STRATEGY, &pool, &config).unwrap();
            assert_eq!(result.candidate, run_pool_tournament(&DEX_RULESET, DEX_OPTIMAL_STRATEGY, &pool, &config).unwrap());
            assert_eq!(result.baseline, run_pool_tournament(&DEX_RULESET, BASELINE_STRATEGY, &pool, &config).unwrap());
            assert_eq!(result.both_win_count + result.candidate_only_win_count, result.candidate.candidate_win_count);
            assert_eq!(result.both_win_count + result.baseline_only_win_count, result.baseline.candidate_win_count);
            assert_eq!(
                result.both_win_count + result.neither_win_count + result.candidate_only_win_count + result.baseline_only_win_count,
                config.game_count
            );
        }
    }

    #[test]
    fn swapping_policies_reverses_discordant_counts() {
        let config = TournamentConfig {
            game_count: 100,
            max_turn_count: 1000,
            seed: 3,
            player_count: 4,
        };
        let pool = [BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY];
        let result = compare_policies(&DEX_RULESET, DEX_OPTIMAL_STRATEGY, BASELINE_STRATEGY, &pool, &config).unwrap();
        let reversed = compare_policies(&DEX_RULESET, BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY, &pool, &config).unwrap();
        assert_eq!(result.candidate_only_win_count, reversed.baseline_only_win_count);
        assert_eq!(result.baseline_only_win_count, reversed.candidate_only_win_count);
        assert_eq!(result.both_win_count, reversed.both_win_count);
        assert_eq!(result.neither_win_count, reversed.neither_win_count);
        assert_eq!(result.win_rate_difference().unwrap(), -reversed.win_rate_difference().unwrap());
        assert!(result.candidate_only_win_count + result.baseline_only_win_count > 0);
    }

    #[test]
    fn identical_policies_never_have_discordant_wins() {
        let config = TournamentConfig {
            game_count: 100,
            max_turn_count: 1000,
            seed: 7,
            player_count: 4,
        };
        let result = compare_policies(&DEX_RULESET, BASELINE_STRATEGY, BASELINE_STRATEGY, &[DEX_OPTIMAL_STRATEGY], &config).unwrap();
        assert_eq!(result.candidate, result.baseline);
        assert_eq!(result.candidate_only_win_count, 0);
        assert_eq!(result.baseline_only_win_count, 0);
        assert_eq!(result.win_rate_difference(), Some(0.0));
        let empty = compare_policies(
            &DEX_RULESET,
            BASELINE_STRATEGY,
            BASELINE_STRATEGY,
            &[DEX_OPTIMAL_STRATEGY],
            &TournamentConfig { game_count: 0, ..config },
        )
        .unwrap();
        assert_eq!(empty.win_rate_difference(), None);
    }
}
