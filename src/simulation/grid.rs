use std::collections::BTreeMap;

use anyhow::{
    Context,
    Result,
    ensure,
};
use serde::Serialize;
use serde_json::Value;

use super::tournament::{
    TournamentConfig,
    TournamentResult,
    run_pool_tournament,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::strategy::configurable::ConfigurableStrategy;

pub type StrategyAxes = BTreeMap<String, Vec<Value>>;

pub struct StrategyGrid {
    baseline: Value,
    axes: StrategyAxes,
    configuration_count: u64,
}

#[derive(Debug, Serialize)]
pub struct GridEntry {
    pub configuration_id: u64,
    pub strategy: ConfigurableStrategy,
    pub result: TournamentResult,
}

impl StrategyGrid {
    pub fn new(
        baseline: ConfigurableStrategy,
        axes: StrategyAxes,
    ) -> Result<Self> {
        let baseline = serde_json::to_value(baseline)?;
        let mut configuration_count = 1u64;
        for (name, values) in &axes {
            ensure!(baseline.get(name).is_some(), "unknown strategy axis {name}");
            ensure!(!values.is_empty(), "strategy axis {name} must not be empty");
            for (index, value) in values.iter().enumerate() {
                ensure!(!values[..index].contains(value), "duplicate value in strategy axis {name}");
                let mut candidate = baseline.clone();
                candidate[name] = value.clone();
                serde_json::from_value::<ConfigurableStrategy>(candidate).with_context(|| format!("invalid value {value} for strategy axis {name}"))?;
            }
            configuration_count = configuration_count.checked_mul(values.len() as u64).context("strategy grid configuration count overflows u64")?;
        }
        Ok(Self { baseline, axes, configuration_count })
    }

    pub const fn configuration_count(&self) -> u64 {
        self.configuration_count
    }

    pub fn strategy_at(
        &self,
        configuration_id: u64,
    ) -> Result<ConfigurableStrategy> {
        ensure!(configuration_id < self.configuration_count, "configuration id is outside the strategy grid");
        let mut remainder = configuration_id;
        let mut candidate = self.baseline.clone();
        for (name, values) in self.axes.iter().rev() {
            candidate[name] = values[(remainder % values.len() as u64) as usize].clone();
            remainder /= values.len() as u64;
        }
        Ok(serde_json::from_value(candidate)?)
    }

    pub fn run_range(
        &self,
        rules: &Ruleset,
        opponents: &[ConfigurableStrategy],
        config: &TournamentConfig,
        start: u64,
        count: u64,
    ) -> Result<Vec<GridEntry>> {
        ensure!(count > 0, "grid configuration count must be positive");
        let end = start.checked_add(count).context("grid range overflows u64")?;
        ensure!(end <= self.configuration_count, "grid range exceeds {} configurations", self.configuration_count);
        ensure!(config.game_count > 0 && config.max_turn_count > 0, "grid requires positive game and turn counts");
        (start..end)
            .map(|configuration_id| {
                let strategy = self.strategy_at(configuration_id)?;
                let result = run_pool_tournament(rules, strategy, opponents, config)?;
                Ok(GridEntry { configuration_id, strategy, result })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::configurable::{
        BuildingAllocation,
        DevelopmentCeiling,
    };

    fn axes() -> StrategyAxes {
        serde_json::from_value(serde_json::json!({
            "building_allocation": ["Spread", "Concentrate"],
            "development_ceiling": ["ThreeHouses", "FourHouses", "Hotel"],
            "jail_camping_unowned_tile_threshold": [null, 4],
            "mortgages_before_selling_buildings": [false, true]
        }))
        .unwrap()
    }

    #[test]
    fn enumerates_every_combination_once_without_changing_other_settings() {
        let baseline = ConfigurableStrategy {
            cash_reserve: 123,
            ..ConfigurableStrategy::new()
        };
        let grid = StrategyGrid::new(baseline, axes()).unwrap();
        assert_eq!(grid.configuration_count(), 24);
        let strategies: std::collections::HashSet<_> = (0..24).map(|id| grid.strategy_at(id).unwrap()).collect();
        assert_eq!(strategies.len(), 24);
        assert!(strategies.iter().all(|strategy| strategy.cash_reserve == 123));
        assert_eq!(grid.strategy_at(0).unwrap().building_allocation, BuildingAllocation::Spread);
        let last = grid.strategy_at(23).unwrap();
        assert_eq!(last.building_allocation, BuildingAllocation::Concentrate);
        assert_eq!(last.development_ceiling, DevelopmentCeiling::Hotel);
        assert_eq!(last.jail_camping_unowned_tile_threshold, Some(4));
        assert!(last.mortgages_before_selling_buildings);
        assert!(grid.strategy_at(24).is_err());
        let single = StrategyGrid::new(baseline, StrategyAxes::new()).unwrap();
        assert_eq!(single.configuration_count(), 1);
        assert_eq!(single.strategy_at(0).unwrap(), baseline);
    }

    #[test]
    fn rejects_unknown_empty_duplicate_and_mistyped_axes() {
        for value in [
            serde_json::json!({"typo": [1]}),
            serde_json::json!({"cash_reserve": []}),
            serde_json::json!({"cash_reserve": [1, 1]}),
            serde_json::json!({"cash_reserve": [-1]}),
            serde_json::json!({"building_allocation": ["Other"]}),
            serde_json::json!({"pays_bail_when_affordable": [1]}),
        ] {
            assert!(StrategyGrid::new(ConfigurableStrategy::new(), serde_json::from_value(value).unwrap()).is_err());
        }
    }

    #[test]
    fn partitioned_runs_equal_the_complete_grid() {
        let baseline = ConfigurableStrategy::new();
        let grid = StrategyGrid::new(baseline, axes()).unwrap();
        let config = TournamentConfig {
            game_count: 4,
            max_turn_count: 30,
            seed: 11,
            player_count: 4,
        };
        let all = grid.run_range(&DEX_RULESET, &[baseline], &config, 0, 24).unwrap();
        let mut parts = grid.run_range(&DEX_RULESET, &[baseline], &config, 0, 7).unwrap();
        parts.extend(grid.run_range(&DEX_RULESET, &[baseline], &config, 7, 17).unwrap());
        assert_eq!(serde_json::to_value(&all).unwrap(), serde_json::to_value(parts).unwrap());
        for entry in all {
            assert_eq!(entry.result, run_pool_tournament(&DEX_RULESET, entry.strategy, &[baseline], &config).unwrap());
        }
        assert!(grid.run_range(&DEX_RULESET, &[baseline], &config, 24, 1).is_err());
        assert!(grid.run_range(&DEX_RULESET, &[baseline], &config, u64::MAX, 1).is_err());
        assert!(grid.run_range(&DEX_RULESET, &[baseline], &config, 0, 0).is_err());
    }
}
