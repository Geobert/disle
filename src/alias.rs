use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

const DIR_NAME: &str = ".disle";

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub aliases: HashMap<String, String>,
    pub users: HashSet<String>,
}

impl Data {
    fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            users: HashSet::new(),
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

impl AllData {
    pub(crate) async fn set_alias(
        &mut self,
        alias: String,
        command: String,
        chat_id: u64,
    ) -> String {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        let msg = format!("Alias `{}` set", alias);
        data.aliases.insert(alias, command);
        msg
    }

    pub(crate) async fn del_alias(&mut self, alias: &str, chat_id: u64) -> String {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.aliases.remove(alias);
        format!("Alias `{}` deleted", alias)
    }

    pub(crate) async fn get_alias(&self, alias: &str, chat_id: u64) -> Option<String> {
        let (alias, rest) =
            if let Some(idx) = alias.find(|c| c == '+' || c == '-' || c == '*' || c == '/') {
                (&alias[..idx], Some(&alias[idx..]))
            } else {
                (alias, None)
            };
        match self.get(&chat_id) {
            Some(data) => match data.aliases.get(alias) {
                Some(alias) => {
                    if let Some(rest) = rest {
                        Some(format!("{} {}", alias, rest))
                    } else {
                        Some(alias.clone())
                    }
                }
                None => None,
            },
            None => None,
        }
    }

    pub(crate) async fn list_alias(&self, chat_id: u64) -> Vec<String> {
        match self.get(&chat_id) {
            Some(data) => data
                .aliases
                .iter()
                .map(|(k, v)| format!("`{}` = `{}`", k, v))
                .collect(),
            None => vec![],
        }
    }

    pub(crate) async fn allow_user(&mut self, user: &str, chat_id: u64) -> String {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.users.insert(user.to_string());
        format!("{} has been allowed to manipulate alias", user)
    }

    pub(crate) async fn disallow_user(&mut self, user: &str, chat_id: u64) -> String {
        let data = self.entry(chat_id).or_insert_with(Data::new);
        data.users.remove(user);
        format!("{} has been disallowed to manipulate alias", user)
    }

    pub(crate) async fn list_allowed_users(&self, chat_id: u64) -> Vec<String> {
        match self.get(&chat_id) {
            Some(data) => {
                let mut list = Vec::from_iter(data.users.iter());
                list.sort_unstable();
                list.iter().map(|s| s.to_string()).collect()
            }
            None => vec![],
        }
    }

    pub(crate) async fn clear_users(&mut self, chat_id: u64) -> &'static str {
        if let Some(data) = self.get_mut(&chat_id) {
            data.users.clear();
        }
        "Users cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    }

    pub(crate) async fn clear_aliases(&mut self, chat_id: u64) -> &'static str {
        if let Some(data) = self.get_mut(&chat_id) {
            data.aliases.clear();
        }
        "Aliases cleared. You can still undo this with a `load` until a `save` or a bot reboot."
    }

    pub(crate) async fn save_alias_data(&self, chat_id: u64) -> std::io::Result<&'static str> {
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

    pub(crate) async fn load_alias_data(&mut self, chat_id: u64) -> std::io::Result<&'static str> {
        let mut path = PathBuf::from(DIR_NAME);
        if path.exists() {
            path.push(format!("{}.ron", chat_id));
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let data: Data = ron::de::from_str(&content).unwrap();
                    self.insert(chat_id, data);
                }
                Err(e) => return Err(e),
            }
        }
        Ok("Configuration loaded")
    }

    pub(crate) fn save_all(&self) {
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
