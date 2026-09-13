use crate::game::board::data::{
    HOTEL_IMPROVEMENT_LEVEL,
    MAX_HOUSE_IMPROVEMENT_LEVEL,
};
use crate::game::board::model::{
    PlayerId,
    PropertyImprovementLevel,
};
use crate::game::ruleset::model::{
    PropertyImprovementDistribution,
    Ruleset,
};
use crate::game::state::model::GameState;
use crate::game::strategy::model::PlayerStrategy;
use crate::game::tile::data::PROPERTY_COUNT;
use crate::game::tile::lut::{
    HOUSE_PURCHASE_PRICE_BY_TILE_ID,
    OWNERSHIP_GROUP_BY_TILE_ID,
    PROPERTY_ID_BY_TILE_ID,
    TILE_ID_BY_PROPERTY_ID,
    TILE_SET_MASK_BY_OWNERSHIP_GROUP,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
};

pub fn run_improvement_phase<const PLAYER_COUNT: usize, Strategy: PlayerStrategy>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    strategies: &mut [Strategy; PLAYER_COUNT],
    player_id: PlayerId,
) {
    while let Some(property_id) = strategies[player_id as usize].choose_property_to_improve(game_state, ruleset, player_id) {
        if !improve_property(game_state, ruleset, player_id, property_id) {
            return;
        }
        strategies[player_id as usize].record_event(super::event::GameEvent::BuildingPurchased {
            player_id,
            property_id,
            level: game_state.board.improvement_level_by_property_id[property_id as usize],
        });
    }
}

pub fn can_improve_property<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    player_id: PlayerId,
    property_id: PropertyId,
) -> bool {
    let property_index = property_id as usize;
    if property_index >= PROPERTY_COUNT || player_id as usize >= PLAYER_COUNT || game_state.bankrupt_players & (1 << player_id) != 0 {
        return false;
    }

    let tile_id = TILE_ID_BY_PROPERTY_ID[property_index];
    let improvement_level = game_state.board.improvement_level_by_property_id[property_index];
    if improvement_level >= HOTEL_IMPROVEMENT_LEVEL {
        return false;
    }

    let ownership_group = OWNERSHIP_GROUP_BY_TILE_ID[tile_id as usize].expect("property tiles should have an ownership group");
    let group_tiles = TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group as usize];
    if !game_state.board.does_player_own_all_tiles(player_id, group_tiles) || game_state.board.mortgaged_tiles & group_tiles != 0 {
        return false;
    }

    if ruleset.property_improvement_distribution == PropertyImprovementDistribution::Even && improvement_level > find_lowest_group_improvement_level(game_state, ownership_group as usize) {
        return false;
    }

    let is_hotel_purchase = improvement_level == MAX_HOUSE_IMPROVEMENT_LEVEL;
    let has_bank_supply = if is_hotel_purchase {
        game_state.board.bank_hotel_count > 0
    } else {
        game_state.board.bank_house_count > 0
    };

    has_bank_supply && game_state.cash_by_player_id[player_id as usize] >= HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash
}

pub fn improve_property<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    ruleset: &Ruleset,
    player_id: PlayerId,
    property_id: PropertyId,
) -> bool {
    if !can_improve_property(game_state, ruleset, player_id, property_id) {
        return false;
    }

    let property_index = property_id as usize;
    let tile_id = TILE_ID_BY_PROPERTY_ID[property_index];
    let improvement_level = game_state.board.improvement_level_by_property_id[property_index];

    game_state.cash_by_player_id[player_id as usize] -= HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash;
    game_state.board.improvement_level_by_property_id[property_index] = improvement_level + 1;

    if improvement_level == MAX_HOUSE_IMPROVEMENT_LEVEL {
        game_state.board.bank_hotel_count -= 1;
        game_state.board.bank_house_count += MAX_HOUSE_IMPROVEMENT_LEVEL;
    } else {
        game_state.board.bank_house_count -= 1;
    }

    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingSale {
    pub property_id: PropertyId,
    pub previous_level: u8,
    pub level: u8,
    pub proceeds: Cash,
}

pub fn sell_one_building<const PLAYER_COUNT: usize>(
    game_state: &mut GameState<PLAYER_COUNT>,
    player_id: PlayerId,
) -> Option<BuildingSale> {
    let property_id = find_most_improved_property(game_state, player_id)?;

    let property_index = property_id as usize;
    let tile_id = TILE_ID_BY_PROPERTY_ID[property_index];
    let improvement_level = game_state.board.improvement_level_by_property_id[property_index];
    let building_sale_price = HOUSE_PURCHASE_PRICE_BY_TILE_ID[tile_id as usize] as Cash / 2;

    if improvement_level < HOTEL_IMPROVEMENT_LEVEL {
        game_state.board.improvement_level_by_property_id[property_index] = improvement_level - 1;
        game_state.board.bank_house_count += 1;
        game_state.cash_by_player_id[player_id as usize] += building_sale_price;

        return Some(BuildingSale {
            property_id,
            previous_level: improvement_level,
            level: improvement_level - 1,
            proceeds: building_sale_price,
        });
    }

    game_state.board.bank_hotel_count += 1;

    if game_state.board.bank_house_count >= MAX_HOUSE_IMPROVEMENT_LEVEL {
        game_state.board.bank_house_count -= MAX_HOUSE_IMPROVEMENT_LEVEL;
        game_state.board.improvement_level_by_property_id[property_index] = MAX_HOUSE_IMPROVEMENT_LEVEL;
        game_state.cash_by_player_id[player_id as usize] += building_sale_price;

        return Some(BuildingSale {
            property_id,
            previous_level: improvement_level,
            level: MAX_HOUSE_IMPROVEMENT_LEVEL,
            proceeds: building_sale_price,
        });
    }

    let hotel_sale_price = building_sale_price * HOTEL_IMPROVEMENT_LEVEL as Cash;
    game_state.board.improvement_level_by_property_id[property_index] = 0;
    game_state.cash_by_player_id[player_id as usize] += hotel_sale_price;

    Some(BuildingSale {
        property_id,
        previous_level: improvement_level,
        level: 0,
        proceeds: hotel_sale_price,
    })
}

fn find_most_improved_property<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    player_id: PlayerId,
) -> Option<PropertyId> {
    let owned_tiles = game_state.board.owned_tiles_by_player_id[player_id as usize];
    let mut most_improved_property_id = None;
    let mut highest_improvement_level = 0;

    for property_id in 0..PROPERTY_COUNT {
        let improvement_level = game_state.board.improvement_level_by_property_id[property_id];
        let is_owned = owned_tiles & (1 << TILE_ID_BY_PROPERTY_ID[property_id]) != 0;

        if is_owned && improvement_level > highest_improvement_level {
            highest_improvement_level = improvement_level;
            most_improved_property_id = Some(property_id as PropertyId);
        }
    }

    most_improved_property_id
}

fn find_lowest_group_improvement_level<const PLAYER_COUNT: usize>(
    game_state: &GameState<PLAYER_COUNT>,
    ownership_group_index: usize,
) -> PropertyImprovementLevel {
    let mut group_tiles = TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group_index];
    let mut lowest_improvement_level = HOTEL_IMPROVEMENT_LEVEL;

    while group_tiles != 0 {
        let tile_id = group_tiles.trailing_zeros() as usize;
        group_tiles &= group_tiles - 1;

        if let Some(property_id) = PROPERTY_ID_BY_TILE_ID[tile_id] {
            lowest_improvement_level = lowest_improvement_level.min(game_state.board.improvement_level_by_property_id[property_id as usize]);
        }
    }

    lowest_improvement_level
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::board::data::{
        BANK_STARTING_HOTEL_COUNT,
        BANK_STARTING_HOUSE_COUNT,
    };

    const PARK_PLACE_TILE_ID: u8 = 37;
    const BOARDWALK_TILE_ID: u8 = 39;
    const PARK_PLACE_PROPERTY_ID: PropertyId = 20;
    const BOARDWALK_PROPERTY_ID: PropertyId = 21;

    fn create_dark_blue_owner_state(ruleset: &Ruleset) -> GameState<2> {
        let mut game_state = GameState::<2>::create_starting_state(ruleset, 1);
        game_state.board.owned_tiles_by_player_id[0] = 1 << PARK_PLACE_TILE_ID | 1 << BOARDWALK_TILE_ID;
        game_state.cash_by_player_id[0] = 5000;

        game_state
    }

    #[test]
    fn improves_evenly_within_ownership_group() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);

        assert!(improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
        assert!(!improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
        assert!(improve_property(&mut game_state, &ruleset, 0, BOARDWALK_PROPERTY_ID));
        assert!(improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
    }

    #[test]
    fn returns_houses_to_bank_when_building_hotel() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);

        for _ in 0..HOTEL_IMPROVEMENT_LEVEL {
            assert!(improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
            assert!(improve_property(&mut game_state, &ruleset, 0, BOARDWALK_PROPERTY_ID));
        }

        assert_eq!(game_state.board.improvement_level_by_property_id[BOARDWALK_PROPERTY_ID as usize], HOTEL_IMPROVEMENT_LEVEL);
        assert_eq!(game_state.board.bank_house_count, BANK_STARTING_HOUSE_COUNT);
        assert_eq!(game_state.board.bank_hotel_count, BANK_STARTING_HOTEL_COUNT - 2);
    }

    #[test]
    fn refuses_to_improve_without_bank_supply() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);
        game_state.board.bank_house_count = 0;

        assert!(!can_improve_property(&game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
    }

    #[test]
    fn refuses_to_improve_mortgaged_ownership_group() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);
        game_state.board.mortgaged_tiles = 1 << BOARDWALK_TILE_ID;

        assert!(!can_improve_property(&game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID));
    }

    #[test]
    fn sells_buildings_from_the_most_improved_property() {
        let ruleset = Ruleset::default();
        let mut game_state = create_dark_blue_owner_state(&ruleset);
        improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID);
        improve_property(&mut game_state, &ruleset, 0, BOARDWALK_PROPERTY_ID);
        improve_property(&mut game_state, &ruleset, 0, PARK_PLACE_PROPERTY_ID);

        let sale_price = sell_one_building(&mut game_state, 0);

        assert_eq!(sale_price.unwrap().proceeds, 100);
        assert_eq!(game_state.board.improvement_level_by_property_id[PARK_PLACE_PROPERTY_ID as usize], 1);
        assert_eq!(game_state.board.bank_house_count, BANK_STARTING_HOUSE_COUNT - 2);
    }

    #[test]
    fn hotel_sale_reports_downgrade_or_complete_liquidation() {
        for (houses_available, expected_level, expected_proceeds) in [(4, 4, 100), (0, 0, 500)] {
            let mut state = create_dark_blue_owner_state(&Ruleset::default());
            state.board.improvement_level_by_property_id[PARK_PLACE_PROPERTY_ID as usize] = HOTEL_IMPROVEMENT_LEVEL;
            state.board.bank_hotel_count -= 1;
            state.board.bank_house_count = houses_available;
            let cash_before = state.cash_by_player_id[0];
            let sale = sell_one_building(&mut state, 0).unwrap();
            assert_eq!(sale.property_id, PARK_PLACE_PROPERTY_ID);
            assert_eq!(sale.previous_level, HOTEL_IMPROVEMENT_LEVEL);
            assert_eq!(sale.level, expected_level);
            assert_eq!(sale.proceeds, expected_proceeds);
            assert_eq!(state.cash_by_player_id[0], cash_before + expected_proceeds);
            assert_eq!(state.board.improvement_level_by_property_id[PARK_PLACE_PROPERTY_ID as usize], expected_level);
            assert_eq!(state.board.bank_hotel_count, BANK_STARTING_HOTEL_COUNT);
            assert_eq!(state.board.bank_house_count, 0);
        }
    }

    #[test]
    fn rejects_invalid_building_actions_without_mutation() {
        let ruleset = Ruleset::default();
        let mut state = create_dark_blue_owner_state(&ruleset);
        let original = state;
        for (player, property) in [(255, 20), (0, 255), (1, 20)] {
            assert!(!improve_property(&mut state, &ruleset, player, property));
            assert_eq!(state, original);
        }
        state.bankrupt_players |= 1;
        let original = state;
        assert!(!improve_property(&mut state, &ruleset, 0, 20));
        assert_eq!(state, original);
    }
}
