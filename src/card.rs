use rusqlite::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::fs::read_to_string;
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

use crate::cache::CardMetaData;
use crate::folders::get_category_from_id_from_fs;
use crate::{cache, Conn};
use crate::{common::current_time, Id};

/*
pub struct VerifiedCardPath(PathBuf);

impl VerifiedCardPath {
    pub fn new(path: PathBuf) -> Option<Self> {
        if path.exists() {
            return Some(VerifiedCardPath(path));
        }
        None
    }
}
*/

pub struct CardState {}

pub struct StatefulCard(Card, CardMetaData);

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Card {
    pub front: Side,
    pub back: Side,
    pub meta: Meta,
    #[serde(default)]
    pub history: Vec<Review>,
}

// public
impl Card {
    pub fn is_ready_for_review(&self) -> bool {
        self.meta.finished && !self.meta.suspended
    }

    pub fn delete_card(id: Id, conn: &Conn) {
        let path = get_category_from_id_from_fs(id)
            .unwrap()
            .as_path_with_id(id);
        std::fs::remove_file(path).unwrap();
        cache::delete_the_card_cache(conn, id);
    }

    pub fn load_from_id(id: Id, conn: &Conn) -> Option<Self> {
        let category = Self::get_category_from_id(id, conn)?;
        let card = Self::parse_toml_to_card(&category.as_path_with_id(id)).ok()?;
        //cache::cache_card_from_id(conn, id);
        Some(card)
    }

    pub fn get_card_path_from_id(conn: &Conn, id: Id) -> PathBuf {
        let category = Self::get_category_from_id(id, conn).unwrap();
        category.as_path_with_id(id)
    }

    pub fn get_card_question(id: Id, conn: &Conn) -> String {
        Self::load_from_id(id, conn).unwrap().front.text
    }

    pub fn get_category_from_id(id: Id, conn: &Conn) -> Option<Category> {
        if let Some(path) = cache::get_cached_path_from_db(id, conn) {
            if path.as_path_with_id(id).exists() {
                return Some(path);
            }
        }
        get_category_from_id_from_fs(id)
    }

    // Will either update the card if it exists, or create a new one

    /*

    */
    pub fn save_card(self, incoming_category: Option<Category>, conn: &Conn) {
        let incoming_category = incoming_category
            .or_else(|| get_category_from_id_from_fs(self.meta.id))
            .unwrap_or(Category(vec![]));

        let id = self.meta.id;

        let x = cache::get_cached_path_from_db(id, conn);

        // this implies it's an update operation, since we found the path :D
        if let Some(category) = x {
            if category != incoming_category {
                Self::move_card(id, &incoming_category, conn).unwrap();
            }
            cache::cache_card(conn, &self, &category);
        }
        // this implies it's a create operation, since we didn't find the path here.
        else {
            self.save_card_to_toml(&incoming_category).unwrap();
        }
    }

    pub fn save_card_to_toml(self, category: &Category) -> Result<PathBuf, toml::ser::Error> {
        let toml = toml::to_string(&self).unwrap();
        let path = category
            .as_path()
            .join(self.front.text)
            .with_extension("toml");

        std::fs::write(&path, toml).expect("Unable to write file");
        Ok(path)
    }

    /// Moves the card and updates the cache.
    pub fn move_card(id: Id, category: &Category, conn: &Conn) -> io::Result<()> {
        let old_path = get_category_from_id_from_fs(id)
            .unwrap()
            .as_path_with_id(id);
        let new_path = category.as_path_with_id(id);

        fs::rename(old_path, new_path)?;
        let card = Self::load_from_id(id, conn).unwrap();
        cache::cache_card(conn, &card, category);
        Ok(())
    }

    // The closure takes a Card and returns a Result.
    // This allows it to handle errors that might occur during processing.
    pub fn process_cards<F>(dir: &Path, func: &mut F) -> io::Result<()>
    where
        F: FnMut(Card, &Category) -> io::Result<()>,
    {
        if dir.is_dir() {
            let entries = fs::read_dir(dir)?;
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    Self::process_cards(&path, func)?;
                } else if path.extension() == Some(OsStr::new("toml")) {
                    let card = Self::parse_toml_to_card(&path).unwrap(); // Assuming parse_toml_to_card returns Result<Card, io::Error>
                    let category = Category::from_card_path(&path);
                    func(card, &category)?;
                }
            }
        }
        Ok(())
    }

    pub fn edit_card(id: Id, conn: &Conn) {
        let card = Self::load_from_id(id, conn).unwrap();
        let path = get_category_from_id_from_fs(id)
            .unwrap()
            .as_path_with_id(id);
        Command::new("nvim").arg(&path).status().unwrap();
        cache::cache_card(conn, &card, &Category::from_card_path(&path));
    }

    pub fn create_new(front: &str, back: &str, category: &Category) -> Result<Id> {
        let card = Card {
            front: Side {
                text: front.into(),
                ..Default::default()
            },
            back: Side {
                text: back.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let id = card.meta.id;

        card.save_card_to_toml(category).unwrap();
        Ok(id)
    }

    /*




    */

    pub fn calculate_strength(&self) -> f64 {
        let current_time = current_time();
        Self::calculate_strength_from_reviews(&self.history, current_time)
    }

    pub fn new_review(mut self, grade: Grade, category: &Category) {
        let review = Review::new(grade);
        self.history.push(review);
        self.save_card_to_toml(category).unwrap();
    }
}

// private
impl Card {
    pub fn parse_toml_to_card(file_path: &Path) -> Result<Card, toml::de::Error> {
        let content = read_to_string(file_path).expect("Could not read the TOML file");
        let mut card: Card = toml::from_str(&content)?;
        card.history.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(card)
    }

    /// Search through the folders for the card, if it finds it, update the cache.
    fn find_and_index(id: Id, conn: &Conn) -> Option<Self> {
        if let Some(path) = get_category_from_id_from_fs(id) {
            let card = Self::parse_toml_to_card(path.as_path().as_path()).ok()?;
            cache::cache_card_from_id(conn, id);
            return Some(card);
        }
        None
    }

    fn calculate_strength_from_reviews(reviews: &[Review], current_time: Duration) -> f64 {
        if reviews.is_empty() {
            return 0.;
        }

        let mut interval = 1.0;
        let mut ease = 2.5;

        for review in reviews {
            let grade = match review.grade {
                Grade::None => 0,
                Grade::Late => 1,
                Grade::Some => 3,
                Grade::Perfect => 5,
            };
            if grade >= 3 {
                interval *= ease;
            }

            ease = (ease - 0.8 + 0.28 * grade as f64 - 0.02 * (grade as f64).powf(2.)).max(1.3);
        }

        let lapse_duration = current_time - reviews.last().unwrap().timestamp;
        let lapse_days = lapse_duration.as_secs() as f64 / 86400.0;
        2.0f64.powf(-lapse_days / interval)
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    // no recall, not even when you saw the answer.
    #[default]
    None,
    // no recall, but you remember the answer when you read it.
    Late,
    // struggled but you got the answer right or somewhat right/
    Some,
    // no hesitation, perfect recall.
    Perfect,
}

impl std::str::FromStr for Grade {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" => Ok(Self::None),
            "2" => Ok(Self::Late),
            "3" => Ok(Self::Some),
            "4" => Ok(Self::Perfect),
            _ => Err(()),
        }
    }
}

use crate::common::{serde_duration_as_secs, Category};

#[derive(Deserialize, Clone, Serialize, Debug, Default)]
pub struct Review {
    // When (unix time) did the review take place?
    #[serde(with = "serde_duration_as_secs")]
    pub timestamp: Duration,
    // Recall grade.
    pub grade: Grade,
    // How long you spent before attempting recall.
    #[serde(with = "serde_duration_as_secs")]
    pub time_spent: Duration,
}

impl Review {
    fn new(grade: Grade) -> Self {
        Self {
            timestamp: current_time(),
            grade,
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Clone, Serialize, Debug, Default)]
pub struct Side {
    pub text: String,
    pub audio: Option<String>,
    pub image: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Meta {
    pub id: Id,
    pub dependencies: Vec<Id>,
    pub dependents: Vec<Id>,
    pub suspended: bool,
    pub finished: bool,
    pub tags: Vec<String>,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            dependencies: vec![],
            dependents: vec![],
            suspended: false,
            finished: true,
            tags: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use super::*;

    #[test]
    fn test_load_cards_from_folder() {
        let _category = Category(vec!["maths".into(), "calculus".into()]);

        //let cards = Card::load_cards_from_folder(&category);
        //insta::assert_debug_snapshot!(cards);
    }

    #[test]
    fn test_card_roundtrip() {
        let mut card = Card::default();
        card.meta.id = uuid!("000a0a00-c943-4c4b-b7bf-f7d483208eb0");
        let category = Category(vec![]);
        let path = card.save_card_to_toml(&category).unwrap();
        let card = Card::parse_toml_to_card(path.as_path());
        insta::assert_debug_snapshot!(card);
    }

    #[test]
    fn test_calc_strength() {
        let day = 86400;
        let _year = day * 365;
        let current_time = Duration::from_secs(day * 2);

        let reviews = vec![Review {
            grade: Grade::Some,
            time_spent: Duration::default(),
            timestamp: Duration::from_secs(day),
        }];

        let _strength = Card::calculate_strength_from_reviews(&reviews, current_time);

        let reviews = vec![Review {
            grade: Grade::None,
            time_spent: Duration::default(),
            timestamp: Duration::from_secs(day),
        }];

        let _strength = Card::calculate_strength_from_reviews(&reviews, current_time);
    }
}
