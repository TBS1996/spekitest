use serde::{Deserialize, Serialize, de};
use toml::Value;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
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



#[derive(Default)]
pub struct CardCache(pub HashMap<Id, AnnoCard>);

impl CardCache{
   pub fn new() -> Self {
       let mut cache = Self::default();
       cache.cache_all();
       cache
   } 

    fn cache_all(&mut self) {
        let all_cards = AnnoCard::load_all();
        for card in all_cards{
            self.cache_one(card);
        }
    }
    
    fn cache_one(&mut self, card: AnnoCard) {
        self.0.insert(card.card.meta.id, card);
    }
    
    pub fn find_updated_card(&mut self, id: &Id) -> Option<(&AnnoCard, bool)> {
        if let Some(card) = self.0.get(id) {
            return Some((card, card.is_outdated()));
        }
        None
    }
    
    pub fn get_or_fetch(&mut self, id: &Id) -> Option<&AnnoCard>{
        if let Some((card, is_outdated)) = self.find_updated_card(id) {
            if !is_outdated{
                return Some(card);
            } else {
                match AnnoCard::from_id(id) {
                    Some(_card) =>  {
                        
                    }
                    None => return None,
                }
            }

        }
        
        None
        
    }
}


#[derive(Default)]
pub struct CardLocationCache(pub HashMap<Id, CardLocation>);

impl CardLocationCache {
   pub fn new() -> Self {
       let mut cache = Self::default();
       cache.cache_all();
       cache
   } 

    fn cache_all(&mut self) {
        let all_cards = AnnoCard::load_all();
        for card in all_cards{
            self.cache_one(&card);
        }
    }
    
    fn cache_one(&mut self, card: &AnnoCard) {
        self.0.insert(card.card.meta.id, card.location.clone());
    }
}

#[derive(Hash, Clone, Debug)]
pub struct CardLocation {
    pub file_name: OsString,
    pub category: Category,
}

impl CardLocation {
    pub fn new(path: &Path) -> Self {
        let file_name = path.file_name().unwrap().to_owned();
        let category = Category::from_card_path(path);
        Self {file_name, category}
    }

    fn as_path(&self) -> PathBuf {
        let mut path = self.category.as_path().join(self.file_name.clone());
        path.set_extension("toml");
        path
    }
}

#[derive(Hash, Clone)]
pub struct CardFileData {
    pub last_modified: Duration,
    pub location: CardLocation,
}

impl CardFileData {
    pub fn from_path(path: &Path) -> Self {

        let last_modified = {
            let system_time = std::fs::metadata(path).unwrap().modified().unwrap();
            system_time_as_unix_time(system_time)
        };
        let location = CardLocation::new(path);

        Self {
            last_modified,
            location
        }
    }

    pub fn as_path(&self) -> PathBuf {
        self.location.as_path()
    }
}

#[derive(Clone, Hash, Debug)]
pub struct AnnoCard {
    pub card: Card,
    pub location: CardLocation,
    pub last_modified: Duration,
}


impl std::fmt::Display for AnnoCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.card_as_ref().front.text)
    }
}

pub struct CardWithRecall(pub AnnoCard, Option<f32>);

impl From<AnnoCard> for Card {
    fn from(value: AnnoCard) -> Self {
        value.card
    }
}

impl VisitStuff for AnnoCard {
    fn get_children(&self) -> Vec<Self> {
        AnnoCard::from_ids(self.card.meta.dependencies.clone().into_iter().collect())
    }

    fn matches_predicate(&self) -> bool {
        !self.card.meta.finished
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

type GetIds = Box<dyn FnMut(Id) -> Vec<Id>>;

impl AnnoCard {
    
    pub fn set_dependent(&mut self, id: &Id) {
        self.card.meta.dependents.insert(*id);
        self.update_card();
        
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependencies.insert(*id);
        other_card.update_card();
    }

    pub fn set_dependency(&mut self, id: &Id) {
        self.card.meta.dependencies.insert(*id);
        self.update_card();
        
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependents.insert(*id);
        other_card.update_card();
    }
    

    pub fn get_dependencies(& self, cache: &mut CardLocationCache) -> Vec<Self> {
        let mut get_children: Box<dyn FnMut(Id, &mut CardLocationCache) -> Vec<Id>> = Box::new(|card_id: Id, cache: &mut CardLocationCache| {
            AnnoCard::from_cached_id(&card_id, cache)
                .card
                .meta
                .dependencies
                .into_iter()
                .collect()
        });
        let ids = match visit_collect_all_descendants(self.card.meta.id, &mut get_children, cache)  {
            Ok(ids) => ids,
            Err(id) => {
                let card = AnnoCard::from_id(&id).unwrap();
                panic!("Infinite recursion found with: {:?}", card);
            }
        };

        let mut hey = vec![];

        for id in ids {
            hey.push(AnnoCard::from_id(&id).unwrap());
        }
        hey
    }

    pub fn get_dependents(&self, cache: &mut CardLocationCache) -> Vec<Self> {
        let mut get_children: Box<dyn FnMut(Id, &mut CardLocationCache) -> Vec<Id>> = Box::new(|card_id: Id, cache: &mut CardLocationCache| {
            AnnoCard::from_cached_id(&card_id, cache)
                .card
                .meta
                .dependents
                .into_iter()
                .collect()
        });
        
        let ids = match visit_collect_all_descendants(self.card.meta.id, &mut get_children, cache)  {
            Ok(ids) => ids,
            Err(id) => {
                let card = AnnoCard::from_id(&id).unwrap();
                panic!("Infinite recursion found with: {:?}", card);
            }
        };

        let mut hey = vec![];

        for id in ids {
            hey.push(AnnoCard::from_id(&id).unwrap());
        }
        hey
    }
    

    pub fn as_path(&self) -> PathBuf {
        self.location.as_path()
    }
    
    pub fn update_cache(&self, cache: &mut CardLocationCache) {
        cache.cache_one(self);
    }

    pub fn pending_filter(&self, cache: &mut CardLocationCache) -> bool {
        self.card.meta.stability.is_none()
            && self.card.meta.suspended.is_suspended()
            && self.card.meta.finished
            && self.is_confidently_resolved(cache)
    }

    pub fn unfinished_filter(&self, cache: &mut CardLocationCache) -> bool {
        !self.card.meta.finished && !self.card.meta.suspended.is_suspended() && self.is_resolved(cache)
    }

    pub fn review_filter(&self, cache: &mut CardLocationCache) -> bool {
        match (self.card.meta.stability, self.card.time_since_last_review()) {
            (Some(stability), Some(last_review_time)) => {
                self.card.meta.finished
                    && self.card.meta.suspended == IsSuspended::False
                    && last_review_time > Duration::from_secs(60) // Lets not review if its less than a minute since last time
                    && stability < last_review_time
                    && self.is_confidently_resolved(cache)
            }
            (_, _) => false,
        }
    }

    /// Checks if corresponding file has been modified after this type got deserialized from the file.
    pub fn is_outdated(&self) -> bool {
        let file_last_modified = {
            let path = self.as_path();
            let system_time = std::fs::metadata(path).unwrap().modified().unwrap();
            system_time_as_unix_time(system_time)
        };

        let in_memory_last_modified = self.last_modified;

        match in_memory_last_modified.cmp(&file_last_modified) {
            Ordering::Less => true,
            Ordering::Equal => false,
            Ordering::Greater => panic!("Card in-memory shouldn't have a last_modified more recent than its corresponding file"),
        }
    }


    pub fn is_resolved(&self, cache: &mut CardLocationCache) -> bool {
        self.get_dependencies(cache)
            .iter()
            .all(|card| card.card.meta.finished)
    }

    /// Checks that its dependencies are not only marked finished, but they're also strong memories.
    pub fn is_confidently_resolved(&self, cache: &mut CardLocationCache) -> bool {
        let min_stability = Duration::from_secs(86400 * 2);
        let min_recall: f32 = 0.95;

        self.get_dependencies(cache).iter().all(|card| {
            let (Some(stability), Some(recall)) = (card.card.meta.stability, card.card.recall_rate()) else {return false};
            
            card.card.meta.finished && stability > min_stability && recall > min_recall
        })
    }

    /// Moves card by deleting it and then creating it again in a new location
    /// warning: will refresh file name
    pub fn move_card(self, destination: &Category, cache: &mut CardLocationCache) -> Self {
        if self.location.category == *destination {
            return self;
        }
        assert!(self.as_path().exists());
        std::fs::remove_file(self.as_path()).unwrap();
        assert!(!self.as_path().exists());
        self.into_card().save_new_card(destination, cache)
    }


    pub fn get_review_type(&self) -> ReviewType {
        match (self.card.meta.stability, self.card.meta.finished) {
            (Some(_), true) => ReviewType::Normal,
            (_, false) => ReviewType::Unfinished,
            (None, true) => ReviewType::Pending,
        }
    }

    pub fn delete(self, cache: &mut CardLocationCache) {
        cache.0.remove(&self.card.meta.id);
        let path = self.as_path();
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
        let mut cards: Vec<Self> = Self::load_all()
            .into_iter()
            .filter(|card| {
                card.card
                    .front
                    .text
                    .to_ascii_lowercase()
                    .contains(&input.to_ascii_lowercase())
                    || card
                        .card
                        .back
                        .text
                        .to_ascii_lowercase()
                        .contains(&input.to_ascii_lowercase())
            })
            .collect();
        Self::sort_by_last_modified(&mut cards);
        cards
    }
    
        
    pub fn search_in_cards<'a>(input: &'a str, cards: &'a Vec<AnnoCard>) -> Vec<&'a AnnoCard> {
        cards
        .iter()
        .filter(|card| {
            card.card.front.text.to_ascii_lowercase().contains(&input.to_ascii_lowercase())
                || card.card.back.text.to_ascii_lowercase().contains(&input.to_ascii_lowercase())
        })
        .collect()
}




    

    pub fn from_ids(ids: Vec<Id>) -> Vec<Self> {
        let mut vec = vec![];
        for id in ids {
            if let Some(card) = Self::from_id(&id) {
                vec.push(card);
            }
        }
        vec
    }
    
    pub fn from_cached_id(id: &Id, cache: &mut CardLocationCache) -> Self {
        let Some(location) = cache.0.get(id) else {
            let card = Self::from_id(id);
            panic!("oh shit, didn't find the card: {:?} from this id: {}", card, id);
        };

        Self::from_location(location)
    }
    
    pub fn from_location(location: &CardLocation) -> Self {
        Self::from_path(&location.as_path())
    }

    pub fn from_id(id: &Id) -> Option<Self> {
        Self::load_all()
            .into_iter()
            .find(|card| &card.card.meta.id == id)
    }
    pub fn load_all() -> Vec<Self> {
        Self::get_cards_from_category_recursively(&Category::root())
    }

    pub fn get_id_map() -> HashMap<Id, Self> {
        let mut map = HashMap::new();
        let cards = Self::load_all();
        for card in cards {
            map.insert(card.card.meta.id, card);
        }
        map
    }

    pub fn sort_by_last_modified(vec: &mut [Self]) {
        vec.sort_by_key(|k| Reverse(k.last_modified));
    }

    pub fn get_full_card_from_id(id: Id) -> Option<Self> {
        let cards = get_all_cards_full();
        cards.into_iter().find(|card| card.card.meta.id == id)
    }

    pub fn edit_with_vim(&self) -> Self {
        let path = self.as_path();
        open_file_with_vim(path.as_path()).unwrap();
        Self::from_path(path.as_path())
    }

    pub fn from_path(path: &Path) -> Self {
        let content = read_to_string(path).expect("Could not read the TOML file");
        let card: Card = toml::from_str(&content).unwrap();
        let location = CardLocation::new(path);

        let last_modified = {
            let system_time = std::fs::metadata(path).unwrap().modified().unwrap();
            system_time_as_unix_time(system_time)
        };

        Self {
            card, location, last_modified
        }
    }

    pub fn into_card(self) -> Card {
        self.card
    }

    pub fn card_as_ref(&self) -> &Card {
        &self.card
    }

    pub fn card_as_mut_ref(&mut self) -> &mut Card {
        &mut self.card
    }


    // Gets called automatically when object goes out of scope.
    pub fn update_card(&self) -> Self {
        let path = self.as_path();
        if !path.exists() {
            let msg = format!("following path doesn't really exist: {}", path.display());
            panic!("{msg}");
        }

        let toml = toml::to_string(self.card_as_ref()).unwrap();

        std::fs::write(&path, toml).unwrap();

        AnnoCard::from_path(path.as_path())
    }

    pub fn refresh(&mut self) {
        *self = Self::from_path(&self.location.as_path())
    }

    pub fn new_review(&mut self, grade: Grade) -> Self {
        let review = Review::new(grade.clone());
        let time_passed = self.card.time_passed_since_last_review();
        self.card.history.push(review);
        self.card.meta.stability = Some(Card::new_stability(grade, time_passed));
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

    pub fn calculate_strength(days_passed: Duration, stability: Duration) -> RecallRate {
        let base: f32 = 0.9;
        let ratio = days_passed.as_secs_f32() / stability.as_secs_f32();
        (base.ln() * ratio).exp()
    }

    pub fn time_since_last_review(&self) -> Option<Duration> {
        let last_unix = self.history.last()?.timestamp;
        let current_unix = current_time();
        current_unix.checked_sub(last_unix)
    }

    pub fn save_new_card(self, category: &Category, cache: &mut CardLocationCache) -> AnnoCard {
        let toml = toml::to_string(&self).unwrap();
        std::fs::create_dir_all(category.as_path()).unwrap();
        let max_char_len = 40;
        let front_text = self.front.text.chars().filter(|c|c.is_ascii_alphanumeric() || c.is_ascii_whitespace() ).collect::<String>().replace(' ', "_");
       let mut file_name = PathBuf::from(truncate_string(front_text, max_char_len));
        if file_name.exists() {
            file_name = PathBuf::from(self.meta.id.to_string());
        }

        let path = category.as_path().join(file_name).with_extension("toml");

        std::fs::write(&path, toml).unwrap();

        let full_card = AnnoCard::from_path(path.as_path());
        full_card.update_cache(cache);
        full_card
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
        let factor = match self {
            Grade::None => 0.1,
            Grade::Late => 0.25,
            Grade::Some => 2.,
            Grade::Perfect => 3.,
        };
        factor * Self::randomize_factor()
    }

    // gets a random number from 0.8 to 1.2
    fn randomize_factor() -> f32 {
        1.2 - (((current_time().as_micros() % 40) as f32) / 100.)
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
    visit_collect_all_descendants,
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

use serde::de::{Deserializer};

#[derive(Hash, Debug, Clone, PartialEq)]
pub enum IsSuspended{
    False,
    True,
    TrueUntil(Duration),
}

impl From<bool> for IsSuspended{
    fn from(value: bool) -> Self {
        match value {
            true => Self::True,
            false => Self::False,
        }
    }
}

impl IsSuspended{
    
    fn verify_time(self) -> Self {
        if let Self::TrueUntil(dur) = self {
            if dur < current_time() {
                return Self::False;
            }
        }
        self
    }

    pub fn is_suspended(&self) -> bool {
        if let IsSuspended::False = self {
            return false;
        }
        true
    }

    // prefer this if you have mutable reference
    pub fn is_suspended_mut(&mut self) -> bool {
        match self {
            IsSuspended::False => false,
            IsSuspended::True => true,
            IsSuspended::TrueUntil(dur) => {
                let now = current_time();
                if now > *dur {
                    *self = Self::False;
                     false
                } else {
                    true
                }
            },
        }
    }
}

impl Serialize for IsSuspended {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self.clone().verify_time() {
            IsSuspended::False => serializer.serialize_bool(false),
            IsSuspended::True => serializer.serialize_bool(true),
            IsSuspended::TrueUntil(duration) => serializer.serialize_u64(duration.as_secs()),
        }
    }
}



impl<'de> Deserialize<'de> for IsSuspended {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        match value {
            Value::Boolean(b) => Ok(b.into()),
                        Value::Integer(i) => {
                if let Ok(secs) = std::convert::TryInto::<u64>::try_into(i) {
                    Ok(IsSuspended::TrueUntil(Duration::from_secs(secs)).verify_time())
                } else {
                    Err(de::Error::custom("Invalid duration format"))
                }
            },

            _ => Err(serde::de::Error::custom("Invalid value for IsDisabled")),
        }
    }
}




#[derive(Hash, Deserialize, Serialize, Debug, Clone)]
pub struct Meta {
    pub id: Id,
    pub dependencies: BTreeSet<Id>,
    pub dependents: BTreeSet<Id>,
    pub suspended: IsSuspended,
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
            dependencies: BTreeSet::new(),
            dependents: BTreeSet::new(),
            suspended: IsSuspended::False,
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


pub fn ccalculate_left_memory(days_passed: Duration, stability: Duration) -> f32 {
    let base: f32 = 0.9;
    let ratio = days_passed.as_secs_f32() / stability.as_secs_f32();
    let lambda = -base.ln();

    (lambda * ratio).exp() / lambda
}

pub fn calculate_left_memory(t1: Duration, stability: Duration) -> f32 {
    let base: f32 = 0.9;
    let lambda = -base.ln();
    let ratio1 = t1.as_secs_f32() / stability.as_secs_f32();

    (-lambda * ratio1).exp() / lambda
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_integral(){
        let days_passed = Duration::from_secs(86400 * 1);
        let stability = Duration::from_secs(86400 * 1);
        dbg!(calculate_left_memory(days_passed, stability));
    }

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
