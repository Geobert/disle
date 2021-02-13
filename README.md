[![Crates.io](https://img.shields.io/crates/d/disle.svg)](https://crates.io/crates/disle)
[![Crates.io](https://img.shields.io/crates/v/disle.svg)](https://crates.io/crates/disle)

# Dìsle

A Discord bot that rolls RPG dices, with aliases management and reaction to critics,
written in Rust.

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
t#  : Minimum value to be a success
tt# : Minimum value to be double success
t[x, y, z, …]: Value to consider as success

Failure: 
f#  : Value under which it is count as failure

Reason:
:  : Any text after `:` will be a comment
```

See the underlying crate `caith`'s [Readme for the full syntax](https://github.com/Geobert/caith/blob/master/README.md)

## Specific game support

Some games have special rules to interpret the dices and Dìsle supports some:
- "OVA: The Anime Role-Playing Game" result (`/r ova(<number>)`, ex: `/r ova(4)`)
- "Hong-Kong : Les Chroniques de l'Étrange" (`/r cde(<number of dice>, <element>)`, 
  ex: `/r cde(5, fire)`)
  - PS in French: Vu que le jeu est Français, on peut écrire les éléments en FR aussi, le
    résultat sera en Français !

## Cards

You can now use a deck of cards. In a server channel:

- create a shuffled deck of cards: `/newdeck or /nd <nb_of_joker>`
- draw cards from the deck: `/draw or /d <nb_of_cards>` 
- add `s` to draw cards secretly: `/d <nb_of_cards> s`
- reveal your secret draw: `/reveal or /rev`
- discard your secret draw: `/discard or /dis`
- shuffle the deck again: `/shuffle or /sh`
- query how many cards left in the deck: `/remain`

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

Only specifically allowed users can manage global aliases. A dedicated role "Dìsle Alias"
is created and any user member of this role can edit global aliases.

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

## Reaction on Critics

In a roll expression, if a dice reach its minimum value or maximum value, a reaction is
added to the response to highlight it.

Another reaction is added if the expression does not contain any die, ex: `/r 20 + 4`
(because sometime, we miss the `d` when typing `/r d20` :p).