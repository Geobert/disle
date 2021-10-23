use std::collections::{HashMap, HashSet};

use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{
        channel::Message,
        id::{GuildId, RoleId},
    },
    prelude::TypeMapKey,
};

use crate::alias;

use super::send_message;

pub(crate) struct AliasMgrRole;
impl TypeMapKey for AliasMgrRole {
    type Value = HashMap<GuildId, RoleId>;
}

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
    save_alias,
    load_alias,
    clear_global_aliases
)]
struct Alias;

async fn is_allowed(ctx: &Context, msg: &Message) -> bool {
    let data = ctx.data.read().await;
    let all_roles = data.get::<AliasMgrRole>().unwrap();
    match msg.guild_id {
        Some(guild_id) => {
            let role_id = all_roles.get(&guild_id).unwrap();
            match msg.author.has_role(&ctx.http, guild_id, role_id).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error on verifying role: {}", e);
                    false
                }
            }
        }
        None => {
            eprintln!("no guild_id in msg");
            false
        }
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
) -> Result<(String, bool), String> {
    let data = ctx.data.read().await;
    let all_data = data.get::<Aliases>().unwrap();
    all_data.expand_alias(args.rest(), chat_id(msg), *msg.author.id.as_u64(), true)
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
    let msg_to_send = if is_allowed(ctx, msg).await {
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
    let msg_to_send = if is_allowed(ctx, msg).await {
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
    send_message(ctx, msg, msg_to_send).await?;
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
                acc.push_str(s);
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
#[aliases("save")]
#[max_args(0)]
/// ```
/// /alias save
///
/// Persist alias data
/// ```
async fn save_alias(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let msg_to_send = if is_allowed(ctx, msg).await {
        let data = ctx.data.read().await;
        let all_data = data.get::<Aliases>().unwrap();
        all_data.save_alias_data(chat_id(msg))?
    } else {
        "Only allowed users can save the configuration"
    };
    send_message(ctx, msg, msg_to_send).await?;
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
    let msg_to_send = if is_allowed(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();

        all_data.load_alias_data(chat_id(msg))?
    } else {
        "Only allowed users can load the configuration"
    };

    send_message(ctx, msg, msg_to_send).await?;
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
    let msg_to_send = if is_allowed(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<Aliases>().unwrap();
        all_data.clear_aliases(chat_id(msg))
    } else {
        "Only admin users can clear all the aliases"
    };
    send_message(ctx, msg, msg_to_send).await?;
    Ok(())
}
