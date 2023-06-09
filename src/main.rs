#![allow(dead_code)]

use config::Config;

use frontend::run;
use git::git_stuff;

use std::path::PathBuf;

use uuid::Uuid;

mod card;
mod common;
mod config;
mod folders;
mod frontend;
mod git;

pub fn get_cards_path() -> PathBuf {
    get_share_path().join("cards")
}

#[cfg(not(test))]
pub fn get_share_path() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join(".local/share/speki/")
}

#[cfg(test)]
pub fn get_share_path() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join("./")
}

type Id = Uuid;

fn main() {
    std::fs::create_dir_all(get_cards_path()).unwrap();
    let config = Config::load().unwrap();
    git_stuff(config.read_git_remote());
    run(config);
}
