use anyhow::Result;
use serde::Serialize;

use super::tournament::{
    TournamentConfig,
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

pub fn tune_strategy_over_generations(
    ruleset: &Ruleset,
    config: &TournamentConfig,
    round_count: u32,
    generation_count: u32,
) -> Result<Vec<TuningReport>> {
    let mut opponent_pool = vec![ConfigurableStrategy::new()];
    let mut reports = Vec::new();

    for _generation in 0..generation_count {
        let report = tune_strategy(ruleset, config, round_count, &opponent_pool)?;
        opponent_pool.push(report.best_strategy);
        reports.push(report);
    }

    Ok(reports)
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
    let baseline_strategy = *opponent_pool.last().expect("opponent pool must not be empty");
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
