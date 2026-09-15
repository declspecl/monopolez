use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{
    Result,
    bail,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{
    Value,
    json,
};

use super::provenance::BuildProvenance;
use crate::game::board::model::PlayerId;
use crate::game::engine::event::GameEvent;
use crate::game::engine::turn::play_turn;
use crate::game::ruleset::model::Ruleset;
use crate::game::state::model::GameState;
use crate::game::strategy::configurable::ConfigurableStrategy;
use crate::game::strategy::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub player_id: PlayerId,
    pub request: Value,
    pub state: Value,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameTrace {
    pub schema_version: u32,
    pub build: Value,
    pub ruleset: Ruleset,
    pub strategies: Vec<ConfigurableStrategy>,
    pub seed: u64,
    pub max_turn_count: u32,
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub events: Vec<EventRecord>,
    pub turn_states: Vec<Value>,
    pub winner: Option<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub turn: u32,
    pub decisions_before: usize,
    pub event: GameEvent,
}

#[derive(Default)]
struct DecisionTape {
    decisions: Vec<Decision>,
    replay: bool,
    cursor: usize,
    error: Option<String>,
    events: Vec<EventRecord>,
    event_cursor: usize,
    turn: u32,
    event_version: u32,
}

struct RecordedStrategy {
    strategy: ConfigurableStrategy,
    tape: Rc<RefCell<DecisionTape>>,
}

macro_rules! record_decision {
    ($method:ident, $response:ty, $fallback:expr $(, $argument:ident: $argument_type:ty)*) => {
        fn $method<const PLAYER_COUNT: usize>(
            &mut self,
            game_state: &GameState<PLAYER_COUNT>,
            ruleset: &Ruleset,
            player_id: PlayerId,
            $($argument: $argument_type),*
        ) -> $response {
            let request = json!({"method": stringify!($method), $(stringify!($argument): $argument),*});
            let state = snapshot(game_state);
            let mut tape = self.tape.borrow_mut();
            if tape.error.is_some() {
                return $fallback;
            }
            if tape.replay {
                let index = tape.cursor;
                tape.cursor += 1;
                if let Some(decision) = tape.decisions.get(index)
                    && decision.player_id == player_id && decision.request == request && decision.state == state
                    && valid_response(&decision.request, &decision.response)
                    && let Ok(response) = serde_json::from_value::<$response>(decision.response.clone())
                {
                    return response;
                }
                tape.error = Some(format!("decision {index} does not match the replay request or state"));
                return $fallback;
            }
            let response = self.strategy.$method(game_state, ruleset, player_id, $($argument),*);
            tape.decisions.push(Decision { player_id, request, state, response: json!(response) });
            response
        }
    };
}

impl RecordedStrategy {
    record_decision!(choose_liquidation_action, Option<crate::game::engine::liquidation::LiquidationAction>, None, required_amount: Cash);
    record_decision!(counter_trade_offer, Option<TradeOffer>, None, rejected_offer: &TradeOffer);
}

impl PlayerStrategy for RecordedStrategy {
    fn observe_public_event(
        &mut self,
        event: GameEvent,
    ) {
        self.strategy.observe_public_event(event);
    }

    fn choose_liquidation_action<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        required_amount: Cash,
    ) -> Option<crate::game::engine::liquidation::LiquidationAction> {
        let tape = self.tape.borrow();
        if tape.error.is_some() {
            return None;
        }
        if tape.replay && tape.event_version < 4 {
            return crate::game::engine::liquidation::default_liquidation_action(state, rules, player);
        }
        drop(tape);
        RecordedStrategy::choose_liquidation_action(self, state, rules, player, required_amount)
    }

    fn counter_trade_offer<const N: usize>(
        &mut self,
        state: &GameState<N>,
        rules: &Ruleset,
        player: PlayerId,
        rejected_offer: &TradeOffer,
    ) -> Option<TradeOffer> {
        let tape = self.tape.borrow();
        if tape.error.is_some() || tape.replay && tape.event_version < 8 {
            return None;
        }
        drop(tape);
        RecordedStrategy::counter_trade_offer(self, state, rules, player, rejected_offer)
    }

    fn record_event(
        &mut self,
        event: GameEvent,
    ) {
        let mut tape = self.tape.borrow_mut();
        if tape.replay && event.trace_version() > tape.event_version {
            return;
        }
        if tape.error.is_some() {
            return;
        }
        if matches!(event, GameEvent::TurnStarted { .. }) {
            tape.turn += 1;
        }
        let record = EventRecord {
            turn: tape.turn,
            decisions_before: if tape.replay { tape.cursor } else { tape.decisions.len() },
            event,
        };
        if tape.replay {
            if tape.event_version >= 2 {
                let index = tape.event_cursor;
                if tape.events.get(index) != Some(&record) {
                    tape.error = Some(format!("event {index} differs from the recording"));
                }
                tape.event_cursor += 1;
            }
        } else {
            tape.events.push(record);
        }
    }

    record_decision!(should_purchase_property, bool, false, tile_id: TileId);
    record_decision!(choose_max_auction_bid, Cash, 0, tile_id: TileId);
    record_decision!(choose_jail_action, JailAction, JailAction::RollForDoubles);
    record_decision!(choose_property_to_improve, Option<PropertyId>, None);
    record_decision!(choose_tile_to_unmortgage, Option<TileId>, None);
    record_decision!(propose_trade, Option<TradeOffer>, None);
    record_decision!(should_accept_trade, bool, false, trade_offer: &TradeOffer);
    record_decision!(should_accept_counteroffer, bool, false, original_offer: &TradeOffer, counteroffer: &TradeOffer);
}

fn valid_response(
    request: &Value,
    response: &Value,
) -> bool {
    match request["method"].as_str() {
        Some("choose_tile_to_unmortgage") => response.is_null() || response.as_u64().is_some_and(|tile| tile < crate::game::tile::data::TILE_COUNT as u64),
        Some("choose_property_to_improve") => response.is_null() || response.as_u64().is_some_and(|property| property < crate::game::tile::data::PROPERTY_COUNT as u64),
        _ => true,
    }
}

pub(crate) fn snapshot<const N: usize>(state: &GameState<N>) -> Value {
    json!({
        "cash": state.cash_by_player_id.as_slice(),
        "positions": state.position_by_player_id.as_slice(),
        "jail_turns": state.jail_turn_count_by_player_id.as_slice(),
        "jailed_players": state.jailed_players,
        "bankrupt_players": state.bankrupt_players,
        "current_player": state.current_player_id,
        "consecutive_doubles": state.consecutive_double_count,
        "free_parking": state.free_parking_jackpot,
        "jail_cards": state.get_out_of_jail_free_card_holder_by_deck_kind,
        "decks": state.deck_state_by_deck_kind.iter().map(|deck| json!({
            "cards": deck.card_id_by_draw_position, "next": deck.next_draw_position
        })).collect::<Vec<_>>(),
        "rng": state.rng.state(),
        "owned_tiles": state.board.owned_tiles_by_player_id.as_slice(),
        "mortgaged_tiles": state.board.mortgaged_tiles,
        "improvements": state.board.improvement_level_by_property_id,
        "bank_houses": state.board.bank_house_count,
        "bank_hotels": state.board.bank_hotel_count
    })
}

pub fn record_game(
    ruleset: Ruleset,
    strategies: Vec<ConfigurableStrategy>,
    seed: u64,
    max_turn_count: u32,
) -> Result<GameTrace> {
    let mut trace = GameTrace {
        schema_version: 8,
        build: serde_json::to_value(BuildProvenance::current())?,
        ruleset,
        strategies,
        seed,
        max_turn_count,
        decisions: Vec::new(),
        events: Vec::new(),
        turn_states: Vec::new(),
        winner: None,
    };
    dispatch(&mut trace, false)?;
    Ok(trace)
}

pub fn replay_game(trace: &GameTrace) -> Result<()> {
    let mut replay = trace.clone();
    dispatch(&mut replay, true)
}

fn dispatch(
    trace: &mut GameTrace,
    replay: bool,
) -> Result<()> {
    if !(1..=8).contains(&trace.schema_version) || trace.max_turn_count == 0 {
        bail!("trace requires schema version 1 through 8 and a positive turn limit");
    }
    if trace.schema_version == 1 && !trace.events.is_empty() {
        bail!("schema version 1 does not support event verification");
    }
    match trace.strategies.len() {
        2 => run::<2>(trace, replay),
        3 => run::<3>(trace, replay),
        4 => run::<4>(trace, replay),
        5 => run::<5>(trace, replay),
        6 => run::<6>(trace, replay),
        7 => run::<7>(trace, replay),
        8 => run::<8>(trace, replay),
        _ => bail!("trace requires 2 to 8 player strategies"),
    }
}

fn run<const N: usize>(
    trace: &mut GameTrace,
    replay: bool,
) -> Result<()> {
    let tape = Rc::new(RefCell::new(DecisionTape {
        replay,
        decisions: if replay { trace.decisions.clone() } else { Vec::new() },
        events: if replay { trace.events.clone() } else { Vec::new() },
        event_version: trace.schema_version,
        ..DecisionTape::default()
    }));
    let mut strategies: [RecordedStrategy; N] = core::array::from_fn(|index| RecordedStrategy {
        strategy: trace.strategies[index],
        tape: tape.clone(),
    });
    let mut state = GameState::<N>::create_starting_state(&trace.ruleset, trace.seed);
    let mut turn_states = vec![snapshot(&state)];
    let mut winner = None;
    for _ in 0..trace.max_turn_count {
        play_turn(&mut state, &trace.ruleset, &mut strategies);
        if let Some(error) = &tape.borrow().error {
            bail!("{error}");
        }
        turn_states.push(snapshot(&state));
        let active = ((1u16 << N) - 1) & !(state.bankrupt_players as u16);
        if active.count_ones() == 1 {
            winner = Some(active.trailing_zeros() as PlayerId);
            break;
        }
    }
    if replay {
        if tape.borrow().cursor != trace.decisions.len() {
            bail!("replay left unused decisions");
        }
        if trace.schema_version >= 2 && tape.borrow().event_cursor != trace.events.len() {
            bail!("replay left unused events");
        }
        if turn_states != trace.turn_states || winner != trace.winner {
            bail!("replay turn states or winner differ from the recording");
        }
    } else {
        trace.decisions = std::mem::take(&mut tape.borrow_mut().decisions);
        trace.events = std::mem::take(&mut tape.borrow_mut().events);
        trace.turn_states = turn_states;
        trace.winner = winner;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::engine::turn::play_game;
    use crate::game::ruleset::data::DEX_RULESET;
    use crate::game::strategy::data::{
        BASELINE_STRATEGY,
        DEX_OPTIMAL_STRATEGY,
    };

    fn downgrade(
        trace: &mut GameTrace,
        version: u32,
    ) {
        if version < 8 {
            let unsupported = |decision: &Decision| {
                version < 4 && decision.request["method"] == "choose_liquidation_action"
                    || version < 8 && matches!(decision.request["method"].as_str(), Some("counter_trade_offer" | "should_accept_counteroffer"))
            };
            for record in &mut trace.events {
                record.decisions_before = trace.decisions[..record.decisions_before].iter().filter(|decision| !unsupported(decision)).count();
            }
            trace.decisions.retain(|decision| !unsupported(decision));
        }
        trace.events.retain(|record| record.event.trace_version() <= version);
        trace.schema_version = version;
    }

    #[test]
    fn replay_records_trade_proposals_and_rejections() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY], 7, 1000).unwrap();
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::TradeProposed { .. })));
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::TradeRejected { .. })));
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::TradeExecuted { .. })));
        let mut legacy = trace.clone();
        downgrade(&mut legacy, 6);
        replay_game(&legacy).unwrap();
        let mut corrupt = trace;
        let record = corrupt.events.iter_mut().find(|record| matches!(record.event, GameEvent::TradeProposed { .. })).unwrap();
        let GameEvent::TradeProposed { offer } = record.event else { unreachable!() };
        record.event = GameEvent::TradeRejected { offer };
        assert!(replay_game(&corrupt).is_err());
    }

    #[test]
    fn replay_supports_traces_before_cash_income_events() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 4], 7, 1000).unwrap();
        for kind in ["SalaryPaid", "BankRewardCollected", "FreeParkingCollected"] {
            assert!(trace.events.iter().any(|record| serde_json::to_value(record.event).unwrap()["kind"] == kind));
        }
        let mut legacy = trace.clone();
        downgrade(&mut legacy, 5);
        replay_game(&legacy).unwrap();
        let mut corrupt = trace;
        let record = corrupt.events.iter_mut().find(|record| matches!(record.event, GameEvent::SalaryPaid { .. })).unwrap();
        if let GameEvent::SalaryPaid { amount, .. } = &mut record.event {
            *amount += 1;
        }
        assert!(replay_game(&corrupt).is_err());
    }

    #[test]
    fn replay_supports_traces_before_jail_events() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 4], 7, 1000).unwrap();
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::SentToJail { .. })));
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::ReleasedFromJail { .. })));
        for version in 1..=4 {
            let mut legacy = trace.clone();
            downgrade(&mut legacy, version);
            replay_game(&legacy).unwrap();
        }
        let mut corrupt = trace;
        let event = corrupt.events.iter_mut().find(|record| matches!(record.event, GameEvent::SentToJail { .. })).unwrap();
        event.event = GameEvent::ReleasedFromJail { player_id: 0 };
        assert!(replay_game(&corrupt).is_err());
    }

    #[test]
    fn recording_preserves_game_and_replay_ignores_strategy_changes() {
        let policies = vec![BASELINE_STRATEGY, DEX_OPTIMAL_STRATEGY];
        let trace = record_game(DEX_RULESET, policies.clone(), 7, 1000).unwrap();
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 7);
        play_game(&mut state, &DEX_RULESET, &mut [policies[0], policies[1]], 1000);
        assert_eq!(trace.turn_states.last(), Some(&snapshot(&state)));
        let mut restored: GameTrace = serde_json::from_str(&serde_json::to_string(&trace).unwrap()).unwrap();
        restored.strategies = vec![DEX_OPTIMAL_STRATEGY; 2];
        replay_game(&restored).unwrap();
        restored.decisions[0].request = json!("corrupt");
        assert!(replay_game(&restored).is_err());
    }

    #[test]
    fn replay_rejects_missing_extra_and_changed_data() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 4], 17, 10).unwrap();
        let mut changed = trace.clone();
        changed.decisions.pop();
        assert!(replay_game(&changed).is_err());
        let mut changed = trace.clone();
        changed.decisions.push(changed.decisions[0].clone());
        assert!(replay_game(&changed).is_err());
        let mut changed = trace.clone();
        changed.turn_states[0] = Value::Null;
        assert!(replay_game(&changed).is_err());
        let mut changed = trace;
        changed.seed += 1;
        assert!(replay_game(&changed).is_err());
    }

    #[test]
    fn replay_checks_all_supported_player_counts() {
        for players in 2..=8 {
            let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; players], 3, 5).unwrap();
            replay_game(&trace).unwrap();
        }
    }

    #[test]
    fn replay_rejects_out_of_range_unmortgage_choices() {
        let mut trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 2], 3, 5).unwrap();
        let decision = trace.decisions.iter_mut().find(|decision| decision.request["method"] == "choose_tile_to_unmortgage").unwrap();
        decision.response = json!(255);
        assert!(replay_game(&trace).is_err());
    }

    #[test]
    fn event_records_preserve_turn_and_decision_order() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 4], 3, 20).unwrap();
        assert_eq!(
            trace.events[0],
            EventRecord {
                turn: 1,
                decisions_before: 0,
                event: GameEvent::TurnStarted { player_id: 0 }
            }
        );
        assert_eq!(trace.events.iter().filter(|record| matches!(record.event, GameEvent::TurnStarted { .. })).count(), 20);
        assert!(trace.events.windows(2).all(|pair| pair[0].turn <= pair[1].turn && pair[0].decisions_before <= pair[1].decisions_before));
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::DiceRolled { .. })));
        assert!(trace.events.iter().any(|record| matches!(record.event, GameEvent::Landed { .. })));
        replay_game(&trace).unwrap();
        let mut changed = trace.clone();
        changed.events[0].turn += 1;
        assert!(replay_game(&changed).is_err());
        let mut changed = trace.clone();
        changed.events.pop();
        assert!(replay_game(&changed).is_err());
        let mut changed = trace.clone();
        changed.events.push(changed.events.last().unwrap().clone());
        assert!(replay_game(&changed).is_err());
        let mut legacy = trace;
        downgrade(&mut legacy, 1);
        replay_game(&legacy).unwrap();
    }

    #[test]
    fn liquidation_events_record_sale_and_mortgage_before_payment() {
        use crate::game::engine::payment::{
            Creditor,
            charge_player,
        };
        let tape = Rc::new(RefCell::new(DecisionTape::default()));
        let mut strategies: [RecordedStrategy; 2] = core::array::from_fn(|_| RecordedStrategy {
            strategy: BASELINE_STRATEGY,
            tape: tape.clone(),
        });
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 0);
        state.cash_by_player_id[0] = 0;
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.improvement_level_by_property_id[0] = 1;
        state.board.bank_house_count -= 1;
        charge_player(&mut state, &DEX_RULESET, &mut strategies, 0, 50, Creditor::Bank);
        let events: Vec<_> = tape.borrow().events.iter().map(|record| record.event).collect();
        assert_eq!(
            events,
            vec![
                GameEvent::PaymentDue {
                    player_id: 0,
                    creditor: Creditor::Bank,
                    amount: 50
                },
                GameEvent::BuildingSold {
                    player_id: 0,
                    property_id: 0,
                    previous_level: 1,
                    level: 0,
                    proceeds: 25
                },
                GameEvent::TileMortgaged {
                    player_id: 0,
                    tile_id: 1,
                    proceeds: 30
                },
                GameEvent::PaymentCompleted {
                    player_id: 0,
                    creditor: Creditor::Bank,
                    amount: 50
                }
            ]
        );
        assert_eq!(state.cash_by_player_id[0], 5);
    }

    #[test]
    fn older_event_traces_skip_new_liquidation_events() {
        let trace = record_game(DEX_RULESET, vec![BASELINE_STRATEGY; 4], 3, 1000).unwrap();
        assert!(trace.events.iter().any(|record| record.event.trace_version() == 3));
        replay_game(&trace).unwrap();
        for version in [3, 2, 1] {
            let mut legacy = trace.clone();
            downgrade(&mut legacy, version);
            replay_game(&legacy).unwrap();
        }
    }

    #[test]
    fn building_policy_combinations_record_and_replay() {
        use crate::game::strategy::configurable::{
            BuildingAllocation,
            DevelopmentCeiling,
        };
        for building_allocation in [BuildingAllocation::Spread, BuildingAllocation::Concentrate] {
            for development_ceiling in [DevelopmentCeiling::ThreeHouses, DevelopmentCeiling::FourHouses, DevelopmentCeiling::Hotel] {
                let strategy = ConfigurableStrategy {
                    building_allocation,
                    development_ceiling,
                    ..DEX_OPTIMAL_STRATEGY
                };
                let trace = record_game(DEX_RULESET, vec![strategy; 4], 3, 1000).unwrap();
                assert!(
                    trace
                        .decisions
                        .iter()
                        .any(|decision| decision.request["method"] == "choose_property_to_improve" && !decision.response.is_null())
                );
                let mut restored: GameTrace = serde_json::from_str(&serde_json::to_string(&trace).unwrap()).unwrap();
                assert_eq!(restored.strategies, vec![strategy; 4]);
                restored.strategies = vec![BASELINE_STRATEGY; 4];
                replay_game(&restored).unwrap();
            }
        }
    }

    #[test]
    fn adaptive_jail_decisions_round_trip_and_replay_without_the_policy() {
        let strategy = ConfigurableStrategy {
            jail_camping_unowned_tile_threshold: Some(4),
            ..DEX_OPTIMAL_STRATEGY
        };
        let trace = record_game(DEX_RULESET, vec![strategy; 4], 3, 1000).unwrap();
        assert!(trace.decisions.iter().any(|decision| decision.request["method"] == "choose_jail_action"));
        let mut restored: GameTrace = serde_json::from_str(&serde_json::to_string(&trace).unwrap()).unwrap();
        assert_eq!(restored.strategies, vec![strategy; 4]);
        restored.strategies = vec![BASELINE_STRATEGY; 4];
        replay_game(&restored).unwrap();
    }

    #[test]
    fn replay_uses_recorded_liquidation_instead_of_current_policy() {
        let strategy = ConfigurableStrategy {
            mortgages_before_selling_buildings: true,
            ..BASELINE_STRATEGY
        };
        let mut trace = record_game(DEX_RULESET, vec![strategy; 4], 3, 1000).unwrap();
        assert!(trace.decisions.iter().any(|decision| decision.request["method"] == "choose_liquidation_action"));
        trace.strategies = vec![BASELINE_STRATEGY; 4];
        replay_game(&trace).unwrap();
    }

    #[test]
    fn payment_events_distinguish_success_and_bankruptcy() {
        use crate::game::engine::payment::{
            Creditor,
            charge_player,
        };
        let tape = Rc::new(RefCell::new(DecisionTape::default()));
        let mut strategies: [RecordedStrategy; 2] = core::array::from_fn(|_| RecordedStrategy {
            strategy: BASELINE_STRATEGY,
            tape: tape.clone(),
        });
        let mut state = GameState::<2>::create_starting_state(&DEX_RULESET, 0);
        charge_player(&mut state, &DEX_RULESET, &mut strategies, 0, 100, Creditor::Player(1));
        let events: Vec<_> = tape.borrow().events.iter().map(|record| record.event).collect();
        assert_eq!(
            events,
            vec![
                GameEvent::PaymentDue {
                    player_id: 0,
                    creditor: Creditor::Player(1),
                    amount: 100
                },
                GameEvent::PaymentCompleted {
                    player_id: 0,
                    creditor: Creditor::Player(1),
                    amount: 100
                }
            ]
        );
        tape.borrow_mut().events.clear();
        charge_player(&mut state, &DEX_RULESET, &mut strategies, 0, 2000, Creditor::Bank);
        let events: Vec<_> = tape.borrow().events.iter().map(|record| record.event).collect();
        assert_eq!(
            events,
            vec![
                GameEvent::PaymentDue {
                    player_id: 0,
                    creditor: Creditor::Bank,
                    amount: 2000
                },
                GameEvent::Bankrupt {
                    player_id: 0,
                    creditor: Creditor::Bank,
                    remaining_cash: 1700
                }
            ]
        );
    }
}
