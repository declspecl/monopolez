mod game;
mod simulation;

use std::fs;
use std::path::PathBuf;

use anyhow::{
    Context,
    Result,
};
use clap::Parser;

use crate::game::ruleset::model::{
    FreeParkingJackpotMode,
    Ruleset,
};
use crate::game::tile::model::{
    Cash,
    Money,
};
use crate::simulation::model::{
    SimulationConfig,
    SimulationSummary,
    StrategyKind,
};
use crate::simulation::runner::run_simulation;

#[derive(Debug, Parser)]
#[command(name = "monopolez", about = "high performance monopoly simulator")]
struct CliArguments {
    #[arg(long, default_value_t = 1_000)]
    game_count: u32,

    #[arg(long, default_value_t = 4)]
    player_count: usize,

    #[arg(long, default_value_t = 10_000)]
    max_turn_count: u32,

    #[arg(long, default_value_t = 0)]
    seed: u64,

    #[arg(long, default_value_t = 100)]
    cash_reserve: Cash,

    #[arg(long, default_value_t = 200)]
    go_landing_salary: Money,

    #[arg(long)]
    free_parking_jackpot: bool,

    #[arg(long, value_delimiter = ',', default_value = "greedy")]
    strategies: Vec<StrategyKind>,

    #[arg(long)]
    ruleset_file: Option<PathBuf>,

    #[arg(long)]
    print_ruleset: bool,

    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let arguments = CliArguments::parse();

    let ruleset = load_ruleset(&arguments)?;
    let config = SimulationConfig {
        game_count: arguments.game_count,
        max_turn_count: arguments.max_turn_count,
        seed: arguments.seed,
        cash_reserve: arguments.cash_reserve,
        strategy_kinds: arguments.strategies.clone(),
    };

    if arguments.print_ruleset {
        println!("{}", serde_json::to_string_pretty(&ruleset)?);

        return Ok(());
    }

    let summary = run_simulation(&ruleset, &config, arguments.player_count)?;

    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary, &arguments.strategies);
    }

    Ok(())
}

fn load_ruleset(arguments: &CliArguments) -> Result<Ruleset> {
    let Some(ruleset_file) = &arguments.ruleset_file else {
        return Ok(build_ruleset(arguments));
    };

    let ruleset_json = fs::read_to_string(ruleset_file).with_context(|| format!("failed to read ruleset file {}", ruleset_file.display()))?;

    serde_json::from_str(&ruleset_json).with_context(|| format!("failed to parse ruleset file {}", ruleset_file.display()))
}

fn build_ruleset(arguments: &CliArguments) -> Ruleset {
    let free_parking_jackpot_mode = if arguments.free_parking_jackpot {
        FreeParkingJackpotMode::TaxesAndFees
    } else {
        FreeParkingJackpotMode::Disabled
    };

    Ruleset::builder()
        .with_go_landing_salary(arguments.go_landing_salary)
        .with_free_parking_jackpot_mode(free_parking_jackpot_mode)
        .build()
}

fn print_summary(
    summary: &SimulationSummary,
    strategy_kinds: &[StrategyKind],
) {
    println!("games           {}", summary.game_count);
    println!("decisive        {:.1}%", summary.calculate_decisive_game_ratio() * 100.0);
    println!("average turns   {:.1}", summary.calculate_average_turn_count());

    for (player_id, win_count) in summary.win_count_by_player_id.iter().enumerate() {
        let strategy_kind = strategy_kinds[player_id % strategy_kinds.len()];

        println!("player {player_id} wins    {win_count} ({strategy_kind:?})");
    }
}
