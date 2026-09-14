use serde::{
    Deserialize,
    Serialize,
};

use super::payment::Creditor;
use crate::game::board::model::PlayerId;
use crate::game::card::model::{
    CardEffect,
    DeckKind,
};
use crate::game::tile::model::{
    Cash,
    PropertyId,
    TileId,
};
use crate::game::trade::model::TradeOffer;

pub fn publish_event<const N: usize, S: crate::game::strategy::model::PlayerStrategy>(
    strategies: &mut [S; N],
    recorder: usize,
    event: GameEvent,
) {
    strategies[recorder].record_event(event);
    for strategy in strategies {
        strategy.observe_public_event(event);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GameEvent {
    SentToJail {
        player_id: PlayerId,
    },
    ReleasedFromJail {
        player_id: PlayerId,
    },
    JailCardUsed {
        player_id: PlayerId,
        deck: DeckKind,
    },
    TurnStarted {
        player_id: PlayerId,
    },
    DiceRolled {
        player_id: PlayerId,
        first: u8,
        second: u8,
    },
    Landed {
        player_id: PlayerId,
        tile_id: TileId,
    },
    CardDrawn {
        player_id: PlayerId,
        deck: DeckKind,
        effect: CardEffect,
    },
    PropertyPurchased {
        player_id: PlayerId,
        tile_id: TileId,
        price: Cash,
        auction: bool,
    },
    PaymentDue {
        player_id: PlayerId,
        creditor: Creditor,
        amount: Cash,
    },
    PaymentCompleted {
        player_id: PlayerId,
        creditor: Creditor,
        amount: Cash,
    },
    Bankrupt {
        player_id: PlayerId,
        creditor: Creditor,
        remaining_cash: Cash,
    },
    TradeExecuted {
        offer: TradeOffer,
    },
    BuildingPurchased {
        player_id: PlayerId,
        property_id: PropertyId,
        level: u8,
    },
    TileUnmortgaged {
        player_id: PlayerId,
        tile_id: TileId,
    },
    BuildingSold {
        player_id: PlayerId,
        property_id: PropertyId,
        previous_level: u8,
        level: u8,
        proceeds: Cash,
    },
    TileMortgaged {
        player_id: PlayerId,
        tile_id: TileId,
        proceeds: Cash,
    },
}

impl GameEvent {
    pub const fn trace_version(self) -> u32 {
        match self {
            Self::SentToJail { .. } | Self::ReleasedFromJail { .. } | Self::JailCardUsed { .. } => 5,
            Self::BuildingSold { .. } | Self::TileMortgaged { .. } => 3,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ruleset::model::Ruleset;
    use crate::game::state::model::GameState;
    use crate::game::strategy::configurable::ConfigurableStrategy;
    use crate::game::strategy::model::{
        JailAction,
        PlayerStrategy,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Observer {
        public: Vec<GameEvent>,
        diagnostic: Rc<RefCell<Vec<GameEvent>>>,
    }

    fn observers() -> [Observer; 2] {
        let diagnostic = Rc::new(RefCell::new(Vec::new()));
        core::array::from_fn(|_| Observer {
            public: Vec::new(),
            diagnostic: diagnostic.clone(),
        })
    }

    fn assert_public_events(
        strategies: &[Observer; 2],
        expected: &[GameEvent],
    ) {
        assert_eq!(*strategies[0].diagnostic.borrow(), expected);
        for strategy in strategies {
            assert_eq!(strategy.public, expected);
        }
    }

    fn rng_for_roll(double: bool) -> crate::game::rng::WyRand {
        (0..100).map(crate::game::rng::WyRand::new).find(|rng| rng.clone().roll_dice().is_double() == double).unwrap()
    }

    #[test]
    fn jail_entry_is_public_for_tiles_cards_and_three_doubles() {
        use super::super::landing::{
            RentModifier,
            resolve_landing,
        };
        use super::super::turn::{
            TurnPhase,
            advance_turn,
        };
        use crate::game::card::data::{
            CHANCE_CARD_DEFINITIONS,
            COMMUNITY_CHEST_CARD_DEFINITIONS,
        };
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let roll = crate::game::rng::DiceRoll { first_die: 1, second_die: 2 };
        for (tile, deck, definitions) in [(7, DeckKind::Chance, CHANCE_CARD_DEFINITIONS), (2, DeckKind::CommunityChest, COMMUNITY_CHEST_CARD_DEFINITIONS)] {
            let mut state = GameState::<2>::create_starting_state(&rules, 7);
            let mut strategies = observers();
            state.position_by_player_id[0] = tile;
            let deck_state = &mut state.deck_state_by_deck_kind[deck as usize];
            deck_state.next_draw_position = deck_state
                .card_id_by_draw_position
                .iter()
                .position(|id| definitions[*id as usize].effect == CardEffect::GoToJail)
                .unwrap() as u8;
            resolve_landing(&mut state, &rules, &mut strategies, 0, roll, RentModifier::Standard);
            assert_eq!(state.jailed_players, 1);
            assert_public_events(
                &strategies,
                &[
                    GameEvent::Landed { player_id: 0, tile_id: tile },
                    GameEvent::CardDrawn {
                        player_id: 0,
                        deck,
                        effect: CardEffect::GoToJail,
                    },
                    GameEvent::SentToJail { player_id: 0 },
                ],
            );
        }
        let mut state = GameState::<2>::create_starting_state(&rules, 7);
        let mut strategies = observers();
        state.position_by_player_id[0] = 30;
        resolve_landing(&mut state, &rules, &mut strategies, 0, roll, RentModifier::Standard);
        assert_public_events(&strategies, &[GameEvent::Landed { player_id: 0, tile_id: 30 }, GameEvent::SentToJail { player_id: 0 }]);

        let mut state = GameState::<2>::create_starting_state(&rules, 7);
        let mut strategies = observers();
        state.rng = rng_for_roll(true);
        let roll = state.rng.clone().roll_dice();
        state.consecutive_double_count = 2;
        assert_eq!(advance_turn(&mut state, &rules, &mut strategies, TurnPhase::Rolling), TurnPhase::End);
        assert_eq!(state.jailed_players, 1);
        assert_public_events(
            &strategies,
            &[
                GameEvent::DiceRolled {
                    player_id: 0,
                    first: roll.first_die,
                    second: roll.second_die,
                },
                GameEvent::SentToJail { player_id: 0 },
            ],
        );
    }

    #[test]
    fn jail_card_use_reveals_the_deck_before_release() {
        use super::super::action::execute_jail_action;
        let rules = crate::game::ruleset::data::DEX_RULESET;
        for deck in [DeckKind::Chance, DeckKind::CommunityChest] {
            let mut state = GameState::<2>::create_starting_state(&rules, 7);
            let mut strategies = observers();
            super::super::movement::send_player_to_jail(&mut state, 0);
            state.get_out_of_jail_free_card_holder_by_deck_kind[deck as usize] = Some(0);
            execute_jail_action(&mut state, &rules, &mut strategies, 0, JailAction::UseGetOutOfJailFreeCard).unwrap();
            assert_eq!(state.get_out_of_jail_free_card_holder_by_deck_kind, [None; 2]);
            assert_eq!(state.jailed_players, 0);
            assert_public_events(&strategies, &[GameEvent::JailCardUsed { player_id: 0, deck }, GameEvent::ReleasedFromJail { player_id: 0 }]);
        }
        let mut state = GameState::<2>::create_starting_state(&rules, 7);
        let mut strategies = observers();
        super::super::movement::send_player_to_jail(&mut state, 0);
        assert!(execute_jail_action(&mut state, &rules, &mut strategies, 0, JailAction::UseGetOutOfJailFreeCard).is_err());
        assert_public_events(&strategies, &[]);
    }

    #[test]
    fn bail_and_doubles_publish_release_but_failed_rolls_do_not() {
        use super::super::turn::apply_jail_decision;
        let rules = crate::game::ruleset::data::DEX_RULESET;
        for (action, double, last_turn, released) in [
            (JailAction::PayBail, false, false, true),
            (JailAction::RollForDoubles, true, false, true),
            (JailAction::RollForDoubles, false, true, true),
            (JailAction::RollForDoubles, false, false, false),
        ] {
            let mut state = GameState::<2>::create_starting_state(&rules, 7);
            let mut strategies = observers();
            super::super::movement::send_player_to_jail(&mut state, 0);
            state.rng = rng_for_roll(double);
            state.jail_turn_count_by_player_id[0] = if last_turn { rules.max_jail_turn_count - 1 } else { 0 };
            apply_jail_decision(&mut state, &rules, &mut strategies, action).unwrap();
            let events = strategies[0].diagnostic.borrow();
            assert_eq!(
                events.iter().filter(|event| matches!(event, GameEvent::ReleasedFromJail { player_id: 0 })).count(),
                usize::from(released)
            );
            assert_public_events(&strategies, &events);
        }
    }

    macro_rules! delegate {
        ($method:ident, $response:ty $(, $argument:ident: $kind:ty)*) => {
            fn $method<const N: usize>(&mut self, state: &GameState<N>, rules: &Ruleset, player: PlayerId, $($argument: $kind),*) -> $response {
                ConfigurableStrategy::new().$method(state, rules, player, $($argument),*)
            }
        };
    }

    impl PlayerStrategy for Observer {
        fn observe_public_event(
            &mut self,
            event: GameEvent,
        ) {
            self.public.push(event);
        }
        fn record_event(
            &mut self,
            event: GameEvent,
        ) {
            self.diagnostic.borrow_mut().push(event);
        }
        delegate!(should_purchase_property, bool, tile: TileId);
        delegate!(choose_max_auction_bid, Cash, tile: TileId);
        delegate!(choose_jail_action, JailAction);
        delegate!(choose_property_to_improve, Option<PropertyId>);
        delegate!(choose_tile_to_unmortgage, Option<TileId>);
        delegate!(propose_trade, Option<TradeOffer>);
        delegate!(should_accept_trade, bool, offer: &TradeOffer);
    }

    fn verify_observers<const N: usize>() {
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let diagnostic = Rc::new(RefCell::new(Vec::new()));
        let mut strategies: [Observer; N] = core::array::from_fn(|_| Observer {
            public: Vec::new(),
            diagnostic: diagnostic.clone(),
        });
        let mut state = GameState::<N>::create_starting_state(&rules, 3);
        super::super::turn::play_game(&mut state, &rules, &mut strategies, 1000);
        let events = diagnostic.borrow();
        assert!(!events.is_empty());
        assert!(events.iter().any(|event| matches!(event, GameEvent::CardDrawn { .. })));
        for strategy in strategies {
            assert_eq!(strategy.public, *events);
        }
        let mut expected = GameState::<N>::create_starting_state(&rules, 3);
        super::super::turn::play_game(&mut expected, &rules, &mut [ConfigurableStrategy::new(); N], 1000);
        assert_eq!(state, expected);
    }

    #[test]
    fn public_events_reach_every_player_without_duplicating_trace_records() {
        verify_observers::<2>();
        verify_observers::<4>();
        verify_observers::<8>();
    }

    #[test]
    fn liquidation_is_observed_by_creditors_and_other_players() {
        let rules = crate::game::ruleset::data::DEX_RULESET;
        let diagnostic = Rc::new(RefCell::new(Vec::new()));
        let mut strategies: [Observer; 4] = core::array::from_fn(|_| Observer {
            public: Vec::new(),
            diagnostic: diagnostic.clone(),
        });
        let mut state = GameState::<4>::create_starting_state(&rules, 3);
        state.cash_by_player_id[0] = 0;
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.improvement_level_by_property_id[0] = 1;
        state.board.bank_house_count -= 1;
        super::super::payment::charge_player(&mut state, &rules, &mut strategies, 0, 50, Creditor::Player(1));
        let events = diagnostic.borrow();
        assert!(events.iter().any(|event| matches!(event, GameEvent::BuildingSold { .. })));
        assert!(events.iter().any(|event| matches!(event, GameEvent::TileMortgaged { .. })));
        for strategy in strategies {
            assert_eq!(strategy.public, *events);
        }
    }
}
