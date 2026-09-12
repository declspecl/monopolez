use clap::ValueEnum;
use serde::Serialize;

use crate::game::board::data::MAX_PLAYER_COUNT;
use crate::game::ruleset::data::{
    DEX_RULESET,
    OFFICIAL_RULESET,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::strategy::any::AnyStrategy;
use crate::game::strategy::cautious::CautiousStrategy;
use crate::game::strategy::greedy::GreedyStrategy;
use crate::game::tile::model::Cash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum RulesetKind {
    Official,
    Dex,
}

impl RulesetKind {
    pub const fn to_ruleset(self) -> Ruleset {
        match self {
            Self::Official => OFFICIAL_RULESET,
            Self::Dex => DEX_RULESET,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum StrategyKind {
    Greedy,
    Cautious,
}

impl StrategyKind {
    pub const fn create_strategy(
        self,
        cash_reserve: Cash,
    ) -> AnyStrategy {
        match self {
            Self::Greedy => AnyStrategy::Greedy(GreedyStrategy { cash_reserve }),
            Self::Cautious => AnyStrategy::Cautious(CautiousStrategy { cash_reserve }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimulationConfig {
    pub game_count: u32,
    pub max_turn_count: u32,
    pub seed: u64,
    pub cash_reserve: Cash,
    pub strategy_kinds: Vec<StrategyKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SimulationSummary {
    pub game_count: u32,
    pub win_count_by_player_id: Vec<u32>,
    pub turn_limit_reached_count: u32,
    pub total_turn_count: u64,
}

impl SimulationSummary {
    pub fn new(player_count: usize) -> Self {
        Self {
            game_count: 0,
            win_count_by_player_id: vec![0; player_count.min(MAX_PLAYER_COUNT)],
            turn_limit_reached_count: 0,
            total_turn_count: 0,
        }
    }

    pub fn calculate_average_turn_count(&self) -> f64 {
        if self.game_count == 0 {
            return 0.0;
        }

        self.total_turn_count as f64 / self.game_count as f64
    }

    pub fn calculate_decisive_game_ratio(&self) -> f64 {
        if self.game_count == 0 {
            return 0.0;
        }

        (self.game_count - self.turn_limit_reached_count) as f64 / self.game_count as f64
    }
}
