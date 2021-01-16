use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use caith::cards::{Card, Deck};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
    prelude::TypeMapKey,
};

use super::alias_cmd::chat_id;

pub(crate) struct Decks;
impl TypeMapKey for Decks {
    type Value = AllDecks;
}

// room_id, Deck
pub(crate) struct AllDecks(HashMap<String, Deck>);

impl Deref for AllDecks {
    type Target = HashMap<String, Deck>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllDecks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AllDecks {
    pub fn new() -> Self {
        AllDecks(HashMap::new())
    }
}

pub(crate) struct PrivateDraws;
impl TypeMapKey for PrivateDraws {
    type Value = AllPrivateDraws;
}
pub(crate) struct AllPrivateDraws(HashMap<String, Vec<Card>>);
impl Deref for AllPrivateDraws {
    type Target = HashMap<String, Vec<Card>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllPrivateDraws {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AllPrivateDraws {
    pub fn new() -> Self {
        AllPrivateDraws(HashMap::new())
    }
}

#[group]
#[commands(draw, newdeck, shuffle, remain, reveal, discard)]
struct Cards;

#[command]
/// Query how many cards left
async fn remain(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let decks = data.get::<Decks>().unwrap();
    let size = match decks.get(&chat_id(&msg).to_string()) {
        Some(deck) => deck.len(),
        None => {
            super::send_message(ctx, msg, "No deck to query: use /newdeck <nb of jokers>").await?;
            return Ok(());
        }
    };
    super::send_message(ctx, msg, &format!("There's {} cards left", size)).await?;
    Ok(())
}

#[command]
#[aliases("sh")]
/// Shuffle the deck
async fn shuffle(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let mut data = ctx.data.write().await;
    let decks = data.get_mut::<Decks>().unwrap();
    match decks.get_mut(&chat_id(&msg).to_string()) {
        Some(deck) if !deck.is_empty() => deck.shuffle(),
        Some(_) => {
            super::send_message(ctx, msg, "Deck is empty").await?;
        }
        None => {
            super::send_message(ctx, msg, "No deck to shuffle: use /newdeck <nb of jokers>")
                .await?;
        }
    }
    super::send_message(ctx, msg, "Deck shuffled").await?;
    Ok(())
}

#[command]
#[aliases("nd")]
/// Create a new deck with the specified number of jokers
async fn newdeck(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let number = if args.is_empty() {
        0
    } else {
        match args.single::<u32>() {
            Ok(number) => number,
            Err(_) => {
                super::send_message(ctx, msg, "Bad parameter: `/newdeck <nb_of_jokers>").await?;
                return Ok(());
            }
        }
    };

    let mut data = ctx.data.write().await;
    let decks = data.get_mut::<Decks>().unwrap();
    match decks.get_mut(&chat_id(&msg).to_string()) {
        Some(deck) => deck.reset(number as usize),
        None => {
            decks.insert(
                chat_id(&msg).to_string(),
                caith::cards::Deck::new(number as usize),
            );
        }
    }

    super::send_message(
        ctx,
        msg,
        &format!("New deck created with {} jokers", number),
    )
    .await?;
    Ok(())
}

#[command]
#[aliases("rev")]
/// Reveal your secret draw
async fn reveal(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    do_on_private_draw(ctx, msg, |privates, id| match privates.get(&id) {
        Some(privates) if !privates.is_empty() => Ok(Some(print_vec_of_cards(privates))),
        Some(_) | None => Err("You don't have any private draw"),
    })
    .await?;
    Ok(())
}

#[command]
#[aliases("dis")]
/// Discard your secret draw
async fn discard(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    do_on_private_draw(ctx, msg, |privates, id| match privates.get_mut(&id) {
        Some(privates) if !privates.is_empty() => {
            privates.clear();
            Ok(Some("You secret draw was discarded".to_string()))
        }
        Some(_) | None => Err("You don't have any private draw"),
    })
    .await?;
    Ok(())
}

#[command]
#[aliases("d")]
/// Draw the specified number of cards
async fn draw(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let (nb, reason, secret) = process_draw_args(ctx, msg, &args).await?;

    let mut drawn_cards = draw_card(ctx, msg, nb).await?;

    let mut msg_to_send = if drawn_cards.is_empty() {
        "Deck was empty".to_string()
    } else if drawn_cards.len() < nb as usize {
        "Not enough cards left in deck".to_string()
    } else {
        let msg_to_send = print_vec_of_cards(&drawn_cards);
        if secret {
            do_on_private_draw(ctx, msg, |privates, id| {
                println!("save secret");
                privates
                    .entry(id)
                    .and_modify(|v| v.append(&mut drawn_cards))
                    .or_insert(drawn_cards);
                Ok(None)
            })
            .await?;
        }
        msg_to_send
    };

    if let Some(reason) = reason {
        msg_to_send.push_str(&format!(": `{}`", reason));
    }
    if !secret {
        super::send_message(ctx, msg, &msg_to_send).await?;
    } else {
        let channel_name = match msg.channel(&ctx.cache).await {
            Some(serenity::model::channel::Channel::Guild(channel)) => channel.name,
            Some(_) | None => "Unknown channel".to_string(),
        };

        let msg_to_send = format!("Your draw from `#{}`: {}", channel_name, msg_to_send);
        msg.author
            .direct_message(&ctx, |m| m.content(&msg_to_send))
            .await?;
        super::send_message(ctx, msg, "Your draw has been sent as a private message").await?;
    }
    Ok(())
}

fn print_vec_of_cards(v: &[Card]) -> String {
    v.iter().enumerate().fold(String::new(), |mut s, (i, c)| {
        if i != 0 {
            s.push_str(", ");
        }
        let suit = match c.suit {
            caith::cards::Suit::None => ":black_joker:",
            caith::cards::Suit::Clubs => ":clubs:",
            caith::cards::Suit::Diamonds => ":diamonds:",
            caith::cards::Suit::Hearts => ":hearts:",
            caith::cards::Suit::Spades => ":spades:",
        };

        match c.suit {
            caith::cards::Suit::None => s.push_str(&suit.to_string()),
            _ => s.push_str(&format!("{}{}", c.value, suit)),
        }

        s
    })
}

// action is a function that takes a `AllPrivateDraws`and a `String`. If it returns some string, it
// will send to the channel
async fn do_on_private_draw<F>(ctx: &Context, msg: &Message, action: F) -> CommandResult
where
    F: FnOnce(&mut AllPrivateDraws, String) -> Result<Option<String>, &'static str>,
{
    match msg.channel(&ctx.cache).await {
        Some(channel) => {
            let id = format!("{}#{}", msg.author, channel);
            let mut data = ctx.data.write().await;
            let privates = data.get_mut::<PrivateDraws>().unwrap();
            match action(privates, id) {
                Ok(Some(msg_to_send)) => {
                    super::send_message(ctx, msg, &msg_to_send).await?;
                }
                Err(e) => {
                    super::send_message(ctx, msg, &e).await?;
                }
                _ => {}
            }
        }
        None => {
            super::send_message(ctx, msg, "Error while accessing secret drawing").await?;
        }
    }
    Ok(())
}

async fn process_draw_args<'a>(
    ctx: &Context,
    msg: &Message,
    args: &'a Args,
) -> Result<(u32, Option<&'a str>, bool), serenity::Error> {
    let args = args.rest();
    let reason_idx = args.find(':');
    let secret_idx = args.find('s');
    let secret = match secret_idx {
        Some(i) => match reason_idx {
            Some(reason_idx) => i < reason_idx,
            None => true,
        },
        None => false,
    };
    let end = secret_idx.unwrap_or_else(|| args.len());
    let (nb, reason) = match reason_idx {
        Some(idx) => {
            let end = if end < idx { end } else { idx };
            let param = args[..end].trim();
            let nb = parse_number(ctx, msg, param).await?;
            (nb, Some(&args[idx..end]))
        }
        None => {
            let nb = parse_number(ctx, msg, args[..end].trim()).await?;
            (nb, None)
        }
    };

    Ok((nb, reason, secret))
}

async fn draw_card(
    ctx: &Context,
    msg: &Message,
    nb: u32,
) -> Result<Vec<caith::cards::Card>, serenity::Error> {
    let mut data = ctx.data.write().await;
    let decks = data.get_mut::<Decks>().unwrap();
    match decks.get_mut(&chat_id(&msg).to_string()) {
        Some(deck) => Ok(deck.draw(nb as usize)),
        None => {
            super::send_message(ctx, msg, "No deck to draw: use /newdeck <nb of jokers>").await?;
            Err(serenity::Error::Other("No deck"))
        }
    }
}

async fn parse_number(ctx: &Context, msg: &Message, param: &str) -> Result<u32, serenity::Error> {
    if param.is_empty() {
        Ok(1)
    } else {
        match param.parse::<u32>() {
            Ok(nb) => {
                // Can't draw 0 card, force to 1
                if nb == 0 {
                    Ok(1)
                } else {
                    Ok(nb)
                }
            }
            Err(_) => {
                super::send_message(ctx, msg, "Bad parameter: should be a number").await?;
                Err(serenity::Error::Other("Bad Param"))
            }
        }
    }
}
