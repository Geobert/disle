# 0.9.4
- update [`caith` 3.0.2](https://github.com/Geobert/caith/blob/master/CHANGELOG.md)

# 0.9.3
- update [`caith` 3.0.1](https://github.com/Geobert/caith/blob/master/CHANGELOG.md)

# 0.9.2
- update [`caith` 3.0.0](https://github.com/Geobert/caith/blob/master/CHANGELOG.md)

# 0.9.1
- update caith to 2.2.1

# 0.9.0
- if no dice in a roll, add a "game die" reaction to notify

# 0.8.0
- on critic, add a reaction emoji to the message

# 0.7.1
- on `/roll`, we can now reference other aliases in the `/roll` expression using `$`.
  Before, and since 0.6.2, we could call an alias without `$` prefix but we couldn't use
  another alias in the roll, now this works: `/r fs + $force`, with `fs` and `force` being
  2 defined aliases.

# 0.7.0
- Global alias can reference a user alias, error detection done on roll

# 0.6.2
- `$` is not mandatory to call alias on roll, still mandatory in alias definition when
  referring to another alias 
- Add a warning if ever an alias named "ova" is set as it is also a roll command in
  `caith`

# 0.6.1
- Update to [`caith` 2.0.1](https://github.com/Geobert/caith/blob/master/CHANGELOG.md)

# 0.6.0
- Revamp aliases with global and per user aliases
- Update to [`caith` 2.0.0](https://github.com/Geobert/caith/blob/master/CHANGELOG.md)

# 0.5.1
- Fix deadlock on reroll
- Refactor alias to not depends on Discord data
  - permission check are done in the Discord command rather than in alias module because
    of locking twice on Context.data, once to access AllData, second to access AllowList

# 0.5.0
- Update to `caith` 1.0 with breaking change for the bot: 
  - reason is now `1d6 : reason` instead of `1d6 ! reason`
  - `!` is an alias for `ie`, ex: `1d6!6` it equivalent to `1d6ie6`
  - number is optional for exploding dice: `1d6!` == `1d6!6`, `1d20!` == `1d20!20`

# 0.4.5
- No need for Admin permission for the owner anymore
- Permission check on clear users/aliases
- Permission check on save/load config
- Handle SIGTERM to save configuration on shutdown by supervisord (unix only)

# 0.4.4
- More efficient workaround to auto load config in DM

# 0.4.3
- Fix rolling in DM
- Add a workaround to auto load config on DM channel

# 0.4.2
- In DM, there's no role, so allow alias management by default

# 0.4.1
- Alias config saved per server now

# 0.4.0
- Update to `caith` 0.5.0 to get fudge dice support
- Add `/help`
- Alias system (see Readme)
- Added `reroll_dice` (`rd`): reroll only the first dice without any option of the last
  roll
- Manage Ctrl-C

# 0.3.0
- Update to `caith` 0.3.0
- Added alias `r` for `roll`
- Added `reroll` command with `rr` alias

# 0.2.0
- Update to `caith` 0.2.0
    - Better error feedback
    - Accept uppercase `D`

# 0.1.0
- Initial release