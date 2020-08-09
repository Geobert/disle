use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    path::PathBuf,
    sync::Mutex,
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::send_message;

use serenity::{
    client::Context,
    framework::standard::Args,
    model::{channel::Message, Permissions},
};

const DIR_NAME: &str = ".disle";

static DATA_TABLE: Lazy<Mutex<HashMap<u64, Data>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Serialize, Deserialize)]
struct Data {
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
    }
    false
}

fn is_allowed(guild_id: u64, user: &str) -> bool {
    let all_data = DATA_TABLE.lock().unwrap();
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
    let msg_to_send = if is_allowed(guild_id, &msg.author.to_string()) || is_admin(ctx, msg).await {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let mut all_data = DATA_TABLE.lock().unwrap();
        let data = all_data.entry(guild_id).or_insert(Data::new());
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
    let msg_to_send = if is_allowed(guild_id, &msg.author.to_string()) || is_admin(ctx, msg).await {
        let mut all_data = DATA_TABLE.lock().unwrap();
        let data = all_data.entry(guild_id).or_insert(Data::new());
        data.aliases.remove(alias);
        format!("Alias `{}` deleted", alias)
    } else {
        "Only admin can delete aliases".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) fn get_alias(msg: &Message, alias: &str) -> Option<String> {
    let guild_id = guild_id(msg);
    let all_data = DATA_TABLE.lock().unwrap();
    match all_data.get(&guild_id) {
        Some(data) => data.aliases.get(alias).map(|s| s.clone()),
        None => None,
    }
}

pub(crate) fn list_aliases(msg: &Message) -> Vec<String> {
    let guild_id = guild_id(msg);
    let all_data = DATA_TABLE.lock().unwrap();
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
    let msg_to_send = if is_admin(ctx, msg).await {
        let mut all_data = DATA_TABLE.lock().unwrap();
        let data = all_data.entry(guild_id).or_insert(Data::new());
        data.users.insert(user.to_string());
        format!("{} has been allowed to manipulate alias", user)
    } else {
        "Only admin can allow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn disallow_user(ctx: &Context, msg: &Message, user: &str) {
    let guild_id = guild_id(msg);
    let msg_to_send = if is_admin(ctx, msg).await {
        let mut all_data = DATA_TABLE.lock().unwrap();
        let data = all_data.entry(guild_id).or_insert(Data::new());
        data.users.remove(user);
        format!("{} has been disallowed to manipulate alias", user)
    } else {
        "Only admin can disallow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) fn list_allowed_users(msg: &Message) -> Vec<String> {
    let guild_id = guild_id(msg);
    let all_data = DATA_TABLE.lock().unwrap();
    match all_data.get(&guild_id) {
        Some(data) => {
            let mut list = Vec::from_iter(data.users.iter());
            list.sort_unstable();
            list.iter().map(|s| format!("{}", s)).collect()
        }
        None => vec![],
    }
}

pub(crate) fn clear_users(msg: &Message) {
    let guild_id = guild_id(msg);
    let mut all_data = DATA_TABLE.lock().unwrap();
    if let Some(data) = all_data.get_mut(&guild_id) {
        data.users.clear();
    }
}

pub(crate) fn clear_aliases(msg: &Message) {
    let guild_id = guild_id(msg);
    let mut all_data = DATA_TABLE.lock().unwrap();
    if let Some(data) = all_data.get_mut(&guild_id) {
        data.aliases.clear();
    }
}

pub(crate) fn save_alias_data(guild_id: u64) -> std::io::Result<()> {
    let all_data = DATA_TABLE.lock().unwrap();

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

pub(crate) fn save_all() {
    let keys: Vec<_> = {
        let all_data = DATA_TABLE.lock().unwrap();
        all_data.keys().cloned().collect()
    };
    for k in keys {
        match save_alias_data(k) {
            Ok(_) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

pub(crate) fn load_alias_data(guild_id: u64) -> std::io::Result<()> {
    let mut path = PathBuf::from(DIR_NAME);
    if path.exists() {
        path.push(format!("{}.ron", guild_id));
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let data: Data = ron::de::from_str(&content).unwrap();
                let mut all_data = DATA_TABLE.lock().unwrap();
                all_data.insert(guild_id, data);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
