use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, JsonSchema,
)]
pub enum BJCard {
    Ace,
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
}

impl Display for BJCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

pub type Hand = Vec<BJCard>;

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
