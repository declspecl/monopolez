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
    RulesetKind,
    SimulationConfig,
    SimulationSummary,
    StrategyKind,
};
use crate::simulation::runner::run_simulation;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::simulation::tournament::{
    TournamentConfig,
    run_pool_tournament,
    run_tournament,
};
use crate::simulation::tuner::{
    TuningReport,
    run_league,
    sweep_parameters,
    tune_strategy_over_generations,
};

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

    #[arg(long)]
    go_landing_salary: Option<Money>,

    #[arg(long)]
    free_parking_jackpot: Option<bool>,

    #[arg(long, value_enum, default_value_t = RulesetKind::Official)]
    ruleset_name: RulesetKind,

    #[arg(long)]
    tune: bool,

    #[arg(long, default_value_t = 3)]
    tune_rounds: u32,

    #[arg(long, default_value_t = 1)]
    tune_generations: u32,

    #[arg(long)]
    sweep: bool,

    #[arg(long)]
    strategy_file: Option<PathBuf>,

    #[arg(long)]
    head_to_head: bool,

    #[arg(long)]
    league_file: Option<PathBuf>,

    #[arg(long)]
    tune_pool_file: Option<PathBuf>,

    #[arg(long)]
    vs_pool: bool,

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

    if arguments.vs_pool {
        let candidate = load_candidate_strategy(&arguments)?;
        let opponent_pool = load_strategy_pool(&arguments)?;

        let tournament_config = TournamentConfig {
            game_count: arguments.game_count,
            max_turn_count: arguments.max_turn_count,
            seed: arguments.seed,
            player_count: arguments.player_count,
        };

        let result = run_pool_tournament(&ruleset, candidate, &opponent_pool, &tournament_config)?;
        if arguments.json {
            println!("{}", serde_json::to_string(&result)?);
        } else {
            println!("win {:.2}%  decisive {:.1}%  turns {:.0}", result.calculate_candidate_win_rate() * 100.0, result.calculate_decisive_game_ratio() * 100.0, result.calculate_average_turn_count());
        }

        return Ok(());
    }

    if let Some(league_file) = &arguments.league_file {
        let league_json = fs::read_to_string(league_file).with_context(|| format!("failed to read league file {}", league_file.display()))?;
        let strategies: Vec<ConfigurableStrategy> = serde_json::from_str(&league_json).with_context(|| format!("failed to parse league file {}", league_file.display()))?;

        let tournament_config = TournamentConfig {
            game_count: arguments.game_count,
            max_turn_count: arguments.max_turn_count,
            seed: arguments.seed,
            player_count: arguments.player_count,
        };

        let entries = run_league(&ruleset, &strategies, &tournament_config)?;
        if arguments.json {
            println!("{}", serde_json::to_string_pretty(&entries)?);

            return Ok(());
        }

        println!("win rate  decisive  turns  reserve  buy%  bid%  build%  unmort%  offer%  accept%  bail");
        for entry in &entries {
            let strategy = entry.strategy;

            println!(
                "{:>7.2}%  {:>7.1}%  {:>5.0}  {:>7}  {:>4}  {:>4}  {:>6}  {:>7}  {:>6}  {:>7}  {:>4}",
                entry.win_rate * 100.0,
                entry.decisive_game_ratio * 100.0,
                entry.average_turn_count,
                strategy.cash_reserve,
                strategy.purchase_cash_percent,
                strategy.auction_bid_percent,
                strategy.improvement_cash_percent,
                strategy.unmortgage_cash_percent,
                strategy.trade_offer_percent,
                strategy.trade_accept_percent,
                u8::from(strategy.pays_bail_when_affordable)
            );
        }

        return Ok(());
    }

    if arguments.head_to_head {
        let candidate = load_candidate_strategy(&arguments)?;
        let baseline = ConfigurableStrategy::new();

        println!("{candidate:#?}");
        println!();
        println!("players  win rate  decisive  avg turns");

        for player_count in 2..=8 {
            let tournament_config = TournamentConfig {
                game_count: arguments.game_count,
                max_turn_count: arguments.max_turn_count,
                seed: arguments.seed,
                player_count,
            };

            let result = run_tournament(&ruleset, candidate, baseline, &tournament_config)?;
            let fair_share = 100.0 / player_count as f64;

            println!(
                "{player_count:<8} {:>6.2}% (fair {fair_share:.1}%)  {:>5.1}%  {:>8.1}",
                result.calculate_candidate_win_rate() * 100.0,
                result.calculate_decisive_game_ratio() * 100.0,
                result.calculate_average_turn_count()
            );
        }

        return Ok(());
    }

    if arguments.tune {
        let tournament_config = TournamentConfig {
            game_count: arguments.game_count,
            max_turn_count: arguments.max_turn_count,
            seed: arguments.seed,
            player_count: arguments.player_count,
        };

        let starting_pool = match &arguments.tune_pool_file {
            Some(tune_pool_file) => {
                let pool_json = fs::read_to_string(tune_pool_file).with_context(|| format!("failed to read pool file {}", tune_pool_file.display()))?;

                serde_json::from_str::<Vec<ConfigurableStrategy>>(&pool_json).with_context(|| format!("failed to parse pool file {}", tune_pool_file.display()))?
            },
            None => vec![ConfigurableStrategy::new()],
        };

        let session = tune_strategy_over_generations(&ruleset, &tournament_config, arguments.tune_rounds, arguments.tune_generations, &starting_pool)?;
        if arguments.json {
            println!("{}", serde_json::to_string_pretty(&session)?);
        } else {
            for (generation_index, report) in session.reports.iter().enumerate() {
                println!("=== generation {generation_index}");
                print_tuning_report(report);
                println!();
            }
        }

        if arguments.sweep {
            let champion = session.champion();
            let steps = sweep_parameters(&ruleset, champion, &session.opponent_pool, &tournament_config)?;

            println!("=== sensitivity sweep of the champion against the opponent pool");
            for step in &steps {
                println!("{:<26} {:<6} win {:.2}%  decisive {:.1}%", step.parameter_name, step.value, step.win_rate * 100.0, step.decisive_game_ratio * 100.0);
            }
        }

        return Ok(());
    }

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

fn load_strategy_pool(arguments: &CliArguments) -> Result<Vec<ConfigurableStrategy>> {
    let Some(league_file) = &arguments.league_file else {
        return Ok(vec![ConfigurableStrategy::new()]);
    };

    let league_json = fs::read_to_string(league_file).with_context(|| format!("failed to read league file {}", league_file.display()))?;

    serde_json::from_str(&league_json).with_context(|| format!("failed to parse league file {}", league_file.display()))
}

fn load_candidate_strategy(arguments: &CliArguments) -> Result<ConfigurableStrategy> {
    let Some(strategy_file) = &arguments.strategy_file else {
        return Ok(ConfigurableStrategy::new());
    };

    let strategy_json = fs::read_to_string(strategy_file).with_context(|| format!("failed to read strategy file {}", strategy_file.display()))?;

    serde_json::from_str(&strategy_json).with_context(|| format!("failed to parse strategy file {}", strategy_file.display()))
}

fn build_ruleset(arguments: &CliArguments) -> Ruleset {
    let mut ruleset = arguments.ruleset_name.to_ruleset();

    if let Some(go_landing_salary) = arguments.go_landing_salary {
        ruleset.go_landing_salary = go_landing_salary;
    }

    if let Some(free_parking_jackpot) = arguments.free_parking_jackpot {
        ruleset.free_parking_jackpot_mode = if free_parking_jackpot {
            FreeParkingJackpotMode::TaxesAndFees
        } else {
            FreeParkingJackpotMode::Disabled
        };
    }

    ruleset
}

fn print_tuning_report(report: &TuningReport) {
    println!("baseline win rate   {:.2}%", report.baseline_win_rate * 100.0);
    println!("tuned win rate      {:.2}%", report.best_win_rate * 100.0);
    println!("games evaluated     {}", report.evaluated_game_count);
    println!();
    println!("{:#?}", report.best_strategy);
    println!();

    for step in report.steps.iter().filter(|step| step.is_improvement) {
        println!("improved {:<26} -> {:<5} win {:.2}%  decisive {:.1}%", step.parameter_name, step.value, step.win_rate * 100.0, step.decisive_game_ratio * 100.0);
    }
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
