use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
};

use futures::future::FutureExt;

use serenity::{
    async_trait,
    client::{Context, EventHandler},
    framework::{
        standard::{
            help_commands,
            macros::{help, hook},
            Args, CommandGroup, CommandResult, DispatchError, HelpOptions,
        },
        Framework, StandardFramework,
    },
    http::Http,
    model::{
        channel::{ChannelType, GuildChannel, Message},
        event::MessageUpdateEvent,
        guild::GuildStatus,
        id::{ChannelId, GuildId, MessageId, UserId},
        prelude::Ready,
    },
    prelude::TypeMapKey,
    Client,
};

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

use crate::alias;

mod alias_cmd;
mod roll_cmd;

use alias_cmd::*;
use roll_cmd::*;

pub(crate) struct FrameworkContainer;
impl TypeMapKey for FrameworkContainer {
    type Value = Arc<Box<dyn Framework + Send + Sync + 'static>>;
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

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.is_private() {
            load_private_alias(ctx, *msg.channel_id.as_u64()).await;
        }
    }

    async fn channel_create(&self, ctx: Context, channel: &GuildChannel) {
        if channel.kind == ChannelType::Private {
            load_private_alias(ctx, *channel.id.as_u64()).await
        }
    }

    async fn message_update(
        &self,
        ctx: Context,
        _old_if_available: Option<Message>,
        new: Option<Message>,
        _event: MessageUpdateEvent,
    ) {
        if let Some(msg) = new {
            let framework = {
                let ctx_clone = ctx.clone();
                let data = ctx_clone.data.read().await;
                data.get::<FrameworkContainer>().unwrap().clone()
            };
            let id = msg.id.clone();
            let channel_id = msg.channel_id.clone();
            let ctx_clone = ctx.clone();
            framework.dispatch(ctx, msg).await;
            strikethrough_previous_reply(ctx_clone, id, channel_id).await;
        }
    }
}

async fn strikethrough_previous_reply(ctx: Context, ref_msg_id: MessageId, channel_id: ChannelId) {
    match channel_id
        .messages(&ctx.http, |retriever| retriever.after(&ref_msg_id))
        .await
    {
        Ok(mut messages) if !messages.is_empty() => {
            // We are guarded against empty vec, unwrap is safe
            let last_id = if messages.len() > 1 {
                Some(messages.first().unwrap().id.clone())
            } else {
                None
            };
            if let Some(msg_to_edit) = messages.iter_mut().rev().find(|m| {
                if let Some(ref_msg) = &m.referenced_message {
                    // Do not strikethrough a message twice
                    ref_msg.id == ref_msg_id
                        && !m.content.starts_with("~~")
                        && last_id.map_or_else(|| true, |the_id| m.id != the_id)
                } else {
                    false
                }
            }) {
                let content = msg_to_edit.content.clone();
                if let Err(e) = msg_to_edit
                    .edit(&ctx, |new_msg| new_msg.content(&format!("~~{}~~", content)))
                    .await
                {
                    eprintln!("Error while editing: {}", e);
                }
            }
        }
        Err(e) => eprintln!("{}", e),
        _ => {}
    }
}

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

fn err_message(err: caith::RollError) -> String {
    match err {
        caith::RollError::ParseError(_) => format!("Error:\n```\n{}\n```", err),
        caith::RollError::ParamError(err) => format!("Error: {}", err),
    }
}

#[inline]
pub(crate) async fn send_message(
    ctx: &Context,
    orig_msg: &Message,
    msg_to_send: &str,
) -> Result<Message, serenity::Error> {
    match orig_msg.reply_ping(&ctx.http, msg_to_send).await {
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

    let framework = Arc::new(Box::new(
        StandardFramework::new()
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
            .group(&ALIAS_GROUP),
    ) as Box<dyn Framework + Send + Sync + 'static>);

    let framework2 = framework.clone();

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework_arc(framework2)
        .await
        .expect("Err creating client");
    client.cache_and_http.cache.set_max_messages(10).await;

    {
        let mut data = client.data.write().await;
        data.insert::<InitDMTable>(HashSet::new());
        data.insert::<RerollTable>(HashMap::new());
        data.insert::<Aliases>(alias::AllData::new());
        data.insert::<FrameworkContainer>(framework);
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
