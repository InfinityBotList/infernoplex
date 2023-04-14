use once_cell::sync::Lazy;
use poise::serenity_prelude::GuildId;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, num::NonZeroU64};

use crate::Error;

/// Global config object
pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::load().expect("Failed to load config"));

#[derive(Serialize, Deserialize)]
pub struct Servers {
    pub main: GuildId,
    pub staff: GuildId,
}

impl Default for Servers {
    fn default() -> Self {
        Self {
            main: GuildId(NonZeroU64::new(758641373074423808).unwrap()),
            staff: GuildId(NonZeroU64::new(870950609291972618).unwrap()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub client_secret: String,
    pub token: String,
    pub servers: Servers,
    pub frontend_url: String,
    pub proxy_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: String::from(""),
            token: String::from(""),
            client_secret: String::from(""),
            servers: Servers::default(),
            frontend_url: String::from("https://infinitybots.gg"),
            proxy_url: String::from("http://127.0.0.1:3219"),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        // Delete config.yaml.sample if it exists
        if std::path::Path::new("config.yaml.sample").exists() {
            std::fs::remove_file("config.yaml.sample")?;
        }

        // Create config.yaml.sample
        let mut sample = File::create("config.yaml.sample")?;

        // Write default config to config.yaml.sample
        sample.write_all(serde_yaml::to_string(&Config::default())?.as_bytes())?;

        // Open config.yaml
        let file = File::open("config.yaml");

        match file {
            Ok(file) => {
                // Parse config.yaml
                let cfg: Config = serde_yaml::from_reader(file)?;

                // Return config
                Ok(cfg)
            }
            Err(e) => {
                // Print error
                println!("config.yaml could not be loaded: {}", e);

                // Exit
                std::process::exit(1);
            }
        }
    }
}
