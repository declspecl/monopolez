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
    assert_eq!(trace["schema_version"], 4);
    assert_eq!(replay["events_verified"], true);
    assert_eq!(replay["event_count"], trace["events"].as_array().unwrap().len());
}

#[test]
fn trace_rejects_batch_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(["--trace", "--game-count", "20"]).output().unwrap();
    assert!(!output.status.success());
}
