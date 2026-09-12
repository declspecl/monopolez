use super::improvement::run_improvement_phase;
use super::landing::{
    RentModifier,
    resolve_landing,
};
use super::mortgage::run_mortgage_phase;
use super::movement::{
    move_player_forward,
    release_player_from_jail,
    send_player_to_jail,
};
use super::payment::{
    charge_player,
    select_fee_creditor,
};
use super::trade::run_trade_phase;
use crate::game::board::model::PlayerId;
use crate::game::rng::DiceRoll;
use crate::game::ruleset::model::{
    PermittedBarterTimesMask,
    Ruleset,
};
use crate::game::state::model::{
    GameState,
    PlayerSetMask,
};
use crate::game::strategy::model::{
    JailAction,
    PlayerStrategy,
};
use crate::game::tile::model::Cash;

pub const MAX_CONSECUTIVE_DOUBLE_COUNT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameOutcome {
    Winner(PlayerId),
    TurnLimitReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameSummary {
    pub outcome: GameOutcome,
    pub turn_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JailTurnResult {
    StayInJail,
    RollNormally,
    MoveWithoutRollingAgain(DiceRoll),
}

pub fn play_game<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    max_turn_count: u32,
) -> GameSummary {
    let all_players = ((1u16 << PLAYER_COUNT) - 1) as PlayerSetMask;

    for turn_index in 0..max_turn_count {
        play_turn(game_state, ruleset, strategies);

        let active_players = all_players & !game_state.bankrupt_players;
        if active_players.count_ones() == 1 {
            return GameSummary {
                outcome: GameOutcome::Winner(active_players.trailing_zeros() as PlayerId),
                turn_count: turn_index + 1,
            };
        }
    }

    GameSummary {
        outcome: GameOutcome::TurnLimitReached,
        turn_count: max_turn_count,
    }
}

pub fn play_turn<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
) {
    let player_id = game_state.current_player_id;

    run_trade_phase(game_state, ruleset, strategies, player_id, PermittedBarterTimesMask::START_OF_TURN);
    run_mortgage_phase(game_state, ruleset, strategies, player_id);
    run_improvement_phase(game_state, ruleset, strategies, player_id);

    if game_state.jailed_players & (1 << player_id) != 0 {
        match take_jail_turn(game_state, ruleset, strategies, player_id) {
            JailTurnResult::StayInJail => {
                end_turn(game_state);

                return;
            },
            JailTurnResult::MoveWithoutRollingAgain(dice_roll) => {
                move_player_forward(game_state, ruleset, player_id, dice_roll.total());
                resolve_landing(game_state, ruleset, strategies, player_id, dice_roll, RentModifier::Standard);
                end_turn(game_state);

                return;
            },
            JailTurnResult::RollNormally => {},
        }
    }

    loop {
        let dice_roll = game_state.rng.roll_dice();

        if dice_roll.is_double() {
            game_state.consecutive_double_count += 1;
            if game_state.consecutive_double_count == MAX_CONSECUTIVE_DOUBLE_COUNT {
                send_player_to_jail(game_state, player_id);
                break;
            }
        }

        move_player_forward(game_state, ruleset, player_id, dice_roll.total());
        resolve_landing(game_state, ruleset, strategies, player_id, dice_roll, RentModifier::Standard);

        let is_bankrupt = game_state.bankrupt_players & (1 << player_id) != 0;
        let is_jailed = game_state.jailed_players & (1 << player_id) != 0;
        if !dice_roll.is_double() || is_bankrupt || is_jailed {
            break;
        }
    }

    end_turn(game_state);
}

fn take_jail_turn<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
) -> JailTurnResult {
    let player_index = player_id as usize;
    let jail_bail_amount = ruleset.jail_bail_amount as Cash;

    match strategies[player_index].choose_jail_action(game_state, ruleset, player_id) {
        JailAction::UseGetOutOfJailFreeCard => {
            let held_deck_index = game_state.get_out_of_jail_free_card_holder_by_deck_kind.iter().position(|holder| *holder == Some(player_id));
            if let Some(held_deck_index) = held_deck_index {
                game_state.get_out_of_jail_free_card_holder_by_deck_kind[held_deck_index] = None;
                release_player_from_jail(game_state, player_id);

                return JailTurnResult::RollNormally;
            }
        },
        JailAction::PayBail => {
            if game_state.cash_by_player_id[player_index] >= jail_bail_amount {
                charge_player(game_state, player_id, jail_bail_amount, select_fee_creditor(ruleset));
                release_player_from_jail(game_state, player_id);

                return JailTurnResult::RollNormally;
            }
        },
        JailAction::RollForDoubles => {},
    }

    let dice_roll = game_state.rng.roll_dice();
    if dice_roll.is_double() {
        release_player_from_jail(game_state, player_id);

        return JailTurnResult::MoveWithoutRollingAgain(dice_roll);
    }

    game_state.jail_turn_count_by_player_id[player_index] += 1;
    if game_state.jail_turn_count_by_player_id[player_index] < ruleset.max_jail_turn_count {
        return JailTurnResult::StayInJail;
    }

    charge_player(game_state, player_id, jail_bail_amount, select_fee_creditor(ruleset));
    if game_state.bankrupt_players & (1 << player_id) != 0 {
        return JailTurnResult::StayInJail;
    }

    release_player_from_jail(game_state, player_id);

    JailTurnResult::MoveWithoutRollingAgain(dice_roll)
}

fn end_turn<const PLAYER_COUNT: usize>(game_state: &mut GameState<PLAYER_COUNT>) {
    game_state.consecutive_double_count = 0;

    for turn_order_offset in 1..=PLAYER_COUNT {
        let next_player_id = ((game_state.current_player_id as usize + turn_order_offset) % PLAYER_COUNT) as PlayerId;

        if game_state.bankrupt_players & (1 << next_player_id) == 0 {
            game_state.current_player_id = next_player_id;

            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::board::data::{
        BANK_STARTING_HOTEL_COUNT,
        BANK_STARTING_HOUSE_COUNT,
        HOTEL_IMPROVEMENT_LEVEL,
    };
    use crate::game::strategy::greedy::GreedyStrategy;
    use crate::game::tile::lut::OWNABLE_TILE_SET_MASK;

    const PLAYER_COUNT: usize = 4;
    const MAX_TURN_COUNT: u32 = 10_000;

    fn create_strategies() -> [GreedyStrategy; PLAYER_COUNT] {
        [GreedyStrategy { cash_reserve: 100 }; PLAYER_COUNT]
    }

    #[test]
    fn keeps_ownership_consistent_across_many_games() {
        let ruleset = Ruleset::default();

        for seed in 0..200 {
            let mut game_state = GameState::<PLAYER_COUNT>::create_starting_state(&ruleset, seed);
            play_game(&mut game_state, &ruleset, &mut create_strategies(), MAX_TURN_COUNT);

            let mut all_owned_tiles = 0;
            for player_index in 0..PLAYER_COUNT {
                let owned_tiles = game_state.board.owned_tiles_by_player_id[player_index];
                assert_eq!(all_owned_tiles & owned_tiles, 0, "a tile should have at most one owner");
                all_owned_tiles |= owned_tiles;

                if game_state.bankrupt_players & (1 << player_index) != 0 {
                    assert_eq!(owned_tiles, 0);
                    assert_eq!(game_state.cash_by_player_id[player_index], 0);
                }
            }

            let mut houses_in_play = 0;
            let mut hotels_in_play = 0;
            for improvement_level in game_state.board.improvement_level_by_property_id {
                if improvement_level == HOTEL_IMPROVEMENT_LEVEL {
                    hotels_in_play += 1;
                } else {
                    houses_in_play += improvement_level as u32;
                }
            }

            assert_eq!(
                houses_in_play + game_state.board.bank_house_count as u32,
                BANK_STARTING_HOUSE_COUNT as u32,
                "houses should be conserved"
            );
            assert_eq!(
                hotels_in_play + game_state.board.bank_hotel_count as u32,
                BANK_STARTING_HOTEL_COUNT as u32,
                "hotels should be conserved"
            );

            assert_eq!(all_owned_tiles & !OWNABLE_TILE_SET_MASK, 0, "only ownable tiles should be owned");
        }
    }

    #[test]
    fn repeats_game_for_same_seed() {
        let ruleset = Ruleset::default();
        let mut first_game_state = GameState::<PLAYER_COUNT>::create_starting_state(&ruleset, 99);
        let mut second_game_state = GameState::<PLAYER_COUNT>::create_starting_state(&ruleset, 99);

        let first_summary = play_game(&mut first_game_state, &ruleset, &mut create_strategies(), MAX_TURN_COUNT);
        let second_summary = play_game(&mut second_game_state, &ruleset, &mut create_strategies(), MAX_TURN_COUNT);

        assert_eq!(first_summary, second_summary);
        assert_eq!(first_game_state, second_game_state);
    }

    #[test]
    fn charges_double_rent_for_unimproved_monopoly() {
        const PARK_PLACE_TILE_ID: u8 = 37;
        const BOARDWALK_TILE_ID: u8 = 39;

        let ruleset = Ruleset::default();
        let mut game_state = GameState::<2>::create_starting_state(&ruleset, 1);
        game_state.board.owned_tiles_by_player_id[0] = 1 << PARK_PLACE_TILE_ID | 1 << BOARDWALK_TILE_ID;
        game_state.position_by_player_id[1] = BOARDWALK_TILE_ID;

        let dice_roll = DiceRoll { first_die: 1, second_die: 2 };
        resolve_landing(&mut game_state, &ruleset, &mut [GreedyStrategy { cash_reserve: 0 }; 2], 1, dice_roll, RentModifier::Standard);

        assert_eq!(game_state.cash_by_player_id[0], 1600);
        assert_eq!(game_state.cash_by_player_id[1], 1400);
    }

    #[test]
    fn pays_landing_salary_instead_of_passing_salary() {
        let ruleset = Ruleset::builder().with_go_landing_salary(400).build();
        let mut game_state = GameState::<2>::create_starting_state(&ruleset, 1);
        game_state.position_by_player_id[0] = 36;

        move_player_forward(&mut game_state, &ruleset, 0, 4);

        assert_eq!(game_state.position_by_player_id[0], 0);
        assert_eq!(game_state.cash_by_player_id[0], 1900);
    }
}
