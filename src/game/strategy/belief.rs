use super::observation::PublicObservation;
use crate::game::board::model::BoardState;
use crate::game::card::belief::DeckBelief;
use crate::game::card::model::DeckKind;
use crate::game::rng::WyRand;
use crate::game::state::model::GameState;
use crate::game::tile::data::PROPERTY_COUNT;

#[derive(Debug, Clone)]
pub struct RolloutBelief<'a> {
    observation: PublicObservation<'a>,
    decks: [DeckBelief; DeckKind::COUNT],
}

impl<'a> RolloutBelief<'a> {
    pub fn new(observation: PublicObservation<'a>) -> Result<Self, &'static str> {
        let count = observation.cash_by_player_id.len();
        if !(2..=8).contains(&count)
            || observation.owned_tiles_by_player_id.len() != count
            || observation.position_by_player_id.len() != count
            || observation.jail_turn_count_by_player_id.len() != count
            || observation.improvement_level_by_property_id.len() != PROPERTY_COUNT
            || observation.get_out_of_jail_free_card_holder_by_deck_kind.len() != DeckKind::COUNT
        {
            return Err("observation has unsupported or inconsistent dimensions");
        }
        let decks = [
            DeckBelief::from_start_history(DeckKind::Chance, observation.history)?,
            DeckBelief::from_start_history(DeckKind::CommunityChest, observation.history)?,
        ];
        if decks
            .iter()
            .zip(observation.get_out_of_jail_free_card_holder_by_deck_kind)
            .any(|(deck, holder)| deck.holder() != *holder)
        {
            return Err("public card history does not match observed jail card holders");
        }
        Ok(Self { observation, decks })
    }

    pub fn sample<const N: usize>(
        &self,
        rng: &mut WyRand,
    ) -> Result<GameState<N>, &'static str> {
        let observation = self.observation;
        if observation.cash_by_player_id.len() != N {
            return Err("rollout player count does not match observation");
        }
        let board = BoardState {
            owned_tiles_by_player_id: observation.owned_tiles_by_player_id.try_into().unwrap(),
            mortgaged_tiles: observation.mortgaged_tiles,
            improvement_level_by_property_id: observation.improvement_level_by_property_id.try_into().unwrap(),
            bank_house_count: observation.bank_house_count,
            bank_hotel_count: observation.bank_hotel_count,
        };
        Ok(GameState {
            board,
            cash_by_player_id: observation.cash_by_player_id.try_into().unwrap(),
            position_by_player_id: observation.position_by_player_id.try_into().unwrap(),
            jail_turn_count_by_player_id: observation.jail_turn_count_by_player_id.try_into().unwrap(),
            jailed_players: observation.jailed_players,
            bankrupt_players: observation.bankrupt_players,
            get_out_of_jail_free_card_holder_by_deck_kind: observation.get_out_of_jail_free_card_holder_by_deck_kind.try_into().unwrap(),
            deck_state_by_deck_kind: core::array::from_fn(|index| self.decks[index].sample(rng)),
            free_parking_jackpot: observation.free_parking_jackpot,
            current_player_id: observation.current_player_id,
            consecutive_double_count: observation.consecutive_double_count,
            rng: WyRand::new(rng.generate_u64()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::model::CardEffect;
    use crate::game::engine::event::GameEvent;
    use crate::game::ruleset::data::DEX_RULESET;

    fn verify_public_state<const N: usize>() {
        let mut state = GameState::<N>::create_starting_state(&DEX_RULESET, 7);
        state.cash_by_player_id = core::array::from_fn(|player| 700 + player as u32);
        state.position_by_player_id[0] = 17;
        state.position_by_player_id[N - 1] = 10;
        state.jailed_players = 1 << (N - 1);
        state.jail_turn_count_by_player_id[N - 1] = 1;
        state.board.owned_tiles_by_player_id[0] = (1 << 1) | (1 << 3);
        state.board.owned_tiles_by_player_id[1] = 1 << 5;
        state.board.mortgaged_tiles = 1 << 5;
        state.board.improvement_level_by_property_id[0] = 1;
        state.board.improvement_level_by_property_id[1] = 1;
        state.board.bank_house_count -= 2;
        state.get_out_of_jail_free_card_holder_by_deck_kind[0] = Some(0);
        state.free_parking_jackpot = 125;
        state.consecutive_double_count = 1;
        let history = [GameEvent::CardDrawn {
            player_id: 0,
            deck: DeckKind::Chance,
            effect: CardEffect::GetOutOfJailFree,
        }];
        let observation = PublicObservation::new(&state, &DEX_RULESET, 0, &history);
        let belief = RolloutBelief::new(observation).unwrap();
        let before = state;
        let mut rng = WyRand::new(99);
        let mut same_rng = rng;
        let mut previous = None;
        for _ in 0..8 {
            let mut sample = belief.sample::<N>(&mut rng).unwrap();
            assert_eq!(sample, belief.sample::<N>(&mut same_rng).unwrap());
            assert_eq!(
                serde_json::to_value(observation).unwrap(),
                serde_json::to_value(PublicObservation::new(&sample, &DEX_RULESET, 0, &history)).unwrap()
            );
            assert_ne!(sample.rng, state.rng);
            assert!(previous.is_none_or(|previous| sample != previous));
            previous = Some(sample);
            sample.cash_by_player_id[0] = 0;
            sample.rng.roll_dice();
            assert_eq!(state, before);
        }
        let mut changed_hidden = state;
        changed_hidden.rng = WyRand::new(1000);
        for deck in &mut changed_hidden.deck_state_by_deck_kind {
            deck.card_id_by_draw_position.reverse();
            deck.next_draw_position = 13;
        }
        let other = RolloutBelief::new(PublicObservation::new(&changed_hidden, &DEX_RULESET, 0, &history)).unwrap();
        assert_eq!(belief.sample::<N>(&mut WyRand::new(55)).unwrap(), other.sample::<N>(&mut WyRand::new(55)).unwrap());
    }

    #[test]
    fn samples_preserve_public_state_and_ignore_live_hidden_state() {
        verify_public_state::<2>();
        verify_public_state::<3>();
        verify_public_state::<4>();
        verify_public_state::<5>();
        verify_public_state::<6>();
        verify_public_state::<7>();
        verify_public_state::<8>();
    }

    #[test]
    fn invalid_dimensions_and_inconsistent_holders_are_rejected() {
        let state = GameState::<4>::create_starting_state(&DEX_RULESET, 7);
        let observation = PublicObservation::new(&state, &DEX_RULESET, 0, &[]);
        let belief = RolloutBelief::new(observation).unwrap();
        let mut rng = WyRand::new(17);
        let before = rng;
        assert!(belief.sample::<2>(&mut rng).is_err());
        assert_eq!(rng, before);
        let invalid = [
            PublicObservation {
                cash_by_player_id: &[],
                ..observation
            },
            PublicObservation {
                owned_tiles_by_player_id: &[],
                ..observation
            },
            PublicObservation {
                position_by_player_id: &[],
                ..observation
            },
            PublicObservation {
                jail_turn_count_by_player_id: &[],
                ..observation
            },
            PublicObservation {
                improvement_level_by_property_id: &[],
                ..observation
            },
            PublicObservation {
                get_out_of_jail_free_card_holder_by_deck_kind: &[],
                ..observation
            },
        ];
        for observation in invalid {
            assert!(RolloutBelief::new(observation).is_err());
        }
        let holders = [Some(0), None];
        assert!(
            RolloutBelief::new(PublicObservation {
                get_out_of_jail_free_card_holder_by_deck_kind: &holders,
                ..observation
            })
            .is_err()
        );
        let history = [GameEvent::CardDrawn {
            player_id: 0,
            deck: DeckKind::Chance,
            effect: CardEffect::GetOutOfJailFree,
        }];
        assert!(RolloutBelief::new(PublicObservation { history: &history, ..observation }).is_err());
    }

    #[test]
    fn observed_policies_can_resume_sampled_worlds_through_the_engine() {
        use super::super::observation::{
            ObservedPolicy,
            ObservedStrategy,
        };
        use crate::game::engine::action::{
            ManagementAction,
            ManagementPhase,
            legal_management_actions,
        };
        use crate::game::engine::turn::{
            TurnPhase,
            advance_turn,
            play_game,
        };
        use crate::game::strategy::configurable::ConfigurableStrategy;
        #[derive(Default)]
        struct Probe {
            sampled: bool,
        }
        impl ObservedPolicy for Probe {
            fn choose_management_action(
                &mut self,
                observation: PublicObservation<'_>,
                phase: ManagementPhase,
                legal: &[ManagementAction],
            ) -> Option<ManagementAction> {
                if self.sampled {
                    return None;
                }
                self.sampled = true;
                let belief = RolloutBelief::new(observation).unwrap();
                let mut rng = WyRand::new(99);
                for _ in 0..3 {
                    let mut state = belief.sample::<4>(&mut rng).unwrap();
                    assert_eq!(legal_management_actions(&state, observation.rules, observation.player_id, phase).iter().collect::<Vec<_>>(), legal);
                    let mut strategies = [ConfigurableStrategy::new(); 4];
                    let mut turn_phase = match phase {
                        ManagementPhase::Building => TurnPhase::Building,
                        ManagementPhase::Unmortgaging => TurnPhase::Unmortgaging,
                    };
                    while turn_phase != TurnPhase::Finished {
                        turn_phase = advance_turn(&mut state, observation.rules, &mut strategies, turn_phase);
                    }
                    play_game(&mut state, observation.rules, &mut strategies, 20);
                }
                None
            }
        }
        let mut state = GameState::<4>::create_starting_state(&DEX_RULESET, 7);
        let mut strategies = core::array::from_fn(|_| ObservedStrategy::new(Probe::default()));
        play_game(&mut state, &DEX_RULESET, &mut strategies, 20);
        assert!(strategies.iter().all(|strategy| strategy.policy.sampled));
        struct Passive;
        impl ObservedPolicy for Passive {}
        let mut expected = GameState::<4>::create_starting_state(&DEX_RULESET, 7);
        let mut passive = core::array::from_fn(|_| ObservedStrategy::new(Passive));
        play_game(&mut expected, &DEX_RULESET, &mut passive, 20);
        assert_eq!(state, expected);
        assert_eq!(strategies[0].history(), passive[0].history());
    }
}
