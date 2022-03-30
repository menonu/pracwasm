use rand::prelude::SliceRandom;

use crate::card::{BJCard, CARDLIST};

pub fn draw_one<T: rand::Rng>(rng: &mut T) -> BJCard {
    CARDLIST
        .choose(rng)
        .expect("something went wrong")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use rand::{prelude::SmallRng, SeedableRng};

    use super::*;

    #[test]
    fn test_draw_one() {
        let mut rng = SmallRng::seed_from_u64(0_u64);
        let card = draw_one(&mut rng);

        assert_eq!(BJCard::Seven, card);
    }
}
