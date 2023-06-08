use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use crate::cache::get_cached_path_from_db;
use crate::card::Card;
use crate::common::Category;
use crate::frontend;
use crate::{Conn, Id};

pub fn get_last_modified_map_from_category(
    conn: &Conn,
    category: &Category,
) -> HashMap<String, Duration> {
    todo!()
    /*
    let mut stmt = conn.prepare("SELECT id, last_modified FROM cards").unwrap();
    let rows: Option<Vec<(String, u64)>> = stmt
        .query_map(rusqlite::NO_PARAMS, |row| {
            Ok((row.get(0).unwrap(), row.get(1).unwrap()))
        })
        .unwrap();
    let rows = rows.unwrap();

    let mut map = HashMap::new();

    for row in rows {
        map.insert(row.0, Duration::from_secs(row.1));
    }
    map
    */
}

pub fn get_cards_from_category(category: &Category) -> Vec<Card> {
    let directory = category.as_path();
    let mut cards = Vec::new();

    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let card = Card::parse_toml_to_card(path.as_path()).unwrap();
            cards.push(card);
        }
    }
    cards
}

pub fn get_card_ids_from_category(category: &Category) -> Vec<Id> {
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

pub fn get_size_from_id(id: Id, conn: &Conn) -> u64 {
    let cat = Card::get_category_from_id(id, conn).unwrap();
    get_size_from_path(&cat.as_path_with_id(id))
}

pub fn get_size_from_path(path: &PathBuf) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

pub fn get_all_cards() -> Vec<Card> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        cards.extend(get_cards_from_category(cat));
    }
    cards
}

pub fn get_all_cards_ids() -> Vec<Id> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        cards.extend(get_card_ids_from_category(cat));
    }
    cards
}

pub fn review_card_in_directory(conn: &Conn, category: &Category) {
    let cards = get_all_cards();
    frontend::review_cards(conn, cards, category);
}

pub fn get_category_from_id_from_fs(id: Id) -> Option<Category> {
    let folders = Category::load_all().unwrap();

    for folder in folders {
        let full_path = folder.as_path_with_id(id);

        if full_path.exists() {
            return Some(folder);
        }
    }
    None
}

pub fn create_category(category: &Category) -> Category {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let input = normalize_category_name(&input);
    let category = category.clone().append(&input);
    let path = category.as_path();
    std::fs::create_dir(path).unwrap();
    category
}

pub fn normalize_category_name(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());

    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == ' ' {
            normalized.push(c);
        }
    }

    normalized
}
