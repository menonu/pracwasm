use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, JsonSchema,
)]
pub enum BJCard {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queeen,
    King,
    Ace,
}

impl Display for BJCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (*self as u8 + 2u8) % 13) // Card starts from two...ace
    }
}

pub type Hand = Vec<BJCard>;

pub fn hand_to_string(hand: &Hand) -> String {
    let str: String = hand.iter().map(|c| c.to_string() + " ").collect();
    format!("{}", str)
}

pub const CARDLIST: [BJCard; 13] = [
    BJCard::Two,
    BJCard::Three,
    BJCard::Four,
    BJCard::Five,
    BJCard::Six,
    BJCard::Seven,
    BJCard::Eight,
    BJCard::Nine,
    BJCard::Ten,
    BJCard::Jack,
    BJCard::Queeen,
    BJCard::King,
    BJCard::Ace,
];
