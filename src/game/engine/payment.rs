use super::auction::run_auction;
use super::improvement::sell_one_building;
use super::mortgage::calculate_mortgage_transfer_interest;
use super::trade::run_trade_phase;
use crate::game::board::model::PlayerId;
use crate::game::ruleset::model::{
    FreeParkingJackpotMode,
    PermittedBarterTimesMask,
    PropertyPurchaseDeclineMode,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::lut::MORTGAGE_VALUE_BY_TILE_ID;
use crate::game::tile::model::{
    Cash,
    TileId,
    TileSetMask,
};

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

pub fn charge_player<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    debtor_player_id: PlayerId,
    amount: Cash,
    creditor: Creditor,
) {
    let debtor_index = debtor_player_id as usize;
    if game_state.cash_by_player_id[debtor_index] < amount {
        run_trade_phase(game_state, ruleset, strategies, debtor_player_id, PermittedBarterTimesMask::DURING_PAYMENT);
    }

    if game_state.cash_by_player_id[debtor_index] < amount {
        raise_cash(game_state, debtor_player_id, amount);
    }

    if game_state.cash_by_player_id[debtor_index] >= amount {
        game_state.cash_by_player_id[debtor_index] -= amount;
        credit_creditor(game_state, creditor, amount);
    } else {
        declare_bankruptcy(game_state, ruleset, strategies, debtor_player_id, creditor);
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

    while game_state.cash_by_player_id[player_index] < required_amount {
        if sell_one_building(game_state, player_id) == 0 {
            break;
        }
    }

    let mut unmortgaged_owned_tiles = game_state.board.owned_tiles_by_player_id[player_index] & !game_state.board.mortgaged_tiles;
    while unmortgaged_owned_tiles != 0 && game_state.cash_by_player_id[player_index] < required_amount {
        let tile_id = unmortgaged_owned_tiles.trailing_zeros();
        unmortgaged_owned_tiles &= unmortgaged_owned_tiles - 1;

        game_state.board.mortgaged_tiles |= 1 << tile_id;
        game_state.cash_by_player_id[player_index] += MORTGAGE_VALUE_BY_TILE_ID[tile_id as usize] as Cash;
    }
}

fn declare_bankruptcy<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
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
            let transfer_interest = calculate_mortgage_transfer_interest(game_state, owned_tiles);

            game_state.board.owned_tiles_by_player_id[creditor_player_id as usize] |= owned_tiles;
            game_state.cash_by_player_id[creditor_player_id as usize] += remaining_cash;
            game_state.cash_by_player_id[creditor_player_id as usize] = game_state.cash_by_player_id[creditor_player_id as usize].saturating_sub(transfer_interest);
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

    if matches!(creditor, Creditor::Bank | Creditor::FreeParkingJackpot) && ruleset.property_purchase_decline_mode == PropertyPurchaseDeclineMode::Auction {
        auction_bank_owned_tiles(game_state, ruleset, strategies, debtor_player_id, owned_tiles);
    }
}

fn auction_bank_owned_tiles<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    bankrupt_player_id: PlayerId,
    bank_owned_tiles: TileSetMask,
) {
    let mut remaining_tiles = bank_owned_tiles;
    while remaining_tiles != 0 {
        let tile_id = remaining_tiles.trailing_zeros() as TileId;
        remaining_tiles &= remaining_tiles - 1;

        run_auction(game_state, ruleset, strategies, bankrupt_player_id, tile_id);
    }
}
