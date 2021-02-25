use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

const DIR_NAME: &str = ".disle";

#[derive(Serialize, Deserialize)]
pub struct Data {
    // alias, command
    pub global_aliases: HashMap<String, String>,
    // user id, map of aliases (alias, command)
    pub users_aliases: HashMap<u64, HashMap<String, String>>,
}

impl Data {
    fn new() -> Self {
        Self {
            global_aliases: HashMap::new(),
            users_aliases: HashMap::new(),
        }
    }
}

// room_id, Data
pub struct AllData(HashMap<u64, Data>);

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

#[derive(Debug, PartialEq, Eq)]
struct Alias {
    name: String,
    args: Vec<String>,
}

impl Display for Alias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.args.is_empty() {
            write!(f, "{}", self.name)
        } else {
            let need_comma = self.args.len();
            let args = self
                .args
                .iter()
                .enumerate()
                .fold(String::new(), |mut s, (i, a)| {
                    s.push_str(a.as_str());
                    if i < need_comma {
                        s.push_str(", ")
                    }
                    s
                });
            write!(f, "{}|{}", args, self.name)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SplitPart {
    Alias(Alias),
    Expr(String),
    Comment(String),
    Err(String),
}

impl PartialOrd for SplitPart {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SplitPart {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self, other) {
            (SplitPart::Alias(_), SplitPart::Alias(_))
            | (SplitPart::Alias(_), SplitPart::Expr(_))
            | (SplitPart::Expr(_), SplitPart::Alias(_))
            | (SplitPart::Comment(_), SplitPart::Comment(_))
            | (SplitPart::Expr(_), SplitPart::Expr(_)) => Equal,

            (SplitPart::Alias(_), SplitPart::Comment(_))
            | (SplitPart::Expr(_), SplitPart::Comment(_)) => Less,

            (SplitPart::Comment(_), SplitPart::Alias(_))
            | (SplitPart::Comment(_), SplitPart::Expr(_))
            | (SplitPart::Comment(_), SplitPart::Err(_))
            | (SplitPart::Err(_), SplitPart::Alias(_))
            | (SplitPart::Err(_), SplitPart::Expr(_))
            | (SplitPart::Err(_), SplitPart::Comment(_))
            | (SplitPart::Err(_), SplitPart::Err(_))
            | (SplitPart::Alias(_), SplitPart::Err(_))
            | (SplitPart::Expr(_), SplitPart::Err(_)) => Greater,
        }
    }
}

impl<'a> Display for SplitPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitPart::Alias(a) => write!(f, "{}", &a),
            SplitPart::Expr(e) => write!(f, "{}", &e),
            SplitPart::Err(e) => write!(f, "{}", &e),
            SplitPart::Comment(c) => write!(f, " {}", &c),
        }
    }
}

impl SplitPart {
    pub fn is_empty(&self) -> bool {
        match self {
            SplitPart::Alias(a) => a.name.is_empty(),
            SplitPart::Expr(e) => e.is_empty(),
            SplitPart::Err(e) => e.is_empty(),
            SplitPart::Comment(c) => c.is_empty(),
        }
    }
}

fn split_alias_args(cmd: &str) -> Result<Alias, String> {
    match cmd.find("|") {
        Some(pipe_idx) => {
            if pipe_idx == 0 {
                return Err("Syntax error, no parameter before `|`".to_string());
            }
            if pipe_idx >= cmd.len() - 1 {
                return Err("Syntax error, no alias after `|`".to_string());
            }
            let args = &cmd[..pipe_idx];
            let name = &cmd[pipe_idx + 1..];
            if name.find("|").is_some() {
                return Err("Syntax error, can't have another `|`".to_string());
            }
            let args: Vec<String> = args.split(',').map(|s| s.to_owned()).collect();
            Ok(Alias {
                name: name.to_owned(),
                args,
            })
        }
        None => Ok(Alias {
            name: cmd.to_owned(),
            args: Vec::new(),
        }),
    }
}

// Split the cmd into aliases part and expr part
fn split_cmd(mut cmd: &str) -> Result<Vec<SplitPart>, String> {
    let mut splitted = Vec::new();
    let comment = cmd.find(':').map(|comment_start| {
        let comment = cmd[comment_start..].to_string();
        cmd = &cmd[..comment_start - 1];
        SplitPart::Comment(comment)
    });

    while let Some(start_alias) = cmd.find('$') {
        let end_alias = cmd[start_alias..]
            .find(|c: char| c.is_whitespace() || c == '+' || c == '-' || c == '*' || c == '/')
            .or_else(|| Some(cmd.len() - start_alias))
            .unwrap()
            + start_alias;
        if start_alias > 0 {
            // there some text before the alias, save it
            splitted.push(SplitPart::Expr(cmd[..start_alias].to_string()))
        }

        let alias = split_alias_args(&cmd[start_alias + 1..end_alias])?;
        splitted.push(SplitPart::Alias(alias));
        cmd = &cmd[end_alias..];
    }

    if !cmd.is_empty() {
        splitted.push(SplitPart::Expr(cmd.to_string()));
    }

    if let Some(comment) = comment {
        splitted.push(comment);
    }

    Ok(splitted)
}

fn collect_expanded(mut expanded: Vec<SplitPart>) -> Result<String, String> {
    let mut had_error = false;
    expanded.sort();
    let s = expanded
        .into_iter()
        .fold(String::new(), |acc, part| match part {
            SplitPart::Alias(_) | SplitPart::Expr(_) | SplitPart::Comment(_) => {
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
                    if !part.is_empty() {
                        format!("{}", part)
                    } else {
                        "".to_string()
                    }
                } else if !part.is_empty() {
                    format!("{}\n{}", acc, part)
                } else {
                    acc
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
    pub fn new() -> Self {
        AllData(HashMap::new())
    }

    pub fn expand_alias(
        &self,
        cmd: &str,
        chat_id: u64,
        user_id: u64,
        expand_args: bool,
    ) -> Result<String, String> {
        let mut alias_seen = HashSet::new();
        collect_expanded(self.user_alias_expansion(
            split_cmd(cmd)?,
            chat_id,
            user_id,
            &mut alias_seen,
            expand_args,
        )?)
    }

    fn expand_global_alias(
        &self,
        cmd: &str,
        chat_id: u64,
        expand_args: bool,
    ) -> Result<String, String> {
        let mut alias_seen = HashSet::new();
        collect_expanded(self.global_alias_expansion(
            split_cmd(cmd)?,
            chat_id,
            &mut alias_seen,
            expand_args,
        )?)
    }

    fn apply_args(&self, args: &Vec<String>, expansion: String) -> Result<String, String> {
        let mut applied = String::new();
        let mut slice = expansion.as_str();
        while let Some(start_pos) = slice.find('%') {
            let end_pos = {
                let end_idx = slice[start_pos + 1..].find(|c: char| !c.is_ascii_digit());
                match end_idx {
                    Some(end_idx) => end_idx + start_pos + 1,
                    None => slice.len(),
                }
            };

            let idx = {
                let pos = slice[start_pos + 1..end_pos]
                    .parse::<usize>()
                    .map_err(|e| format!("Can't parse: {}", e))?;
                pos - 1
            };
            if idx >= args.len() {
                return Err("Parameter reference is above number of parameter".to_string());
            }
            if start_pos > 0 {
                applied.push_str(&slice[..start_pos]);
            }
            applied.push_str(args.get(idx).unwrap().as_str());
            slice = &slice[end_pos..];
        }
        applied.push_str(slice);
        Ok(applied)
    }

    fn get_global_value_and_expand(
        &self,
        alias: &Alias,
        chat_id: u64,
        acc: &mut Vec<SplitPart>,
        alias_seen: &mut HashSet<String>,
        expand_args: bool,
    ) -> Result<(), String> {
        match self.get_global_alias_value(&alias.name, chat_id) {
            Ok(Some(expanded)) => {
                let expanded = if expand_args {
                    self.apply_args(&alias.args, expanded)?
                } else {
                    expanded
                };
                let expanded = split_cmd(&expanded)?;
                let mut expanded =
                    self.global_alias_expansion(expanded, chat_id, alias_seen, expand_args)?;
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
        Ok(())
    }

    fn global_alias_expansion(
        &self,
        splitted: Vec<SplitPart>,
        chat_id: u64,
        alias_seen: &mut HashSet<String>,
        expand_args: bool,
    ) -> Result<Vec<SplitPart>, String> {
        splitted.into_iter().try_fold(Vec::new(), |mut acc, part| {
            match part {
                p @ SplitPart::Expr(_) | p @ SplitPart::Err(_) | p @ SplitPart::Comment(_) => {
                    acc.push(p)
                }
                SplitPart::Alias(alias) => {
                    if alias_seen.contains(&alias.name) {
                        acc.push(SplitPart::Err(format!(
                            "`{}` was already expanded, we have a cycle definition",
                            alias.name
                        )));
                    } else if alias.name.chars().all(|c| c.is_lowercase()) {
                        // reference to a future user alias
                        acc.push(SplitPart::Expr(format!("${}", alias)))
                    } else {
                        alias_seen.insert(alias.name.clone());
                        self.get_global_value_and_expand(
                            &alias,
                            chat_id,
                            &mut acc,
                            alias_seen,
                            expand_args,
                        )?;
                    }
                }
            };
            Ok(acc)
        })
    }

    fn user_alias_expansion(
        &self,
        splitted: Vec<SplitPart>,
        chat_id: u64,
        user_id: u64,
        alias_seen: &mut HashSet<String>,
        expand_args: bool,
    ) -> Result<Vec<SplitPart>, String> {
        splitted.into_iter().try_fold(
            Vec::new(),
            |mut acc, part| -> Result<Vec<SplitPart>, String> {
                match part {
                    p @ SplitPart::Expr(_) | p @ SplitPart::Err(_) | p @ SplitPart::Comment(_) => {
                        acc.push(p)
                    }
                    SplitPart::Alias(alias) => {
                        if alias_seen.contains(&alias.name) {
                            acc.push(SplitPart::Err(format!(
                                "`${}` was already expanded, we have a cycle definition",
                                alias
                            )));
                        } else {
                            alias_seen.insert(alias.name.clone());
                            match self.get_alias_value(&alias.name, chat_id, user_id) {
                                Ok(Some(expanded)) => {
                                    let expanded = if expand_args {
                                        self.apply_args(&alias.args, expanded)?
                                    } else {
                                        expanded
                                    };
                                    let expanded = split_cmd(&expanded)?;
                                    let mut expanded = self.user_alias_expansion(
                                        expanded,
                                        chat_id,
                                        user_id,
                                        alias_seen,
                                        expand_args,
                                    )?;
                                    acc.append(&mut expanded);
                                }
                                Ok(None) => self.get_global_value_and_expand(
                                    &alias,
                                    chat_id,
                                    &mut acc,
                                    alias_seen,
                                    expand_args,
                                )?,
                                Err(err) => acc.push(SplitPart::Err(err)),
                            }
                        }
                    }
                };
                Ok(acc)
            },
        )
    }

    // get the correct HashMap and get the value of the alias
    fn get_alias_value(
        &self,
        alias: &str,
        chat_id: u64,
        user_id: u64,
    ) -> Result<Option<String>, String> {
        match self.get(&chat_id) {
            Some(data) => match data.users_aliases.get(&user_id) {
                Some(user_aliases) => match user_aliases.get(alias) {
                    p @ Some(_) => Ok(p.cloned()),
                    None => Ok(None),
                },
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    // get the correct HashMap and get the value of the alias
    fn get_global_alias_value(&self, alias: &str, chat_id: u64) -> Result<Option<String>, String> {
        match self.get(&chat_id) {
            Some(data) => match data.global_aliases.get(&alias.to_uppercase()) {
                p @ Some(_) => Ok(p.cloned()),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    pub fn set_global_alias(&mut self, alias: String, command: String, chat_id: u64) -> String {
        let alias = alias.trim_matches(|c: char| c == '$' || c.is_whitespace());
        // expand to check for cycles
        match self.expand_global_alias(&command, chat_id, false) {
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
        match self.expand_alias(&command, chat_id, user_id, false) {
            Ok(_) => {
                let data = self.entry(chat_id).or_insert_with(Data::new);
                let user_aliases = data
                    .users_aliases
                    .entry(user_id)
                    .or_insert_with(HashMap::new);
                let alias = alias.to_lowercase();
                let msg = format!("Alias `${}` set for user {}", alias, user_name);
                let msg = match alias.as_str() {
                    "ova" => {
                        format!("{}\nWarning: `ova` is also a roll command, if you want to call it, don't add space before parenthesis:\n`ova(5)`, not `ova (5)`", msg)
                    }
                    "cde" => {
                        format!("{}\nWarning: `cde` is also a roll command, if you want to call it, don't add space before parenthesis:\n`cde(5, fire)`, not `cde (5, fire)`", msg)
                    }
                    _ => msg,
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
        match data.users_aliases.get_mut(&user_id) {
            Some(user_aliases) => match user_aliases.remove(&alias) {
                Some(_) => format!("Alias `${}` deleted", alias),
                None => "Alias to delete not found".to_string(),
            },
            None => "Alias to delete not found".to_string(),
        }
    }

    pub fn clear_user_aliases(&mut self, chat_id: u64, user_id: u64) -> &'static str {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        match data.users_aliases.get_mut(&user_id) {
            Some(user_aliases) => {
                user_aliases.clear();
                "All your aliases have been deleted"
            }
            None => "You don't have any alias set",
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
        assert_eq!(expected, split_cmd("1d6 + 4d4").unwrap());
    }

    #[test]
    fn split_only_alias() {
        let expected = vec![SplitPart::Alias(Alias {
            name: "alias".to_string(),
            args: Vec::new(),
        })];
        assert_eq!(expected, split_cmd("$alias").unwrap());
    }

    #[test]
    fn split_alias() {
        let expected = vec![
            SplitPart::Alias(Alias {
                name: "fs".to_string(),
                args: Vec::new(),
            }),
            SplitPart::Expr(" + 4d4".to_string()),
        ];
        assert_eq!(expected, split_cmd("$fs + 4d4").unwrap());
    }

    #[test]
    fn split_two_aliases() {
        let expected = vec![
            SplitPart::Alias(Alias {
                name: "fs".to_string(),
                args: Vec::new(),
            }),
            SplitPart::Expr(" + 4d4 + ".to_string()),
            SplitPart::Alias(Alias {
                name: "attack".to_string(),
                args: Vec::new(),
            }),
        ];
        assert_eq!(expected, split_cmd("$fs + 4d4 + $attack").unwrap());
    }

    #[test]
    fn split_two_consecutive_aliases() {
        let expected = vec![
            SplitPart::Alias(Alias {
                name: "fs".to_string(),
                args: Vec::new(),
            }),
            SplitPart::Expr(" + ".to_string()),
            SplitPart::Alias(Alias {
                name: "attack".to_string(),
                args: Vec::new(),
            }),
            SplitPart::Expr(" + 4d4".to_string()),
        ];
        assert_eq!(expected, split_cmd("$fs + $attack + 4d4").unwrap());
    }

    #[test]
    fn get_global_alias_test() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4".to_string()),
            all.expand_alias("$GALIAS1", 0, 1, true)
        );
    }

    #[test]
    fn expand_no_alias() {
        let all = create_all_data();
        assert_eq!(Ok("1d8".to_string()), all.expand_alias("1d8", 0, 1, true));
    }

    #[test]
    fn expand_one_user_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d10".to_string()),
            all.expand_alias("$alias1", 0, 1, true)
        );
    }

    #[test]
    fn expand_global_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4".to_string()),
            all.expand_alias("$GALIAS1", 0, 1, true)
        );
    }

    #[test]
    fn expand_user_and_global_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4 + 1d10".to_string()),
            all.expand_alias("$GALIAS1 + $alias1", 0, 1, true)
        );
    }

    #[test]
    fn expand_user_with_global_alias() {
        let all = create_all_data();
        assert_eq!(
            Ok("1d4 + 1d6".to_string()),
            all.expand_alias("$alias2", 0, 1, true)
        );
    }

    #[test]
    fn expand_self_call_alias() {
        let all = create_all_data();
        assert_eq!(
            Err("`$alias_call_self` was already expanded, we have a cycle definition".to_string()),
            all.expand_alias("$alias_call_self", 0, 1, true)
        );
    }

    #[test]
    fn expand_alias_calling_self_call_alias() {
        let all = create_all_data();
        assert_eq!(
            Err("`$alias_call_self` was already expanded, we have a cycle definition".to_string()),
            all.expand_alias("$alias_call_self", 0, 1, true)
        );
    }

    #[test]
    fn expand_cycling_alias() {
        let all = create_all_data();
        assert_eq!(
            Err("`$cycle_alias1` was already expanded, we have a cycle definition".to_string()),
            all.expand_alias("$cycle_alias1", 0, 1, true)
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
            Ok("d6! - d6!".to_string()),
            all.expand_alias("$FS", 0, 1, true)
        );
    }

    #[test]
    fn add_alias_with_param_test() {
        let mut all = AllData::new();
        assert_eq!(
            "Global alias `$FS` set".to_string(),
            all.set_global_alias("fs".to_string(), "%1d6!".to_string(), 0)
        );
        assert_eq!(
            Ok("4d6!".to_string()),
            all.expand_alias("$4|FS", 0, 1, true)
        );
    }

    #[test]
    fn add_alias_with_2_params_test() {
        let mut all = AllData::new();
        assert_eq!(
            "Global alias `$FS` set".to_string(),
            all.set_global_alias("fs".to_string(), "%1d6! + %2".to_string(), 0)
        );
        assert_eq!(
            Ok("4d6! + 5".to_string()),
            all.expand_alias("$4,5|FS", 0, 1, true)
        );
    }

    #[test]
    fn add_alias_2_params_ref_same_test() {
        let mut all = AllData::new();
        assert_eq!(
            "Global alias `$FS` set".to_string(),
            all.set_global_alias("fs".to_string(), "%1d6! + %1".to_string(), 0)
        );
        assert_eq!(
            Ok("4d6! + 4".to_string()),
            all.expand_alias("$4|FS", 0, 1, true)
        );
    }

    #[test]
    fn add_alias_too_much_params_test() {
        let mut all = AllData::new();
        assert_eq!(
            "Global alias `$FS` set".to_string(),
            all.set_global_alias("fs".to_string(), "%1d6! + %2 * %3".to_string(), 0)
        );
        assert_eq!(
            Err("Parameter reference is above number of parameter".to_string()),
            all.expand_alias("$4,5|FS", 0, 1, true)
        );
    }

    #[test]
    fn get_alias() {
        let mut all = create_all_data();
        assert_eq!(
            Ok("1d4 + 1d6".to_string()),
            all.expand_alias("$alias2", 0, 1, true)
        );

        all.set_global_alias("$GALIAS1".to_string(), "4".to_string(), 0);
        assert_eq!(
            Ok("4 + 1d6".to_string()),
            all.expand_alias("$alias2", 0, 1, true)
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
        assert_eq!(
            Ok("d20 +4".to_string()),
            all.expand_alias("$att", 0, 1, true)
        );
    }

    #[test]
    fn comment_test() {
        let mut all = create_all_data();
        all.set_user_alias(
            "comm1".to_string(),
            "1d10 : comm1".to_string(),
            0,
            1,
            "toto",
        );
        assert_eq!(
            Ok("1d10 : comm1".to_string()),
            all.expand_alias("$comm1", 0, 1, true)
        );
    }

    #[test]
    fn comment2_test() {
        let mut all = create_all_data();
        all.set_user_alias(
            "comm1".to_string(),
            "1d10 : comm1".to_string(),
            0,
            1,
            "toto",
        );
        all.set_user_alias(
            "comm2".to_string(),
            "$comm1 : comm2".to_string(),
            0,
            1,
            "toto",
        );
        assert_eq!(
            Ok("1d10 : comm1 : comm2".to_string()),
            all.expand_alias("$comm2", 0, 1, true)
        );

        assert_eq!(
            Ok("1d10 + 2 : comm1 : comm2".to_string()),
            all.expand_alias("$comm2 + 2", 0, 1, true)
        );
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

        user_alias.insert("one_param".to_string(), "%1d6".to_string());
        user_alias.insert("two_params".to_string(), "%1d6 + %2".to_string());
        user_alias.insert("two_same_params".to_string(), "%1d6 + %1".to_string());

        data.users_aliases.insert(1, user_alias);
        data.global_aliases
            .insert("GALIAS1".to_string(), "1d4".to_string());

        all.insert(0, data);
        all
    }
}
