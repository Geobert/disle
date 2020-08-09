use std::{
    collections::{HashMap, HashSet},
    env,
    str::FromStr,
    sync::Mutex,
};

use caith::RollResult;
use futures::future::FutureExt;
use once_cell::sync::Lazy;
use serenity::{
    client::Context,
    framework::{
        standard::{
            help_commands,
            macros::{command, group, help, hook},
            Args, CommandGroup, CommandResult, DispatchError, HelpOptions,
        },
        StandardFramework,
    },
    http::Http,
    model::{
        channel::Message,
        id::{ChannelId, UserId},
    },
    Client,
};

mod alias;

#[group]
#[commands(roll, reroll, reroll_dice)]
struct Roll;

#[group]
#[prefix = "alias"]
#[description = "Alias management commands"]
#[commands(
    list_alias,
    set_alias,
    del_alias,
    list_users,
    allow_user_alias,
    disallow_user_alias,
    save_alias,
    load_alias,
    clear_aliases,
    clear_users
)]
struct Alias;

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

#[help]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[indention_prefix = "+"]
#[lacking_permissions = "Hide"]
#[lacking_role = "Nothing"]
#[wrong_channel = "Nothing"]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

fn get_roll_help_msg() -> String {
    "To get help, run `/help`".to_string()
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
                // do not store comment for reroll
                let input = match input.find('!') {
                    Some(idx) => &input[..idx],
                    None => input,
                };
                let mut reroll_table = REROLL_TABLE.lock().unwrap();
                reroll_table.insert(msg.author.to_string(), input.trim().to_owned());
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
///
///     Failure:
///     f#  : Value under which it is count as failure
///
///     Reason:
///     !   : Any text after `!` will be a comment"
/// ```
async fn roll(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if args.len() == 0 {
        if let Err(e) = msg.channel_id.say(&ctx.http, get_roll_help_msg()).await {
            eprintln!("Error sending message: {:?}", e);
        }
    } else {
        let maybe_alias = args.single::<String>().unwrap();
        let input = match alias::get_alias(&maybe_alias) {
            Some(command) => format!("{} {}", command, args.rest()),
            None => {
                args.restore();
                args.rest().to_owned()
            }
        };
        let msg_to_send = if input.starts_with("help") {
            get_roll_help_msg()
        } else {
            match process_roll(&input, ctx, msg).await {
                Ok((name, res)) => format!("{} roll: {}", name, res),
                Err(msg) => msg,
            }
        };
        send_message(ctx, msg.channel_id, &msg_to_send).await;
    }
    Ok(())
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
    let msg_to_send = if input.starts_with("help") {
        get_roll_help_msg()
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

    send_message(ctx, msg.channel_id, &msg_to_send).await;
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
    let msg_to_send = if input.starts_with("help") {
        get_roll_help_msg()
    } else {
        let input = {
            let reroll_table = REROLL_TABLE.lock().unwrap();
            reroll_table.get(&msg.author.to_string()).cloned()
        };
        match input {
            Some(input) => match caith::find_first_dice(&input) {
                Ok(dice) => match process_roll(&dice, ctx, msg).await {
                    Ok((name, res)) => format!("{} reroll `{}`: {}", name, dice, res),
                    Err(msg) => msg,
                },
                Err(e) => e.to_string(),
            },
            None => "No previous roll".to_owned(),
        }
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await;
    Ok(())
}

//
// Alias commands
//

#[command]
#[aliases("set")]
#[min_args(2)]
/// ```
/// /alias set alias_name roll_command
///
/// Create or replace an alias with roll_command.
/// The alias will be callable in a /roll command.
///
/// Command only available to role with Administrator permission.
/// ```
async fn set_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    alias::set_alias(ctx, msg, args).await;
    Ok(())
}

#[command]
#[aliases("del")]
#[min_args(1)]
/// ```
/// /alias del alias_name
///
/// Remove an alias
/// ```
async fn del_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    alias::del_alias(ctx, msg, args.rest()).await;
    Ok(())
}

#[command]
#[aliases("allow", "au")]
#[min_args(1)]
/// ```
/// /alias allow alias_name
///
/// Remove an alias
/// ```
async fn allow_user_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let user = args.rest();
    if user.starts_with("<@") {
        alias::allow_user(ctx, msg, user).await;
    } else {
        send_message(
            ctx,
            msg.channel_id,
            "User to add must be mentioned (with `@`)",
        )
        .await;
    }
    Ok(())
}

#[command]
#[aliases("disallow", "du")]
#[min_args(1)]
/// ```
/// /alias disallow alias_name
///
/// Remove an alias
/// ```
async fn disallow_user_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let user = args.rest();
    if user.starts_with("<@") {
        alias::disallow_user(ctx, msg, user).await;
    } else {
        send_message(
            ctx,
            msg.channel_id,
            "User to add must be mentionne (with `@`)",
        )
        .await;
    }
    Ok(())
}

#[command]
#[aliases("list", "l")]
#[max_args(0)]
/// ```
/// /alias list
///
/// List defined aliases
/// ```
async fn list_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let aliases = alias::list_aliases();
    let msg_to_send = if !aliases.is_empty() {
        aliases
            .iter()
            .fold("Existing aliases:\n".to_string(), |mut acc, s| {
                acc.push_str(&s);
                acc.push('\n');
                acc
            })
    } else {
        "No alias defined".to_owned()
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await;
    Ok(())
}

#[command]
#[aliases("users", "u")]
#[max_args(0)]
/// ```
/// /alias users
///
/// List authorized users
/// ```
async fn list_users(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let users = alias::list_allowed_users();
    let msg_to_send = if !users.is_empty() {
        let mut list = "Allowed users:\n".to_string();
        for user_str in &users {
            let user_id: UserId = UserId::from_str(user_str.as_str()).unwrap();
            match user_id.to_user(&ctx.http).await {
                Ok(user) => {
                    let name = user
                        .nick_in(&ctx.http, msg.guild_id.unwrap())
                        .await
                        .unwrap_or_else(|| user.name.to_owned());
                    list.push_str("- ");
                    list.push_str(&name);
                    list.push('\n');
                }
                Err(_) => (),
            }
        }
        list
    } else {
        "No allowed user".to_owned()
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await;
    Ok(())
}

#[command]
#[aliases("save")]
#[max_args(0)]
/// ```
/// /alias save
///
/// Persist alias data
/// ```
async fn save_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let msg_to_send = match alias::save_alias_data() {
        Ok(_) => "Configuration saved".to_owned(),
        Err(e) => format!("Error on saving: {}", e),
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await;
    Ok(())
}

#[command]
#[aliases("load")]
#[max_args(0)]
/// ```
/// /alias load
///
/// Load persistent alias data
/// ```
async fn load_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let msg_to_send = match alias::load_alias_data() {
        Ok(_) => "Configuration loaded".to_owned(),
        Err(e) => format!("Error on loading: {}", e),
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await;
    Ok(())
}

#[command]
#[max_args(0)]
/// ```
/// /alias clear_aliases
///
/// Delete all aliases. You can still undo this with a `load` until a `save` or a bot reboot.
/// ```
async fn clear_aliases(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    alias::clear_aliases();
    send_message(
        ctx,
        msg.channel_id,
        "Aliases cleared. You can still undo this with a `load` until a `save` or a bot reboot.",
    )
    .await;
    Ok(())
}

#[command]
#[max_args(0)]
/// ```
/// /alias clear_users
///
/// Delete all allowed users. You can still undo this with a `load` until a `save` or a bot reboot.
/// ```
async fn clear_users(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    alias::clear_users();
    send_message(
        ctx,
        msg.channel_id,
        "Users cleared. You can still undo this with a `load` until a `save` or a bot reboot.",
    )
    .await;
    Ok(())
}

#[inline]
pub async fn send_message(ctx: &Context, channel_id: ChannelId, msg_to_send: &str) {
    if let Err(e) = channel_id.say(&ctx.http, msg_to_send).await {
        eprintln!("Error sending message: {:?}", e);
    }
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

    if let Err(e) = alias::load_alias_data() {
        eprintln!("{:?}", e);
    }

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .on_mention(Some(bot_id))
                .prefix("/")
                .delimiters(vec![" "])
                .owners(owners)
        })
        .on_dispatch_error(dispatch_error)
        .help(&MY_HELP)
        .group(&ROLL_GROUP)
        .group(&ALIAS_GROUP);

    let mut client = Client::new(&token)
        .framework(framework)
        .await
        .expect("Err creating client");

    let handle_client = async {
        if let Err(why) = client.start().await {
            println!("Client error: {:?}", why);
        }
    };

    let handle_ctrlc = async {
        println!("Bot running, quit with Ctrl-C");
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for event");
        println!("Exiting…");
        match alias::save_alias_data() {
            Ok(_) => (),
            Err(e) => eprintln!("{:?}", e),
        }
    };

    futures::future::select(handle_client.boxed(), handle_ctrlc.boxed()).await;
}
