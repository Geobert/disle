use std::collections::HashSet;

use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, id::UserId, Permissions},
    prelude::TypeMapKey,
};

use crate::alias;

use super::send_message;

pub(crate) struct Aliases;
impl TypeMapKey for Aliases {
    type Value = alias::AllData;
}

pub(crate) struct InitDMTable;
impl TypeMapKey for InitDMTable {
    type Value = HashSet<u64>;
}

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

pub(crate) fn chat_id(msg: &Message) -> u64 {
    match msg.guild_id {
        Some(guild_id) => *guild_id.as_u64(),
        None => *msg.channel_id.as_u64(),
    }
}

pub(crate) async fn parse_alias(
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> Result<String, String> {
    let data = ctx.data.read().await;
    let all_data = data.get::<Aliases>().unwrap();
    let mut alias_seen = HashSet::new();
    match all_data.get_alias(
        args.rest(),
        chat_id(msg),
        *msg.author.id.as_u64(),
        &mut alias_seen,
    ) {
        Ok(Some(command)) => Ok(command),
        Ok(None) => Ok(args.rest().to_string()),
        Err(e) => Err(e),
    }
}

pub(crate) async fn load_private_alias(ctx: Context, channel_id: u64) {
    let need_init = {
        let mut data = ctx.data.write().await;
        let dm_is_init = data.get_mut::<InitDMTable>().unwrap();
        dm_is_init.get(&channel_id).is_none()
    };

    if need_init {
        let res = {
            let mut data = ctx.data.write().await;
            let all_data = data.get_mut::<Aliases>().unwrap();
            all_data.load_alias_data(channel_id)
        };
        if let Err(e) = res {
            eprintln!("{}", e);
        } else {
            let mut data = ctx.data.write().await;
            let dm_is_init = data.get_mut::<InitDMTable>().unwrap();
            dm_is_init.insert(channel_id);
        }
    }
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

    send_message(ctx, msg, &msg_to_send).await?;
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

    send_message(ctx, msg, &msg_to_send).await?;
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
    send_message(ctx, msg, &msg_to_send).await?;
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
    send_message(ctx, msg, &msg_to_send).await?;
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
    send_message(ctx, msg, &msg_to_send).await?;
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
            send_message(ctx, msg, "User to add must be mentioned (with `@`)").await?;
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
                msg,
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
            msg,
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
            send_message(ctx, msg, "User to remove must be mentionne (with `@`)").await?;
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
                msg,
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
            msg,
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

    send_message(ctx, msg, &msg_to_send).await?;
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
        send_message(ctx, msg, &list).await?;
    } else {
        send_message(ctx, msg, "No allowed user").await?;
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
    send_message(ctx, msg, &msg_to_send).await?;
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

    send_message(ctx, msg, &msg_to_send).await?;
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
    send_message(ctx, msg, msg_to_send).await?;
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
    send_message(ctx, msg, msg_to_send).await?;
    Ok(())
}
