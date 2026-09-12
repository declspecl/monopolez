use anyhow::{
    Result,
    bail,
};
use serde::Serialize;

use super::tournament::{
    TournamentConfig,
    TournamentResult,
    run_pool_tournament,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::tile::model::Cash;

const MIN_WIN_RATE_IMPROVEMENT: f64 = 0.002;

pub const TUNED_STRATEGY_PARAMETERS: [StrategyParameter; 8] = [
    StrategyParameter::CashReserve,
    StrategyParameter::PurchaseCashPercent,
    StrategyParameter::AuctionBidPercent,
    StrategyParameter::ImprovementCashPercent,
    StrategyParameter::UnmortgageCashPercent,
    StrategyParameter::TradeOfferPercent,
    StrategyParameter::TradeAcceptPercent,
    StrategyParameter::PaysBailWhenAffordable,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategyParameter {
    CashReserve,
    PurchaseCashPercent,
    AuctionBidPercent,
    ImprovementCashPercent,
    UnmortgageCashPercent,
    TradeOfferPercent,
    TradeAcceptPercent,
    PaysBailWhenAffordable,
}

impl StrategyParameter {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CashReserve => "cash_reserve",
            Self::PurchaseCashPercent => "purchase_cash_percent",
            Self::AuctionBidPercent => "auction_bid_percent",
            Self::ImprovementCashPercent => "improvement_cash_percent",
            Self::UnmortgageCashPercent => "unmortgage_cash_percent",
            Self::TradeOfferPercent => "trade_offer_percent",
            Self::TradeAcceptPercent => "trade_accept_percent",
            Self::PaysBailWhenAffordable => "pays_bail_when_affordable",
        }
    }

    pub const fn candidate_values(self) -> &'static [Cash] {
        match self {
            Self::CashReserve => &[0, 25, 50, 100, 200, 300, 500, 800],
            Self::PurchaseCashPercent => &[100, 110, 125, 150, 200, 300],
            Self::AuctionBidPercent => &[50, 75, 100, 125, 150, 200, 250, 300, 400, 500],
            Self::ImprovementCashPercent => &[100, 125, 150, 200, 300, 500],
            Self::UnmortgageCashPercent => &[100, 150, 200, 300, 500, 1000],
            Self::TradeOfferPercent => &[0, 75, 100, 125, 150, 200, 250, 300, 400, 500, 700],
            Self::TradeAcceptPercent => &[50, 100, 150, 200, 250, 300, 400, 600, 1000],
            Self::PaysBailWhenAffordable => &[0, 1],
        }
    }

    pub const fn apply(
        self,
        strategy: ConfigurableStrategy,
        value: Cash,
    ) -> ConfigurableStrategy {
        let mut tuned_strategy = strategy;

        match self {
            Self::CashReserve => tuned_strategy.cash_reserve = value,
            Self::PurchaseCashPercent => tuned_strategy.purchase_cash_percent = value,
            Self::AuctionBidPercent => tuned_strategy.auction_bid_percent = value,
            Self::ImprovementCashPercent => tuned_strategy.improvement_cash_percent = value,
            Self::UnmortgageCashPercent => tuned_strategy.unmortgage_cash_percent = value,
            Self::TradeOfferPercent => tuned_strategy.trade_offer_percent = value,
            Self::TradeAcceptPercent => tuned_strategy.trade_accept_percent = value,
            Self::PaysBailWhenAffordable => tuned_strategy.pays_bail_when_affordable = value != 0,
        }

        tuned_strategy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TuningStep {
    pub parameter_name: &'static str,
    pub value: Cash,
    pub win_rate: f64,
    pub decisive_game_ratio: f64,
    pub is_improvement: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SweepReport {
    pub strategy: ConfigurableStrategy,
    pub steps: Vec<TuningStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TuningReport {
    pub baseline_strategy: ConfigurableStrategy,
    pub best_strategy: ConfigurableStrategy,
    pub best_win_rate: f64,
    pub baseline_win_rate: f64,
    pub evaluated_game_count: u64,
    pub steps: Vec<TuningStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TuningSession {
    pub schema_version: u32,
    pub ruleset: Ruleset,
    pub training_config: TournamentConfig,
    pub round_count: u32,
    pub generation_count: u32,
    pub starting_pool: Vec<ConfigurableStrategy>,
    pub reports: Vec<TuningReport>,
    pub opponent_pool: Vec<ConfigurableStrategy>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidationReport {
    pub config: TournamentConfig,
    pub opponent_pool: Vec<ConfigurableStrategy>,
    pub baseline_strategy: ConfigurableStrategy,
    pub champion_strategy: ConfigurableStrategy,
    pub baseline: TournamentResult,
    pub champion: TournamentResult,
}

impl TuningSession {
    pub fn champion(&self) -> ConfigurableStrategy {
        self.reports.last().map(|report| report.best_strategy).unwrap_or_default()
    }
}

pub fn tune_strategy_over_generations(
    ruleset: &Ruleset,
    config: &TournamentConfig,
    round_count: u32,
    generation_count: u32,
    starting_pool: &[ConfigurableStrategy],
) -> Result<TuningSession> {
    if starting_pool.is_empty() || generation_count == 0 || round_count == 0 || config.game_count == 0 || config.max_turn_count == 0 {
        bail!("tuning requires a nonempty pool and positive generation, round, game, and turn counts");
    }
    let mut opponent_pool = starting_pool.to_vec();
    let mut reports = Vec::new();

    for _generation in 0..generation_count {
        let report = tune_strategy(ruleset, config, round_count, &opponent_pool)?;
        opponent_pool.push(report.best_strategy);
        reports.push(report);
    }

    let champion_strategy = reports.last().expect("at least one generation").best_strategy;
    let baseline_strategy = *starting_pool.last().expect("nonempty starting pool");
    let validation_config = TournamentConfig {
        seed: config.seed.wrapping_add(config.game_count as u64),
        ..*config
    };
    let validation_pool = opponent_pool[..opponent_pool.len() - 1].to_vec();
    let validation = ValidationReport {
        config: validation_config,
        baseline_strategy,
        champion_strategy,
        baseline: run_pool_tournament(ruleset, baseline_strategy, &validation_pool, &validation_config)?,
        champion: run_pool_tournament(ruleset, champion_strategy, &validation_pool, &validation_config)?,
        opponent_pool: validation_pool,
    };
    Ok(TuningSession {
        schema_version: 1,
        ruleset: *ruleset,
        training_config: *config,
        round_count,
        generation_count,
        starting_pool: starting_pool.to_vec(),
        reports,
        opponent_pool,
        validation,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LeagueEntry {
    pub strategy: ConfigurableStrategy,
    pub win_rate: f64,
    pub decisive_game_ratio: f64,
    pub average_turn_count: f64,
}

pub fn run_league(
    ruleset: &Ruleset,
    strategies: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> Result<Vec<LeagueEntry>> {
    let mut entries = Vec::new();

    for candidate_strategy in strategies {
        let tournament_result = run_pool_tournament(ruleset, *candidate_strategy, strategies, config)?;

        entries.push(LeagueEntry {
            strategy: *candidate_strategy,
            win_rate: tournament_result.calculate_candidate_win_rate(),
            decisive_game_ratio: tournament_result.calculate_decisive_game_ratio(),
            average_turn_count: tournament_result.calculate_average_turn_count(),
        });
    }

    entries.sort_by(|left, right| right.win_rate.total_cmp(&left.win_rate));

    Ok(entries)
}

pub fn sweep_parameters(
    ruleset: &Ruleset,
    strategy: ConfigurableStrategy,
    opponent_pool: &[ConfigurableStrategy],
    config: &TournamentConfig,
) -> Result<Vec<TuningStep>> {
    let mut steps = Vec::new();

    for parameter in TUNED_STRATEGY_PARAMETERS {
        for value in parameter.candidate_values() {
            let candidate_strategy = parameter.apply(strategy, *value);
            let tournament_result = run_pool_tournament(ruleset, candidate_strategy, opponent_pool, config)?;

            steps.push(TuningStep {
                parameter_name: parameter.name(),
                value: *value,
                win_rate: tournament_result.calculate_candidate_win_rate(),
                decisive_game_ratio: tournament_result.calculate_decisive_game_ratio(),
                is_improvement: false,
            });
        }
    }

    Ok(steps)
}

pub fn tune_strategy(
    ruleset: &Ruleset,
    config: &TournamentConfig,
    round_count: u32,
    opponent_pool: &[ConfigurableStrategy],
) -> Result<TuningReport> {
    let Some(&baseline_strategy) = opponent_pool.last() else {
        bail!("opponent pool must not be empty");
    };
    let mut best_strategy = baseline_strategy;
    let mut best_win_rate = run_pool_tournament(ruleset, best_strategy, opponent_pool, config)?.calculate_candidate_win_rate();

    let baseline_win_rate = best_win_rate;
    let mut evaluated_game_count = config.game_count as u64;
    let mut steps = Vec::new();

    for _tuning_round in 0..round_count {
        let mut has_improved = false;

        for parameter in TUNED_STRATEGY_PARAMETERS {
            for value in parameter.candidate_values() {
                let candidate_strategy = parameter.apply(best_strategy, *value);
                if candidate_strategy == best_strategy {
                    continue;
                }

                let tournament_result = run_pool_tournament(ruleset, candidate_strategy, opponent_pool, config)?;
                let win_rate = tournament_result.calculate_candidate_win_rate();
                evaluated_game_count += config.game_count as u64;

                let is_improvement = win_rate > best_win_rate + MIN_WIN_RATE_IMPROVEMENT;
                steps.push(TuningStep {
                    parameter_name: parameter.name(),
                    value: *value,
                    win_rate,
                    decisive_game_ratio: tournament_result.calculate_decisive_game_ratio(),
                    is_improvement,
                });

                if is_improvement {
                    best_strategy = candidate_strategy;
                    best_win_rate = win_rate;
                    has_improved = true;
                }
            }
        }

        if !has_improved {
            break;
        }
    }

    Ok(TuningReport {
        baseline_strategy,
        best_strategy,
        best_win_rate,
        baseline_win_rate,
        evaluated_game_count,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::data::{
        BASELINE_STRATEGY,
        DEX_OPTIMAL_STRATEGY,
        NEVER_TRADING_STRATEGY,
    };

    #[test]
    fn validation_uses_disjoint_seeds_and_the_same_pool_for_both_strategies() {
        let config = TournamentConfig {
            game_count: 8,
            max_turn_count: 100,
            seed: u64::MAX - 3,
            player_count: 4,
        };
        let session = tune_strategy_over_generations(&DEX_RULESET, &config, 1, 1, &[BASELINE_STRATEGY]).unwrap();
        let validation = &session.validation;
        let training_seeds: Vec<_> = (0..config.game_count).map(|i| config.seed.wrapping_add(i as u64)).collect();
        assert!((0..validation.config.game_count).all(|i| !training_seeds.contains(&validation.config.seed.wrapping_add(i as u64))));
        assert_eq!(validation.opponent_pool, vec![BASELINE_STRATEGY]);
        assert_eq!(validation.champion_strategy, session.champion());
        assert_eq!(
            validation.champion,
            run_pool_tournament(&DEX_RULESET, session.champion(), &validation.opponent_pool, &validation.config).unwrap()
        );
        assert_eq!(
            validation.baseline,
            run_pool_tournament(&DEX_RULESET, BASELINE_STRATEGY, &validation.opponent_pool, &validation.config).unwrap()
        );
    }

    #[test]
    fn rejects_empty_tuning_sessions() {
        let config = TournamentConfig {
            game_count: 8,
            max_turn_count: 100,
            seed: 0,
            player_count: 4,
        };
        assert!(tune_strategy_over_generations(&DEX_RULESET, &config, 1, 1, &[]).is_err());
        assert!(tune_strategy_over_generations(&DEX_RULESET, &config, 1, 0, &[BASELINE_STRATEGY]).is_err());
    }

    #[test]
    fn tuned_strategy_tops_a_mixed_field_and_refusing_to_trade_loses() {
        let config = TournamentConfig {
            game_count: 6_000,
            max_turn_count: 3_000,
            seed: 5,
            player_count: 4,
        };

        let league = [BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY, NEVER_TRADING_STRATEGY];
        let entries = run_league(&DEX_RULESET, &league, &config).expect("4 players should be supported");

        assert_eq!(entries[0].strategy, DEX_OPTIMAL_STRATEGY, "the tuned strategy should top a mixed field");
        assert_eq!(entries[2].strategy, NEVER_TRADING_STRATEGY, "refusing to trade should finish last");
    }
}
