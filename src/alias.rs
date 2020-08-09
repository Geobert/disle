use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    sync::Mutex,
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::send_message;

const FILE_NAME: &str = "disle.ron";

static ALIAS_TABLE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

static ALLOWED_TABLE: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

use serenity::{
    client::Context,
    framework::standard::Args,
    model::{channel::Message, Permissions},
};

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

fn is_allowed(user: &str) -> bool {
    let allowed_table = ALLOWED_TABLE.lock().unwrap();
    allowed_table.contains(user)
}

pub(crate) async fn set_alias(ctx: &Context, msg: &Message, mut args: Args) {
    let msg_to_send = if is_allowed(&msg.author.to_string()) || is_admin(ctx, msg).await {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let mut alias_table = ALIAS_TABLE.lock().unwrap();
        let msg = format!("Alias `{}` set", alias);
        alias_table.insert(alias, command);
        msg
    } else {
        "You are not allowed to set aliases".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn del_alias(ctx: &Context, msg: &Message, alias: &str) {
    let msg_to_send = if is_allowed(&msg.author.to_string()) || is_admin(ctx, msg).await {
        let mut alias_table = ALIAS_TABLE.lock().unwrap();
        alias_table.remove(alias);
        format!("Alias `{}` deleted", alias)
    } else {
        "Only admin can delete aliases".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) fn get_alias(alias: &str) -> Option<String> {
    let alias_table = ALIAS_TABLE.lock().unwrap();
    alias_table.get(alias).map(|s| s.clone())
}

pub(crate) fn list_aliases() -> Vec<String> {
    let alias_table = ALIAS_TABLE.lock().unwrap();
    alias_table
        .iter()
        .map(|(k, v)| format!("`{}` = `{}`", k, v))
        .collect()
}

pub(crate) async fn allow_user(ctx: &Context, msg: &Message, user: &str) {
    let msg_to_send = if is_admin(ctx, msg).await {
        let mut allowed_table = ALLOWED_TABLE.lock().unwrap();
        allowed_table.insert(user.to_string());
        format!("{} has been allowed to manipulate alias", user)
    } else {
        "Only admin can allow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) async fn disallow_user(ctx: &Context, msg: &Message, user: &str) {
    let msg_to_send = if is_admin(ctx, msg).await {
        let mut allowed_table = ALLOWED_TABLE.lock().unwrap();
        allowed_table.remove(user);
        format!("{} has been disallowed to manipulate alias", user)
    } else {
        "Only admin can disallow a user to alias commands".to_owned()
    };
    send_message(ctx, msg.channel_id, &msg_to_send).await;
}

pub(crate) fn list_allowed_users() -> Vec<String> {
    let allowed_table = ALLOWED_TABLE.lock().unwrap();
    let mut list = Vec::from_iter(allowed_table.iter());
    list.sort_unstable();
    list.iter().map(|s| format!("{}", s)).collect()
}

pub(crate) fn clear_users() {
    let mut allowed_table = ALLOWED_TABLE.lock().unwrap();
    allowed_table.clear()
}

pub(crate) fn clear_aliases() {
    let mut alias_table = ALIAS_TABLE.lock().unwrap();
    alias_table.clear()
}

#[derive(Serialize, Deserialize)]
struct Data {
    aliases: HashMap<String, String>,
    users: HashSet<String>,
}

pub(crate) fn save_alias_data() -> std::io::Result<()> {
    let alias_table = ALIAS_TABLE.lock().unwrap();
    let allowed_table = ALLOWED_TABLE.lock().unwrap();
    if !alias_table.is_empty() || !allowed_table.is_empty() {
        let data = Data {
            aliases: alias_table.clone(),
            users: allowed_table.clone(),
        };

        let ser = ron::ser::to_string_pretty(&data, Default::default()).unwrap();
        std::fs::write(FILE_NAME, ser.as_bytes())
    } else {
        Ok(())
    }
}

pub(crate) fn load_alias_data() -> std::io::Result<()> {
    match std::fs::read_to_string(FILE_NAME) {
        Ok(content) => {
            let data: Data = ron::de::from_str(&content).unwrap();
            let mut alias_table = ALIAS_TABLE.lock().unwrap();
            let mut allowed_table = ALLOWED_TABLE.lock().unwrap();
            *alias_table = data.aliases;
            *allowed_table = data.users;
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
