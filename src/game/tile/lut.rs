use super::data::{
    FREE_PARKING_TILE_ID,
    GO_TILE_ID,
    GO_TO_JAIL_TILE_ID,
    JAIL_TILE_ID,
    RAILROAD_RENT_BY_POSSESSION_COUNT,
    TILE_COUNT,
    TILE_DEFINITIONS,
    UNIMPROVED_MONOPOLY_RENT_MULTIPLIER,
    UTILITY_RENT_DICE_MULTIPLIER_BY_POSSESSION_COUNT,
};
use super::model::{
    Money,
    OwnershipGroup,
    RentLevel,
    TileDefinitionKind,
    TileKind,
    TileSetMask,
};

pub const UNIMPROVED_RENT_LEVEL: RentLevel = 0;
pub const UNIMPROVED_MONOPOLY_RENT_LEVEL: RentLevel = 1;
pub const ONE_HOUSE_RENT_LEVEL: RentLevel = 2;
pub const TWO_HOUSES_RENT_LEVEL: RentLevel = 3;
pub const THREE_HOUSES_RENT_LEVEL: RentLevel = 4;
pub const FOUR_HOUSES_RENT_LEVEL: RentLevel = 5;
pub const HOTEL_RENT_LEVEL: RentLevel = 6;

// 7 (above) * 2 byte cells = 14 bytes. 2 bytes padding for easy alignment & shift math
pub const RENT_LEVEL_COUNT: usize = 8;

macro_rules! build_tile_id_lut {
    ($initial_value:expr, |$tile_definition:ident| $derive:expr) => {{
        let mut table = [$initial_value; TILE_COUNT];
        let mut tile_id = 0;
        while tile_id < TILE_COUNT {
            let $tile_definition = &TILE_DEFINITIONS[tile_id];
            table[tile_id] = $derive;
            tile_id += 1;
        }

        table
    }};
}

pub static TILE_NAME_BY_TILE_ID: [&str; TILE_COUNT] = build_tile_id_lut!("", |tile_definition| tile_definition.name);
pub static TILE_KIND_BY_TILE_ID: [TileKind; TILE_COUNT] = build_tile_id_lut!(TileKind::Go, |tile_definition| tile_definition.kind.tile_kind());
pub static OWNERSHIP_GROUP_BY_TILE_ID: [Option<OwnershipGroup>; TILE_COUNT] = build_tile_id_lut!(None, |tile_definition| tile_definition.kind.ownership_group());
pub static PURCHASE_PRICE_BY_TILE_ID: [Money; TILE_COUNT] = build_tile_id_lut!(0, |tile_definition| tile_definition.kind.purchase_price());
pub static MORTGAGE_VALUE_BY_TILE_ID: [Money; TILE_COUNT] = build_tile_id_lut!(0, |tile_definition| tile_definition.kind.calculate_mortgage_value());
pub static UNMORTGAGE_PRICE_BY_TILE_ID: [Money; TILE_COUNT] = build_tile_id_lut!(0, |tile_definition| tile_definition.kind.calculate_unmortgage_price());
pub static HOUSE_PURCHASE_PRICE_BY_TILE_ID: [Money; TILE_COUNT] = build_tile_id_lut!(0, |tile_definition| tile_definition.kind.house_purchase_price());
pub static TAX_AMOUNT_BY_TILE_ID: [Money; TILE_COUNT] = build_tile_id_lut!(0, |tile_definition| tile_definition.kind.tax_amount());

pub static RENT_BY_TILE_ID_BY_RENT_LEVEL: [[Money; RENT_LEVEL_COUNT]; TILE_COUNT] = build_tile_id_lut!([0; RENT_LEVEL_COUNT], |tile_definition| derive_rent_by_rent_level(
    &tile_definition.kind
));

pub const TILE_SET_MASK_BY_OWNERSHIP_GROUP: [TileSetMask; OwnershipGroup::COUNT] = {
    let mut tile_id = 0;
    let mut tiles_by_ownership_group = [0; OwnershipGroup::COUNT];
    while tile_id < TILE_COUNT {
        if let Some(ownership_group) = TILE_DEFINITIONS[tile_id].kind.ownership_group() {
            tiles_by_ownership_group[ownership_group as usize] |= 1 << tile_id;
        }

        tile_id += 1;
    }

    tiles_by_ownership_group
};

pub const RAILROAD_TILE_SET_MASK: TileSetMask = TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Railroad as usize];
pub const UTILITY_TILE_SET_MASK: TileSetMask = TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Utility as usize];

pub const OWNABLE_TILE_SET_MASK: TileSetMask = {
    let mut ownership_group_index = 0;
    let mut ownable_tiles = 0;
    while ownership_group_index < OwnershipGroup::COUNT {
        ownable_tiles |= TILE_SET_MASK_BY_OWNERSHIP_GROUP[ownership_group_index];
        ownership_group_index += 1;
    }

    ownable_tiles
};

const fn derive_rent_by_rent_level(tile_definition_kind: &TileDefinitionKind) -> [Money; RENT_LEVEL_COUNT] {
    match *tile_definition_kind {
        TileDefinitionKind::Property { rent, .. } => {
            let mut rent_by_rent_level = [0; RENT_LEVEL_COUNT];
            rent_by_rent_level[UNIMPROVED_RENT_LEVEL] = rent.unimproved;
            rent_by_rent_level[UNIMPROVED_MONOPOLY_RENT_LEVEL] = rent.unimproved * UNIMPROVED_MONOPOLY_RENT_MULTIPLIER;
            rent_by_rent_level[ONE_HOUSE_RENT_LEVEL] = rent.one_house;
            rent_by_rent_level[TWO_HOUSES_RENT_LEVEL] = rent.two_houses;
            rent_by_rent_level[THREE_HOUSES_RENT_LEVEL] = rent.three_houses;
            rent_by_rent_level[FOUR_HOUSES_RENT_LEVEL] = rent.four_houses;
            rent_by_rent_level[HOTEL_RENT_LEVEL] = rent.hotel;

            rent_by_rent_level
        },
        TileDefinitionKind::Railroad => pad_to_rent_level_count(RAILROAD_RENT_BY_POSSESSION_COUNT),
        TileDefinitionKind::Utility => pad_to_rent_level_count(UTILITY_RENT_DICE_MULTIPLIER_BY_POSSESSION_COUNT),
        _ => [0; RENT_LEVEL_COUNT],
    }
}

const fn pad_to_rent_level_count<const VALUE_COUNT: usize>(values: [u16; VALUE_COUNT]) -> [u16; RENT_LEVEL_COUNT] {
    let mut padded_values = [0; RENT_LEVEL_COUNT];
    let mut value_index = 0;
    while value_index < VALUE_COUNT {
        padded_values[value_index] = values[value_index];
        value_index += 1;
    }

    padded_values
}

const _: () = {
    assert!(TILE_COUNT <= TileSetMask::BITS as usize);

    assert!(matches!(TILE_DEFINITIONS[GO_TILE_ID as usize].kind, TileDefinitionKind::Go));
    assert!(matches!(TILE_DEFINITIONS[JAIL_TILE_ID as usize].kind, TileDefinitionKind::Jail));
    assert!(matches!(
        TILE_DEFINITIONS[FREE_PARKING_TILE_ID as usize].kind,
        TileDefinitionKind::FreeParking
    ));
    assert!(matches!(
        TILE_DEFINITIONS[GO_TO_JAIL_TILE_ID as usize].kind,
        TileDefinitionKind::GoToJail
    ));

    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Brown as usize].count_ones() == 2);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::LightBlue as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Pink as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Orange as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Red as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Yellow as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Green as usize].count_ones() == 3);
    assert!(TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::DarkBlue as usize].count_ones() == 2);
    assert!(RAILROAD_TILE_SET_MASK.count_ones() == 4);
    assert!(UTILITY_TILE_SET_MASK.count_ones() == 2);

    let mut chance_tile_count = 0;
    let mut community_chest_tile_count = 0;
    let mut tax_tile_count = 0;

    let mut tile_id = 0;
    while tile_id < TILE_COUNT {
        match TILE_DEFINITIONS[tile_id].kind {
            TileDefinitionKind::Property { purchase_price, rent, .. } => {
                assert!(purchase_price > 0, "property purchase price should be positive");
                assert!(
                    rent.unimproved < rent.one_house
                        && rent.one_house < rent.two_houses
                        && rent.two_houses < rent.three_houses
                        && rent.three_houses < rent.four_houses
                        && rent.four_houses < rent.hotel,
                    "property rent should increase with each improvement"
                );
            },
            TileDefinitionKind::Chance => chance_tile_count += 1,
            TileDefinitionKind::CommunityChest => community_chest_tile_count += 1,
            TileDefinitionKind::Tax { .. } => tax_tile_count += 1,
            _ => {},
        }

        tile_id += 1;
    }

    assert!(chance_tile_count == 3);
    assert!(community_chest_tile_count == 3);
    assert!(tax_tile_count == 2);
};

#[cfg(test)]
mod tests {
    use super::*;

    const BALTIC_AVENUE_TILE_ID: usize = 3;
    const READING_RAILROAD_TILE_ID: usize = 5;
    const ELECTRIC_COMPANY_TILE_ID: usize = 12;
    const PARK_PLACE_TILE_ID: usize = 37;
    const LUXURY_TAX_TILE_ID: usize = 38;
    const BOARDWALK_TILE_ID: usize = 39;

    #[test]
    fn derives_property_tables() {
        assert_eq!(TILE_KIND_BY_TILE_ID[BOARDWALK_TILE_ID], TileKind::Property);
        assert_eq!(OWNERSHIP_GROUP_BY_TILE_ID[BOARDWALK_TILE_ID], Some(OwnershipGroup::DarkBlue));
        assert_eq!(PURCHASE_PRICE_BY_TILE_ID[BOARDWALK_TILE_ID], 400);
        assert_eq!(HOUSE_PURCHASE_PRICE_BY_TILE_ID[BOARDWALK_TILE_ID], 200);
        assert_eq!(
            RENT_BY_TILE_ID_BY_RENT_LEVEL[BOARDWALK_TILE_ID],
            [50, 100, 200, 600, 1400, 1700, 2000, 0]
        );
        assert_eq!(
            RENT_BY_TILE_ID_BY_RENT_LEVEL[BALTIC_AVENUE_TILE_ID][UNIMPROVED_MONOPOLY_RENT_LEVEL],
            8
        );
    }

    #[test]
    fn rounds_unmortgage_interest_up() {
        assert_eq!(MORTGAGE_VALUE_BY_TILE_ID[PARK_PLACE_TILE_ID], 175);
        assert_eq!(UNMORTGAGE_PRICE_BY_TILE_ID[PARK_PLACE_TILE_ID], 193);
        assert_eq!(UNMORTGAGE_PRICE_BY_TILE_ID[BOARDWALK_TILE_ID], 220);
        assert_eq!(UNMORTGAGE_PRICE_BY_TILE_ID[ELECTRIC_COMPANY_TILE_ID], 83);
    }

    #[test]
    fn derives_non_property_tables() {
        assert_eq!(RENT_BY_TILE_ID_BY_RENT_LEVEL[READING_RAILROAD_TILE_ID][..4], [25, 50, 100, 200]);
        assert_eq!(RENT_BY_TILE_ID_BY_RENT_LEVEL[ELECTRIC_COMPANY_TILE_ID][..2], [4, 10]);
        assert_eq!(TAX_AMOUNT_BY_TILE_ID[LUXURY_TAX_TILE_ID], 100);
        assert_eq!(PURCHASE_PRICE_BY_TILE_ID[LUXURY_TAX_TILE_ID], 0);
        assert_eq!(OWNERSHIP_GROUP_BY_TILE_ID[LUXURY_TAX_TILE_ID], None);
    }

    #[test]
    fn derives_tiles() {
        assert_eq!(
            TILE_SET_MASK_BY_OWNERSHIP_GROUP[OwnershipGroup::Brown as usize],
            1 << 1 | 1 << 3
        );
        assert_eq!(RAILROAD_TILE_SET_MASK, 1 << 5 | 1 << 15 | 1 << 25 | 1 << 35);
        assert_eq!(UTILITY_TILE_SET_MASK, 1 << 12 | 1 << 28);
        assert_eq!(OWNABLE_TILE_SET_MASK.count_ones(), 28);
    }
}
