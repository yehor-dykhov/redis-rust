use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::{OpenOptions};
use std::io::{Write};
use std::time::{Duration, SystemTime};
use thiserror::Error;

const FILE_NAME: &str = "storage.json";

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Save to the Storage unsuccessful")]
    SaveUnsuccessful(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageData {
    data: HashMap<String, CommandData>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommandData {
    pub key: String,
    pub value: String,
    pub created_at: SystemTime,
    pub expires_for: Option<Duration>,
}

fn write_store(json: String) -> std::io::Result<usize> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(FILE_NAME)
        .expect("open file in r/w mode or create not exists file");

    file.write(json.as_ref())
}

fn read_store() -> Option<StorageData> {
    if let Ok(_json) = fs::read_to_string(FILE_NAME) {
        let storage_data: Option<StorageData> = serde_json::from_str(_json.as_str()).unwrap();
        return storage_data;
    }

    None
}

pub fn add(key: &str, value: &str, expires_for: Option<Duration>) -> Result<bool, StorageError> {
    let mut storage_data = if let Some(_data) = read_store() {
        _data
    } else {
        StorageData {
            data: HashMap::new(),
        }
    };

    let command_data = CommandData {
        key: key.to_string(),
        value: value.to_string(),
        created_at: SystemTime::now(),
        expires_for,
    };

    storage_data
        .data
        .insert(command_data.key.clone(), command_data.clone());

    let json = serde_json::to_string(&storage_data).unwrap();

    match write_store(json) {
        Ok(_) => Ok(true),
        Err(e) => Err(StorageError::SaveUnsuccessful(e.to_string())),
    }
}

pub fn get(key: &str) -> Option<CommandData> {
    println!("KEY: {}", key);
    match read_store() {
        None => None,
        Some(storage_data) => {
            storage_data.data.get(key).map(|cd| cd.clone())
        }
    }
}
