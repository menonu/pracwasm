use cosmwasm_std::Timestamp;

use rand::{rngs::SmallRng, RngCore, SeedableRng};

// this is totally idiotic shit.
// do not copy/use otherwise you will be hacked
pub fn _gen_random_u32(timestamp: Timestamp) -> u32 {
    let mut rng = SmallRng::seed_from_u64(timestamp.nanos());
    rng.next_u32()
}

pub fn gen_rng(timestamp: Timestamp) -> SmallRng {
    let rng = SmallRng::seed_from_u64(timestamp.nanos());
    rng
}
