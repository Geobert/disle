# Dìsle

A Discord bot that rolls RPG dices, written in Rust.

# Installation

You need to host the bot yourself.

- Create an application on Discord https://discord.com/developers/applications
- Create a bot
- On your bot server, set an environment variable named `DISCORD_TOKEN` with the bot's
  token.
- Run the bot
- In `https://discordapp.com/oauth2/authorize?client_id=CLIENTID&scope=bot`, replace
  CLIENTID with the value of your application Client ID (in "General Information).
- Go the this modified URL and allow the bot to your Discord's server.

# Usage

```
/roll xdy [OPTIONS][TARGET][FAILURE][REASON]
(or "/r" for short)
  
rolls x dices of y sides

/reroll (or /rr)
    
reroll the last roll of the user

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
    
Target:
t#  : Target value to be a success

Failure: 
f#  : Value under which it is count as failuer

Reason:
!   : Any text after `!` will be a comment
```

See the underlying crate `caith`'s [Readme for the full syntax](https://github.com/Geobert/caith/blob/master/README.md)

