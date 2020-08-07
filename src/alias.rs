use std::{collections::HashMap, sync::Mutex};

use once_cell::sync::Lazy;

static ALIAS_TABLE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
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

pub(crate) async fn set_alias(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let msg_to_send = if is_admin(ctx, msg).await {
        let alias = args.single::<String>().unwrap();
        let command = args.rest().to_string();
        let mut alias_table = ALIAS_TABLE.lock().unwrap();
        let msg = format!("Alias `{}` set", alias);
        alias_table.insert(alias, command);
        msg
    } else {
        "Only admin can set aliases".to_owned()
    };
    if let Err(e) = msg.channel_id.say(&ctx.http, msg_to_send).await {
        eprintln!("Error sending message: {:?}", e);
    }
    Ok(())
}

pub(crate) fn get_alias(alias: &str) -> Option<String> {
    let alias_table = ALIAS_TABLE.lock().unwrap();
    alias_table.get(alias).map(|s| s.clone())
}
