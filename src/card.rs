use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::fmt::Display;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use std::time::Duration;
use uuid::Uuid;

use crate::categories::Category;
use crate::folders::get_all_cards_full;
use crate::media::AudioSource;
use crate::VisitStuff;
use crate::{common::current_time, Id};

pub type StrengthMap = HashMap<AnnoCard, Option<f32>>;
pub type RecallRate = f32;

pub struct CardAndRecall {
    card_path: CardPath,
    recall_rate: RecallRate,
}

impl CardAndRecall {
    fn from_card(card: &AnnoCard) -> Option<Self> {
        let card_path = CardPath::new(card);
        let recall_rate = card.0.recall_rate()?;
        Self {
            card_path,
            recall_rate,
        }
        .into()
    }
}

impl Display for CardAndRecall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}%",
            self.card_path
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            self.recall_rate
        )
    }
}

impl Eq for CardAndRecall {}

impl PartialEq for CardAndRecall {
    fn eq(&self, other: &Self) -> bool {
        self.recall_rate == other.recall_rate
    }
}

impl Ord for CardAndRecall {
    fn cmp(&self, other: &Self) -> Ordering {
        // This will order by recall rate in ascending order
        // Use `cmp` function to order in descending order
        self.recall_rate
            .partial_cmp(&other.recall_rate)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for CardAndRecall {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct CardPath(PathBuf);

impl CardPath {
    fn new(card: &AnnoCard) -> Self {
        Self(card.1.as_path())
    }

    fn as_ref(&self) -> &PathBuf {
        &self.0
    }
}

#[derive(Hash, Clone)]
pub struct CardFileData {
    pub file_name: OsString,
    pub category: Category,
    pub last_modified: Duration,
}

impl CardFileData {
    pub fn from_path(path: &Path) -> Self {
        let file_name = path.file_name().unwrap().to_owned();
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

#[derive(Clone, Hash)]
pub struct AnnoCard(pub Card, pub CardFileData);

impl std::fmt::Display for AnnoCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.card_as_ref().front.text)
    }
}

pub struct CardWithRecall(pub AnnoCard, Option<f32>);

impl From<AnnoCard> for Card {
    fn from(value: AnnoCard) -> Self {
        value.0
    }
}

impl VisitStuff for AnnoCard {
    fn get_children(&self) -> Vec<Self> {
        AnnoCard::from_ids(self.0.meta.dependencies.clone())
    }

    fn matches_predicate(&self) -> bool {
        !self.0.meta.finished
    }
}

/*
impl Drop for AnnoCard {
    fn drop(&mut self) {
        if self.1.as_path().exists() {
            self.update_card();
        }
    }
}
*/

pub enum ReviewType {
    Normal,
    Pending,
    Unfinished,
}

impl AnnoCard {
    pub fn print_by_strength() -> BTreeSet<CardAndRecall> {
        Self::load_all()
            .into_iter()
            .filter_map(|card| CardAndRecall::from_card(&card))
            .collect()
    }

    pub fn get_review_type(&self) -> ReviewType {
        match (self.0.meta.stability, self.0.meta.finished) {
            (Some(_), true) => ReviewType::Normal,
            (_, false) => ReviewType::Unfinished,
            (None, true) => ReviewType::Pending,
        }
    }

    pub fn delete(self) {
        let path = self.full_path();
        std::fs::remove_file(path).unwrap();
    }

    pub fn get_cards_from_category_recursively(category: &Category) -> Vec<Self> {
        let mut cards = vec![];
        let cats = category.get_following_categories();
        for cat in cats {
            cards.extend(cat.get_containing_cards());
        }
        cards
    }

    pub fn search(input: String) -> Vec<Self> {
        Self::load_all()
            .into_iter()
            .filter(|card| {
                card.0
                    .front
                    .text
                    .to_ascii_lowercase()
                    .contains(&input.to_ascii_lowercase())
                    || card
                        .0
                        .back
                        .text
                        .to_ascii_lowercase()
                        .contains(&input.to_ascii_lowercase())
            })
            .collect()
    }

    pub fn is_resolved(&self) -> bool {
        !self.visit()
    }

    pub fn from_ids(ids: Vec<Id>) -> Vec<Self> {
        let mut vec = vec![];
        for id in ids {
            if let Some(card) = Self::from_id(id) {
                vec.push(card);
            }
        }
        vec
    }

    pub fn from_id(id: Id) -> Option<Self> {
        Self::load_all()
            .into_iter()
            .find(|card| card.0.meta.id == id)
    }
    pub fn load_all() -> Vec<Self> {
        Self::get_cards_from_category_recursively(&Category::root())
    }

    pub fn get_id_map() -> HashMap<Id, Self> {
        let mut map = HashMap::new();
        let cards = Self::load_all();
        for card in cards {
            map.insert(card.0.meta.id, card);
        }
        map
    }

    pub fn sort_by_last_modified(vec: &mut [Self]) {
        vec.sort_by_key(|k| Reverse(k.1.last_modified));
    }

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

    // Gets called automatically when object goes out of scope.
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

    pub fn refresh(&mut self) {
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

#[derive(Hash, Deserialize, Serialize, Debug, Default, Clone)]
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

    pub fn is_resolved(&self) -> bool {
        if self.meta.dependencies.is_empty() {
            return true;
        }
        false
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

    pub fn recall_rate(&self) -> Option<RecallRate> {
        let days_passed = self.time_since_last_review()?;
        let stability = self.meta.stability?;
        Some(Self::calculate_strength(days_passed, stability))
    }

    fn calculate_strength(days_passed: Duration, stability: Duration) -> RecallRate {
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
        let max_char_len = 40;
        let mut file_name = PathBuf::from(truncate_string(self.front.text, max_char_len));
        if file_name.exists() {
            file_name = PathBuf::from(self.meta.id.to_string());
        }

        let path = category.as_path().join(file_name).with_extension("toml");

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

#[derive(Hash, Deserialize, Serialize, Debug, Default, Clone)]
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

use crate::common::{
    open_file_with_vim, serde_duration_as_secs, system_time_as_unix_time, truncate_string,
};

#[derive(Hash, Deserialize, Clone, Serialize, Debug, Default)]
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

#[derive(Hash, Deserialize, Clone, Serialize, Debug, Default)]
pub struct Side {
    pub text: String,
    #[serde(flatten)]
    pub audio: AudioSource,
    //#[serde(deserialize_with = "deserialize_image_path")]
    //pub image: ImagePath,
}

#[derive(Hash, Deserialize, Serialize, Debug, Clone)]
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
    pub tags: BTreeSet<String>,
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
            tags: BTreeSet::new(),
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
