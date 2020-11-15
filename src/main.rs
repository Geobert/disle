use std::collections::HashSet;

use caith::{Critic, RollHistory, RollResult, RollResultType, SingleRollResult};

mod alias;
#[cfg(feature = "discord")]
mod discord;

#[tokio::main]
async fn main() {
    #[cfg(feature = "discord")]
    discord::run().await;
}

fn search_crit_simple(res: &SingleRollResult, set: &mut HashSet<Critic>) {
    for r in res.get_history().iter() {
        match r {
            RollHistory::Roll(r) => {
                for dice_res in r.iter() {
                    match dice_res.crit {
                        Critic::No => {}
                        _ => {
                            set.insert(dice_res.crit);
                        }
                    }
                }
                if set.len() >= 2 {
                    return;
                }
            }
            _ => (),
        }
    }
}

pub fn search_crit(res: &RollResult) -> HashSet<Critic> {
    let mut set = HashSet::new();
    match res.get_result() {
        RollResultType::Single(res) => search_crit_simple(&res, &mut set),
        RollResultType::Repeated(res) => {
            for roll in res.iter() {
                search_crit_simple(&roll, &mut set);
                if set.len() >= 2 {
                    return set;
                }
            }
        }
    }
    set
}
