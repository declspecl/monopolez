mod game;
mod simulation;

use anyhow::Result;
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

    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let arguments = CliArguments::parse();

    let ruleset = build_ruleset(&arguments);
    let config = SimulationConfig {
        game_count: arguments.game_count,
        max_turn_count: arguments.max_turn_count,
        seed: arguments.seed,
        cash_reserve: arguments.cash_reserve,
    };

    let summary = run_simulation(&ruleset, &config, arguments.player_count)?;

    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary);
    }

    Ok(())
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

fn print_summary(summary: &SimulationSummary) {
    println!("games           {}", summary.game_count);
    println!("decisive        {:.1}%", summary.calculate_decisive_game_ratio() * 100.0);
    println!("average turns   {:.1}", summary.calculate_average_turn_count());

    for (player_id, win_count) in summary.win_count_by_player_id.iter().enumerate() {
        println!("player {player_id} wins    {win_count}");
    }
}
