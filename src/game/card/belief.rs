use super::data::{
    CARD_COUNT_PER_DECK,
    CHANCE_CARD_DEFINITIONS,
    COMMUNITY_CHEST_CARD_DEFINITIONS,
};
use super::model::{
    CardDefinition,
    CardEffect,
    CardId,
    DeckKind,
};
use crate::game::board::model::PlayerId;
use crate::game::engine::event::GameEvent;
use crate::game::engine::payment::Creditor;
use crate::game::rng::WyRand;
use crate::game::state::model::DeckState;

#[derive(Debug, Clone)]
struct Candidate {
    effects: [Option<CardEffect>; CARD_COUNT_PER_DECK],
    next_draw_position: u8,
    weight: u64,
}

#[derive(Debug, Clone)]
pub struct DeckBelief {
    deck: DeckKind,
    candidates: Vec<Candidate>,
    holder: Option<PlayerId>,
    permutation_count: u64,
}

fn definitions(deck: DeckKind) -> &'static [CardDefinition; CARD_COUNT_PER_DECK] {
    match deck {
        DeckKind::Chance => &CHANCE_CARD_DEFINITIONS,
        DeckKind::CommunityChest => &COMMUNITY_CHEST_CARD_DEFINITIONS,
    }
}

impl DeckBelief {
    pub fn from_start_history(
        deck: DeckKind,
        history: &[GameEvent],
    ) -> Result<Self, &'static str> {
        let mut candidates = Vec::new();
        let mut holder = None;
        for jail_position in 0..CARD_COUNT_PER_DECK {
            if let Some((candidate, final_holder)) = Candidate::from_history(deck, history, jail_position) {
                candidates.push(candidate);
                holder = final_holder;
            }
        }
        let permutation_count = candidates.iter().map(|candidate| candidate.weight).sum();
        if permutation_count == 0 {
            return Err("card history is inconsistent with the starting deck and public jail card transfers");
        }
        Ok(Self {
            deck,
            candidates,
            holder,
            permutation_count,
        })
    }

    pub fn holder(&self) -> Option<PlayerId> {
        self.holder
    }

    pub fn permutation_count(&self) -> u64 {
        self.permutation_count
    }

    pub fn sample(
        &self,
        rng: &mut WyRand,
    ) -> DeckState {
        let mut index = sample_below(rng, self.permutation_count);
        let candidate = self
            .candidates
            .iter()
            .find(|candidate| {
                if index < candidate.weight {
                    true
                } else {
                    index -= candidate.weight;
                    false
                }
            })
            .unwrap();
        let definitions = definitions(self.deck);
        let mut remaining: Vec<CardId> = (0..CARD_COUNT_PER_DECK as CardId).collect();
        let mut order = [0; CARD_COUNT_PER_DECK];
        for (position, effect) in candidate.effects.iter().enumerate() {
            if let Some(effect) = effect {
                let matching: Vec<usize> = remaining
                    .iter()
                    .enumerate()
                    .filter_map(|(index, id)| (definitions[*id as usize].effect == *effect).then_some(index))
                    .collect();
                let index = matching[sample_below(rng, matching.len() as u64) as usize];
                order[position] = remaining.swap_remove(index);
            }
        }
        for (position, effect) in candidate.effects.iter().enumerate() {
            if effect.is_none() {
                let index = sample_below(rng, remaining.len() as u64) as usize;
                order[position] = remaining.swap_remove(index);
            }
        }
        DeckState {
            card_id_by_draw_position: order,
            next_draw_position: candidate.next_draw_position,
        }
    }
}

impl Candidate {
    fn from_history(
        deck: DeckKind,
        history: &[GameEvent],
        jail_position: usize,
    ) -> Option<(Self, Option<PlayerId>)> {
        let mut effects = [None; CARD_COUNT_PER_DECK];
        effects[jail_position] = Some(CardEffect::GetOutOfJailFree);
        let mut cursor = 0;
        let mut holder = None;
        for event in history {
            match *event {
                GameEvent::CardDrawn { player_id, deck: drawn_deck, effect } if drawn_deck == deck => {
                    if holder.is_some() && cursor == jail_position {
                        cursor = (cursor + 1) % CARD_COUNT_PER_DECK;
                    }
                    if let Some(known) = effects[cursor] {
                        if effect != known {
                            return None;
                        }
                    } else {
                        effects[cursor] = Some(effect);
                    }
                    if effect == CardEffect::GetOutOfJailFree {
                        if holder.is_some() || cursor != jail_position {
                            return None;
                        }
                        holder = Some(player_id);
                    }
                    cursor = (cursor + 1) % CARD_COUNT_PER_DECK;
                },
                GameEvent::JailCardUsed { player_id, deck: used_deck } if used_deck == deck => {
                    if holder != Some(player_id) {
                        return None;
                    }
                    holder = None;
                },
                GameEvent::TradeExecuted { offer } => {
                    let bit = 1 << deck as u8;
                    if offer.offered_get_out_of_jail_free_cards & bit != 0 {
                        if holder != Some(offer.proposer_player_id) {
                            return None;
                        }
                        holder = Some(offer.recipient_player_id);
                    }
                    if offer.requested_get_out_of_jail_free_cards & bit != 0 {
                        if holder != Some(offer.recipient_player_id) {
                            return None;
                        }
                        holder = Some(offer.proposer_player_id);
                    }
                },
                GameEvent::Bankrupt { player_id, creditor, .. } if holder == Some(player_id) => {
                    holder = match creditor {
                        Creditor::Player(player) => Some(player),
                        Creditor::Bank | Creditor::FreeParkingJackpot => None,
                    };
                },
                _ => {},
            }
        }
        let definitions = definitions(deck);
        let mut remaining: Vec<CardId> = (0..CARD_COUNT_PER_DECK as CardId).collect();
        let mut weight = 1;
        for effect in effects.iter().flatten() {
            let matching = remaining.iter().filter(|id| definitions[**id as usize].effect == *effect).count();
            let index = remaining.iter().position(|id| definitions[*id as usize].effect == *effect)?;
            weight *= matching as u64;
            remaining.swap_remove(index);
        }
        for count in 2..=remaining.len() {
            weight *= count as u64;
        }
        Some((
            Self {
                effects,
                next_draw_position: cursor as u8,
                weight,
            },
            holder,
        ))
    }
}

fn sample_below(
    rng: &mut WyRand,
    bound: u64,
) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = rng.generate_u64();
        if value >= threshold {
            return value % bound;
        }
    }
}

// 20! fits in u64; candidate weights count permutations of distinct card ids.
const _: () = assert!(CARD_COUNT_PER_DECK <= 20);

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(
        deck: DeckKind,
        effect: CardEffect,
    ) -> GameEvent {
        GameEvent::CardDrawn { player_id: 0, deck, effect }
    }

    fn factorial(count: usize) -> u64 {
        (1..=count as u64).product()
    }

    fn replay(
        deck: DeckKind,
        sample: DeckState,
        history: &[GameEvent],
    ) -> (u8, Option<PlayerId>) {
        let mut cursor = 0;
        let mut holder = None;
        for event in history {
            match *event {
                GameEvent::CardDrawn { player_id, deck: drawn_deck, effect } if drawn_deck == deck => {
                    let actual = loop {
                        let effect = definitions(deck)[sample.card_id_by_draw_position[cursor] as usize].effect;
                        cursor = (cursor + 1) % CARD_COUNT_PER_DECK;
                        if holder.is_none() || effect != CardEffect::GetOutOfJailFree {
                            break effect;
                        }
                    };
                    assert_eq!(effect, actual);
                    if effect == CardEffect::GetOutOfJailFree {
                        holder = Some(player_id);
                    }
                },
                GameEvent::JailCardUsed { player_id, deck: used_deck } if used_deck == deck => {
                    assert_eq!(holder, Some(player_id));
                    holder = None;
                },
                GameEvent::Bankrupt { player_id, creditor, .. } if holder == Some(player_id) => {
                    holder = match creditor {
                        Creditor::Player(player) => Some(player),
                        _ => None,
                    };
                },
                _ => {},
            }
        }
        (cursor as u8, holder)
    }

    fn verify_samples(
        deck: DeckKind,
        history: &[GameEvent],
    ) -> DeckBelief {
        let belief = DeckBelief::from_start_history(deck, history).unwrap();
        let mut rng = WyRand::new(17);
        for _ in 0..32 {
            let sample = belief.sample(&mut rng);
            let mut sorted = sample.card_id_by_draw_position;
            sorted.sort_unstable();
            assert_eq!(sorted, core::array::from_fn(|id| id as CardId));
            assert_eq!(replay(deck, sample, history), (sample.next_draw_position, belief.holder()));
        }
        belief
    }

    #[test]
    fn traded_jail_cards_can_be_used_by_the_new_holder() {
        use crate::game::trade::model::TradeOffer;
        for requested in [false, true] {
            let deck = DeckKind::Chance;
            let offer = TradeOffer {
                proposer_player_id: if requested { 1 } else { 0 },
                recipient_player_id: if requested { 0 } else { 1 },
                offered_cash: 0,
                requested_cash: 0,
                offered_tiles: 0,
                requested_tiles: 0,
                offered_get_out_of_jail_free_cards: u8::from(!requested),
                requested_get_out_of_jail_free_cards: u8::from(requested),
            };
            let mut history = vec![draw(deck, CardEffect::GetOutOfJailFree), GameEvent::TradeExecuted { offer }];
            assert_eq!(DeckBelief::from_start_history(deck, &history).unwrap().holder(), Some(1));
            history.push(GameEvent::JailCardUsed { player_id: 1, deck });
            assert_eq!(DeckBelief::from_start_history(deck, &history).unwrap().holder(), None);
        }
    }

    #[test]
    fn empty_history_preserves_all_orders_and_sampling_is_reproducible() {
        for deck in [DeckKind::Chance, DeckKind::CommunityChest] {
            let belief = verify_samples(deck, &[]);
            assert_eq!(belief.permutation_count(), factorial(16));
            let mut first = WyRand::new(91);
            let mut second = first;
            let samples: Vec<_> = (0..32).map(|_| belief.sample(&mut first)).collect();
            assert_eq!(samples, (0..32).map(|_| belief.sample(&mut second)).collect::<Vec<_>>());
            assert!(samples.windows(2).any(|pair| pair[0] != pair[1]));
        }
    }

    #[test]
    fn repeated_effects_count_distinct_cards_without_inventing_extra_copies() {
        let deck = DeckKind::Chance;
        let event = draw(deck, CardEffect::AdvanceToNearestRailroad);
        assert_eq!(verify_samples(deck, &[event]).permutation_count(), 2 * factorial(15));
        assert_eq!(verify_samples(deck, &[event, event]).permutation_count(), 2 * factorial(14));
        assert!(DeckBelief::from_start_history(deck, &[event, event, event]).is_err());
    }

    #[test]
    fn held_cards_are_skipped_then_return_to_the_original_cycle() {
        for deck in [DeckKind::Chance, DeckKind::CommunityChest] {
            let mut history = vec![draw(deck, CardEffect::GetOutOfJailFree)];
            let other: Vec<_> = definitions(deck)
                .iter()
                .filter(|card| card.effect != CardEffect::GetOutOfJailFree)
                .map(|card| draw(deck, card.effect))
                .collect();
            history.extend_from_slice(&other);
            let first = verify_samples(deck, &history);
            assert_eq!(first.holder(), Some(0));
            history.extend_from_slice(&other);
            assert_eq!(verify_samples(deck, &history).permutation_count(), first.permutation_count());
            history.push(GameEvent::JailCardUsed { player_id: 0, deck });
            assert_eq!(verify_samples(deck, &history).holder(), None);
            history.push(draw(deck, CardEffect::GetOutOfJailFree));
            assert_eq!(verify_samples(deck, &history).holder(), Some(0));
            history.push(draw(deck, CardEffect::GetOutOfJailFree));
            assert!(DeckBelief::from_start_history(deck, &history).is_err());
        }
    }

    #[test]
    fn bankruptcy_transfers_or_returns_cards_and_invalid_use_is_rejected() {
        let deck = DeckKind::Chance;
        let card_draw = draw(deck, CardEffect::GetOutOfJailFree);
        for creditor in [Creditor::Player(1), Creditor::Bank, Creditor::FreeParkingJackpot] {
            let mut history = vec![
                card_draw,
                GameEvent::Bankrupt {
                    player_id: 0,
                    creditor,
                    remaining_cash: 0,
                },
            ];
            let holder = if creditor == Creditor::Player(1) { Some(1) } else { None };
            assert_eq!(verify_samples(deck, &history).holder(), holder);
            if holder.is_some() {
                history.push(GameEvent::JailCardUsed { player_id: 1, deck });
                assert_eq!(verify_samples(deck, &history).holder(), None);
            }
            history.push(GameEvent::JailCardUsed { player_id: 0, deck });
            assert!(DeckBelief::from_start_history(deck, &history).is_err());
        }
        assert!(DeckBelief::from_start_history(deck, &[GameEvent::JailCardUsed { player_id: 0, deck }]).is_err());
        assert!(DeckBelief::from_start_history(deck, &[draw(deck, CardEffect::CollectFromBank { amount: 999 })]).is_err());
    }

    #[test]
    fn real_game_histories_retain_the_actual_deck_as_a_possible_world() {
        use crate::game::engine::turn::play_turn;
        use crate::game::ruleset::data::{
            DEX_RULESET,
            OFFICIAL_RULESET,
        };
        use crate::game::state::model::GameState;
        use crate::game::strategy::model::JailAction;
        use crate::game::strategy::observation::{
            ObservedPolicy,
            ObservedStrategy,
            PublicObservation,
        };
        struct Policy;
        impl ObservedPolicy for Policy {
            fn should_purchase_property(
                &mut self,
                _observation: PublicObservation<'_>,
                _tile: u8,
            ) -> bool {
                true
            }
            fn choose_jail_action(
                &mut self,
                _observation: PublicObservation<'_>,
                legal: &[JailAction],
            ) -> JailAction {
                if legal.contains(&JailAction::UseGetOutOfJailFreeCard) {
                    JailAction::UseGetOutOfJailFreeCard
                } else {
                    JailAction::RollForDoubles
                }
            }
        }
        for rules in [DEX_RULESET, OFFICIAL_RULESET] {
            for seed in 0..8 {
                let mut state = GameState::<4>::create_starting_state(&rules, seed);
                let mut strategies = core::array::from_fn(|_| ObservedStrategy::new(Policy));
                for turn in 0..300 {
                    play_turn(&mut state, &rules, &mut strategies);
                    if turn % 100 != 99 {
                        continue;
                    }
                    for deck in [DeckKind::Chance, DeckKind::CommunityChest] {
                        let history = strategies[0].history();
                        let belief = verify_samples(deck, history);
                        let actual = state.deck_state_by_deck_kind[deck as usize];
                        assert_eq!(replay(deck, actual, history), (actual.next_draw_position, belief.holder()));
                        assert_eq!(belief.holder(), state.get_out_of_jail_free_card_holder_by_deck_kind[deck as usize]);
                        assert!(belief.candidates.iter().any(|candidate| {
                            candidate.next_draw_position == actual.next_draw_position
                                && candidate
                                    .effects
                                    .iter()
                                    .enumerate()
                                    .all(|(position, effect)| effect.is_none_or(|effect| effect == definitions(deck)[actual.card_id_by_draw_position[position] as usize].effect))
                        }));
                    }
                }
            }
        }
    }
}
