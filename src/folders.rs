use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use uuid::Uuid;

use crate::card::{Card, CardFileData, CardWithFileData};
use crate::categories::Category;
use crate::paths::get_share_path;
use crate::Id;

pub fn open_share_path_in_explorer() -> std::io::Result<()> {
    let path = get_share_path();
    open_folder_in_explorer(path.as_path())
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
            let card = Card::parse_toml_to_card(path.as_path()).unwrap();
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
            let card = Card::parse_toml_to_card(path.as_path()).unwrap();
            if card.meta.id == id {
                return Some(path);
            }
        }
    }
    None
}

pub fn get_cards_from_category(category: &Category) -> Vec<CardWithFileData> {
    let directory = category.as_path();
    let mut cards = Vec::new();

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let card = Card::parse_toml_to_card(path.as_path()).unwrap();
            let full_card = CardWithFileData(
                card,
                CardFileData {
                    file_name: path.file_name().unwrap().to_string_lossy().to_string(),
                    category: category.to_owned(),
                    last_modified: get_last_modified(path),
                },
            );
            cards.push(full_card);
        }
    }
    cards
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
/*
pub fn get_size_from_id(id: Id) -> u64 {
    let cat = Card::get_category_from_id(id, conn).unwrap();
    get_size_from_path(&cat.as_path_with_id(id))
}
*/

pub fn _get_all_unfinished_cards() -> Vec<Card> {
    get_all_cards()
        .into_iter()
        .filter(|card| card.meta.suspended)
        .collect()
}

pub fn get_all_cards_full() -> Vec<CardWithFileData> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        let cards_from_category = get_cards_from_category(cat);
        cards.extend(cards_from_category);
    }
    cards
}

pub fn get_all_cards() -> Vec<Card> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        let some_cards = get_cards_from_category(cat);
        let some_cards = CardWithFileData::into_cards(some_cards);
        cards.extend(some_cards);
    }
    cards
}

pub fn _get_all_cards_ids() -> Vec<Id> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        cards.extend(_get_card_ids_from_category(cat));
    }
    cards
}

pub fn get_pending_cards_from_category(category: &Category) -> Vec<Card> {
    let cards = get_cards_from_category(category);
    let cards = CardWithFileData::into_cards(cards);
    cards
        .into_iter()
        .filter(|card| card.meta.stability.is_none() && !card.meta.suspended)
        .collect()
}

pub fn get_review_cards_from_category(category: &Category) -> Vec<Card> {
    let cards = get_cards_from_category(category);
    let cards = CardWithFileData::into_cards(cards);
    cards
        .into_iter()
        .filter(|card| card.is_ready_for_review())
        .collect()
}

pub fn get_category_from_id_from_fs(id: Id) -> Option<Category> {
    let cards = get_all_cards_full();
    for card in cards {
        if card.0.meta.id == id {
            return Some(card.1.category);
        }
    }
    None
}
