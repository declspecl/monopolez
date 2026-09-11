use super::model::{PropertyColor, PropertyRent, TileDefinition, TileDefinitionKind};

pub type TileId = u8;

pub const TILE_COUNT: usize = 40;

pub const GO_TILE_ID: TileId = 0;
pub const JAIL_TILE_ID: TileId = 10;
pub const FREE_PARKING_TILE_ID: TileId = 20;
pub const GO_TO_JAIL_TILE_ID: TileId = 30;

pub const RAILROAD_TILE_PURCHASE_PRICE: u16 = 200;
pub const RAILROAD_RENT_BY_POSSESSION_COUNT: [u16; 4] = [25, 50, 100, 200];

pub const UTILITY_TILE_PURCHASE_PRICE: u16 = 150;
pub const UTILITY_RENT_DICE_MULTIPLIER_BY_POSSESSION_COUNT: [u16; 2] = [4, 10];

pub const UNIMPROVED_MONOPOLY_RENT_MULTIPLIER: u16 = 2;

pub const MORTGAGE_VALUE_PERCENT_OF_PURCHASE_PRICE: u16 = 50;
pub const UNMORTGAGE_INTEREST_PERCENT_OF_MORTGAGE_VALUE: u16 = 10;

pub const TILE_DEFINITIONS: [TileDefinition; TILE_COUNT] = [
    TileDefinition {
        name: "Go",
        kind: TileDefinitionKind::Go,
    },
    TileDefinition {
        name: "Mediterranean Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Brown,
            purchase_price: 60,
            rent: PropertyRent {
                unimproved: 2,
                one_house: 10,
                two_houses: 30,
                three_houses: 90,
                four_houses: 160,
                hotel: 250,
            },
        },
    },
    TileDefinition {
        name: "Community Chest",
        kind: TileDefinitionKind::CommunityChest,
    },
    TileDefinition {
        name: "Baltic Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Brown,
            purchase_price: 60,
            rent: PropertyRent {
                unimproved: 4,
                one_house: 20,
                two_houses: 60,
                three_houses: 180,
                four_houses: 320,
                hotel: 450,
            },
        },
    },
    TileDefinition {
        name: "Income Tax",
        kind: TileDefinitionKind::Tax { amount: 200 },
    },
    TileDefinition {
        name: "Reading Railroad",
        kind: TileDefinitionKind::Railroad,
    },
    TileDefinition {
        name: "Oriental Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::LightBlue,
            purchase_price: 100,
            rent: PropertyRent {
                unimproved: 6,
                one_house: 30,
                two_houses: 90,
                three_houses: 270,
                four_houses: 400,
                hotel: 550,
            },
        },
    },
    TileDefinition {
        name: "Chance",
        kind: TileDefinitionKind::Chance,
    },
    TileDefinition {
        name: "Vermont Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::LightBlue,
            purchase_price: 100,
            rent: PropertyRent {
                unimproved: 6,
                one_house: 30,
                two_houses: 90,
                three_houses: 270,
                four_houses: 400,
                hotel: 550,
            },
        },
    },
    TileDefinition {
        name: "Connecticut Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::LightBlue,
            purchase_price: 120,
            rent: PropertyRent {
                unimproved: 8,
                one_house: 40,
                two_houses: 100,
                three_houses: 300,
                four_houses: 450,
                hotel: 600,
            },
        },
    },
    TileDefinition {
        name: "Jail",
        kind: TileDefinitionKind::Jail,
    },
    TileDefinition {
        name: "St. Charles Place",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Pink,
            purchase_price: 140,
            rent: PropertyRent {
                unimproved: 10,
                one_house: 50,
                two_houses: 150,
                three_houses: 450,
                four_houses: 625,
                hotel: 750,
            },
        },
    },
    TileDefinition {
        name: "Electric Company",
        kind: TileDefinitionKind::Utility,
    },
    TileDefinition {
        name: "States Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Pink,
            purchase_price: 140,
            rent: PropertyRent {
                unimproved: 10,
                one_house: 50,
                two_houses: 150,
                three_houses: 450,
                four_houses: 625,
                hotel: 750,
            },
        },
    },
    TileDefinition {
        name: "Virginia Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Pink,
            purchase_price: 160,
            rent: PropertyRent {
                unimproved: 12,
                one_house: 60,
                two_houses: 180,
                three_houses: 500,
                four_houses: 700,
                hotel: 900,
            },
        },
    },
    TileDefinition {
        name: "Pennsylvania Railroad",
        kind: TileDefinitionKind::Railroad,
    },
    TileDefinition {
        name: "St. James Place",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Orange,
            purchase_price: 180,
            rent: PropertyRent {
                unimproved: 14,
                one_house: 70,
                two_houses: 200,
                three_houses: 550,
                four_houses: 750,
                hotel: 950,
            },
        },
    },
    TileDefinition {
        name: "Community Chest",
        kind: TileDefinitionKind::CommunityChest,
    },
    TileDefinition {
        name: "Tennessee Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Orange,
            purchase_price: 180,
            rent: PropertyRent {
                unimproved: 14,
                one_house: 70,
                two_houses: 200,
                three_houses: 550,
                four_houses: 750,
                hotel: 950,
            },
        },
    },
    TileDefinition {
        name: "New York Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Orange,
            purchase_price: 200,
            rent: PropertyRent {
                unimproved: 16,
                one_house: 80,
                two_houses: 220,
                three_houses: 600,
                four_houses: 800,
                hotel: 1000,
            },
        },
    },
    TileDefinition {
        name: "Free Parking",
        kind: TileDefinitionKind::FreeParking,
    },
    TileDefinition {
        name: "Kentucky Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Red,
            purchase_price: 220,
            rent: PropertyRent {
                unimproved: 18,
                one_house: 90,
                two_houses: 250,
                three_houses: 700,
                four_houses: 875,
                hotel: 1050,
            },
        },
    },
    TileDefinition {
        name: "Chance",
        kind: TileDefinitionKind::Chance,
    },
    TileDefinition {
        name: "Indiana Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Red,
            purchase_price: 220,
            rent: PropertyRent {
                unimproved: 18,
                one_house: 90,
                two_houses: 250,
                three_houses: 700,
                four_houses: 875,
                hotel: 1050,
            },
        },
    },
    TileDefinition {
        name: "Illinois Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Red,
            purchase_price: 240,
            rent: PropertyRent {
                unimproved: 20,
                one_house: 100,
                two_houses: 300,
                three_houses: 750,
                four_houses: 925,
                hotel: 1100,
            },
        },
    },
    TileDefinition {
        name: "B. & O. Railroad",
        kind: TileDefinitionKind::Railroad,
    },
    TileDefinition {
        name: "Atlantic Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Yellow,
            purchase_price: 260,
            rent: PropertyRent {
                unimproved: 22,
                one_house: 110,
                two_houses: 330,
                three_houses: 800,
                four_houses: 975,
                hotel: 1150,
            },
        },
    },
    TileDefinition {
        name: "Ventnor Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Yellow,
            purchase_price: 260,
            rent: PropertyRent {
                unimproved: 22,
                one_house: 110,
                two_houses: 330,
                three_houses: 800,
                four_houses: 975,
                hotel: 1150,
            },
        },
    },
    TileDefinition {
        name: "Water Works",
        kind: TileDefinitionKind::Utility,
    },
    TileDefinition {
        name: "Marvin Gardens",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Yellow,
            purchase_price: 280,
            rent: PropertyRent {
                unimproved: 24,
                one_house: 120,
                two_houses: 360,
                three_houses: 850,
                four_houses: 1025,
                hotel: 1200,
            },
        },
    },
    TileDefinition {
        name: "Go To Jail",
        kind: TileDefinitionKind::GoToJail,
    },
    TileDefinition {
        name: "Pacific Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Green,
            purchase_price: 300,
            rent: PropertyRent {
                unimproved: 26,
                one_house: 130,
                two_houses: 390,
                three_houses: 900,
                four_houses: 1100,
                hotel: 1275,
            },
        },
    },
    TileDefinition {
        name: "North Carolina Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Green,
            purchase_price: 300,
            rent: PropertyRent {
                unimproved: 26,
                one_house: 130,
                two_houses: 390,
                three_houses: 900,
                four_houses: 1100,
                hotel: 1275,
            },
        },
    },
    TileDefinition {
        name: "Community Chest",
        kind: TileDefinitionKind::CommunityChest,
    },
    TileDefinition {
        name: "Pennsylvania Avenue",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::Green,
            purchase_price: 320,
            rent: PropertyRent {
                unimproved: 28,
                one_house: 150,
                two_houses: 450,
                three_houses: 1000,
                four_houses: 1200,
                hotel: 1400,
            },
        },
    },
    TileDefinition {
        name: "Short Line",
        kind: TileDefinitionKind::Railroad,
    },
    TileDefinition {
        name: "Chance",
        kind: TileDefinitionKind::Chance,
    },
    TileDefinition {
        name: "Park Place",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::DarkBlue,
            purchase_price: 350,
            rent: PropertyRent {
                unimproved: 35,
                one_house: 175,
                two_houses: 500,
                three_houses: 1100,
                four_houses: 1300,
                hotel: 1500,
            },
        },
    },
    TileDefinition {
        name: "Luxury Tax",
        kind: TileDefinitionKind::Tax { amount: 100 },
    },
    TileDefinition {
        name: "Boardwalk",
        kind: TileDefinitionKind::Property {
            color: PropertyColor::DarkBlue,
            purchase_price: 400,
            rent: PropertyRent {
                unimproved: 50,
                one_house: 200,
                two_houses: 600,
                three_houses: 1400,
                four_houses: 1700,
                hotel: 2000,
            },
        },
    },
];
