use super::model::{
    AuctionEligibility,
    FreeParkingJackpotMode,
    PermittedBarterTacticsMask,
    PermittedBarterTimesMask,
    PropertyImprovementDistribution,
    PropertyPurchaseDeclineMode,
    Ruleset,
    RulesetBuilder,
    TurnCount,
};
use crate::game::tile::model::Money;

pub const OFFICIAL_STARTING_PLAYER_MONEY: Money = 1500;

pub const OFFICIAL_GO_PASSING_SALARY: Money = 200;
pub const OFFICIAL_GO_LANDING_SALARY: Money = 200;

pub const OFFICIAL_PROPERTY_PURCHASE_DECLINE_MODE: PropertyPurchaseDeclineMode = PropertyPurchaseDeclineMode::Auction;
pub const OFFICIAL_AUCTION_ELIGIBILITY: AuctionEligibility = AuctionEligibility::AllPlayers;

pub const OFFICIAL_PERMITTED_BARTER_TACTICS: PermittedBarterTacticsMask = PermittedBarterTacticsMask::MONEY
    .union(PermittedBarterTacticsMask::UNMORTGAGED_PROPERTIES)
    .union(PermittedBarterTacticsMask::MORTGAGED_PROPERTIES)
    .union(PermittedBarterTacticsMask::GET_OUT_OF_JAIL_FREE_CARDS);

pub const OFFICIAL_PERMITTED_BARTER_TIMES: PermittedBarterTimesMask = PermittedBarterTimesMask::START_OF_TURN
    .union(PermittedBarterTimesMask::BEFORE_PURCHASE)
    .union(PermittedBarterTimesMask::DURING_PAYMENT);

pub const OFFICIAL_PROPERTY_IMPROVEMENT_DISTRIBUTION: PropertyImprovementDistribution = PropertyImprovementDistribution::Even;

pub const OFFICIAL_FREE_PARKING_JACKPOT_MODE: FreeParkingJackpotMode = FreeParkingJackpotMode::Disabled;

pub const OFFICIAL_JAIL_BAIL_AMOUNT: Money = 50;
pub const OFFICIAL_MAX_JAIL_TURN_COUNT: TurnCount = 3;

pub const OFFICIAL_RULESET: Ruleset = RulesetBuilder::new().build();

pub const DEX_RULESET: Ruleset = RulesetBuilder::new()
    .with_starting_player_money(1800)
    .with_free_parking_jackpot_mode(FreeParkingJackpotMode::TaxesAndFees)
    .with_go_landing_salary(0)
    .with_go_passing_salary(200)
    .with_permitted_barter_tactics(
        PermittedBarterTacticsMask::MONEY
            .union(PermittedBarterTacticsMask::UNMORTGAGED_PROPERTIES)
            .union(PermittedBarterTacticsMask::MORTGAGED_PROPERTIES)
            .union(PermittedBarterTacticsMask::GET_OUT_OF_JAIL_FREE_CARDS)
            .union(PermittedBarterTacticsMask::REVENUE_SHARING)
            .union(PermittedBarterTacticsMask::MODIFIED_RENT_PAYMENTS),
    )
    .with_permitted_barter_times(PermittedBarterTimesMask::START_OF_TURN)
    .with_property_improvement_distribution(PropertyImprovementDistribution::Arbitrary)
    .with_jail_bail_amount(100)
    .build();
