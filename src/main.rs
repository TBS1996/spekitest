use config::Config;
use folders::*;
use frontend::run;
use git::git_stuff;

use std::error::Error;
use std::io::{self};
use std::path::PathBuf;

use uuid::Uuid;

use crate::common::Category;

//mod cache;
mod card;
mod common;
mod config;
mod folders;
mod frontend;
mod git;

pub fn get_cards_path() -> PathBuf {
    GET_SHARE_PATH().join("cards")
}

#[cfg(not(test))]
pub fn GET_SHARE_PATH() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join(".local/share/speki/")
}

#[cfg(test)]
pub fn GET_SHARE_PATH() -> PathBuf {
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
