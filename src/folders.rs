use uuid::Uuid;

use crate::card::Card;
use crate::common::Category;
use crate::Id;

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
pub fn get_all_cards() -> Vec<Card> {
    let cats = Category::load_all().unwrap();
    let mut cards = vec![];

    for cat in &cats {
        cards.extend(get_cards_from_category(cat));
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
    cards
        .into_iter()
        .filter(|card| card.meta.stability.is_none() && !card.meta.suspended)
        .collect()
}

pub fn get_review_cards_from_category(category: &Category) -> Vec<Card> {
    let cards = get_cards_from_category(category);
    cards
        .into_iter()
        .filter(|card| card.is_ready_for_review())
        .collect()
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
