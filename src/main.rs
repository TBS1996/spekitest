use folders::*;
use frontend::main_loop;
use rusqlite::Result;

use std::io::{self};
use std::path::PathBuf;

use uuid::Uuid;

use crate::common::Category;

mod cache;
mod card;
mod common;
mod folders;
mod frontend;

type Conn = rusqlite::Connection;

const GIT_REMOTE: &str = "git@github.com:TBS1996/spekiremote.git";

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

/*

mvp:

no categories

just, you run the program, and choose between adding cards or reviewing cards

if you click review cards it'll first calculate all the strengths for each card, and return a list of cards with strength below 0.9,
then you'll start reviewing those :D



 */

use std::process::Command;

fn git_save() {
    Command::new("git").args(["add", "."]).output().unwrap();
    Command::new("git")
        .args(["commit", "-m", "save"])
        .output()
        .unwrap();
    Command::new("git")
        .args(["push", "-u", "origin", "main"])
        .output()
        .unwrap();
}

fn git_pull() {
    Command::new("git").args(["pull"]).output().unwrap();
}

pub fn git_stuff() {
    std::env::set_current_dir(GET_SHARE_PATH()).unwrap();

    // Initiate git
    Command::new("git").arg("init").output().unwrap();

    // Check if the remote repository is already set
    let remote_check_output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .unwrap();

    if remote_check_output.status.success() {
        git_pull();
    } else {
        // Set the remote repository
        Command::new("git")
            .args(["remote", "add", "origin", GIT_REMOTE])
            .output()
            .unwrap();
    }
    git_save();
}

fn main() -> Result<()> {
    let conn = cache::init()?;
    std::fs::create_dir_all(get_cards_path()).unwrap();

    git_stuff();
    main_loop(&conn);
    Ok(())
}

/*
TODO list! :D








 */
