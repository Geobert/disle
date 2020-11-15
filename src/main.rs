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

fn search_crit_simple(res: &SingleRollResult, set: &mut HashSet<Critic>) -> Result<(), ()> {
    let mut has_roll = false;
    for r in res.get_history().iter() {
        match r {
            RollHistory::Roll(r) => {
                has_roll = true;
                for dice_res in r.iter() {
                    match dice_res.crit {
                        Critic::No => {}
                        _ => {
                            set.insert(dice_res.crit);
                        }
                    }
                }
                if set.len() >= 2 {
                    return Ok(());
                }
            }
            RollHistory::Fudge(_) => has_roll = true,
            _ => (),
        }
    }
    if has_roll {
        Ok(())
    } else {
        Err(())
    }
}

pub fn search_crit(res: &RollResult) -> Result<HashSet<Critic>, ()> {
    let mut set = HashSet::new();
    match res.get_result() {
        RollResultType::Single(res) => {
            search_crit_simple(&res, &mut set)?;
            Ok(set)
        }
        RollResultType::Repeated(res) => {
            for roll in res.iter() {
                search_crit_simple(&roll, &mut set)?;
                if set.len() >= 2 {
                    return Ok(set);
                }
            }
            Ok(set)
        }
    }
}

pub fn process_crit(set: Result<HashSet<Critic>, ()>) -> Option<HashSet<Critic>> {
    if let Ok(set) = set {
        if set.is_empty() {
            None
        } else {
            Some(set)
        }
    } else {
        let mut h = HashSet::new();
        h.insert(Critic::No);
        Some(h)
    }
}
