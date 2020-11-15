use std::{
    collections::{HashMap, HashSet},
    env,
};

use caith::{Critic, RollResult};
use futures::future::FutureExt;

use serenity::{
    async_trait,
    client::{Context, EventHandler},
    framework::{
        standard::{
            help_commands,
            macros::{command, group, help, hook},
            Args, CommandGroup, CommandResult, DispatchError, HelpOptions,
        },
        StandardFramework,
    },
    http::Http,
    model::channel::ReactionType,
    model::{
        channel::{Message, PrivateChannel},
        guild::GuildStatus,
        id::{ChannelId, GuildId, UserId},
        prelude::Ready,
        Permissions,
    },
    prelude::TypeMapKey,
    Client,
};

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

use crate::alias;

async fn is_owner(ctx: &Context, msg: &Message) -> bool {
    if let Some(guild_id) = msg.guild_id {
        ctx.cache
            .guild_field(guild_id, |guild| msg.author.id == guild.owner_id)
            .await
            .unwrap_or(false)
    } else {
        false
    }
}

async fn is_admin(ctx: &Context, msg: &Message) -> bool {
    if let Some(member) = &msg.member {
        for role in &member.roles {
            if role
                .to_role_cached(&ctx.cache)
                .await
                .map_or(false, |r| r.has_permission(Permissions::ADMINISTRATOR))
            {
                return true;
            }
        }
        false
    } else {
        true // not in a chatroom = direct message, no role, allow all
    }
}

async fn is_super_user(ctx: &Context, msg: &Message) -> bool {
    is_owner(ctx, msg).await || is_admin(ctx, msg).await
}

async fn is_allowed(ctx: &Context, msg: &Message) -> bool {
    let data = ctx.data.read().await;
    let all_data = data.get::<Aliases>().unwrap();
    match all_data.get(&chat_id(msg)) {
        Some(data) => data.allowed.contains(msg.author.id.as_u64()),
        None => false,
    }
}

fn chat_id(msg: &Message) -> u64 {
    match msg.guild_id {
        Some(guild_id) => *guild_id.as_u64(),
        None => *msg.channel_id.as_u64(),
    }
}

struct InitDMTable;

impl TypeMapKey for InitDMTable {
    type Value = HashSet<u64>;
}

struct RerollTable;

impl TypeMapKey for RerollTable {
    type Value = HashMap<String, caith::Roller>;
}

pub(crate) struct Aliases;
impl TypeMapKey for Aliases {
    type Value = alias::AllData;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        for guild in ready.guilds {
            let guild_id = match guild {
                GuildStatus::OnlinePartialGuild(g) => g.id,
                GuildStatus::OnlineGuild(g) => g.id,
                GuildStatus::Offline(g) => g.id,
                _ => GuildId(0),
            };
            if guild_id != GuildId(0) {
                let mut data = ctx.data.write().await;
                let all_data = data.get_mut::<Aliases>().unwrap();
                if let Err(e) = all_data.load_alias_data(*guild_id.as_u64()) {
                    eprintln!("{}", e);
                }
            }
        }
    }

    async fn private_channel_create(&self, ctx: Context, channel: &PrivateChannel) {
        let need_init = {
            let mut data = ctx.data.write().await;
            let dm_is_init = data.get_mut::<InitDMTable>().unwrap();
            dm_is_init.get(channel.id.as_u64()).is_none()
        };

        if need_init {
            let res = {
                let mut data = ctx.data.write().await;
                let all_data = data.get_mut::<Aliases>().unwrap();
                all_data.load_alias_data(*channel.id.as_u64())
            };
            if let Err(e) = res {
                eprintln!("{}", e);
            } else {
                let mut data = ctx.data.write().await;
                let dm_is_init = data.get_mut::<InitDMTable>().unwrap();
                dm_is_init.insert(*channel.id.as_u64());
            }
        }
    }
}

#[group]
#[commands(roll, reroll, reroll_dice)]
struct Roll;

#[group]
#[prefix = "alias"]
#[description = "Alias management commands"]
#[commands(
    list_alias,
    set_global_alias,
    del_global_alias,
    set_user_alias,
    del_user_alias,
    clear_user_alias,
    list_users,
    allow_user_alias,
    disallow_user_alias,
    save_alias,
    load_alias,
    clear_global_aliases,
    clear_users
)]
struct Alias;

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    if let DispatchError::Ratelimited(seconds) = error {
        let _ = msg
            .channel_id
            .say(
                &ctx.http,
                &format!("Try this again in {} seconds.", seconds.as_secs()),
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

fn err_message(err: caith::RollError) -> String {
    match err {
        caith::RollError::ParseError(_) => format!("Error:\n```\n{}\n```", err),
        caith::RollError::ParamError(err) => format!("Error: {}", err),
    }
}

async fn process_roll_str(input: &str, ctx: &Context, msg: &Message) -> Result<RollResult, String> {
    // TODO: once caith can save the parsed result, manage error on `new`
    process_roll(caith::Roller::new(input).unwrap(), ctx, msg).await
}

async fn get_user_name(ctx: &Context, msg: &Message) -> String {
    match msg.guild_id {
        Some(guild_id) => msg
            .author
            .nick_in(&ctx.http, guild_id)
            .await
            .unwrap_or_else(|| msg.author.name.to_owned()),
        None => msg.author.name.to_owned(),
    }
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
                Critic::No => {}
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
        let (msg_to_send, crit) = if args.rest().starts_with("help") {
            (get_roll_help_msg(), None)
        } else {
            let input = {
                let data = ctx.data.read().await;
                let all_data = data.get::<Aliases>().unwrap();
                let mut alias_seen = HashSet::new();
                match all_data.get_alias(
                    args.rest(),
                    chat_id(msg),
                    *msg.author.id.as_u64(),
                    &mut alias_seen,
                ) {
                    Ok(Some(command)) => command,
                    Ok(None) => args.rest().to_string(),
                    Err(err) => err,
                }
            };
            match process_roll_str(&input, ctx, msg).await {
                Ok(res) => {
                    let set = crate::search_crit(&res);
                    (
                        format!(
                            "{} roll: {}{}",
                            msg.author,
                            if res.as_repeated().is_some() {
                                "\n"
                            } else {
                                ""
                            },
                            res
                        ),
                        if set.is_empty() { None } else { Some(set) },
                    )
                }
                Err(mut msg) => {
                    msg.insert_str(msg.len() - 4, ", or an alias");
                    (msg, None)
                }
            }
        };
        let sent_msg = send_message(ctx, msg.channel_id, &msg_to_send).await?;
        react_to(ctx, &sent_msg, crit).await?;
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
                        let set = crate::search_crit(&res);
                        (
                            format!("{} reroll `{}`: {}", msg.author, cmd, res),
                            if set.is_empty() { None } else { Some(set) },
                        )
                    }
                    Err(msg) => (msg, None),
                }
            }
            None => ("No previous roll".to_owned(), None),
        }
    };

    let sent_msg = send_message(ctx, msg.channel_id, &msg_to_send).await?;
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
    let msg_to_send = if input.starts_with("help") {
        get_roll_help_msg()
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
                Ok(res) => format!("{} reroll `{}`: {}", msg.author, dice, res),
                Err(e) => e.to_string(),
            },
            Err(err) => err,
        }
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

//
// Alias commands
//

#[command]
#[aliases("sg", "setg")]
#[min_args(2)]
/// ```
/// /alias sg alias_name roll_command
///
/// Create or replace an alias with roll_command.
/// The alias will be callable in a /roll command.
///
/// Command only available to role with Administrator permission.
/// ```
async fn set_global_alias(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let msg_to_send = if is_allowed(ctx, msg).await || is_super_user(ctx, msg).await {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.set_global_alias(alias, command, chat_id(msg))
    } else {
        "You are not allowed to set global aliases".to_owned()
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
#[aliases("dg", "delg")]
#[min_args(1)]
/// ```
/// /alias dg alias_name
///
/// Remove an alias
/// ```
async fn del_global_alias(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let msg_to_send = if is_allowed(ctx, msg).await || is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.del_global_alias(args.rest(), chat_id(msg))
    } else {
        "Only allowed users can delete global aliases".to_owned()
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
#[aliases("su", "set")]
#[min_args(2)]
/// ```
/// /alias set alias_name
///
/// Remove an alias
/// ```
async fn set_user_alias(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let msg_to_send = {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.set_user_alias(
            alias,
            command,
            chat_id(msg),
            *msg.author.id.as_u64(),
            &get_user_name(ctx, msg).await,
        )
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
#[aliases("du", "del")]
#[min_args(1)]
/// ```
/// /alias set alias_name
///
/// Remove an alias
/// ```
async fn del_user_alias(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let msg_to_send = {
        let alias = args.single::<String>().unwrap();
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.del_user_alias(&alias, chat_id(msg), *msg.author.id.as_u64())
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
/// ```
/// /alias clear_user_alias
///
/// Remove all calling user's aliases
/// ```
async fn clear_user_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let msg_to_send = {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.clear_user_aliases(chat_id(msg), *msg.author.id.as_u64())
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
#[aliases("allow", "au")]
#[min_args(1)]
/// ```
/// /alias allow user mention(s)
///
/// Allow the mentioned users to modify global aliases
/// ```
async fn allow_user_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if is_super_user(ctx, msg).await {
        if msg.mentions.is_empty() {
            send_message(
                ctx,
                msg.channel_id,
                "User to add must be mentioned (with `@`)",
            )
            .await?;
        } else {
            let mut users = String::new();
            for user in msg.mentions.iter() {
                let mut data = ctx.data.write().await;
                let all_data = data.get_mut::<Aliases>().unwrap();
                all_data.allow_user(*user.id.as_u64(), chat_id(msg));
                let name = user
                    .nick_in(&ctx.http, msg.guild_id.unwrap())
                    .await
                    .unwrap_or_else(|| user.name.to_owned());
                users = format!("{}, {}", users, name);
            }
            send_message(
                ctx,
                msg.channel_id,
                &format!(
                    "{} {} been allowed to manage global aliases",
                    users,
                    if msg.mentions.len() > 1 {
                        "have"
                    } else {
                        "has"
                    }
                ),
            )
            .await?;
        }
    } else {
        send_message(
            ctx,
            msg.channel_id,
            "Only administrator or server's owner can allow a user to manage global aliases",
        )
        .await?;
    }

    Ok(())
}

#[command]
#[aliases("disallow", "du")]
#[min_args(1)]
/// ```
/// /alias disallow user mentions
///
/// Forbid mentioned users to manage global aliases
/// ```
async fn disallow_user_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if is_super_user(ctx, msg).await {
        if msg.mentions.is_empty() {
            send_message(
                ctx,
                msg.channel_id,
                "User to remove must be mentionne (with `@`)",
            )
            .await?;
        } else {
            let mut users = String::new();
            for user in msg.mentions.iter() {
                let mut data = ctx.data.write().await;
                let all_data = data.get_mut::<Aliases>().unwrap();
                all_data.disallow_user(*user.id.as_u64(), chat_id(msg));
                let name = user
                    .nick_in(&ctx.http, msg.guild_id.unwrap())
                    .await
                    .unwrap_or_else(|| user.name.to_owned());
                users = format!("{}, {}", users, name);
            }
            send_message(
                ctx,
                msg.channel_id,
                &format!(
                    "{} {} been forbidden to manage global aliases",
                    users,
                    if msg.mentions.len() > 1 {
                        "have"
                    } else {
                        "has"
                    }
                ),
            )
            .await?;
        }
    } else {
        send_message(
            ctx,
            msg.channel_id,
            "Only administrator or server's owner can disallow a user to manage global aliases",
        )
        .await?;
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
    let msg_to_send = {
        let data = ctx.data.read().await;
        let all_data = data.get::<Aliases>().unwrap();
        let (user_aliases, global_aliases) =
            all_data.list_alias(chat_id(msg), *msg.author.id.as_u64());
        let fmt_aliases = |list: Vec<String>, title: String| {
            list.iter().fold(title, |mut acc, s| {
                acc.push_str(&s);
                acc.push('\n');
                acc
            })
        };
        let name = get_user_name(ctx, msg).await;
        let user_aliases = if !user_aliases.is_empty() {
            fmt_aliases(user_aliases, format!("{}'s aliases:\n", name))
        } else {
            format!("{} has no aliases set", name)
        };
        let global_aliases = if !global_aliases.is_empty() {
            fmt_aliases(global_aliases, "Global aliases:\n".to_string())
        } else {
            "No global aliases defined".to_owned()
        };
        format!("{}\n{}", user_aliases, global_aliases)
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await?;
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
    let data = ctx.data.read().await;
    let all_data = data.get::<Aliases>().unwrap();
    let users = all_data.list_allowed_users(chat_id(msg));
    if !users.is_empty() {
        let mut list = "Allowed users:\n".to_string();
        for user_id in users {
            let user_id = UserId(user_id);
            if let Ok(user) = user_id.to_user(&ctx.http).await {
                let name = user
                    .nick_in(&ctx.http, msg.guild_id.unwrap())
                    .await
                    .unwrap_or_else(|| user.name.to_owned());
                list.push_str("- ");
                list.push_str(&name);
                list.push('\n');
            }
        }
        send_message(ctx, msg.channel_id, &list).await?;
    } else {
        send_message(ctx, msg.channel_id, "No allowed user").await?;
    }

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
    let msg_to_send = if is_allowed(ctx, msg).await || is_super_user(ctx, msg).await {
        let data = ctx.data.read().await;
        let all_data = data.get::<Aliases>().unwrap();
        all_data.save_alias_data(chat_id(msg))?
    } else {
        "Only allowed users can save the configuration"
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await?;
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
    let msg_to_send = if is_allowed(ctx, msg).await || is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();

        all_data.load_alias_data(chat_id(msg))?
    } else {
        "Only allowed users can load the configuration"
    };

    send_message(ctx, msg.channel_id, &msg_to_send).await?;
    Ok(())
}

#[command]
#[max_args(0)]
/// ```
/// /alias clear_aliases
///
/// Delete all aliases. You can still undo this with a `load` until a `save` or a bot reboot.
/// ```
async fn clear_global_aliases(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let msg_to_send = if is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.clear_aliases(chat_id(msg))
    } else {
        "Only admin users can clear all the aliases"
    };
    send_message(ctx, msg.channel_id, msg_to_send).await?;
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
    let msg_to_send = if is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.clear_users(chat_id(msg))
    } else {
        "Only administrator or owner can clear allowed users list"
    };
    send_message(ctx, msg.channel_id, msg_to_send).await?;
    Ok(())
}

#[inline]
pub async fn send_message(
    ctx: &Context,
    channel_id: ChannelId,
    msg_to_send: &str,
) -> Result<Message, serenity::Error> {
    match channel_id.say(&ctx.http, msg_to_send).await {
        Err(e) => {
            eprintln!("Error sending message: {:?}", e);
            Err(e)
        }
        Ok(msg) => Ok(msg),
    }
}

pub async fn run() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN");
    if token.is_err() {
        eprintln!("No `DISCORD_TOKEN` env var, giving up Discord connection");
        return;
    }

    let token = token.unwrap();
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
        .help(&MY_HELP)
        .group(&ROLL_GROUP)
        .group(&ALIAS_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<InitDMTable>(HashSet::new());
        data.insert::<RerollTable>(HashMap::new());
        data.insert::<Aliases>(alias::AllData::new());
    }

    // save for exit bot saving
    let data = client.data.clone();

    let handle_client = async {
        if let Err(why) = client.start().await {
            eprintln!("Client error: {:?}", why);
        }
    };

    let handle_ctrlc = async {
        println!("Bot running, quit with Ctrl-C");
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for event");
        println!("Exiting…");
        let data = data.read().await;
        let all_data = data.get::<Aliases>().unwrap();
        all_data.save_all();
    };

    #[cfg(unix)]
    let handle_sigterm = async {
        let mut stream = signal(SignalKind::terminate()).expect("Error on getting sigterm stream");
        stream.recv().await;
        println!("Stoping…");
        let mut data = data.read().await;
        let all_data = data.get::<Aliases>().unwrap();
        all_data.save_all();
    };

    let all_fut = vec![
        handle_client.boxed(),
        handle_ctrlc.boxed(),
        #[cfg(unix)]
        handle_sigterm.boxed(),
    ];

    futures::future::select_all(all_fut).await;
}
