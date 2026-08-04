use once_cell::sync::Lazy;
use poise::serenity_prelude::GuildId;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write};

use crate::Error;

pub const CURRENT_ENV_PROD: &str = "prod";
pub const CURRENT_ENV_STAGING: &str = "staging";
pub const CURRENT_ENV_DEV: &str = "dev";

pub static CURRENT_ENV: Lazy<&str> = Lazy::new(|| {
    let current_env = include_bytes!("../current-env");

    let env = std::str::from_utf8(current_env).unwrap().trim();

    if env != CURRENT_ENV_PROD && env != CURRENT_ENV_STAGING && env != CURRENT_ENV_DEV {
        panic!("invalid environment in current-env: {}", env);
    }

    env
});

/// Global config object
pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::load().expect("Failed to load config"));

/// Common struct for values that differ between staging and production
/// environments, plus an optional per-developer override for local dev use.
#[derive(Serialize, Deserialize, Default)]
pub struct Differs<T: Default + Clone> {
    /// Only actually read when current-env is "staging" (or "dev" with no
    /// override, see `dev` below) — defaults to T::default() when absent so
    /// a prod-only or staging-only config.yaml doesn't have to carry a value
    /// for the environment it isn't running as.
    #[serde(default)]
    staging: T,
    /// Only actually read when current-env is "prod". See `staging` above.
    #[serde(default)]
    prod: T,

    /// Only consulted when running with current-env set to "dev", and even
    /// then only if it has been set — an unset dev falls back to staging, so
    /// config.yaml files that predate this field keep working unchanged.
    #[serde(default)]
    dev: Option<T>,
}

impl<T: Default + Clone> Differs<T> {
    /// Get the value for a given environment
    pub fn get_for_env(&self, env: &str) -> T {
        if env == CURRENT_ENV_DEV {
            return self.dev.clone().unwrap_or_else(|| self.staging.clone());
        }

        if env == CURRENT_ENV_STAGING {
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
    pub frontend_url: Differs<String>,
    pub proxy_url: String,
    pub cdn_main_scope_path: String,
    pub server_port: Differs<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_port: Differs {
                staging: 61000,
                prod: 61001,
                dev: None,
            },
            database_url: String::from(""),
            token: Differs {
                staging: String::from(""),
                prod: String::from(""),
                dev: None,
            },
            prefix: Differs {
                staging: String::from("sls!"),
                prod: String::from("sl!"),
                dev: None,
            },
            client_secret: String::from(""),
            servers: Servers::default(),
            frontend_url: Differs {
                staging: String::from("https://reedwhisker.infinitybots.gg"),
                prod: String::from("https://infinitybots.gg"),
                dev: None,
            },
            proxy_url: String::from("https://gateway.nodebyte.host/proxy/discord"),
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
