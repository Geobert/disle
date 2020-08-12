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