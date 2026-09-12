use super::model::{
    CardDefinition,
    CardEffect,
};
use crate::game::tile::data::{
    BOARDWALK_TILE_ID,
    GO_TILE_ID,
    ILLINOIS_AVENUE_TILE_ID,
    READING_RAILROAD_TILE_ID,
    ST_CHARLES_PLACE_TILE_ID,
};

pub const CARD_COUNT_PER_DECK: usize = 16;

pub const NEAREST_RAILROAD_RENT_MULTIPLIER: u16 = 2;
pub const NEAREST_UTILITY_RENT_DICE_MULTIPLIER: u16 = 10;

pub const CHANCE_CARD_DEFINITIONS: [CardDefinition; CARD_COUNT_PER_DECK] = [
    CardDefinition {
        text: "Advance to Boardwalk",
        effect: CardEffect::AdvanceToTile { tile_id: BOARDWALK_TILE_ID },
    },
    CardDefinition {
        text: "Advance to Go (Collect $200)",
        effect: CardEffect::AdvanceToTile { tile_id: GO_TILE_ID },
    },
    CardDefinition {
        text: "Advance to Illinois Avenue. If you pass Go, collect $200",
        effect: CardEffect::AdvanceToTile { tile_id: ILLINOIS_AVENUE_TILE_ID },
    },
    CardDefinition {
        text: "Advance to St. Charles Place. If you pass Go, collect $200",
        effect: CardEffect::AdvanceToTile { tile_id: ST_CHARLES_PLACE_TILE_ID },
    },
    CardDefinition {
        text: "Advance to the nearest Railroad. If owned, pay owner twice the rental to which they are otherwise entitled",
        effect: CardEffect::AdvanceToNearestRailroad,
    },
    CardDefinition {
        text: "Advance to the nearest Railroad. If owned, pay owner twice the rental to which they are otherwise entitled",
        effect: CardEffect::AdvanceToNearestRailroad,
    },
    CardDefinition {
        text: "Advance token to nearest Utility. If owned, throw dice and pay owner a total ten times amount thrown",
        effect: CardEffect::AdvanceToNearestUtility,
    },
    CardDefinition {
        text: "Bank pays you dividend of $50",
        effect: CardEffect::CollectFromBank { amount: 50 },
    },
    CardDefinition {
        text: "Get Out of Jail Free",
        effect: CardEffect::GetOutOfJailFree,
    },
    CardDefinition {
        text: "Go Back 3 Spaces",
        effect: CardEffect::MoveBackward { tile_count: 3 },
    },
    CardDefinition {
        text: "Go to Jail. Go directly to Jail, do not pass Go, do not collect $200",
        effect: CardEffect::GoToJail,
    },
    CardDefinition {
        text: "Make general repairs on all your property. For each house pay $25. For each hotel pay $100",
        effect: CardEffect::PayForRepairs {
            amount_per_house: 25,
            amount_per_hotel: 100,
        },
    },
    CardDefinition {
        text: "Speeding fine $15",
        effect: CardEffect::PayBank { amount: 15 },
    },
    CardDefinition {
        text: "Take a trip to Reading Railroad. If you pass Go, collect $200",
        effect: CardEffect::AdvanceToTile { tile_id: READING_RAILROAD_TILE_ID },
    },
    CardDefinition {
        text: "You have been elected Chairman of the Board. Pay each player $50",
        effect: CardEffect::PayEachPlayer { amount: 50 },
    },
    CardDefinition {
        text: "Your building loan matures. Collect $150",
        effect: CardEffect::CollectFromBank { amount: 150 },
    },
];

pub const COMMUNITY_CHEST_CARD_DEFINITIONS: [CardDefinition; CARD_COUNT_PER_DECK] = [
    CardDefinition {
        text: "Advance to Go (Collect $200)",
        effect: CardEffect::AdvanceToTile { tile_id: GO_TILE_ID },
    },
    CardDefinition {
        text: "Bank error in your favor. Collect $200",
        effect: CardEffect::CollectFromBank { amount: 200 },
    },
    CardDefinition {
        text: "Doctor's fee. Pay $50",
        effect: CardEffect::PayBank { amount: 50 },
    },
    CardDefinition {
        text: "From sale of stock you get $50",
        effect: CardEffect::CollectFromBank { amount: 50 },
    },
    CardDefinition {
        text: "Get Out of Jail Free",
        effect: CardEffect::GetOutOfJailFree,
    },
    CardDefinition {
        text: "Go to Jail. Go directly to jail, do not pass Go, do not collect $200",
        effect: CardEffect::GoToJail,
    },
    CardDefinition {
        text: "Holiday fund matures. Receive $100",
        effect: CardEffect::CollectFromBank { amount: 100 },
    },
    CardDefinition {
        text: "Income tax refund. Collect $20",
        effect: CardEffect::CollectFromBank { amount: 20 },
    },
    CardDefinition {
        text: "It is your birthday. Collect $10 from every player",
        effect: CardEffect::CollectFromEachPlayer { amount: 10 },
    },
    CardDefinition {
        text: "Life insurance matures. Collect $100",
        effect: CardEffect::CollectFromBank { amount: 100 },
    },
    CardDefinition {
        text: "Pay hospital fees of $100",
        effect: CardEffect::PayBank { amount: 100 },
    },
    CardDefinition {
        text: "Pay school fees of $50",
        effect: CardEffect::PayBank { amount: 50 },
    },
    CardDefinition {
        text: "Receive $25 consultancy fee",
        effect: CardEffect::CollectFromBank { amount: 25 },
    },
    CardDefinition {
        text: "You are assessed for street repair. $40 per house. $115 per hotel",
        effect: CardEffect::PayForRepairs {
            amount_per_house: 40,
            amount_per_hotel: 115,
        },
    },
    CardDefinition {
        text: "You have won second prize in a beauty contest. Collect $10",
        effect: CardEffect::CollectFromBank { amount: 10 },
    },
    CardDefinition {
        text: "You inherit $100",
        effect: CardEffect::CollectFromBank { amount: 100 },
    },
];
