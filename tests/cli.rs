use std::process::Command;

fn run_json(arguments: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(arguments).output().expect("CLI should launch");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout must contain exactly one JSON document");
    assert_eq!(report["build"]["package_version"], env!("CARGO_PKG_VERSION"));
    assert!(report["build"]["rustc"].as_str().unwrap().starts_with("rustc "));
    assert!(!report["build"]["target"].as_str().unwrap().is_empty());
    report
}

#[test]
fn analysis_records_resolved_inputs_and_excludes_short_games() {
    let output = run_json(&[
        "--analyze",
        "--json",
        "--ruleset-name",
        "dex",
        "--game-count",
        "4",
        "--max-turn-count",
        "99",
        "--seed",
        "17",
        "--strategy-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/dex-optimal.json"),
    ]);
    assert_eq!(output["schema_version"], 1);
    assert_eq!(output["ruleset"]["starting_player_money"], 1800);
    assert_eq!(output["strategy"]["trade_offer_percent"], 400);
    assert_eq!(output["config"]["seed"], 17);
    assert_eq!(output["report"]["game_count"], 4);
    assert_eq!(output["report"]["snapshot_game_count"], 0);
}

#[test]
fn tuning_with_sweep_emits_one_document_including_validation() {
    let output = run_json(&["--tune", "--sweep", "--json", "--game-count", "4", "--max-turn-count", "10", "--tune-rounds", "1", "--seed", "17"]);
    assert_eq!(output["training_config"]["seed"], 17);
    assert_eq!(output["validation"]["config"]["seed"], 21);
    assert_eq!(output["validation"]["baseline"]["game_count"], 4);
    assert_eq!(output["validation"]["champion"]["game_count"], 4);
    assert_eq!(output["validation"]["pairing"], "same_seed_seat_and_opponent_lineup");
    assert_eq!(output["validation"]["neither_win_count"], 4);
    assert_eq!(output["validation"]["champion_only_win_count"], 0);
    assert_eq!(output["validation"]["baseline_only_win_count"], 0);
    assert_eq!(output["validation"]["both_win_count"], 0);
    for candidate in ["baseline", "champion"] {
        let interval = &output["validation"][candidate]["candidate_win_rate_interval"];
        assert_eq!(interval["sample_count"], 4);
        assert_eq!(interval["method"], "wilson_binomial_approximation");
        assert_eq!(interval["confidence_level"], 0.95);
        assert!(interval["upper"].as_f64().unwrap() > 0.0);
    }
    assert!(!output["sweep"].as_array().unwrap().is_empty());
    assert_eq!(output["starting_pool"].as_array().unwrap().len(), 1);
}

#[test]
fn simulation_records_strategy_order_and_rule_overrides() {
    let output = run_json(&["--json", "--game-count", "4", "--strategies", "cautious,greedy", "--go-landing-salary", "450"]);
    assert_eq!(output["ruleset"]["go_landing_salary"], 450);
    assert_eq!(output["config"]["strategy_kinds"], serde_json::json!(["Cautious", "Greedy"]));
    assert_eq!(output["player_count"], 4);
    assert_eq!(output["summary"]["game_count"], 4);
}

#[test]
fn pool_tournament_records_candidate_and_opponents() {
    let output = run_json(&[
        "--vs-pool",
        "--json",
        "--game-count",
        "4",
        "--strategy-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/dex-optimal.json"),
    ]);
    assert_eq!(output["candidate"]["trade_offer_percent"], 400);
    assert_eq!(output["opponent_pool"][0]["trade_offer_percent"], 150);
    assert_eq!(output["result"]["game_count"], 4);
    assert_eq!(output["result"]["candidate_win_rate_interval"]["sample_count"], 4);
}

#[test]
fn pool_tournament_prints_interval_and_sample_semantics() {
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez"))
        .args(["--vs-pool", "--game-count", "4", "--max-turn-count", "1"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("approximate 95% Wilson interval"));
    assert!(text.contains("n=4 including unfinished games"));
}

#[test]
fn head_to_head_json_records_each_player_count() {
    let output = run_json(&["--head-to-head", "--json", "--game-count", "4", "--seed", "19"]);
    let entries = output["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 7);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry["config"]["player_count"], index + 2);
        assert_eq!(entry["config"]["seed"], 19);
        assert_eq!(entry["result"]["game_count"], 4);
        assert_eq!(entry["result"]["candidate_win_rate_interval"]["sample_count"], 4);
    }
}

#[test]
fn trace_round_trips_through_replay_cli() {
    let trace = run_json(&["--trace", "--ruleset-name", "dex", "--seed", "7", "--max-turn-count", "20"]);
    assert_eq!(trace["turn_states"].as_array().unwrap().len(), 21);
    assert!(!trace["decisions"].as_array().unwrap().is_empty());
    let path = std::env::temp_dir().join(format!("monopolez-trace-{}.json", std::process::id()));
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    serde_json::to_writer(file, &trace).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez"))
        .args(["--replay-file", path.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let replay: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(replay["verified"], true);
    assert_eq!(replay["turn_count"], 20);
    assert_eq!(trace["schema_version"], 5);
    assert_eq!(replay["events_verified"], true);
    assert_eq!(replay["event_count"], trace["events"].as_array().unwrap().len());
}

#[test]
fn grid_cli_records_the_requested_configuration_range() {
    let output = run_json(&[
        "--grid-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/building-grid.json"),
        "--league-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/benchmark-pool.json"),
        "--grid-start",
        "2",
        "--grid-count",
        "3",
        "--game-count",
        "4",
        "--max-turn-count",
        "10",
        "--strategy-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/dex-optimal.json"),
        "--json",
    ]);
    assert_eq!(output["purpose"], "configuration_screening");
    assert_eq!(output["configuration_count"], 6);
    assert_eq!(output["opponent_pool"].as_array().unwrap().len(), 3);
    assert_eq!(output["opponent_pool"][0]["trade_offer_percent"], 150);
    assert_eq!(output["opponent_pool"][1]["trade_offer_percent"], 400);
    assert_eq!(output["opponent_pool"][2]["trade_offer_percent"], 0);
    assert_eq!(output["range_start"], 2);
    assert_eq!(output["range_count"], 3);
    let entries = output["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    for (offset, entry) in entries.iter().enumerate() {
        assert_eq!(entry["configuration_id"], offset + 2);
        assert_eq!(entry["strategy"]["trade_offer_percent"], 400);
        assert_eq!(entry["result"]["game_count"], 4);
    }
    assert_eq!(entries[0]["strategy"]["building_allocation"], "Spread");
    assert_eq!(entries[0]["strategy"]["development_ceiling"], "Hotel");
    assert_eq!(entries[1]["strategy"]["building_allocation"], "Concentrate");
    assert_eq!(entries[1]["strategy"]["development_ceiling"], "ThreeHouses");
}

#[test]
fn grid_cli_requires_an_explicit_valid_range() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/building-grid.json");
    for arguments in [vec!["--grid-file", path], vec!["--grid-file", path, "--grid-count", "7"], vec!["--grid-count", "1"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(arguments).output().unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn branching_cli_reports_legal_alternatives_and_diagnostic_scope() {
    let output = run_json(&[
        "--branch-management-at",
        "0",
        "--ruleset-name",
        "dex",
        "--seed",
        "3",
        "--max-turn-count",
        "1000",
        "--strategy-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/dex-optimal.json"),
        "--json",
    ]);
    assert_eq!(output["mode"], "diagnostic_exact_hidden_state");
    assert_eq!(output["turn_limit_scope"], "total_game_turns_including_prefix");
    assert_eq!(output["report"]["decision_index"], 0);
    let branches = output["report"]["branches"].as_array().unwrap();
    assert!(branches.len() >= 2);
    assert!(branches[0]["action"].is_null());
    for branch in branches {
        assert!(branch["completed_turn_count"].as_u64().unwrap() <= 1000);
        assert!(branch["final_state"].is_object());
    }
    let rejected = Command::new(env!("CARGO_BIN_EXE_monopolez"))
        .args(["--branch-management-at", "0", "--game-count", "2"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
}

#[test]
fn jail_branching_cli_uses_separate_decision_indices() {
    let output = run_json(&["--branch-jail-at", "0", "--seed", "3", "--max-turn-count", "1000", "--json"]);
    assert_eq!(output["decision_index_scope"], "jail_decisions");
    assert_eq!(output["report"]["phase"], "Jail");
    let branches = output["report"]["branches"].as_array().unwrap();
    assert!(branches.iter().any(|branch| branch["action"] == "RollForDoubles"));
    assert!(branches.iter().all(|branch| branch["action"].is_string()));
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez"))
        .args(["--branch-jail-at", "0", "--branch-management-at", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn grid_checkpoint_resumes_and_recovers_an_interrupted_tail() {
    let path = std::env::temp_dir().join(format!("monopolez-grid-checkpoint-{}.jsonl", std::process::id()));
    assert!(!path.exists());
    let args = [
        "--grid-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/building-grid.json"),
        "--grid-count",
        "6",
        "--game-count",
        "4",
        "--max-turn-count",
        "10",
        "--json",
        "--grid-checkpoint",
        path.to_str().unwrap(),
    ];
    let original_report = run_json(&args);
    let original_bytes = std::fs::read(&path).unwrap();
    assert_eq!(original_bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).count(), 7);
    assert_eq!(run_json(&args), original_report);
    assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
    let mut interrupted = original_bytes.split_inclusive(|byte| *byte == b'\n').take(2).flatten().copied().collect::<Vec<_>>();
    interrupted.extend_from_slice(b"{\"configuration_id\":");
    std::fs::write(&path, interrupted).unwrap();
    assert_eq!(run_json(&args), original_report);
    assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
    let mismatched = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(args).args(["--seed", "17"]).output().unwrap();
    assert!(!mismatched.status.success());
    assert!(String::from_utf8_lossy(&mismatched.stderr).contains("checkpoint inputs or executable differ"));
    assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
    let mut records: Vec<serde_json::Value> = original_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    records[1]["configuration_id"] = serde_json::json!(5);
    let mut corrupt = records
        .iter()
        .flat_map(|record| {
            let mut bytes = serde_json::to_vec(record).unwrap();
            bytes.push(b'\n');
            bytes
        })
        .collect::<Vec<_>>();
    corrupt.extend_from_slice(b"{unfinished");
    std::fs::write(&path, &corrupt).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(args).output().unwrap();
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn paired_comparison_cli_preserves_inputs_and_counts_unfinished_games() {
    let output = run_json(&[
        "--vs-pool",
        "--compare-strategy-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/dex-optimal.json"),
        "--league-file",
        concat!(env!("CARGO_MANIFEST_DIR"), "/strategies/benchmark-pool.json"),
        "--game-count",
        "4",
        "--max-turn-count",
        "1",
        "--seed",
        "17",
        "--json",
    ]);
    assert_eq!(output["mode"], "paired_policy_comparison");
    assert_eq!(output["baseline"]["trade_offer_percent"], 400);
    assert_eq!(output["candidate"]["trade_offer_percent"], 150);
    assert_eq!(output["config"]["seed"], 17);
    assert_eq!(output["pairing"], "same_seed_seat_and_opponent_lineup");
    assert_eq!(output["result"]["neither_win_count"], 4);
    assert_eq!(output["result"]["candidate"]["game_count"], 4);
    assert_eq!(output["result"]["baseline"]["game_count"], 4);
    assert_eq!(output["win_rate_difference"], 0.0);
}

#[test]
fn trace_rejects_batch_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(["--trace", "--game-count", "20"]).output().unwrap();
    assert!(!output.status.success());
}
