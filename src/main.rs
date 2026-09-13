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
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::tile::model::{
    Cash,
    Money,
};
use crate::simulation::analysis::{
    MONOPOLY_COUNT_BUCKET_COUNT,
    MonopolyReport,
    analyze_monopolies,
};
use crate::simulation::model::{
    RulesetKind,
    SimulationConfig,
    SimulationSummary,
    StrategyKind,
};
use crate::simulation::provenance::BuildProvenance;
use crate::simulation::runner::run_simulation;
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

    #[arg(long, requires = "grid_count", help = "JSON object mapping strategy fields to arrays of values; league-file supplies opponents", conflicts_with_all = ["trace", "replay_file", "analyze", "tune", "sweep", "head_to_head", "vs_pool", "print_ruleset", "strategies", "tune_pool_file", "tune_rounds", "tune_generations", "cash_reserve"])]
    grid_file: Option<PathBuf>,

    #[arg(long, requires = "grid_file", default_value_t = 0)]
    grid_start: u64,

    #[arg(long, requires = "grid_file")]
    grid_count: Option<u64>,

    #[arg(long, help = "Branch the zero-based management decision with legal asset actions; diagnostic exact hidden-state rollouts", conflicts_with_all = ["grid_file", "trace", "replay_file", "analyze", "tune", "sweep", "head_to_head", "vs_pool", "print_ruleset", "strategies", "tune_pool_file", "tune_rounds", "tune_generations", "cash_reserve", "game_count"])]
    branch_management_at: Option<u64>,

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

    #[arg(long)]
    analyze: bool,

    #[arg(long, value_delimiter = ',', default_value = "greedy")]
    strategies: Vec<StrategyKind>,

    #[arg(long)]
    ruleset_file: Option<PathBuf>,

    #[arg(long)]
    print_ruleset: bool,

    #[arg(long)]
    json: bool,

    #[arg(long, conflicts_with_all = ["replay_file", "analyze", "tune", "head_to_head", "vs_pool", "print_ruleset", "strategies", "game_count", "cash_reserve", "sweep", "tune_pool_file"])]
    trace: bool,

    #[arg(long, conflicts_with_all = ["analyze", "tune", "head_to_head", "vs_pool", "print_ruleset", "strategy_file", "league_file", "ruleset_file", "ruleset_name", "seed", "player_count", "max_turn_count", "game_count", "strategies", "go_landing_salary", "free_parking_jackpot", "cash_reserve"])]
    replay_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let arguments = CliArguments::parse();

    if let Some(path) = &arguments.replay_file {
        let input = fs::read_to_string(path).with_context(|| format!("failed to read trace {}", path.display()))?;
        let trace: crate::simulation::trace::GameTrace = serde_json::from_str(&input).context("failed to parse trace")?;
        crate::simulation::trace::replay_game(&trace)?;
        if arguments.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "build": BuildProvenance::current(),
                    "verified": true, "turn_count": trace.turn_states.len() - 1,
                    "decision_count": trace.decisions.len(), "winner": trace.winner,
                    "event_count": trace.events.len(), "events_verified": trace.schema_version >= 2
                }))?
            );
        } else {
            println!(
                "verified {} turns and {} decisions ({} recorded events)",
                trace.turn_states.len() - 1,
                trace.decisions.len(),
                trace.events.len()
            );
        }
        return Ok(());
    }

    let ruleset = load_ruleset(&arguments)?;
    if let Some(decision_index) = arguments.branch_management_at {
        anyhow::ensure!((2..=8).contains(&arguments.player_count), "branching requires 2 to 8 players");
        let policies = if arguments.league_file.is_some() {
            anyhow::ensure!(arguments.strategy_file.is_none(), "branching accepts either a strategy file or a league file");
            let policies = load_strategy_pool(&arguments)?;
            anyhow::ensure!(policies.len() == arguments.player_count, "branching league must contain exactly player-count strategies");
            policies
        } else {
            vec![load_candidate_strategy(&arguments)?; arguments.player_count]
        };
        let report = simulation::branching::branch_management(&ruleset, &policies, arguments.seed, arguments.max_turn_count, decision_index)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1, "build": BuildProvenance::current(), "mode": "diagnostic_exact_hidden_state",
                "ruleset": ruleset, "strategies": policies, "seed": arguments.seed,
                "max_turn_count": arguments.max_turn_count, "turn_limit_scope": "total_game_turns_including_prefix",
                "decision_index_scope": "management_decisions_with_at_least_one_legal_asset_action", "report": report
            }))?
        );
        return Ok(());
    }
    if let Some(path) = &arguments.grid_file {
        let input = fs::read_to_string(path).with_context(|| format!("failed to read grid {}", path.display()))?;
        let axes: simulation::grid::StrategyAxes = serde_json::from_str(&input).context("failed to parse strategy grid")?;
        let baseline = load_candidate_strategy(&arguments)?;
        let opponents = load_strategy_pool(&arguments)?;
        let grid = simulation::grid::StrategyGrid::new(baseline, axes.clone())?;
        let config = TournamentConfig {
            game_count: arguments.game_count,
            max_turn_count: arguments.max_turn_count,
            seed: arguments.seed,
            player_count: arguments.player_count,
        };
        let count = arguments.grid_count.context("grid requires an explicit configuration count")?;
        let entries = grid.run_range(&ruleset, &opponents, &config, arguments.grid_start, count)?;
        if arguments.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "purpose": "configuration_screening", "build": BuildProvenance::current(),
                    "ruleset": ruleset, "config": config, "baseline": baseline, "opponent_pool": opponents,
                    "axes": axes, "configuration_count": grid.configuration_count(),
                    "configuration_id_order": "axis_names_ascending_last_axis_changes_fastest",
                    "range_start": arguments.grid_start, "range_count": count, "entries": entries
                }))?
            );
        } else {
            println!(
                "{} of {} configurations, starting at {} (screening, not held-out evaluation)",
                count,
                grid.configuration_count(),
                arguments.grid_start
            );
            for entry in entries {
                println!(
                    "configuration {}  win {:.2}%  {:?}",
                    entry.configuration_id,
                    entry.result.calculate_candidate_win_rate() * 100.0,
                    entry.strategy
                );
                print_win_rate_interval("candidate", &entry.result);
            }
        }
        return Ok(());
    }
    if arguments.trace {
        let strategies = if arguments.league_file.is_some() {
            anyhow::ensure!(arguments.strategy_file.is_none(), "trace accepts either a strategy file or a league file");
            let strategies = load_strategy_pool(&arguments)?;
            anyhow::ensure!(strategies.len() == arguments.player_count, "trace league must contain exactly player-count strategies in seat order");
            strategies
        } else {
            anyhow::ensure!((2..=8).contains(&arguments.player_count), "trace requires 2 to 8 players");
            vec![load_candidate_strategy(&arguments)?; arguments.player_count]
        };
        let trace = crate::simulation::trace::record_game(ruleset, strategies, arguments.seed, arguments.max_turn_count)?;
        println!("{}", serde_json::to_string_pretty(&trace)?);
        return Ok(());
    }
    let config = SimulationConfig {
        game_count: arguments.game_count,
        max_turn_count: arguments.max_turn_count,
        seed: arguments.seed,
        cash_reserve: arguments.cash_reserve,
        strategy_kinds: arguments.strategies.clone(),
    };

    if arguments.analyze {
        let strategy = load_candidate_strategy(&arguments)?;
        let tournament_config = TournamentConfig {
            game_count: arguments.game_count,
            max_turn_count: arguments.max_turn_count,
            seed: arguments.seed,
            player_count: arguments.player_count,
        };

        let report = analyze_monopolies(&ruleset, strategy, &tournament_config)?;
        if arguments.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1,
                    "build": BuildProvenance::current(),
                    "ruleset": ruleset,
                    "config": tournament_config,
                    "strategy": strategy,
                    "snapshot": {
                        "turn": crate::simulation::analysis::MONOPOLY_SNAPSHOT_TURN,
                        "turn_unit": "individual_player_turn",
                        "population": "all_original_seats_in_games_reaching_snapshot",
                        "development_unit": "improvement_level_sum_hotel_counts_as_five",
                        "ownership": "all_complete_groups_including_railroads_utilities_and_mortgaged_tiles"
                    },
                    "report": report
                }))?
            );
        } else {
            print_monopoly_report(&report);
        }

        return Ok(());
    }

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
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "ruleset": ruleset, "config": tournament_config,
                    "candidate": candidate, "opponent_pool": opponent_pool, "result": result,
                    "build": BuildProvenance::current()
                }))?
            );
        } else {
            println!(
                "win {:.2}%  decisive {:.1}%  turns {:.0}",
                result.calculate_candidate_win_rate() * 100.0,
                result.calculate_decisive_game_ratio() * 100.0,
                result.calculate_average_turn_count()
            );
            print_win_rate_interval("candidate", &result);
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
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "ruleset": ruleset, "config": tournament_config,
                    "opponent_pool": strategies, "entries": entries,
                    "build": BuildProvenance::current()
                }))?
            );

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

        if !arguments.json {
            println!("{candidate:#?}");
            println!();
            println!("players  win rate  decisive  avg turns");
        }
        let mut entries = Vec::new();

        for player_count in 2..=8 {
            let tournament_config = TournamentConfig {
                game_count: arguments.game_count,
                max_turn_count: arguments.max_turn_count,
                seed: arguments.seed,
                player_count,
            };

            let result = run_tournament(&ruleset, candidate, baseline, &tournament_config)?;
            if arguments.json {
                entries.push(serde_json::json!({"config": tournament_config, "result": result}));
                continue;
            }
            let fair_share = 100.0 / player_count as f64;

            println!(
                "{player_count:<8} {:>6.2}% (fair {fair_share:.1}%)  {:>5.1}%  {:>8.1}",
                result.calculate_candidate_win_rate() * 100.0,
                result.calculate_decisive_game_ratio() * 100.0,
                result.calculate_average_turn_count()
            );
            print_win_rate_interval("candidate", &result);
        }

        if arguments.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": 1, "ruleset": ruleset, "candidate": candidate,
                    "baseline": baseline, "entries": entries,
                    "build": BuildProvenance::current()
                }))?
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
        let sweep = if arguments.sweep {
            Some(sweep_parameters(&ruleset, session.champion(), &session.opponent_pool, &tournament_config)?)
        } else {
            None
        };
        if arguments.json {
            let mut output = serde_json::to_value(&session)?;
            output["sweep"] = serde_json::to_value(&sweep)?;
            output["build"] = serde_json::to_value(BuildProvenance::current())?;
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        } else {
            for (generation_index, report) in session.reports.iter().enumerate() {
                println!("=== generation {generation_index}");
                print_tuning_report(report);
                println!();
            }
            println!("held-out seed {}  games {} per strategy", session.validation.config.seed, session.validation.config.game_count);
            println!(
                "held-out baseline {:.2}%  champion {:.2}%",
                session.validation.baseline.calculate_candidate_win_rate() * 100.0,
                session.validation.champion.calculate_candidate_win_rate() * 100.0
            );
            print_win_rate_interval("held-out baseline", &session.validation.baseline);
            print_win_rate_interval("held-out champion", &session.validation.champion);
            println!("marginal intervals do not test the paired champion-minus-baseline difference");
        }

        if let Some(steps) = sweep {
            println!("=== sensitivity sweep of the champion against the opponent pool");
            for step in &steps {
                println!(
                    "{:<26} {:<6} win {:.2}%  decisive {:.1}%",
                    step.parameter_name,
                    step.value,
                    step.win_rate * 100.0,
                    step.decisive_game_ratio * 100.0
                );
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
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1, "ruleset": ruleset, "config": config,
                "player_count": arguments.player_count, "summary": summary,
                "build": BuildProvenance::current()
            }))?
        );
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

fn print_monopoly_report(report: &MonopolyReport) {
    const GROUP_NAMES: [&str; 10] = ["brown", "light blue", "pink", "orange", "red", "yellow", "green", "dark blue", "railroads", "utilities"];

    println!("games {}  decisive {:.1}%", report.game_count, report.decisive_game_count as f64 / report.game_count as f64 * 100.0);
    println!(
        "snapshot games {}  excluded {} (ended before 100 total player turns)",
        report.snapshot_game_count,
        report.game_count - report.snapshot_game_count
    );
    println!();
    println!("group        completed  first holder wins  avg completion turn");

    for (group_index, group_name) in GROUP_NAMES.iter().enumerate() {
        let entry = report.group_entries[group_index];
        if entry.first_completion_count == 0 {
            println!("{group_name:<12} {:>9}  {:>16}  {:>19}", 0, "-", "-");

            continue;
        }

        println!(
            "{group_name:<12} {:>8.1}%  {:>15.1}%  {:>19.1}",
            entry.first_completion_count as f64 / report.game_count as f64 * 100.0,
            entry.first_completion_win_count as f64 / entry.first_completion_count as f64 * 100.0,
            entry.total_completion_turn as f64 / entry.first_completion_count as f64
        );
    }

    println!();
    println!("sole monopoly at turn 100   players  win rate");

    for (group_index, group_name) in GROUP_NAMES.iter().enumerate() {
        let entry = report.sole_monopoly_entries[group_index];
        if entry.player_count == 0 {
            continue;
        }

        println!("{group_name:<27} {:>7}  {:>7.1}%", entry.player_count, entry.win_count as f64 / entry.player_count as f64 * 100.0);
    }

    println!();
    println!("sole monopoly development (hotel = 5 levels)    players  win rate");

    const DEVELOPMENT_LABELS: [&str; 4] = ["none", "1-4 levels", "5-9 levels", "10+ levels"];
    for (bucket_index, label) in DEVELOPMENT_LABELS.iter().enumerate() {
        let entry = report.sole_monopoly_development_entries[bucket_index];
        if entry.player_count == 0 {
            continue;
        }

        println!("{label:<25} {:>8}  {:>7.1}%", entry.player_count, entry.win_count as f64 / entry.player_count as f64 * 100.0);
    }

    println!();
    println!("monopolies at turn 100   players  win rate");

    for (bucket_index, entry) in report.monopoly_count_entries.iter().enumerate() {
        if entry.player_count == 0 {
            continue;
        }

        let label = if bucket_index == MONOPOLY_COUNT_BUCKET_COUNT - 1 {
            format!("{bucket_index}+")
        } else {
            bucket_index.to_string()
        };

        println!("{label:<24} {:>7}  {:>7.1}%", entry.player_count, entry.win_count as f64 / entry.player_count as f64 * 100.0);
    }
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

fn print_win_rate_interval(
    label: &str,
    result: &simulation::tournament::TournamentResult,
) {
    match result.calculate_candidate_win_rate_interval() {
        Some(interval) => println!(
            "{label} approximate 95% Wilson interval [{:.2}%, {:.2}%]  n={} including unfinished games",
            interval.lower * 100.0,
            interval.upper * 100.0,
            interval.sample_count
        ),
        None => println!("{label} win-rate interval unavailable"),
    }
}

fn print_tuning_report(report: &TuningReport) {
    println!("baseline win rate   {:.2}%", report.baseline_win_rate * 100.0);
    println!("tuned win rate      {:.2}%", report.best_win_rate * 100.0);
    println!("games evaluated     {}", report.evaluated_game_count);
    println!();
    println!("{:#?}", report.best_strategy);
    println!();

    for step in report.steps.iter().filter(|step| step.is_improvement) {
        println!(
            "improved {:<26} -> {:<5} win {:.2}%  decisive {:.1}%",
            step.parameter_name,
            step.value,
            step.win_rate * 100.0,
            step.decisive_game_ratio * 100.0
        );
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
