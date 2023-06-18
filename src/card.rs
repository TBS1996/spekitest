use serde::{Deserialize, Serialize};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use std::time::Duration;
use uuid::Uuid;

use crate::categories::Category;
use crate::folders::get_all_cards_full;
use crate::media::AudioSource;
use crate::{common::current_time, Id};

pub struct CardPath(PathBuf);

impl CardPath {
    fn from_path(path: &Path) -> Self {
        let _ = AnnoCard::from_path(path);
        Self(PathBuf::from(path))
    }
}

#[derive(Clone)]
pub struct CardFileData {
    pub file_name: String,
    pub category: Category,
    pub last_modified: Duration,
}

impl CardFileData {
    pub fn from_path(path: &Path) -> Self {
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        let category = Category::from_card_path(path);
        let last_modified = {
            let system_time = std::fs::metadata(path).unwrap().modified().unwrap();
            system_time_as_unix_time(system_time)
        };

        Self {
            file_name,
            category,
            last_modified,
        }
    }

    pub fn as_path(&self) -> PathBuf {
        let mut path = self.category.as_path().join(&self.file_name);
        path.set_extension("toml");
        path
    }
}

#[derive(Clone)]
pub struct AnnoCard(pub Card, pub CardFileData);

impl From<AnnoCard> for Card {
    fn from(value: AnnoCard) -> Self {
        value.0
    }
}

impl AnnoCard {
    pub fn get_full_card_from_id(id: Id) -> Option<Self> {
        let cards = get_all_cards_full();

        cards.into_iter().find(|card| card.0.meta.id == id)
    }

    pub fn edit_with_vim(&self) -> Self {
        let path = self.full_path();
        open_file_with_vim(path.as_path()).unwrap();
        Self::from_path(path.as_path())
    }

    pub fn from_path(path: &Path) -> Self {
        let content = read_to_string(path).expect("Could not read the TOML file");
        let card: Card = toml::from_str(&content).unwrap();
        let file_data = CardFileData::from_path(path);
        Self(card, file_data)
    }

    pub fn into_card(self) -> Card {
        self.0
    }

    pub fn card_as_ref(&self) -> &Card {
        &self.0
    }

    pub fn card_as_mut_ref(&mut self) -> &mut Card {
        &mut self.0
    }

    pub fn full_path(&self) -> PathBuf {
        let mut path = self.1.category.as_path().join(self.1.file_name.clone());
        path.set_extension("toml");
        path
    }

    pub fn update_card(&self) -> Self {
        let path = self.1.as_path();
        if !path.exists() {
            let msg = format!("following path doesn't really exist: {}", path.display());
            panic!("{msg}");
        }

        let toml = toml::to_string(self.card_as_ref()).unwrap();

        std::fs::write(&path, toml).unwrap();

        AnnoCard::from_path(path.as_path())
    }

    pub fn refresh_card(&mut self) {
        *self = Self::from_path(&self.1.as_path())
    }

    pub fn new_review(&mut self, grade: Grade) -> Self {
        let review = Review::new(grade.clone());
        let time_passed = self.0.time_passed_since_last_review();
        self.0.history.push(review);
        self.0.meta.stability = Some(Card::new_stability(grade, time_passed));
        self.update_card()
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
        let days_passed = self.time_since_last_review()?;
        let stability = self.meta.stability?;
        Some(Self::calculate_strength(days_passed, stability))
    }

    fn calculate_strength(days_passed: Duration, stability: Duration) -> f32 {
        let base: f32 = 0.9;
        let ratio = days_passed.as_secs_f32() / stability.as_secs_f32();
        (base.ln() * ratio).exp()
    }

    pub fn time_since_last_review(&self) -> Option<Duration> {
        let last_unix = self.history.last()?.timestamp;
        let current_unix = current_time();
        current_unix.checked_sub(last_unix)
    }

    pub fn pending_filter(&self) -> bool {
        self.meta.stability.is_none() && !self.meta.suspended && self.meta.finished
    }

    pub fn unfinished_filter(&self) -> bool {
        !self.meta.finished && !self.meta.suspended
    }

    pub fn review_filter(&self) -> bool {
        match (self.meta.stability, self.time_since_last_review()) {
            (Some(stability), Some(last_review_time)) => {
                self.meta.finished
                    && !self.meta.suspended
                    && last_review_time > Duration::from_secs(60) // Lets not review if its less than a minute since last time
                    && stability < last_review_time
            }
            (_, _) => false,
        }
    }

    pub fn save_new_card(self, category: &Category) -> AnnoCard {
        let toml = toml::to_string(&self).unwrap();
        std::fs::create_dir_all(category.as_path()).unwrap();
        let path = category
            .as_path()
            .join(self.front.text)
            .with_extension("toml");

        std::fs::write(&path, toml).unwrap();

        AnnoCard::from_path(path.as_path())
    }

    fn new_stability(grade: Grade, time_passed: Option<Duration>) -> Duration {
        let grade_factor = grade.get_factor();
        let time_passed = time_passed.unwrap_or(Duration::from_secs(86400));
        time_passed.mul_f32(grade_factor)
    }

    fn time_passed_since_last_review(&self) -> Option<Duration> {
        Some(current_time() - self.history.last()?.timestamp)
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    // No recall, not even when you saw the answer.
    #[default]
    None,
    // No recall, but you remember the answer when you read it.
    Late,
    // Struggled but you got the answer right or somewhat right.
    Some,
    // No hesitation, perfect recall.
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

use crate::common::{open_file_with_vim, serde_duration_as_secs, system_time_as_unix_time};

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
    #[serde(
        default,
        serialize_with = "optional_duration_to_days",
        deserialize_with = "optional_days_to_duration"
    )]
    pub stability: Option<Duration>,
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

fn optional_duration_to_days<S>(
    duration: &Option<Duration>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match duration {
        Some(d) => serializer.serialize_some(&(d.as_secs_f32() / (24.0 * 60.0 * 60.0))),
        None => serializer.serialize_none(),
    }
}

fn optional_days_to_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<f32> = Option::deserialize(deserializer)?;
    match opt {
        Some(f) => Ok(Some(Duration::from_secs_f32(f * 24.0 * 60.0 * 60.0))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_cards_from_folder() {
        let _category = Category(vec!["maths".into(), "calculus".into()]);

        //let cards = Card::load_cards_from_folder(&category);
        //insta::assert_debug_snapshot!(cards);
    }

    #[test]
    fn test_strength() {
        let stability = Duration::from_secs(86400);
        let days_passed = Duration::default();
        let recall_rate = Card::calculate_strength(days_passed, stability);
        assert_eq!(recall_rate, 1.0);

        let days_passed = Duration::from_secs(86400);
        let recall_rate = Card::calculate_strength(days_passed, stability);
        assert_eq!(recall_rate, 0.9);
    }
}
