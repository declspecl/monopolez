use anyhow::{
    Result,
    bail,
};

use super::model::{
    SimulationConfig,
    SimulationSummary,
};
use crate::game::engine::turn::{
    GameOutcome,
    play_game,
};
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::any::AnyStrategy;

pub fn run_simulation(
    ruleset: &Ruleset,
    config: &SimulationConfig,
    player_count: usize,
) -> Result<SimulationSummary> {
    if config.strategy_kinds.is_empty() {
        bail!("at least one strategy is required");
    }

    match player_count {
        2 => Ok(simulate_games::<2>(ruleset, config)),
        3 => Ok(simulate_games::<3>(ruleset, config)),
        4 => Ok(simulate_games::<4>(ruleset, config)),
        5 => Ok(simulate_games::<5>(ruleset, config)),
        6 => Ok(simulate_games::<6>(ruleset, config)),
        7 => Ok(simulate_games::<7>(ruleset, config)),
        8 => Ok(simulate_games::<8>(ruleset, config)),
        _ => bail!("player count {player_count} is outside the supported range of 2 to 8"),
    }
}

fn simulate_games<const PLAYER_COUNT: usize>(
    ruleset: &Ruleset,
    config: &SimulationConfig,
) -> SimulationSummary {
    let mut summary = SimulationSummary::new(PLAYER_COUNT);

    for game_index in 0..config.game_count {
        let mut game_state = GameState::<PLAYER_COUNT>::create_starting_state(ruleset, config.seed.wrapping_add(game_index as u64));
        let mut strategies: [AnyStrategy; PLAYER_COUNT] = core::array::from_fn(|player_index| config.strategy_kinds[player_index % config.strategy_kinds.len()].create_strategy(config.cash_reserve));

        let game_summary = play_game(&mut game_state, ruleset, &mut strategies, config.max_turn_count);

        summary.game_count += 1;
        summary.total_turn_count += game_summary.turn_count as u64;

        match game_summary.outcome {
            GameOutcome::Winner(player_id) => summary.win_count_by_player_id[player_id as usize] += 1,
            GameOutcome::TurnLimitReached => summary.turn_limit_reached_count += 1,
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::model::StrategyKind;

    fn create_config(game_count: u32) -> SimulationConfig {
        SimulationConfig {
            game_count,
            max_turn_count: 2_000,
            seed: 7,
            cash_reserve: 100,
            strategy_kinds: vec![StrategyKind::Greedy],
        }
    }

    #[test]
    fn summarizes_every_simulated_game() {
        let summary = run_simulation(&Ruleset::default(), &create_config(25), 4).expect("4 players should be supported");
        let total_win_count: u32 = summary.win_count_by_player_id.iter().sum();

        assert_eq!(summary.game_count, 25);
        assert_eq!(total_win_count + summary.turn_limit_reached_count, 25);
        assert!(summary.calculate_average_turn_count() > 0.0);
    }

    #[test]
    fn finishes_most_games_when_trading_is_permitted() {
        let summary = run_simulation(&Ruleset::default(), &create_config(50), 4).expect("4 players should be supported");

        assert!(summary.calculate_decisive_game_ratio() > 0.9, "trading bots should finish nearly every game");
    }

    #[test]
    fn assigns_strategies_in_order_across_players() {
        let mut config = create_config(20);
        config.strategy_kinds = vec![StrategyKind::Greedy, StrategyKind::Cautious];

        let summary = run_simulation(&Ruleset::default(), &config, 4).expect("4 players should be supported");

        assert_eq!(summary.game_count, 20);
    }

    #[test]
    fn rejects_empty_strategy_lists() {
        let mut config = create_config(1);
        config.strategy_kinds.clear();

        assert!(run_simulation(&Ruleset::default(), &config, 4).is_err());
    }

    #[test]
    fn rejects_unsupported_player_counts() {
        assert!(run_simulation(&Ruleset::default(), &create_config(1), 9).is_err());
    }
}
