use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::fs::read_to_string;
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

use crate::common::Category;
use crate::folders::{get_all_cards_full, get_category_from_id_from_fs};
use crate::media::AudioSource;
use crate::{common::current_time, Id};

pub struct CardFileData {
    pub file_name: String,
    pub category: Category,
    pub last_modified: Duration,
}

pub struct CardWithFileData(pub Card, pub CardFileData);

impl CardWithFileData {
    pub fn get_full_card_from_id(id: Id) -> Option<Self> {
        let cards = get_all_cards_full();

        cards.into_iter().find(|card| card.0.meta.id == id)
    }

    pub fn into_card(self) -> Card {
        self.0
    }

    pub fn into_cards(v: Vec<Self>) -> Vec<Card> {
        v.into_iter().map(|c| c.into_card()).collect()
    }

    pub fn full_path(&self) -> PathBuf {
        let mut path = self.1.category.as_path().join(self.1.file_name.clone());
        path.set_extension("toml");
        path
    }
}

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
    pub fn new(front: Side, back: Side, meta: Meta) -> Self {
        Card {
            front,
            back,
            meta,
            history: Vec::new(),
        }
    }

    pub fn new_simple(front: String, back: String) -> Self {
        Card {
            front: Side {
                text: front,
                ..Default::default()
            },
            back: Side {
                text: back,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn recall_rate(&self) -> Option<f32> {
        let days_passed = self.days_since_last_review()?;
        let stability = self.meta.stability?;
        Some(Self::calculate_strength(days_passed, stability))
    }

    fn calculate_strength(days_passed: f32, stability: f32) -> f32 {
        let base: f32 = 0.9;
        (base.ln() * days_passed / stability).exp()
    }

    pub fn days_since_last_review(&self) -> Option<f32> {
        let last_unix = self.history.last()?.timestamp;
        let current_unix = current_time();
        Some((current_unix - last_unix).as_secs_f32() / 86400.)
    }

    pub fn is_ready_for_review(&self) -> bool {
        match (self.meta.stability, self.days_since_last_review()) {
            (Some(stability), Some(last_review_time)) => {
                self.meta.finished
                    && !self.meta.suspended
                    && last_review_time > (1. / 1440.) // Lets not review if its less than a minute since last time
                    && stability < last_review_time
            }
            (_, _) => false,
        }
    }

    pub fn load_from_id(id: Id) -> Option<Self> {
        let card = CardWithFileData::get_full_card_from_id(id)?;
        Some(card.into_card())
    }

    pub fn save_card(self, incoming_category: Option<Category>) {
        let incoming_category = incoming_category
            .or_else(|| get_category_from_id_from_fs(self.meta.id))
            .unwrap_or(Category(vec![]));

        let id = self.meta.id;

        self.save_card_to_toml(&incoming_category).unwrap();
    }

    pub fn save_card_to_toml(&self, category: &Category) -> Result<PathBuf, toml::ser::Error> {
        let toml = toml::to_string(&self).unwrap();
        std::fs::create_dir_all(category.as_path()).unwrap();
        let path = category
            .as_path()
            .join(self.front.text.clone())
            .with_extension("toml");

        let _ = std::fs::write(&path, toml);
        Ok(path)
    }

    // The closure takes a Card and returns a Result.
    // This allows it to handle errors that might occur during processing.
    pub fn _process_cards<F>(dir: &Path, func: &mut F) -> io::Result<()>
    where
        F: FnMut(Card, &Category) -> io::Result<()>,
    {
        if dir.is_dir() {
            let entries = fs::read_dir(dir)?;
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    Self::_process_cards(&path, func)?;
                } else if path.extension() == Some(OsStr::new("toml")) {
                    let card = Self::parse_toml_to_card(&path).unwrap(); // Assuming parse_toml_to_card returns Result<Card, io::Error>
                    let category = Category::_from_card_path(&path);
                    func(card, &category)?;
                }
            }
        }
        Ok(())
    }

    pub fn _create_new(front: &str, back: &str, category: &Category) -> Id {
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
        id
    }

    fn new_stability(grade: Grade, time_passed: Option<f32>) -> f32 {
        grade.get_factor() * time_passed.unwrap_or(1.)
    }

    pub fn new_review(&mut self, grade: Grade, category: &Category) {
        let review = Review::new(grade.clone());
        self.history.push(review);
        self.meta.stability = Some(Self::new_stability(grade, self.meta.stability));
        self.save_card_to_toml(category).unwrap();
    }

    pub fn parse_toml_to_card(file_path: &Path) -> Result<Card, toml::de::Error> {
        let content = read_to_string(file_path).expect("Could not read the TOML file");
        let mut card: Card = toml::from_str(&content)?;
        card.history.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        Ok(card)
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

impl Grade {
    pub fn get_factor(&self) -> f32 {
        match self {
            Grade::None => 0.1,
            Grade::Late => 0.25,
            Grade::Some => 2.,
            Grade::Perfect => 3.,
        }
    }
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

use crate::common::serde_duration_as_secs;

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
    #[serde(flatten)]
    pub audio: AudioSource,
    //#[serde(deserialize_with = "deserialize_image_path")]
    //pub image: ImagePath,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Meta {
    pub id: Id,
    pub dependencies: Vec<Id>,
    pub dependents: Vec<Id>,
    pub suspended: bool,
    pub finished: bool,
    pub stability: Option<f32>,
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
            stability: None,
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
    fn test_strength() {
        let stability = 1.0;
        let days_passed = 0.0;
        let recall_rate = Card::calculate_strength(days_passed, stability);
        assert_eq!(recall_rate, 1.0);

        let days_passed = 1.;
        let recall_rate = Card::calculate_strength(days_passed, stability);
        assert_eq!(recall_rate, 0.9);
    }
}
