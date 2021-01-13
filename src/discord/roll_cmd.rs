use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use caith::{Critic, RollResult};

use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
    model::channel::ReactionType,
    prelude::TypeMapKey,
};

use crate::Interpreter;

use super::{alias_cmd::parse_alias, err_message, send_message};

#[group]
#[commands(roll, reroll, reroll_dice)]
struct Roll;

pub(crate) struct RerollTable;
impl TypeMapKey for RerollTable {
    type Value = HashMap<String, caith::Roller>;
}

fn get_roll_help_msg() -> String {
    "To get help, run `/help`".to_string()
}

async fn process_roll_str(input: &str, ctx: &Context, msg: &Message) -> Result<RollResult, String> {
    // TODO: once caith can save the parsed result, manage error on `new`
    process_roll(caith::Roller::new(input).unwrap(), ctx, msg).await
}

async fn process_roll(
    mut roller: caith::Roller,
    ctx: &Context,
    msg: &Message,
) -> Result<RollResult, String> {
    match roller.roll() {
        Ok(res) => {
            {
                // do not store comment for reroll
                roller.trim_reason();
                let mut data = ctx.data.write().await;
                let reroll_table = data.get_mut::<RerollTable>().unwrap();
                reroll_table.insert(msg.author.to_string(), roller);
            }
            Ok(res)
        }
        Err(err) => Err(err_message(err)),
    }
}

async fn react_to(ctx: &Context, msg: &Message, crit: Option<HashSet<Critic>>) -> CommandResult {
    if let Some(crit) = crit {
        for c in crit.iter() {
            match c {
                Critic::No => {
                    // this is constructed in roll* to say there was no dice in the expr
                    msg.react(&ctx.http, ReactionType::Unicode("🎲".to_string()))
                        .await?;
                }
                Critic::Min => {
                    msg.react(&ctx.http, ReactionType::Unicode("🤬".to_string()))
                        .await?;
                }
                Critic::Max => {
                    msg.react(&ctx.http, ReactionType::Unicode("🥳".to_string()))
                        .await?;
                }
            }
        }
    }
    Ok(())
}

#[command]
#[aliases("r")]
/// ```
/// /roll xdy [OPTIONS][TARGET][FAILURE][REASON]
///
/// rolls `x` dices of `y` sides
///
/// `y` can also be "F" or "f" for Fudge/Fate dice.
///
/// Options:
///     + - / * : modifiers
///     e#  : Explode value
///     ie# : Indefinite explode value
///     K#  : Keeping # highest (upperacse "K")
///     k#  : Keeping # lowest (lowercase "k")
///     D#  : Dropping the highest (uppercase "D")
///     d#  : Dropping the lowest (lowercase "d")
///     r#  : Reroll if <= value
///     ir# : Indefinite reroll if <= value
///
///     Target:
///     t#  : Target value to be a success
///     t[<list of number>] : values to consider as success
///     tt# : minimum value to count as two successes
///
///     Failure:
///     f#  : Value under which it is count as failure
///
///     Repeatition:
///     a roll can be repeated with `^` operator: `(2d6 + 6) ^ 8` will roll eight times the expression.
///
///     Summed repeatition:
///     with the `^+` operator, the roll will be repeated and all the totals summed.
///
///     Sorted repeatition:
///     with the `^#` operator, the roll will be repeated and sorted by total.
///
///     OVA roll:
///     positive: `ova(12)` or negative: `ova(-5)`
///
///     Reason:
///     :   : Any text after `:` will be a comment"
/// ```
async fn roll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        if let Err(e) = msg.channel_id.say(&ctx.http, get_roll_help_msg()).await {
            eprintln!("Error sending message: {:?}", e);
        }
    } else {
        let (msg_to_send, crit) = parse_args(ctx, msg, args).await;

        let sent_msg = send_message(ctx, msg, &msg_to_send).await?;
        react_to(ctx, &sent_msg, crit).await?;
    }
    Ok(())
}

async fn parse_args(ctx: &Context, msg: &Message, args: Args) -> (String, Option<HashSet<Critic>>) {
    if args.rest().starts_with("help") {
        return (get_roll_help_msg(), None);
    }

    let input = parse_alias(ctx, msg, args).await;
    let input = match input {
        Ok(input) => input,
        Err(err) => return (err, None),
    };

    match parse_interpreter(&input) {
        Ok((input, interpreter)) => match process_roll_str(&input, ctx, msg).await {
            Ok(res) => {
                let crit_set = match interpreter {
                    Interpreter::None => crate::search_crit(&res),
                    _ => Ok(HashSet::new()),
                };

                let sep = if res.as_repeated().is_some() {
                    "\n"
                } else {
                    ""
                };
                let res = match interpreter {
                    Interpreter::None => res.to_string(),
                    Interpreter::Ova(number) => match caith::helpers::compute_ova(&res, number) {
                        Ok(roll_res) => roll_res.to_string(),
                        Err(err) => return (err.to_string(), None),
                    },
                    Interpreter::Cde(element) => match caith::helpers::compute_cde(&res, element) {
                        Ok(res) => res.to_string(),
                        Err(_err) => {
                            return (
                                "Syntax error, expected: `cde(number_of_dice, element)`"
                                    .to_string(),
                                None,
                            )
                        }
                    },
                };
                (format!("{}{}", sep, res), crate::process_crit(crit_set))
            }
            Err(mut msg) => {
                msg.insert_str(msg.len() - 4, ", or an alias");
                (msg, None)
            }
        },
        Err(err) => (err, None),
    }
}

fn parse_interpreter<'a>(input: &'a str) -> Result<(Cow<'a, str>, Interpreter), String> {
    if input.starts_with("ova(") {
        let number = input[4..input.len() - 1]
            .trim()
            .parse::<i32>()
            .map_err(|e| e.to_string())?;
        Ok((
            Cow::Owned(format!("{}d6", number.abs())),
            Interpreter::Ova(number),
        ))
    } else if input.starts_with("cde(") {
        let comma = input.find(',').ok_or("")?;
        let number = input[4..comma]
            .trim()
            .parse::<u32>()
            .map_err(|_| "Syntax error, expected: `cde(number_of_dice, element)`".to_string())?;
        let element = input[comma + 1..input.len() - 1].trim();
        Ok((
            Cow::Owned(format!("{}d10", number)),
            Interpreter::Cde(element),
        ))
    } else {
        Ok((Cow::Borrowed(input), Interpreter::None))
    }
}

#[command]
#[aliases("rr")]
/// ```
/// /reroll (or /rr)
///
/// Reroll the last roll of the user
/// ```
async fn reroll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let (msg_to_send, crit) = if input.starts_with("help") {
        (get_roll_help_msg(), None)
    } else {
        let roller = {
            let mut data = ctx.data.write().await;
            let reroll_table = data.get_mut::<RerollTable>().unwrap();
            reroll_table.remove(&msg.author.to_string())
        };
        match roller {
            Some(roller) => {
                let cmd = roller.as_str().to_string();
                match process_roll(roller, ctx, msg).await {
                    Ok(res) => {
                        let crit_set = crate::search_crit(&res);
                        (
                            format!("reroll `{}`: {}", cmd, res),
                            crate::process_crit(crit_set),
                        )
                    }
                    Err(msg) => (msg, None),
                }
            }
            None => ("No previous roll".to_owned(), None),
        }
    };

    let sent_msg = send_message(ctx, msg, &msg_to_send).await?;
    react_to(ctx, &sent_msg, crit).await?;
    Ok(())
}

#[command]
#[aliases("rd")]
/// ```
/// /reroll_dice (or /rd)
///
/// Reroll the first dice of the last roll
/// ```
async fn reroll_dice(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let (msg_to_send, crit) = if input.starts_with("help") {
        (get_roll_help_msg(), None)
    } else {
        let dice = {
            let mut data = ctx.data.write().await;
            let roller = {
                let reroll_table = data.get_mut::<RerollTable>().unwrap();
                reroll_table.get(&msg.author.to_string())
            };
            match roller {
                Some(roller) => match roller.dices() {
                    Ok(mut dices) => match dices.next() {
                        Some(dice) => Ok(dice),
                        _ => Err("No dice to reroll".to_string()),
                    },
                    Err(e) => Err(e.to_string()),
                },
                None => Err("No previous roll".to_string()),
            }
        };
        match dice {
            Ok(dice) => match process_roll_str(&dice, ctx, msg).await {
                Ok(res) => {
                    let crit_set = crate::search_crit(&res);
                    (
                        format!("reroll `{}`: {}", dice, res),
                        crate::process_crit(crit_set),
                    )
                }
                Err(e) => (e, None),
            },
            Err(err) => (err, None),
        }
    };

    let sent_msg = send_message(ctx, msg, &msg_to_send).await?;
    react_to(ctx, &sent_msg, crit).await?;
    Ok(())
}
