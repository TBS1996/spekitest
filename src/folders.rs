use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use uuid::Uuid;

use crate::card::{Card, SavedCard};
use crate::categories::Category;
use crate::paths::get_cards_path;
use crate::Id;

pub fn view_cards_in_explorer() {
    open_folder_in_explorer(&get_cards_path()).unwrap()
}

fn open_folder_in_explorer(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).status()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).status()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(path).status()?;
    }

    Ok(())
}

pub fn get_last_modified(path: PathBuf) -> Duration {
    let metadata = std::fs::metadata(path).unwrap();
    let modified_time = metadata.modified().unwrap();
    let secs = modified_time
        .duration_since(UNIX_EPOCH)
        .map(|s| s.as_secs())
        .unwrap();
    Duration::from_secs(secs)
}

pub fn get_card_from_id(id: Id, category: &Category) -> Option<Card> {
    let directory = category.as_path();

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let card = SavedCard::from_path(path.as_path()).into_card();
            if card.meta.id == id {
                return Some(card);
            }
        }
    }
    None
}

pub fn get_path_from_id(id: Id, category: &Category) -> Option<PathBuf> {
    let directory = category.as_path();

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let card = SavedCard::from_path(path.as_path()).into_card();
            if card.meta.id == id {
                return Some(path);
            }
        }
    }
    None
}

pub fn _get_card_ids_from_category(category: &Category) -> Vec<Id> {
    let directory = category.as_path();
    let mut toml_files = Vec::new();

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                let id = Uuid::parse_str(filename).unwrap();
                toml_files.push(id);
            }
        }
    }
    toml_files
}

pub fn get_all_cards_full() -> Vec<SavedCard> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        let cards_from_category = cat.get_containing_cards();
        cards.extend(cards_from_category);
    }
    cards
}
