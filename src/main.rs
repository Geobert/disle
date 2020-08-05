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
    model::{channel::Message, id::UserId},
    Client,
};
use std::{collections::HashSet, env};

#[group]
#[commands(roll)]
struct Roll;

#[help]
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

#[command]
#[help_available]
#[min_args(1)]
#[description("Roll dice(s)")]
#[usage(
    r#"/roll xdy [OPTIONS]

Options:
+ - / * : modifiers
e# : Explode value
ie# : Indefinite explode value
K# : How many dice to keep out the roll, keeping the highest
k# : How many dice to keep out the roll, keeping the lowest
D# : How many dice to drop out the roll, dropping the highest
d# : How many dice to drop out the roll, dropping the lowest
r# : Reroll if <= value
ir# : Indefinite reroll if <= value
t# : Target value to be a success
f# : Value under which it is count as failuer
! : Any text after `!` will be a comment"#
)]
async fn roll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let res = caith::roll(input)?;
    if let Err(e) = msg
        .channel_id
        .say(&ctx.http, format!("{} roll: {}", msg.author, res))
        .await
    {
        eprintln!("Error sending message: {:?}", e);
    }

    Ok(())
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
        .help(&MY_HELP)
        .group(&ROLL_GROUP);

    let mut client = Client::new(&token)
        .framework(framework)
        .await
        .expect("Err creating client");

    // {
    //     let mut data = client.data.write().await;
    //     data.insert::<CommandCounter>(HashMap::default());
    //     data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    // }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
