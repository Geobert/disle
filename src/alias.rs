use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::send_message;

use serenity::{
    client::Context,
    framework::standard::Args,
    model::{channel::Message, Permissions},
};

const DIR_NAME: &str = ".disle";

#[derive(Serialize, Deserialize)]
pub(crate) struct Data {
    aliases: HashMap<String, String>,
    users: HashSet<String>,
}

impl Data {
    fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            users: HashSet::new(),
        }
    }
}

pub(crate) async fn is_super_user(ctx: &Context, msg: &Message) -> bool {
    is_owner(ctx, msg).await || is_admin(ctx, msg).await
}

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

fn is_allowed(all_data: &mut HashMap<u64, Data>, guild_id: u64, user: &str) -> bool {
    match all_data.get(&guild_id) {
        Some(data) => data.users.contains(user),
        None => false,
    }
}

pub(crate) fn guild_id(msg: &Message) -> u64 {
    match msg.guild_id {
        Some(guild_id) => *guild_id.as_u64(),
        None => *msg.channel_id.as_u64(),
    }
}

pub(crate) async fn set_alias(ctx: &Context, msg: &Message, mut args: Args) {
    let guild_id = guild_id(msg);
    let mut data = ctx.data.write().await;
    let all_data = data.get_mut::<crate::Aliases>().unwrap();
    let msg_to_send = if is_allowed(all_data, guild_id, &msg.author.to_string())
        || is_super_user(ctx, msg).await
    {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let data = all_data.entry(guild_id).or_insert_with(Data::new);
        let msg = format!("Alias `{}` set", alias);
        data.aliases.insert(alias, command);
        msg
    } else {
        "You are not allowed to set aliases".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn del_alias(ctx: &Context, msg: &Message, alias: &str) {
    let guild_id = guild_id(msg);
    let mut data = ctx.data.write().await;
    let all_data = data.get_mut::<crate::Aliases>().unwrap();
    let msg_to_send = if is_allowed(all_data, guild_id, &msg.author.to_string())
        || is_super_user(ctx, msg).await
    {
        let data = all_data.entry(guild_id).or_insert_with(Data::new);
        data.aliases.remove(alias);
        format!("Alias `{}` deleted", alias)
    } else {
        "Only admin can delete aliases".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn get_alias(ctx: &Context, msg: &Message, alias: &str) -> Option<String> {
    let guild_id = guild_id(msg);
    let data = ctx.data.read().await;
    let all_data = data.get::<crate::Aliases>().unwrap();
    match all_data.get(&guild_id) {
        Some(data) => data.aliases.get(alias).cloned(),
        None => None,
    }
}

pub(crate) async fn list_aliases(ctx: &Context, msg: &Message) -> Vec<String> {
    let guild_id = guild_id(msg);
    let data = ctx.data.read().await;
    let all_data = data.get::<crate::Aliases>().unwrap();
    match all_data.get(&guild_id) {
        Some(data) => data
            .aliases
            .iter()
            .map(|(k, v)| format!("`{}` = `{}`", k, v))
            .collect(),
        None => vec![],
    }
}

pub(crate) async fn allow_user(ctx: &Context, msg: &Message, user: &str) {
    let guild_id = guild_id(msg);
    let msg_to_send = if is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<crate::Aliases>().unwrap();
        let data = all_data.entry(guild_id).or_insert_with(Data::new);
        data.users.insert(user.to_string());
        format!("{} has been allowed to manipulate alias", user)
    } else {
        "Only administrator or server's owner can allow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn disallow_user(ctx: &Context, msg: &Message, user: &str) {
    let guild_id = guild_id(msg);
    let msg_to_send = if is_super_user(ctx, msg).await {
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<crate::Aliases>().unwrap();
        let data = all_data.entry(guild_id).or_insert_with(Data::new);
        data.users.remove(user);
        format!("{} has been disallowed to manipulate alias", user)
    } else {
        "Only administrator or server's owner disallow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn list_allowed_users(ctx: &Context, msg: &Message) -> Vec<String> {
    let guild_id = guild_id(msg);
    let data = ctx.data.read().await;
    let all_data = data.get::<crate::Aliases>().unwrap();

    match all_data.get(&guild_id) {
        Some(data) => {
            let mut list = Vec::from_iter(data.users.iter());
            list.sort_unstable();
            list.iter().map(|s| s.to_string()).collect()
        }
        None => vec![],
    }
}

pub(crate) async fn clear_users(ctx: &Context, msg: &Message) {
    let msg_to_send = if is_super_user(ctx, msg).await {
        let guild_id = guild_id(msg);
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<crate::Aliases>().unwrap();
        if let Some(data) = all_data.get_mut(&guild_id) {
            data.users.clear();
        }
        "Users cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    } else {
        "Only administrator or owner can clear allowed users list"
    };

    send_message(ctx, msg.channel_id, msg_to_send).await;
}

pub(crate) async fn clear_aliases(ctx: &Context, msg: &Message) {
    let msg_to_send = if is_super_user(ctx, msg).await {
        let guild_id = guild_id(msg);
        let mut data = ctx.data.write().await;
        let all_data = data.get_mut::<crate::Aliases>().unwrap();
        if let Some(data) = all_data.get_mut(&guild_id) {
            data.aliases.clear();
        }
        "Aliases cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    } else {
        "Only administrator or owner can clear aliases"
    };

    send_message(ctx, msg.channel_id, msg_to_send).await;
}

// save and load are not protected by permission here because they can be called without a command
// at startup/shutdown of the bot.
//
// The permissions are checked in the commands
pub(crate) fn save_alias_data(all_data: &HashMap<u64, Data>, guild_id: u64) -> std::io::Result<()> {
    match all_data.get(&guild_id) {
        Some(data) => {
            let ser = ron::ser::to_string_pretty(&data, Default::default()).unwrap();
            let mut path = PathBuf::from(DIR_NAME);
            if !path.exists() {
                match std::fs::create_dir(DIR_NAME) {
                    Ok(_) => (),
                    Err(e) => eprintln!("{}", e),
                }
            }
            path.push(format!("{}.ron", guild_id));
            std::fs::write(path, ser.as_bytes())
        }
        None => Ok(()),
    }
}

pub(crate) fn save_all(alias_data: &HashMap<u64, Data>) {
    let keys: Vec<_> = { alias_data.keys().cloned().collect() };
    for k in keys {
        match save_alias_data(alias_data, k) {
            Ok(_) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

pub(crate) async fn load_alias_data(ctx: &Context, guild_id: u64) -> std::io::Result<()> {
    let mut path = PathBuf::from(DIR_NAME);
    if path.exists() {
        path.push(format!("{}.ron", guild_id));
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let data: Data = ron::de::from_str(&content).unwrap();
                let mut client_data = ctx.data.write().await;
                let all_data = client_data.get_mut::<crate::Aliases>().unwrap();
                all_data.insert(guild_id, data);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
