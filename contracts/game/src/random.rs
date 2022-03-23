use cosmwasm_std::Timestamp;

use rand::{rngs::SmallRng, SeedableRng, RngCore};

// this is totally idiotic shit.
// do not copy/use otherwise you will be hacked
pub fn gen_random(timestamp: Timestamp) -> u32 {
    let mut rng = SmallRng::seed_from_u64(timestamp.nanos());
    rng.next_u32()
}
