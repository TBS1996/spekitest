use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::fs::read_to_string;
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

use crate::folders::get_category_from_id_from_fs;
use crate::media::AudioSource;
use crate::paths::{get_media_path, get_share_path};
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

pub struct CardFileData {
    file_name: String,
    category: Category,
    last_modified: u64,
}

pub struct CardWithFileData(pub Card, pub CardFileData);

impl CardWithFileData {
    pub fn into_card(self) -> Card {
        self.0
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

    pub fn is_ready_for_review(&self, strength: Option<f64>) -> bool {
        let x = self.meta.finished && !self.meta.suspended;
        if !x {
            return x;
        };

        match strength {
            Some(strength) => self.calculate_strength() < strength,
            None => true,
        }
    }

    pub fn load_from_id(id: Id) -> Option<Self> {
        let category = get_category_from_id_from_fs(id)?;
        let card = Self::parse_toml_to_card(&category.as_path_with_id(id)).ok()?;
        //cache::cache_card_from_id(conn, id);
        Some(card)
    }
    /*
        pub fn delete_card(id: Id, conn: &Conn) {
            let path: PathBuf = get_category_from_id_from_fs(id)
                .unwrap()
                .as_path_with_id(id);
            std::fs::remove_file(path).unwrap();
            // cache::delete_the_card_cache(conn, id);
        }


        pub fn get_card_path_from_id(conn: &Conn, id: Id) -> PathBuf {
            let category = Self::get_category_from_id(id, conn).unwrap();
            category.as_path_with_id(id)
        }

        pub fn get_card_question(id: Id, conn: &Conn) -> String {
            Self::load_from_id(id).unwrap().front.text
        }

        pub fn get_category_from_id(id: Id, conn: &Conn) -> Option<Category> {
            /*
            if let Some(path) = cache::get_cached_path_from_db(id, conn) {
                if path.as_path_with_id(id).exists() {
                    return Some(path);
                }
            }
            */
            get_category_from_id_from_fs(id)
        }
    */

    // Will either update the card if it exists, or create a new one

    /*

    */
    pub fn save_card(self, incoming_category: Option<Category>) {
        let incoming_category = incoming_category
            .or_else(|| get_category_from_id_from_fs(self.meta.id))
            .unwrap_or(Category(vec![]));

        let id = self.meta.id;

        let x = get_category_from_id_from_fs(id);

        // this implies it's an update operation, since we found the path :D
        if let Some(category) = x {
            if category != incoming_category {
                Self::move_card(id, &incoming_category).unwrap();
            }
        //      cache::cache_card(conn, &self, &category);
        }
        // this implies it's a create operation, since we didn't find the path here.
        else {
            self.save_card_to_toml(&incoming_category).unwrap();
        }
    }

    pub fn save_card_to_toml(self, category: &Category) -> Result<PathBuf, toml::ser::Error> {
        let toml = toml::to_string(&self).unwrap();
        std::fs::create_dir_all(category.as_path()).unwrap();
        let path = category
            .as_path()
            .join(self.front.text)
            .with_extension("toml");

        std::fs::write(&path, toml);
        Ok(path)
    }

    /// Moves the card and updates the cache.
    pub fn move_card(id: Id, category: &Category) -> io::Result<()> {
        let old_path = get_category_from_id_from_fs(id)
            .unwrap()
            .as_path_with_id(id);
        let new_path = category.as_path_with_id(id);

        fs::rename(old_path, new_path)?;
        let _card = Self::load_from_id(id).unwrap();
        //   cache::cache_card(conn, &card, category);
        Ok(())
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

    pub fn _edit_card(id: Id) {
        let _card = Self::load_from_id(id).unwrap();
        let path = get_category_from_id_from_fs(id)
            .unwrap()
            .as_path_with_id(id);
        Command::new("nvim").arg(&path).status().unwrap();
        //  cache::cache_card(conn, &card, &Category::from_card_path(&path));
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
    fn _find_and_index(id: Id) -> Option<Self> {
        if let Some(path) = get_category_from_id_from_fs(id) {
            let card = Self::parse_toml_to_card(path.as_path().as_path()).ok()?;
            //   cache::cache_card_from_id(conn, id);
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
/*
pub fn calc_stability(
    history: &Vec<Review>,
    new_review: &Review,
    prev_stability: Duration,
) -> Duration {
    let gradefactor = new_review.grade.get_factor();
    if history.is_empty() {
        return Duration::from_secs_f32(gradefactor * 86400.);
    }

    let mut newstory;

    let timevec = get_elapsed_time_reviews({
        newstory = history.clone();
        newstory.push(new_review.clone());
        &newstory
    });
    let time_passed = timevec.last().unwrap();

    if gradefactor < 1. {
        return std::cmp::min(time_passed, &prev_stability).mul_f32(gradefactor);
    }

    if time_passed > &prev_stability {
        return time_passed.mul_f32(gradefactor);
    } else {
        let base = prev_stability;
        let max = base.mul_f32(gradefactor);
        let diff = max - base;

        let percentage = time_passed.div_f32(prev_stability.as_secs_f32());

        return (diff.mul_f32(percentage.as_secs_f32())) + base;
    }
}
*/

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
    pub fn _get_factor(&self) -> f32 {
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
