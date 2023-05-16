use uuid::Uuid;

use crate::common::Category;
use crate::frontend;
use crate::{Conn, Id};

pub fn get_cards_from_category(category: &Category) -> Vec<Id> {
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

pub fn get_all_cards() -> Vec<Id> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        cards.extend(get_cards_from_category(cat));
    }
    cards
}

pub fn review_card_in_directory(conn: &Conn, category: &Category) {
    let cards = get_all_cards();
    frontend::review_cards(conn, cards, category);
}

pub fn search_for_id(id: Id) -> Option<Category> {
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
        if c.is_ascii_alphanumeric() {
            normalized.push(c);
        } else if c == ' ' {
            normalized.push(c);
        }
    }

    normalized
}
