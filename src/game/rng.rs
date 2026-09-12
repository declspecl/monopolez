pub const DIE_FACE_COUNT: u64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiceRoll {
    pub first_die: u8,
    pub second_die: u8,
}

impl DiceRoll {
    pub const fn total(&self) -> u8 {
        self.first_die + self.second_die
    }

    pub const fn is_double(&self) -> bool {
        self.first_die == self.second_die
    }
}

const WYRAND_INCREMENT: u64 = 0xa076_1d47_8bd6_42f7;
const WYRAND_MIX: u64 = 0xe703_7ed1_a0b4_28db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WyRand {
    state: u64,
}

impl WyRand {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn generate_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(WYRAND_INCREMENT);
        let product = (self.state as u128).wrapping_mul((self.state ^ WYRAND_MIX) as u128);

        ((product >> 64) ^ product) as u64
    }

    pub fn generate_below(
        &mut self,
        exclusive_upper_bound: u32,
    ) -> u32 {
        (((self.generate_u64() >> 32) * exclusive_upper_bound as u64) >> 32) as u32
    }

    pub fn roll_dice(&mut self) -> DiceRoll {
        let random_bits = self.generate_u64();

        DiceRoll {
            first_die: (((random_bits >> 32) * DIE_FACE_COUNT) >> 32) as u8 + 1,
            second_die: (((random_bits & 0xFFFF_FFFF) * DIE_FACE_COUNT) >> 32) as u8 + 1,
        }
    }

    pub fn shuffle<T>(
        &mut self,
        values: &mut [T],
    ) {
        let mut unshuffled_count = values.len();

        while unshuffled_count > 1 {
            let swap_index = self.generate_below(unshuffled_count as u32) as usize;
            unshuffled_count -= 1;
            values.swap(unshuffled_count, swap_index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_sequence_for_same_seed() {
        let mut first_rng = WyRand::new(42);
        let mut second_rng = WyRand::new(42);

        for _ in 0..100 {
            assert_eq!(first_rng.generate_u64(), second_rng.generate_u64());
        }
    }

    #[test]
    fn rolls_every_face_within_range() {
        let mut rng = WyRand::new(7);
        let mut seen_faces = [false; DIE_FACE_COUNT as usize];

        for _ in 0..10_000 {
            let dice_roll = rng.roll_dice();
            assert!((1..=6).contains(&dice_roll.first_die));
            assert!((1..=6).contains(&dice_roll.second_die));
            seen_faces[dice_roll.first_die as usize - 1] = true;
            seen_faces[dice_roll.second_die as usize - 1] = true;
        }

        assert!(seen_faces.iter().all(|seen| *seen));
    }

    #[test]
    fn shuffles_into_a_permutation() {
        let mut rng = WyRand::new(3);
        let mut values: [u8; 16] = core::array::from_fn(|index| index as u8);
        rng.shuffle(&mut values);

        let mut sorted_values = values;
        sorted_values.sort_unstable();
        assert_eq!(sorted_values, core::array::from_fn(|index| index as u8));
    }
}
