use serenity::{
    client::Context,
    framework::{
        standard::{
            macros::{command, group, hook},
            Args, CommandResult, DispatchError,
        },
        StandardFramework,
    },
    http::Http,
    model::channel::Message,
    Client,
};
use std::{collections::HashSet, env};

#[group]
#[commands(roll)]
struct Roll;

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
async fn roll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let msg_to_send = if input.trim().starts_with("help") {
        r#"```
/roll xdy [OPTIONS]

rolls x dices of y sides

Options:
+ - / * : modifiers
e#  : Explode value
ie# : Indefinite explode value
K#  : Keeping # highest (upperacse "K")
k#  : Keeping # lowest (lowercase "k")
D#  : Dropping the highest (uppercase "D")
d#  : Dropping the lowest (lowercase "d")
r#  : Reroll if <= value
ir# : Indefinite reroll if <= value
t#  : Target value to be a success
f#  : Value under which it is count as failuer
!   : Any text after `!` will be a comment
```"#
            .to_string()
    } else {
        let res = caith::roll(input)?;
        let name = msg
            .author
            .nick_in(&ctx.http, msg.guild_id.unwrap())
            .await
            .unwrap_or_else(|| msg.author.name.to_owned());
        format!("{} roll: {}", name, res)
    };

    if let Err(e) = msg.channel_id.say(&ctx.http, msg_to_send).await {
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
        // .help(&MY_HELP)
        .group(&ROLL_GROUP);

    let mut client = Client::new(&token)
        .framework(framework)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
