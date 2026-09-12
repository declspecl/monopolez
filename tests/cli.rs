use std::process::Command;

fn run_json(arguments: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_monopolez")).args(arguments).output().expect("CLI should launch");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("stdout must contain exactly one JSON document")
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
