use crate::config::dir;

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::time::SystemTime;
const CACHE_LIMIT: u32 = 200;

static NOW: OnceCell<u64> = OnceCell::new();

static NEXT_TTL: OnceCell<u64> = OnceCell::new();

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Cache {
    pub items: HashMap<String, u64>,
}

fn file() -> std::path::PathBuf {
    dir().join("cache.toml")
}

pub fn setup() {
    NOW.set(now()).unwrap();
    NEXT_TTL
        .set(*NOW.get().unwrap() + 60 * 60 * 24 * 7)
        .unwrap();

    let cache = file();
    if !cache.exists() {
        write(Cache {
            items: HashMap::new(),
        });
    }
}

pub fn read() -> Cache {
    let cfg = std::fs::read_to_string(file()).unwrap();
    let cache: Cache = toml::from_str(&cfg).unwrap();

    cache
}

pub fn write(cache: Cache) {
    std::fs::write(dir().join("cache.toml"), toml::to_string(&cache).unwrap()).unwrap();
}

impl Cache {
    pub fn has(&self, code: &str) -> bool {
        match self.items.get(code) {
            Some(item) => *NOW.get().unwrap() > *item,
            None => false,
        }
    }

    pub fn insert(&mut self, code: String) {
        if self.items.len() as u32 >= CACHE_LIMIT {
            self.items
                .remove(&self.items.keys().next().unwrap().to_string());
        }

        self.items
            .insert(code.clone(), NEXT_TTL.get().unwrap().clone());
    }

    pub fn bust(&mut self) {
        for (key, value) in self.items.clone() {
            if *NOW.get().unwrap() > value {
                self.items.remove(&key);
            }
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
