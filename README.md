[![Crates.io](https://img.shields.io/crates/d/disle.svg)](https://crates.io/crates/disle)
[![Crates.io](https://img.shields.io/crates/v/disle.svg)](https://crates.io/crates/disle)

# Dìsle

A Discord bot that rolls RPG dices, with aliases management, written in Rust.

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
## Help
Type `/help` to get all the available help and `/help <command>` to get help on the command.

## Roll basics
```
/roll xdy [OPTIONS][TARGET][FAILURE][REASON]
(or "/r" for short)
  
rolls x dices of y sides

Options:
+ - / * : modifiers
e#  : Explode value
ie# or !# : Indefinite explode value
K#  : Keeping # highest (upperacse "K")
k#  : Keeping # lowest (lowercase "k")
D#  : Dropping the highest (uppercase "D")
d#  : Dropping the lowest (lowercase "d")
r#  : Reroll if <= value
ir# : Indefinite reroll if <= value
    
Target:
t#  : Target value to be a success

Failure: 
f#  : Value under which it is count as failure

Reason:
:  : Any text after `:` will be a comment
```

See the underlying crate `caith`'s [Readme for the full syntax](https://github.com/Geobert/caith/blob/master/README.md)

## Aliases

Aliases can be set per channel and per user. Basically, they store some expression and can
be called instead of a dice command. Imagine the alias `FS` has been setup to be `1d6! -
1d6!`. You can then do:

```
/r $FS
> Geob roll: [6][4] - [5] Result: 5
```

### Global Aliases

Global aliases are accessible to all the user in the channel. 

You can set a global alias:
```
/alias setg fs 1d6ie6 - 1d6ie6
> Alias `$FS` set

/r $FS
> Geob roll: [4] - [5] Result: -1
```

You can delete a global alias:
```
/alias delg fs
```

Global aliases are turned uppercase in order to distinguish them from user's aliases when
using them.

Only the server's owner, user with `Administrator` permission and specifically allowed
users can manage global aliases.

To allow user to tamper with the global aliases:

```
/alias allow @User<Tab> @User2<Tab>
/alias disallow @User<Tab> @User2<Tab>
```

It is important to use the `Tab` key to complete the user to get the mention.

Once in the list, the user can add/delete global aliases without having Administrator
permission.

### User's Aliases

Each user can set their own alias only accessible by them:

```
/alias set att d20
/r $att
> Geob roll: [11] Result: 11
```

Users aliases are turn to lowercase to avoid conflict with global ones.

### Aliases Expansion

When setting an alias, you can use aliases. Global aliases can only use other global
aliases and users can refer to either their own aliases or global ones:

```
/alias setg ATT d20
/alias set att $ATT + 4
/r att
> Geob roll: [12] + 4 Result: 16
```

Alias expansion occurs on use. So you can do things like that:

```
/alias setg ATT d20
/alias set att_bonus +4
/alias set att $ATT $att_bonus
/r att
> Geob roll: [11] +4 Result: 15
/alias set att_bonus +5
/r att
> Geob roll: [11] +5 Result: 16
```

Redefining `att_bonus` has an impact on `$att`.

Global alias can reference a user alias:
```
/alias setg DAG d20 + $dagger
```

but on calling the alias, if the user don't have `$dagger` defined or if a cycle occurs,
you'll get an error.

Of course, if you go messy and delete aliases referenced in others, you'll end with alias
not found errors on use.
