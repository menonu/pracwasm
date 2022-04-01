use rand::prelude::SliceRandom;

use crate::card::{BJCard, Hand, CARDLIST};

pub(crate) fn draw_one<T: rand::Rng>(rng: &mut T) -> BJCard {
    CARDLIST
        .choose(rng)
        .expect("something went wrong")
        .to_owned()
}

pub(crate) fn calc_score(hand: &[BJCard]) -> i32 {
    let mut sum = 0;
    let mut hand = hand.to_owned();
    hand.sort();
    for card in hand.iter() {
        let score = match card {
            BJCard::Two => 2,
            BJCard::Three => 3,
            BJCard::Four => 4,
            BJCard::Five => 5,
            BJCard::Six => 6,
            BJCard::Seven => 7,
            BJCard::Eight => 8,
            BJCard::Nine => 9,
            BJCard::Ten | BJCard::Jack | BJCard::Queeen | BJCard::King => 10,
            BJCard::Ace if sum + 11 <= 21 => 11,
            BJCard::Ace => 1,
        };

        sum += score;
    }
    sum
}

pub(crate) enum Judge {
    DealerBJ,
    DealerBusted,
    PlayerBJ,
    PlayerBusted(i32),
    Continue,
}

pub(crate) fn judge(dealer: &[BJCard], player: &[BJCard]) -> Judge {
    let d_score = calc_score(dealer);
    let p_score = calc_score(player);

    match (d_score, p_score) {
        (_, 21) => Judge::PlayerBJ,
        (_, p) if p > 21 => Judge::PlayerBusted(p),
        (21, _) => Judge::DealerBJ,
        (d, _) if d > 21 => Judge::DealerBusted,
        (_, _) => Judge::Continue,
    }
}

pub(crate) fn first_deal<T: rand::Rng>(rng: &mut T) -> (Hand, Hand) {
    let dealer = draw_one(rng);
    let player1 = draw_one(rng);
    let player2 = draw_one(rng);

    (vec![dealer], vec![player1, player2])
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

    #[test]
    fn test_calc_score() {
        let hand = vec![BJCard::Two];
        assert_eq!(2, calc_score(&hand));

        let hand = vec![BJCard::Three];
        assert_eq!(3, calc_score(&hand));

        let hand = vec![BJCard::Jack];
        assert_eq!(10, calc_score(&hand));

        let hand = vec![BJCard::Two, BJCard::Two];
        assert_eq!(4, calc_score(&hand));

        let hand = vec![BJCard::Ace];
        assert_eq!(11, calc_score(&hand));

        let hand = vec![BJCard::Ace, BJCard::Ace, BJCard::Ace];
        assert_eq!(13, calc_score(&hand));

        let hand = vec![BJCard::Ace, BJCard::King];
        assert_eq!(21, calc_score(&hand));

        let hand = vec![BJCard::Ace, BJCard::Eight, BJCard::Four];
        assert_eq!(13, calc_score(&hand));
    }

    #[test]
    fn test_first_deal() {
        let mut rng = SmallRng::seed_from_u64(0_u64);
        let (d, p) = first_deal(&mut rng);

        dbg!(d);
        dbg!(p);
    }
}
