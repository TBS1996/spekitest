#![allow(dead_code)]

use config::Config;

use frontend::run;
use git::git_stuff;

use std::path::PathBuf;

use uuid::Uuid;

mod card;
mod cli;
mod common;
mod config;
mod folders;
mod frontend;
mod git;
mod media;

pub fn get_cards_path() -> PathBuf {
    get_share_path().join("cards")
}

pub fn get_media_path() -> PathBuf {
    get_share_path().join("media/")
}

#[cfg(not(test))]
pub fn get_share_path() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join(".local/share/speki/")
}

#[cfg(test)]
pub fn get_share_path() -> PathBuf {
    PathBuf::from("./test_dir/")
}

type Id = Uuid;

fn main() {
    std::fs::create_dir_all(get_cards_path()).unwrap();
    std::fs::create_dir_all(get_share_path().join("media/")).unwrap();

    let config = Config::load().unwrap();
    git_stuff(config.read_git_remote());

    run(config);
}
