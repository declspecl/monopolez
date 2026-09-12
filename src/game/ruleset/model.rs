use bitflags::bitflags;
use serde::{
    Deserialize,
    Serialize,
};

use super::data::{
    OFFICIAL_AUCTION_ELIGIBILITY,
    OFFICIAL_FREE_PARKING_JACKPOT_MODE,
    OFFICIAL_GO_LANDING_SALARY,
    OFFICIAL_GO_PASSING_SALARY,
    OFFICIAL_JAIL_BAIL_AMOUNT,
    OFFICIAL_MAX_JAIL_TURN_COUNT,
    OFFICIAL_PERMITTED_BARTER_TACTICS,
    OFFICIAL_PERMITTED_BARTER_TIMES,
    OFFICIAL_PROPERTY_IMPROVEMENT_DISTRIBUTION,
    OFFICIAL_PROPERTY_PURCHASE_DECLINE_MODE,
    OFFICIAL_STARTING_PLAYER_MONEY,
};
use crate::game::tile::model::Money;

// official max jail turns (3) < 255
pub type TurnCount = u8;

bitflags! {
    /// A bitmask representing the permitted barter tactics
    ///
    /// bit        0     1     2     3     4     5     6     7
    ///         ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐
    ///         │  A  │  B  │  C  │  D  │  E  │  F  │  G  │     │
    ///         └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘
    ///
    /// tactics:
    /// - A: money
    /// - B: unmortgaged properties
    /// - C: mortgaged properties
    /// - D: get out of jail free cards
    /// - E: modified rent payments
    /// - F: IOUs / loans
    /// - G: revenue sharing
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct PermittedBarterTacticsMask: u8 {
        const MONEY = 1 << 0;
        const UNMORTGAGED_PROPERTIES = 1 << 1;
        const MORTGAGED_PROPERTIES = 1 << 2;
        const GET_OUT_OF_JAIL_FREE_CARDS = 1 << 3;
        const MODIFIED_RENT_PAYMENTS = 1 << 4;
        const IOUS_AND_LOANS = 1 << 5;
        const REVENUE_SHARING = 1 << 6;
    }

    /// A bitmask representing the permitted times players can initiate a barter
    ///
    /// bit        0     1     2     3     4     5     6     7
    ///         ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐
    ///         │  A  │  B  │  C  │     │     │     │     │     │
    ///         └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘
    ///
    /// times:
    /// - A: start of turn
    /// - B: before purchase
    /// - C: during payment
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct PermittedBarterTimesMask: u8 {
        const START_OF_TURN = 1 << 0;
        const BEFORE_PURCHASE = 1 << 1;
        const DURING_PAYMENT = 1 << 2;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ruleset {
    pub starting_player_money: Money,
    pub go_passing_salary: Money,
    pub go_landing_salary: Money,
    pub property_purchase_decline_mode: PropertyPurchaseDeclineMode,
    pub auction_eligibility: AuctionEligibility,
    pub permitted_barter_tactics: PermittedBarterTacticsMask,
    pub permitted_barter_times: PermittedBarterTimesMask,
    pub property_improvement_distribution: PropertyImprovementDistribution,
    pub free_parking_jackpot_mode: FreeParkingJackpotMode,
    pub jail_bail_amount: Money,
    pub max_jail_turn_count: TurnCount,
}

impl Ruleset {
    pub const fn builder() -> RulesetBuilder {
        RulesetBuilder::new()
    }
}

impl Default for Ruleset {
    fn default() -> Self {
        RulesetBuilder::default().build()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RulesetBuilder {
    ruleset: Ruleset,
}

impl RulesetBuilder {
    pub const fn new() -> Self {
        Self {
            ruleset: Ruleset {
                starting_player_money: OFFICIAL_STARTING_PLAYER_MONEY,
                go_passing_salary: OFFICIAL_GO_PASSING_SALARY,
                go_landing_salary: OFFICIAL_GO_LANDING_SALARY,
                property_purchase_decline_mode: OFFICIAL_PROPERTY_PURCHASE_DECLINE_MODE,
                auction_eligibility: OFFICIAL_AUCTION_ELIGIBILITY,
                permitted_barter_tactics: OFFICIAL_PERMITTED_BARTER_TACTICS,
                permitted_barter_times: OFFICIAL_PERMITTED_BARTER_TIMES,
                property_improvement_distribution: OFFICIAL_PROPERTY_IMPROVEMENT_DISTRIBUTION,
                free_parking_jackpot_mode: OFFICIAL_FREE_PARKING_JACKPOT_MODE,
                jail_bail_amount: OFFICIAL_JAIL_BAIL_AMOUNT,
                max_jail_turn_count: OFFICIAL_MAX_JAIL_TURN_COUNT,
            },
        }
    }

    pub const fn with_starting_player_money(
        mut self,
        starting_player_money: Money,
    ) -> Self {
        self.ruleset.starting_player_money = starting_player_money;
        self
    }

    pub const fn with_go_passing_salary(
        mut self,
        go_passing_salary: Money,
    ) -> Self {
        self.ruleset.go_passing_salary = go_passing_salary;
        self
    }

    pub const fn with_go_landing_salary(
        mut self,
        go_landing_salary: Money,
    ) -> Self {
        self.ruleset.go_landing_salary = go_landing_salary;
        self
    }

    pub const fn with_property_purchase_decline_mode(
        mut self,
        property_purchase_decline_mode: PropertyPurchaseDeclineMode,
    ) -> Self {
        self.ruleset.property_purchase_decline_mode = property_purchase_decline_mode;
        self
    }

    pub const fn with_auction_eligibility(
        mut self,
        auction_eligibility: AuctionEligibility,
    ) -> Self {
        self.ruleset.auction_eligibility = auction_eligibility;
        self
    }

    pub const fn with_permitted_barter_tactics(
        mut self,
        permitted_barter_tactics: PermittedBarterTacticsMask,
    ) -> Self {
        self.ruleset.permitted_barter_tactics = permitted_barter_tactics;
        self
    }

    pub const fn with_permitted_barter_times(
        mut self,
        permitted_barter_times: PermittedBarterTimesMask,
    ) -> Self {
        self.ruleset.permitted_barter_times = permitted_barter_times;
        self
    }

    pub const fn with_property_improvement_distribution(
        mut self,
        property_improvement_distribution: PropertyImprovementDistribution,
    ) -> Self {
        self.ruleset.property_improvement_distribution = property_improvement_distribution;
        self
    }

    pub const fn with_free_parking_jackpot_mode(
        mut self,
        free_parking_jackpot_mode: FreeParkingJackpotMode,
    ) -> Self {
        self.ruleset.free_parking_jackpot_mode = free_parking_jackpot_mode;
        self
    }

    pub const fn with_jail_bail_amount(
        mut self,
        jail_bail_amount: Money,
    ) -> Self {
        self.ruleset.jail_bail_amount = jail_bail_amount;
        self
    }

    pub const fn with_max_jail_turn_count(
        mut self,
        max_jail_turn_count: TurnCount,
    ) -> Self {
        self.ruleset.max_jail_turn_count = max_jail_turn_count;
        self
    }

    pub const fn build(self) -> Ruleset {
        self.ruleset
    }
}

impl Default for RulesetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PropertyPurchaseDeclineMode {
    Auction,
    RemainsUnowned,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuctionEligibility {
    AllPlayers,
    ExcludingDecliningPlayer,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PropertyImprovementDistribution {
    Even,
    Arbitrary,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FreeParkingJackpotMode {
    Disabled,
    TaxesAndFees,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_official_rules() {
        let ruleset = Ruleset::default();

        assert_eq!(ruleset.starting_player_money, 1500);
        assert_eq!(ruleset.go_passing_salary, 200);
        assert_eq!(ruleset.go_landing_salary, 200);
        assert_eq!(ruleset.property_purchase_decline_mode, PropertyPurchaseDeclineMode::Auction);
        assert_eq!(ruleset.auction_eligibility, AuctionEligibility::AllPlayers);
        assert!(ruleset.permitted_barter_tactics.contains(PermittedBarterTacticsMask::MORTGAGED_PROPERTIES));
        assert!(!ruleset.permitted_barter_tactics.contains(PermittedBarterTacticsMask::IOUS_AND_LOANS));
        assert_eq!(ruleset.property_improvement_distribution, PropertyImprovementDistribution::Even);
        assert_eq!(ruleset.free_parking_jackpot_mode, FreeParkingJackpotMode::Disabled);
        assert_eq!(ruleset.jail_bail_amount, 50);
        assert_eq!(ruleset.max_jail_turn_count, 3);
    }

    #[test]
    fn overrides_only_set_fields() {
        let ruleset = Ruleset::builder()
            .with_starting_player_money(2000)
            .with_property_purchase_decline_mode(PropertyPurchaseDeclineMode::RemainsUnowned)
            .build();

        assert_eq!(ruleset.starting_player_money, 2000);
        assert_eq!(ruleset.property_purchase_decline_mode, PropertyPurchaseDeclineMode::RemainsUnowned);
        assert_eq!(ruleset.permitted_barter_times, Ruleset::default().permitted_barter_times);
    }

    #[test]
    fn round_trips_through_json() {
        let ruleset = Ruleset::builder()
            .with_go_landing_salary(400)
            .with_free_parking_jackpot_mode(FreeParkingJackpotMode::TaxesAndFees)
            .build();

        let serialized_ruleset = serde_json::to_string(&ruleset).expect("ruleset should serialize");
        let deserialized_ruleset: Ruleset = serde_json::from_str(&serialized_ruleset).expect("ruleset should deserialize");

        assert_eq!(ruleset, deserialized_ruleset);
    }

    #[test]
    fn applies_common_house_rules() {
        let ruleset = Ruleset::builder()
            .with_go_landing_salary(400)
            .with_free_parking_jackpot_mode(FreeParkingJackpotMode::TaxesAndFees)
            .with_auction_eligibility(AuctionEligibility::ExcludingDecliningPlayer)
            .build();

        assert_eq!(ruleset.go_passing_salary, 200);
        assert_eq!(ruleset.go_landing_salary, 400);
        assert_eq!(ruleset.free_parking_jackpot_mode, FreeParkingJackpotMode::TaxesAndFees);
        assert_eq!(ruleset.auction_eligibility, AuctionEligibility::ExcludingDecliningPlayer);
    }
}
