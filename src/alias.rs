use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    iter::FromIterator,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

const DIR_NAME: &str = ".disle";

#[derive(Serialize, Deserialize)]
pub struct Data {
    // alias, command
    pub global_aliases: HashMap<String, String>,
    // user id
    pub allowed: HashSet<u64>,
    // user id, map of aliases (alias, command)
    pub users_aliases: HashMap<u64, HashMap<String, String>>,
}

impl Data {
    fn new() -> Self {
        Self {
            global_aliases: HashMap::new(),
            allowed: HashSet::new(),
            users_aliases: HashMap::new(),
        }
    }
}

// room_id, Data
pub struct AllData(HashMap<u64, Data>);

impl AllData {
    pub fn new() -> Self {
        AllData(HashMap::new())
    }
}

impl Deref for AllData {
    type Target = HashMap<u64, Data>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, PartialEq)]
enum SplitPart {
    Alias(String),
    Expr(String),
    Err(String),
}

impl<'a> Display for SplitPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitPart::Alias(a) => write!(f, "{}", &a),
            SplitPart::Expr(e) => write!(f, "{}", &e),
            SplitPart::Err(e) => write!(f, "{}", &e),
        }
    }
}

fn split_cmd(mut cmd: &str) -> Vec<SplitPart> {
    let mut splitted = Vec::new();
    while let Some(start_alias) = cmd.find('$') {
        let end_alias = cmd[start_alias..]
            .find(|c| c == ' ' || c == '+' || c == '-' || c == '*' || c == '/')
            .or_else(|| Some(cmd.len() - start_alias))
            .unwrap()
            + start_alias;
        if start_alias > 0 {
            // there some text before the alias, save it
            splitted.push(SplitPart::Expr(cmd[..start_alias].to_string()))
        }

        splitted.push(SplitPart::Alias(
            cmd[start_alias + 1..end_alias].to_string(),
        ));
        cmd = &cmd[end_alias..];
    }

    if !cmd.is_empty() {
        splitted.push(SplitPart::Expr(cmd.to_string()));
    }

    splitted
}

fn collect_expanded(expanded: Vec<SplitPart>) -> Result<String, String> {
    let mut had_error = false;
    // 2nd field of tuple is
    let s = expanded
        .into_iter()
        .fold(String::new(), |acc, part| match part {
            SplitPart::Alias(_) | SplitPart::Expr(_) => {
                if !had_error {
                    format!("{}{}", acc, part)
                } else {
                    acc
                }
            }
            SplitPart::Err(_) => {
                if !had_error {
                    had_error = true;
                    // no errors before, wipe all and only keep errors from now on
                    format!("{}\n", part)
                } else {
                    format!("{}{}\n", acc, part)
                }
            }
        });

    if had_error {
        Err(s)
    } else {
        Ok(s)
    }
}

impl AllData {
    fn expand_alias(&self, cmd: &str, chat_id: u64, user_id: u64) -> Result<String, String> {
        let mut alias_seen = HashSet::new();
        collect_expanded(self.alias_expansion(cmd, chat_id, user_id, &mut alias_seen))
    }

    fn expand_global_alias(&self, cmd: &str, chat_id: u64) -> Result<String, String> {
        let mut alias_seen = HashSet::new();
        collect_expanded(self.global_alias_expansion(cmd, chat_id, &mut alias_seen))
    }

    fn global_alias_expansion(
        &self,
        cmd: &str,
        chat_id: u64,
        alias_seen: &mut HashSet<String>,
    ) -> Vec<SplitPart> {
        let splitted = split_cmd(cmd);
        splitted.into_iter().fold(Vec::new(), |mut acc, part| {
            match part {
                p @ SplitPart::Expr(_) => acc.push(p),
                p @ SplitPart::Err(_) => acc.push(p),
                SplitPart::Alias(alias) => {
                    if alias_seen.contains(&alias) {
                        acc.push(SplitPart::Err(format!(
                            "`{}` was already expanded, we have a cycle definition",
                            alias
                        )));
                    } else {
                        if alias.chars().all(|c| c.is_lowercase()) {
                            // reference to a future user alias
                            acc.push(SplitPart::Expr(format!("${}", alias)))
                        } else {
                            match self.get_global_alias(&alias, chat_id) {
                                Ok(Some(expanded)) => {
                                    alias_seen.insert(alias);
                                    let mut expanded =
                                        self.global_alias_expansion(&expanded, chat_id, alias_seen);
                                    acc.append(&mut expanded);
                                }
                                Ok(None) => {
                                    acc.push(SplitPart::Err(format!(
                                        "`${}` not found amongs global aliases",
                                        alias
                                    )));
                                }
                                Err(err) => acc.push(SplitPart::Err(err)),
                            }
                        }
                    }
                }
            };
            acc
        })
    }

    fn alias_expansion(
        &self,
        cmd: &str,
        chat_id: u64,
        user_id: u64,
        alias_seen: &mut HashSet<String>,
    ) -> Vec<SplitPart> {
        let splitted = split_cmd(cmd);
        splitted.into_iter().fold(Vec::new(), |mut acc, part| {
            match part {
                p @ SplitPart::Expr(_) => acc.push(p),
                p @ SplitPart::Err(_) => acc.push(p),
                SplitPart::Alias(alias) => {
                    if alias_seen.contains(&alias) {
                        acc.push(SplitPart::Err(format!(
                            "`${}` was already expanded, we have a cycle definition",
                            alias
                        )));
                    } else {
                        match self.get_alias(&alias, chat_id, user_id) {
                            Ok(Some(expanded)) => {
                                alias_seen.insert(alias);
                                let mut expanded =
                                    self.alias_expansion(&expanded, chat_id, user_id, alias_seen);
                                acc.append(&mut expanded);
                            }
                            Ok(None) => {
                                acc.push(SplitPart::Err(format!("`${}` not found", alias)));
                            }
                            Err(err) => acc.push(SplitPart::Err(err)),
                        }
                    }
                }
            };
            acc
        })
    }

    pub fn set_global_alias(&mut self, alias: String, command: String, chat_id: u64) -> String {
        let alias = alias.trim_matches(|c: char| c == '$' || c.is_whitespace());
        // expand to check for cycles
        match self.expand_global_alias(&command, chat_id) {
            Ok(_) => {
                let alias = alias.to_uppercase();
                let data = self.entry(chat_id).or_insert_with(Data::new);
                let msg = format!("Global alias `${}` set", alias);
                data.global_aliases.insert(alias, command);
                msg
            }
            Err(s) => s,
        }
    }

    pub fn del_global_alias(&mut self, alias: &str, chat_id: u64) -> String {
        let alias = alias
            .trim_matches(|c: char| c == '$' || c.is_whitespace())
            .to_uppercase();
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.global_aliases.remove(&alias);
        format!("Global alias `${}` deleted", alias)
    }

    pub fn set_user_alias(
        &mut self,
        alias: String,
        command: String,
        chat_id: u64,
        user_id: u64,
        user_name: &str,
    ) -> String {
        let alias = alias.trim_matches(|c: char| c == '$' || c.is_whitespace());
        // expand to check for cycles
        match self.expand_alias(&command, chat_id, user_id) {
            Ok(_) => {
                let data = self.entry(chat_id).or_insert_with(Data::new);
                let user_aliases = data
                    .users_aliases
                    .entry(user_id)
                    .or_insert_with(HashMap::new);
                let alias = alias.to_lowercase();
                let msg = format!("Alias `${}` set for user {}", alias, user_name);
                let msg = if alias == "ova" {
                    format!("{}\nWarning: `ova` is also a roll command, if you want to call it, don't add space before parenthesis:\n`ova(5)`, not `ova (5)`", msg)
                } else {
                    msg
                };
                user_aliases.insert(alias, command);
                msg
            }
            Err(s) => s,
        }
    }

    pub fn del_user_alias(&mut self, alias: &str, chat_id: u64, user_id: u64) -> String {
        let alias = alias
            .trim_matches(|c: char| c == '$' || c.is_whitespace())
            .to_lowercase();
        let data = self.entry(chat_id).or_insert_with(Data::new);
        if let Some(user_aliases) = data.users_aliases.get_mut(&user_id) {
            match user_aliases.remove(&alias) {
                Some(_) => format!("Alias `${}` deleted", alias),
                None => "Alias to delete not found".to_string(),
            }
        } else {
            "Alias to delete not found".to_string()
        }
    }

    pub fn clear_user_aliases(&mut self, chat_id: u64, user_id: u64) -> &'static str {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        if let Some(user_aliases) = data.users_aliases.get_mut(&user_id) {
            user_aliases.clear();
            "All your aliases have been deleted"
        } else {
            "You don't have any alias set"
        }
    }

    fn get_global_alias(&self, alias: &str, chat_id: u64) -> Result<Option<String>, String> {
        let (alias, rest) =
            if let Some(idx) = alias.find(|c| c == '+' || c == '-' || c == '*' || c == '/') {
                (&alias[..idx], Some(&alias[idx..]))
            } else {
                (alias, None)
            };
        let alias = alias
            .trim_matches(|c: char| c == '$' || c.is_whitespace())
            .to_uppercase();
        match self.get(&chat_id) {
            Some(data) => match data.global_aliases.get(&alias) {
                Some(cmd) => match self.expand_global_alias(&cmd, chat_id) {
                    Ok(expanded) => {
                        if let Some(rest) = rest {
                            Ok(Some(format!("{} {}", expanded, rest)))
                        } else {
                            Ok(Some(expanded))
                        }
                    }
                    Err(err_msg) => Err(err_msg),
                },
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    pub fn get_alias(
        &self,
        alias: &str,
        chat_id: u64,
        user_id: u64,
    ) -> Result<Option<String>, String> {
        let (alias, rest) =
            if let Some(idx) = alias.find(|c| c == '+' || c == '-' || c == '*' || c == '/') {
                (&alias[..idx], Some(&alias[idx..]))
            } else {
                (alias, None)
            };
        let alias = alias.trim_matches(|c: char| c == '$' || c.is_whitespace());
        let recompose_rest = |cmd: &String| {
            if let Some(rest) = rest {
                Some(format!("{} {}", cmd, rest))
            } else {
                Some(cmd.clone())
            }
        };

        let search_global =
            |data: &Data, alias: &str| match data.global_aliases.get(&alias.to_uppercase()) {
                Some(cmd) => match self.expand_alias(&cmd, chat_id, user_id) {
                    Ok(expanded) => Ok(recompose_rest(&expanded)),
                    Err(err) => Err(err),
                },
                None => Ok(None),
            };

        match self.get(&chat_id) {
            Some(data) => match data.users_aliases.get(&user_id) {
                Some(user_aliases) => match user_aliases.get(alias) {
                    Some(cmd) => match self.expand_alias(&cmd, chat_id, user_id) {
                        Ok(expanded) => Ok(recompose_rest(&expanded)),
                        Err(err) => Err(err),
                    },
                    None => search_global(data, &alias),
                },
                None => search_global(data, &alias),
            },
            None => Ok(None),
        }
    }

    pub fn list_alias(&self, chat_id: u64, user_id: u64) -> (Vec<String>, Vec<String>) {
        match self.get(&chat_id) {
            Some(data) => (
                match data.users_aliases.get(&user_id) {
                    Some(user_aliases) => user_aliases
                        .iter()
                        .map(|(k, v)| format!("`{}` = `{}`", k, v))
                        .collect(),
                    None => vec![],
                },
                data.global_aliases
                    .iter()
                    .map(|(k, v)| format!("`{}` = `{}`", k, v))
                    .collect(),
            ),
            None => (vec![], vec![]),
        }
    }

    pub fn allow_user(&mut self, user: u64, chat_id: u64) {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.allowed.insert(user);
    }

    pub fn disallow_user(&mut self, user: u64, chat_id: u64) {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.allowed.remove(&user);
    }

    pub fn list_allowed_users(&self, chat_id: u64) -> Vec<u64> {
        match self.get(&chat_id) {
            Some(data) => Vec::from_iter(data.allowed.iter().copied()),
            None => vec![],
        }
    }

    pub fn clear_users(&mut self, chat_id: u64) -> &'static str {
        if let Some(data) = self.get_mut(&chat_id) {
            data.allowed.clear();
        }
        "Users cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    }

    pub fn clear_aliases(&mut self, chat_id: u64) -> &'static str {
        if let Some(data) = self.get_mut(&chat_id) {
            data.global_aliases.clear();
        }
        "Aliases cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    }

    pub fn save_alias_data(&self, chat_id: u64) -> std::io::Result<&'static str> {
        let msg = match self.get(&chat_id) {
            Some(data) => {
                let ser = ron::ser::to_string_pretty(&data, Default::default()).unwrap();
                let mut path = PathBuf::from(DIR_NAME);
                if !path.exists() {
                    match std::fs::create_dir(DIR_NAME) {
                        Ok(_) => (),
                        Err(e) => eprintln!("{}", e),
                    }
                }
                path.push(format!("{}.ron", chat_id));
                std::fs::write(path, ser.as_bytes())?;
                "Configuration saved"
            }
            None => "Nothing to save",
        };

        Ok(msg)
    }

    pub fn load_alias_data(&mut self, chat_id: u64) -> std::io::Result<&'static str> {
        let mut path = PathBuf::from(DIR_NAME);
        if path.exists() {
            path.push(format!("{}.ron", chat_id));
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let data: Data = ron::de::from_str(&content).unwrap_or_else(|_| Data::new());
                    self.insert(chat_id, data);
                }
                Err(e) => return Err(e),
            }
        }
        Ok("Configuration loaded")
    }

    pub fn save_all(&self) {
        let keys: Vec<_> = { self.keys().cloned().collect() };
        for chat_id in keys {
            if let Some(data) = self.get(&chat_id) {
                let ser = ron::ser::to_string_pretty(&data, Default::default()).unwrap();
                let mut path = PathBuf::from(DIR_NAME);
                if !path.exists() {
                    match std::fs::create_dir(DIR_NAME) {
                        Ok(_) => (),
                        Err(e) => eprintln!("{}", e),
                    }
                }
                path.push(format!("{}.ron", chat_id));
                if let Err(e) = std::fs::write(path, ser.as_bytes()) {
                    eprintln!("{}", e.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_no_alias() {
        let expected = vec![SplitPart::Expr("1d6 + 4d4".to_string())];
        assert_eq!(expected, split_cmd("1d6 + 4d4"));
    }

    #[test]
    fn split_only_alias() {
        let expected = vec![SplitPart::Alias("alias".to_string())];
        assert_eq!(expected, split_cmd("$alias"));
    }

    #[test]
    fn split_alias() {
        let expected = vec![
            SplitPart::Alias("fs".to_string()),
            SplitPart::Expr(" + 4d4".to_string()),
        ];
        assert_eq!(expected, split_cmd("$fs + 4d4"));
    }

    #[test]
    fn split_two_aliases() {
        let expected = vec![
            SplitPart::Alias("fs".to_string()),
            SplitPart::Expr(" + 4d4 + ".to_string()),
            SplitPart::Alias("attack".to_string()),
        ];
        assert_eq!(expected, split_cmd("$fs + 4d4 + $attack"));
    }

    #[test]
    fn split_two_consecutive_aliases() {
        let expected = vec![
            SplitPart::Alias("fs".to_string()),
            SplitPart::Expr(" + ".to_string()),
            SplitPart::Alias("attack".to_string()),
            SplitPart::Expr(" + 4d4".to_string()),
        ];
        assert_eq!(expected, split_cmd("$fs + $attack + 4d4"));
    }

    #[test]
    fn get_global_alias_test() {
        let all = create_all_data();
        assert_eq!(Ok(Some("1d4".to_string())), all.get_alias("$GALIAS1", 0, 1));
    }

    #[test]
    fn expand_no_alias() {
        let all = create_all_data();
        assert_eq!(Ok("1d8".to_string()), all.expand_alias("1d8", 0, 1));
    }

    #[test]
    fn expand_one_user_alias() {
        let all = create_all_data();
        assert_eq!(Ok("1d10".to_string()), all.expand_alias("$alias1", 0, 1));
    }

    #[test]
    fn expand_global_alias() {
        let all = create_all_data();
        assert_eq!(Ok("1d4".to_string()), all.expand_alias("$GALIAS1", 0, 1));
    }

    #[test]
    fn expand_user_and_global_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4 + 1d10".to_string()),
            all.expand_alias("$GALIAS1 + $alias1", 0, 1)
        );
    }

    #[test]
    fn expand_user_with_global_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4 + 1d6".to_string()),
            all.expand_alias("$alias2", 0, 1)
        );
    }

    #[test]
    fn expand_self_call_alias() {
        let all = create_all_data();
        assert_eq!(
            Err(
                "`$alias_call_self` was already expanded, we have a cycle definition\n".to_string()
            ),
            all.expand_alias("$alias_call_self", 0, 1)
        );
    }

    #[test]
    fn expand_alias_calling_self_call_alias() {
        let all = create_all_data();
        assert_eq!(
            Err(
                "`$alias_call_self` was already expanded, we have a cycle definition\n".to_string()
            ),
            all.expand_alias("$alias_call_self", 0, 1)
        );
    }

    #[test]
    fn expand_cycling_alias() {
        let all = create_all_data();
        assert_eq!(
            Err("`$cycle_alias1` was already expanded, we have a cycle definition\n".to_string()),
            all.expand_alias("$cycle_alias1", 0, 1)
        );
    }

    #[test]
    fn add_global_alias_test() {
        let mut all = AllData::new();
        assert_eq!(
            "Global alias `$FS` set".to_string(),
            all.set_global_alias("fs".to_string(), "d6! - d6!".to_string(), 0)
        );
        assert_eq!(
            Ok(Some("d6! - d6!".to_string())),
            all.get_global_alias("$FS", 0)
        );
    }

    #[test]
    fn get_alias() {
        let mut all = create_all_data();
        assert_eq!(
            Ok(Some("1d4 + 1d6".to_string())),
            all.get_alias("alias2", 0, 1)
        );

        all.set_global_alias("GALIAS1".to_string(), "4".to_string(), 0);
        assert_eq!(
            Ok(Some("4 + 1d6".to_string())),
            all.get_alias("alias2", 0, 1)
        );

        all.set_global_alias("ATT".to_string(), "d20".to_string(), 0);
        all.set_user_alias("bonus_att".to_string(), "+4".to_string(), 0, 1, "toto");
        all.set_user_alias(
            "att".to_string(),
            "$ATT $bonus_att".to_string(),
            0,
            1,
            "toto",
        );
        assert_eq!(Ok(Some("d20 +4".to_string())), all.get_alias("att", 0, 1));
    }

    fn create_all_data() -> AllData {
        let mut all = AllData::new();
        let mut data = Data::new();
        let mut user_alias = HashMap::new();

        user_alias.insert("alias1".to_string(), "1d10".to_string());
        user_alias.insert("alias2".to_string(), "$GALIAS1 + 1d6".to_string());
        user_alias.insert(
            "alias_call_self".to_string(),
            "$alias_call_self + 1d6".to_string(),
        );
        user_alias.insert(
            "alias_call_alias_that_call_self".to_string(),
            "$alias_call_self + 1d6".to_string(),
        );
        user_alias.insert(
            "cycle_alias1".to_string(),
            "$cycle_alias2 + 1d6".to_string(),
        );
        user_alias.insert(
            "cycle_alias2".to_string(),
            "$cycle_alias3 + 1d10".to_string(),
        );
        user_alias.insert(
            "cycle_alias3".to_string(),
            "$cycle_alias1 + 1d8".to_string(),
        );

        data.users_aliases.insert(1, user_alias);
        data.global_aliases
            .insert("GALIAS1".to_string(), "1d4".to_string());

        all.insert(0, data);
        all
    }
}
