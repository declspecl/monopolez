use super::auction::run_auction;
use super::card::draw_and_apply_card;
use super::movement::send_player_to_jail;
use super::payment::{
    Creditor,
    charge_player,
    select_fee_creditor,
};
use super::trade::run_trade_phase;
use crate::game::board::model::PlayerId;
use crate::game::card::data::{
    NEAREST_RAILROAD_RENT_MULTIPLIER,
    NEAREST_UTILITY_RENT_DICE_MULTIPLIER,
};
use crate::game::card::model::DeckKind;
use crate::game::rng::DiceRoll;
use crate::game::ruleset::model::{
    PermittedBarterTimesMask,
    PropertyPurchaseDeclineMode,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::lut::{
    ONE_HOUSE_RENT_LEVEL,
    OWNERSHIP_GROUP_BY_TILE_ID,
    PROPERTY_ID_BY_TILE_ID,
    RAILROAD_TILE_SET_MASK,
    RENT_BY_TILE_ID_BY_RENT_LEVEL,
    TAX_AMOUNT_BY_TILE_ID,
    TILE_KIND_BY_TILE_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
    UNIMPROVED_MONOPOLY_RENT_LEVEL,
    UNIMPROVED_RENT_LEVEL,
    UTILITY_TILE_SET_MASK,
};
use crate::game::tile::model::{
    Cash,
    TileId,
    TileKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RentModifier {
    Standard,
    NearestRailroadCard,
    NearestUtilityCard,
}

pub fn resolve_landing<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    dice_roll: DiceRoll,
    rent_modifier: RentModifier,
) {
    let tile_id = game_state.position_by_player_id[player_id as usize];
    super::event::publish_event(strategies, player_id as usize, super::event::GameEvent::Landed { player_id, tile_id });

    match TILE_KIND_BY_TILE_ID[tile_id as usize] {
        TileKind::Property | TileKind::Railroad | TileKind::Utility => {
            resolve_ownable_tile_landing(game_state, ruleset, strategies, player_id, tile_id, dice_roll, rent_modifier);
        },
        TileKind::Tax => {
            let tax_amount = TAX_AMOUNT_BY_TILE_ID[tile_id as usize] as Cash;
            charge_player(game_state, ruleset, strategies, player_id, tax_amount, select_fee_creditor(ruleset));
        },
        TileKind::Chance => draw_and_apply_card(game_state, ruleset, strategies, player_id, DeckKind::Chance, dice_roll),
        TileKind::CommunityChest => draw_and_apply_card(game_state, ruleset, strategies, player_id, DeckKind::CommunityChest, dice_roll),
        TileKind::GoToJail => {
            let event = send_player_to_jail(game_state, player_id);
            super::event::publish_event(strategies, player_id as usize, event);
        },
        TileKind::FreeParking => {
            let amount = game_state.free_parking_jackpot;
            game_state.cash_by_player_id[player_id as usize] += amount;
            game_state.free_parking_jackpot = 0;
            if amount > 0 {
                super::event::publish_event(strategies, player_id as usize, super::event::GameEvent::FreeParkingCollected { player_id, amount });
            }
        },
        TileKind::Go | TileKind::Jail => {},
    }
}

fn resolve_ownable_tile_landing<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    tile_id: TileId,
    dice_roll: DiceRoll,
    rent_modifier: RentModifier,
) {
    match game_state.board.get_tile_owner(tile_id) {
        None => {
            run_trade_phase(game_state, ruleset, strategies, player_id, PermittedBarterTimesMask::BEFORE_PURCHASE);
            offer_purchase(game_state, ruleset, strategies, player_id, tile_id);
        },
        Some(owner_player_id) if owner_player_id == player_id || game_state.board.is_tile_mortgaged(tile_id) => {},
        Some(owner_player_id) => {
            let rent = calculate_rent(game_state, tile_id, owner_player_id, dice_roll, rent_modifier);
            charge_player(game_state, ruleset, strategies, player_id, rent, Creditor::Player(owner_player_id));
        },
    }
}

fn offer_purchase<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
    tile_id: TileId,
) {
    let player_index = player_id as usize;
    if super::action::validate_purchase(game_state, player_id, tile_id).is_ok()
        && strategies[player_index].should_purchase_property(game_state, ruleset, player_id, tile_id)
        && let Ok(event) = super::action::execute_purchase(game_state, player_id, tile_id)
    {
        super::event::publish_event(strategies, player_index, event);
        return;
    }
    if game_state.board.get_tile_owner(tile_id).is_none() && ruleset.property_purchase_decline_mode == PropertyPurchaseDeclineMode::Auction {
        run_auction(game_state, ruleset, strategies, player_id, tile_id);
    }
}

fn calculate_rent<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    tile_id: TileId,
    owner_player_id: PlayerId,
    dice_roll: DiceRoll,
    rent_modifier: RentModifier,
) -> Cash {
    let tile_index = tile_id as usize;
    let rent_by_rent_level = &RENT_BY_TILE_ID_BY_RENT_LEVEL[tile_index];

    match TILE_KIND_BY_TILE_ID[tile_index] {
        TileKind::Property => {
            let property_id = PROPERTY_ID_BY_TILE_ID[tile_index].expect("property tiles should have a property id");
            let ownership_group = OWNERSHIP_GROUP_BY_TILE_ID[tile_index].expect("property tiles should have an ownership group");
            let improvement_level = game_state.board.improvement_level_by_property_id[property_id as usize];

            let rent_level = if improvement_level > 0 {
                ONE_HOUSE_RENT_LEVEL + improvement_level as usize - 1
            } else if game_state.board.does_player_own_all_tiles(owner_player_id, TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group as usize]) {
                UNIMPROVED_MONOPOLY_RENT_LEVEL
            } else {
                UNIMPROVED_RENT_LEVEL
            };

            rent_by_rent_level[rent_level] as Cash
        },
        TileKind::Railroad => {
            let owned_railroad_count = game_state.board.get_player_owned_tiles_count(owner_player_id, RAILROAD_TILE_SET_MASK) as usize;
            let rent = rent_by_rent_level[owned_railroad_count - 1] as Cash;

            match rent_modifier {
                RentModifier::NearestRailroadCard => rent * NEAREST_RAILROAD_RENT_MULTIPLIER as Cash,
                RentModifier::Standard | RentModifier::NearestUtilityCard => rent,
            }
        },
        TileKind::Utility => {
            let owned_utility_count = game_state.board.get_player_owned_tiles_count(owner_player_id, UTILITY_TILE_SET_MASK) as usize;
            let dice_multiplier = match rent_modifier {
                RentModifier::NearestUtilityCard => NEAREST_UTILITY_RENT_DICE_MULTIPLIER,
                RentModifier::Standard | RentModifier::NearestRailroadCard => rent_by_rent_level[owned_utility_count - 1],
            };

            dice_multiplier as Cash * dice_roll.total() as Cash
        },
        _ => 0,
    }
}
