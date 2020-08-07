use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Mutex,
};

use caith::RollResult;
use once_cell::sync::Lazy;
use serenity::{
    client::Context,
    framework::{
        standard::{
            macros::{command, group, hook},
            Args, CommandResult, DispatchError,
        },
        StandardFramework,
    },
    http::Http,
    model::channel::Message,
    Client,
};

mod alias;

#[group]
#[commands(roll, reroll, set_alias)]
struct Roll;

static REROLL_TABLE: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    if let DispatchError::Ratelimited(seconds) = error {
        let _ = msg
            .channel_id
            .say(
                &ctx.http,
                &format!("Try this again in {} seconds.", seconds),
            )
            .await;
    }
}

fn get_help_msg() -> String {
    r#"```
    /roll xdy [OPTIONS][TARGET][FAILURE][REASON]
    (or "/r" for short)
    
    rolls `x` dices of `y` sides

    `y` can also be "F" or "f" for Fudge/Fate dice.

    /reroll (or /rr)
    
    reroll the last roll of the user

    Options:
    + - / * : modifiers
    e#  : Explode value
    ie# : Indefinite explode value
    K#  : Keeping # highest (upperacse "K")
    k#  : Keeping # lowest (lowercase "k")
    D#  : Dropping the highest (uppercase "D")
    d#  : Dropping the lowest (lowercase "d")
    r#  : Reroll if <= value
    ir# : Indefinite reroll if <= value
    
    Target:
    t#  : Target value to be a success

    Failure: 
    f#  : Value under which it is count as failuer

    Reason:
    !   : Any text after `!` will be a comment
    ```"#
        .to_string()
}

async fn process_roll(
    input: &str,
    ctx: &Context,
    msg: &Message,
) -> Result<(String, RollResult), String> {
    match caith::roll(input) {
        Ok(res) => {
            let name = msg
                .author
                .nick_in(&ctx.http, msg.guild_id.unwrap())
                .await
                .unwrap_or_else(|| msg.author.name.to_owned());
            {
                let mut reroll_table = REROLL_TABLE.lock().unwrap();
                reroll_table.insert(msg.author.to_string(), input.to_owned());
            }
            Ok((name, res))
        }
        Err(err) => match err {
            caith::RollError::ParseError(_) => Err(format!("Error:\n```\n{}\n```", err)),
            caith::RollError::ParamError(err) => Err(format!("Error: {}", err)),
        },
    }
}

#[command]
#[aliases("r")]
#[description("Roll dice(s)")]
async fn roll(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.len() == 0 {
        if let Err(e) = msg.channel_id.say(&ctx.http, get_help_msg()).await {
            eprintln!("Error sending message: {:?}", e);
        }
    } else {
        let maybe_alias = args.single::<String>().unwrap();
        let input = match alias::get_alias(&maybe_alias) {
            Some(command) => format!("{} {}", command, args.rest()),
            None => args.rest().to_owned(),
        };
        let msg_to_send = if input.starts_with("help") {
            get_help_msg()
        } else {
            match process_roll(&input, ctx, msg).await {
                Ok((name, res)) => format!("{} roll: {}", name, res),
                Err(msg) => msg,
            }
        };

        if let Err(e) = msg.channel_id.say(&ctx.http, msg_to_send).await {
            eprintln!("Error sending message: {:?}", e);
        }
    }
    Ok(())
}

#[command]
#[aliases("rr")]
#[description("Reroll last roll")]
async fn reroll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let msg_to_send = if input.starts_with("help") {
        get_help_msg()
    } else {
        let input = {
            let reroll_table = REROLL_TABLE.lock().unwrap();
            reroll_table.get(&msg.author.to_string()).cloned()
        };
        match input {
            Some(input) => match process_roll(&input, ctx, msg).await {
                Ok((name, res)) => format!("{} reroll `{}`: {}", name, input, res),
                Err(msg) => msg,
            },
            None => "No previous roll".to_owned(),
        }
    };

    if let Err(e) = msg.channel_id.say(&ctx.http, msg_to_send).await {
        eprintln!("Error sending message: {:?}", e);
    }
    Ok(())
}

#[command]
#[aliases("setalias")]
#[min_args(2)]
async fn set_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    alias::set_alias(ctx, msg, args).await
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new_with_token(&token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .on_mention(Some(bot_id))
                .prefix("/")
                .delimiters(vec![" "])
                .owners(owners)
        })
        .on_dispatch_error(dispatch_error)
        // .help(&MY_HELP)
        .group(&ROLL_GROUP);

    let mut client = Client::new(&token)
        .framework(framework)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
