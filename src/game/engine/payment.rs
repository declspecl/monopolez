use crate::game::board::data::HOTEL_IMPROVEMENT_LEVEL;
use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::{
    FreeParkingJackpotMode,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    HOUSE_PURCHASE_PRICE_BY_TILE_ID,
    MORTGAGE_VALUE_BY_TILE_ID,
    TILE_ID_BY_PROPERTY_ID,
};
use crate::game::tile::model::Cash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Creditor {
    Bank,
    FreeParkingJackpot,
    Player(PlayerId),
}

pub const fn select_fee_creditor(ruleset: &Ruleset) -> Creditor {
    match ruleset.free_parking_jackpot_mode {
        FreeParkingJackpotMode::Disabled => Creditor::Bank,
        FreeParkingJackpotMode::TaxesAndFees => Creditor::FreeParkingJackpot,
    }
}

pub fn charge_player<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    debtor_player_id: PlayerId,
    amount: Cash,
    creditor: Creditor,
) {
    let debtor_index = debtor_player_id as usize;
    if game_state.cash_by_player_id[debtor_index] < amount {
        raise_cash(game_state, debtor_player_id, amount);
    }

    if game_state.cash_by_player_id[debtor_index] >= amount {
        game_state.cash_by_player_id[debtor_index] -= amount;
        credit_creditor(game_state, creditor, amount);
    } else {
        declare_bankruptcy(game_state, debtor_player_id, creditor);
    }
}

fn credit_creditor<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    creditor: Creditor,
    amount: Cash,
) {
    match creditor {
        Creditor::Bank => {},
        Creditor::FreeParkingJackpot => game_state.free_parking_jackpot += amount,
        Creditor::Player(creditor_player_id) => game_state.cash_by_player_id[creditor_player_id as usize] += amount,
    }
}

fn raise_cash<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
    required_amount: Cash,
) {
    let player_index = player_id as usize;

    for property_id in 0..PROPERTY_COUNT {
        if game_state.cash_by_player_id[player_index] >= required_amount {
            return;
        }

        let tile_id = TILE_ID_BY_PROPERTY_ID[property_id];
        let improvement_level = game_state.board.improvement_level_by_property_id[property_id];
        if improvement_level == 0 || game_state.board.get_tile_owner(tile_id) != Some(player_id) {
            continue;
        }

        let building_sale_price = HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash / 2;
        game_state.cash_by_player_id[player_index] += building_sale_price * improvement_level as Cash;

        if improvement_level == HOTEL_IMPROVEMENT_LEVEL {
            game_state.board.bank_hotel_count += 1;
        } else {
            game_state.board.bank_house_count += improvement_level;
        }

        game_state.board.improvement_level_by_property_id[property_id] = 0;
    }

    let mut unmortgaged_owned_tiles = game_state.board.owned_tiles_by_player_id[player_index] & !game_state.board.mortgaged_tiles;
    while unmortgaged_owned_tiles != 0 && game_state.cash_by_player_id[player_index] < required_amount {
        let tile_id = unmortgaged_owned_tiles.trailing_zeros();
        unmortgaged_owned_tiles &= unmortgaged_owned_tiles - 1;

        game_state.board.mortgaged_tiles |= 1 << tile_id;
        game_state.cash_by_player_id[player_index] += MORTGAGE_VALUE_BY_TILE_ID[tile_id as usize] as Cash;
    }
}

fn declare_bankruptcy<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    debtor_player_id: PlayerId,
    creditor: Creditor,
) {
    let debtor_index = debtor_player_id as usize;
    let remaining_cash = game_state.cash_by_player_id[debtor_index];
    let owned_tiles = game_state.board.owned_tiles_by_player_id[debtor_index];

    game_state.cash_by_player_id[debtor_index] = 0;
    game_state.board.owned_tiles_by_player_id[debtor_index] = 0;

    match creditor {
        Creditor::Player(creditor_player_id) => {
            game_state.board.owned_tiles_by_player_id[creditor_player_id as usize] |= owned_tiles;
            game_state.cash_by_player_id[creditor_player_id as usize] += remaining_cash;
        },
        Creditor::Bank | Creditor::FreeParkingJackpot => {
            game_state.board.mortgaged_tiles &= !owned_tiles;
            credit_creditor(game_state, creditor, remaining_cash);
        },
    }

    for holder in &mut game_state.get_out_of_jail_free_card_holder_by_deck_kind {
        if *holder == Some(debtor_player_id) {
            *holder = match creditor {
                Creditor::Player(creditor_player_id) => Some(creditor_player_id),
                Creditor::Bank | Creditor::FreeParkingJackpot => None,
            };
        }
    }

    game_state.bankrupt_players |= 1 << debtor_player_id;
    game_state.jailed_players &= !(1 << debtor_player_id);
}
