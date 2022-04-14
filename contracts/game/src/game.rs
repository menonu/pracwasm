use std::fmt::Display;

use rand::prelude::SliceRandom;

use crate::card::{BJCard, Hand, CARDLIST};

pub(crate) fn draw_one<T: rand::Rng>(rng: &mut T) -> BJCard {
    CARDLIST
        .choose(rng)
        .expect("something went wrong")
        .to_owned()
}

pub(crate) fn dealer_action<T: rand::Rng>(hand: &[BJCard], rng: &mut T) -> Vec<BJCard> {
    let mut new_hand = Vec::from(hand);
    let mut score = calc_score(hand);

    while score < 17 {
        new_hand.push(draw_one(rng));
        score = calc_score(&new_hand);
    }

    new_hand
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum GameResult {
    Win,
    Loose,
    Draw,
}

impl Display for GameResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                GameResult::Win => "Win",
                GameResult::Loose => "Loose",
                GameResult::Draw => "Draw",
            }
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Judge {
    DealerBusted(i32),
    PlayerBusted(i32),
    DealerWin(i32, i32),
    PlayerWin(i32, i32),
    PlayerBJWin(i32, i32),
    Draw(i32, i32),
}

impl Display for Judge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Judge::DealerBusted(d) => write!(f, "{} {}", "DealerBusted", d),
            Judge::PlayerBusted(p) => write!(f, "{} {}", "PlayerBusted", p),
            Judge::DealerWin(d, p) => write!(f, "{} {} {}", "DealerWin", d, p),
            Judge::PlayerWin(d, p) => write!(f, "{} {} {}", "PlayerWin", d, p),
            Judge::PlayerBJWin(d, p) => write!(f, "{} {} {}", "PlayerBJWin", d, p),
            Judge::Draw(d, p) => write!(f, "{} {} {}", "Draw", d, p),
        }
    }
}

pub(crate) fn judge(dealer: &[BJCard], player: &[BJCard]) -> Judge {
    let d_score = calc_score(dealer);
    let p_score = calc_score(player);

    match (d_score, p_score, dealer.len(), player.len()) {
        (21, 21, 2, 2) => Judge::Draw(21, 21),
        (d, 21, _, 2) => Judge::PlayerBJWin(d, 21),
        (21, p, 2, _) => Judge::DealerWin(21, p),
        (_, p, _, _) if p > 21 => Judge::PlayerBusted(p),
        (d, _, _, _) if d > 21 => Judge::DealerBusted(d),
        (d, p, _, _) if d < p => Judge::PlayerWin(d, p),
        (d, p, _, _) if d > p => Judge::DealerWin(d, p),
        (d, p, _, _) if d == p => Judge::Draw(d, p),
        (_, _, _, _) => panic!(
            "Judge logic error: {} {} {} {}",
            d_score,
            p_score,
            dealer.len(),
            player.len()
        ),
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
        use BJCard::*;

        let mut rng = SmallRng::seed_from_u64(0_u64);
        let (d, p) = first_deal(&mut rng);

        assert_eq!(vec![Seven], d);
        assert_eq!(vec![Ace, Eight], p);
    }

    #[test]
    fn dealer_hand() {
        use BJCard::*;
        let mut rng = SmallRng::seed_from_u64(0_u64);

        let dealer = vec![Two];
        let dealer_new = dealer_action(&dealer, &mut rng);

        assert!(calc_score(&dealer_new) > 16);

        let dealer = vec![Three];
        let dealer_new = dealer_action(&dealer, &mut rng);

        assert!(calc_score(&dealer_new) > 16);
    }

    #[test]
    fn judge_normal() {
        use BJCard::*;
        let dealer = vec![Ten, Eight];
        let player = vec![Ten, Nine];
        assert_eq!(Judge::PlayerWin(18, 19), judge(&dealer, &player));

        let dealer = vec![Ten, Ten];
        let player = vec![Ten, Nine];
        assert_eq!(Judge::DealerWin(20, 19), judge(&dealer, &player));

        let dealer = vec![Ten, Ten];
        let player = vec![Ten, Jack];
        assert_eq!(Judge::Draw(20, 20), judge(&dealer, &player));
    }

    #[test]
    fn judge_busted() {
        use BJCard::*;

        let dealer = vec![Ten, Five, King];
        let player = vec![Ten, Jack];
        assert_eq!(Judge::DealerBusted(25), judge(&dealer, &player));
        let dealer = vec![Ten, Two, Jack];
        let player = vec![Ten, Jack];
        assert_eq!(Judge::DealerBusted(22), judge(&dealer, &player));
        let dealer = vec![Ten, Ten];
        let player = vec![Ten, Two, Jack];
        assert_eq!(Judge::PlayerBusted(22), judge(&dealer, &player));
    }

    #[test]
    fn judge_blackjack() {
        use BJCard::*;

        let dealer = vec![Ten, Seven];
        let player = vec![Ace, Jack];
        assert_eq!(Judge::PlayerBJWin(17, 21), judge(&dealer, &player));

        let dealer = vec![Ten, Five, Six];
        let player = vec![Ace, Jack];
        assert_eq!(Judge::PlayerBJWin(21, 21), judge(&dealer, &player));
    }

    #[test]
    fn judge_draw() {
        use BJCard::*;

        let dealer = vec![Ten, Seven];
        let player = vec![Ten, Seven];
        assert_eq!(Judge::Draw(17, 17), judge(&dealer, &player));

        let dealer = vec![Ten, Ace];
        let player = vec![Ten, Ace];
        assert_eq!(Judge::Draw(21, 21), judge(&dealer, &player));
    }
}
