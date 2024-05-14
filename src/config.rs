use once_cell::sync::Lazy;
use poise::serenity_prelude::GuildId;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write};

use crate::Error;

pub static CURRENT_ENV: Lazy<&str> = Lazy::new(|| {
    let current_env = include_bytes!("../current-env");

    std::str::from_utf8(current_env).unwrap()
});

/// Global config object
pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::load().expect("Failed to load config"));

#[derive(Serialize, Deserialize, Default)]
pub struct Differs<T: Default + Clone> {
    staging: T,
    prod: T,
}

impl<T: Default + Clone> Differs<T> {
    /// Get the value for a given environment
    pub fn get_for_env(&self, env: &str) -> T {
        if env == "staging" {
            self.staging.clone()
        } else {
            self.prod.clone()
        }
    }

    /// Get the value for the current environment
    pub fn get(&self) -> T {
        self.get_for_env(*CURRENT_ENV)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Servers {
    pub main: GuildId,
    pub staff: GuildId,
}

impl Default for Servers {
    fn default() -> Self {
        Self {
            main: GuildId::new(758641373074423808),
            staff: GuildId::new(870950609291972618),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub client_secret: String,
    pub token: Differs<String>,
    pub prefix: Differs<String>,
    pub servers: Servers,
    pub frontend_url: String,
    pub proxy_url: String,
    pub cdn_main_scope_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: String::from(""),
            token: Differs {
                staging: String::from(""),
                prod: String::from(""),
            },
            prefix: Differs {
                staging: String::from("sls!"),
                prod: String::from("sl!"),
            },
            client_secret: String::from(""),
            servers: Servers::default(),
            frontend_url: String::from("https://infinitybots.gg"),
            proxy_url: String::from("http://127.0.0.1:3219"),
            cdn_main_scope_path: String::from("/silverpelt/cdn/ibl"),
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
