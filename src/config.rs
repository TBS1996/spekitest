use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::GET_SHARE_PATH;

#[derive(Debug, Serialize, Deserialize)]

pub struct Config {
    play_audio: bool,
    show_images: bool,
    git_remote: Option<String>,
}

impl Config {
    fn config_path() -> PathBuf {
        GET_SHARE_PATH().join("config.toml")
    }

    pub fn read_git_remote(&self) -> &Option<String> {
        &self.git_remote
    }
    pub fn read_play_audio(&self) -> &Option<String> {
        &self.git_remote
    }
    pub fn read_show_images(&self) -> &Option<String> {
        &self.git_remote
    }

    pub fn play_audio(&mut self, val: bool) {
        self.play_audio = val;
        self.save();
    }

    pub fn show_image(&mut self, val: bool) {
        self.show_images = val;
        self.save();
    }

    // Save the config to a file
    pub fn save(&self) -> std::io::Result<()> {
        let toml = toml::to_string(&self).expect("Failed to serialize config");
        let mut file = File::create(Self::config_path())?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }

    // Load the config from a file
    pub fn load() -> std::io::Result<Config> {
        let mut file = match File::open(Self::config_path()) {
            Ok(file) => file,
            Err(_) => {
                Self::default().save()?;
                File::open(Self::config_path())?
            }
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = toml::from_str(&contents).expect("Failed to deserialize config");
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            play_audio: true,
            show_images: true,
            git_remote: Default::default(),
        }
    }
}
