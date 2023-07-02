use serde::{Deserialize, Serialize, de, Serializer};
use toml::Value;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use std::time::Duration;
use uuid::Uuid;

use crate::categories::Category;
use crate::media::AudioSource;
use crate::VisitStuff;
use crate::{common::current_time, Id};

pub type RecallRate = f32;



#[derive(Default)]
pub struct CardCache(pub HashMap<Id, SavedCard>);

impl CardCache{
    pub fn _maybe_update(&mut self, id: &Id) {
    let card_needs_update = match self.0.get(id) {
        Some(cached_card) => {
            // Get the file's last_modified time
            let metadata = std::fs::metadata(cached_card.as_path()).unwrap();
            let last_modified_time = system_time_as_unix_time( metadata.modified().unwrap());

            // Check if the file has been modified since we cached it
            last_modified_time > cached_card.last_modified
        },
        None => true, // If card isn't in the cache, then we definitely need to update
    };

    if card_needs_update {
        // Read the card from the disk
        // expensive! it'll comb through all the cards linearly.
        let card = SavedCard::from_id(id).unwrap();
        self.0.insert(*id, card);
    }
    }

    pub fn _get_owned(&mut self, id: &Id) -> SavedCard {
        self._maybe_update(id);
        self._get_ref(id).clone()
    }

    pub fn _get_ref(&mut self, id: &Id) -> &SavedCard {
        self._maybe_update(id);
        self.0.get(id).unwrap()

}
    pub fn _get_mut(&mut self, id: &Id) -> &mut SavedCard {
        self._maybe_update(id);
        self.0.get_mut(id).unwrap()

}
    
   pub fn new() -> Self {
       let mut cache = Self::default();
       cache.cache_all();
       cache
   } 

    fn cache_all(&mut self) {
        let all_cards = SavedCard::load_all();
        for card in all_cards{
            self.cache_one(card);
        }
    }
    
    pub fn cache_one(&mut self, card: SavedCard) {
        self.0.insert(card.card.meta.id, card);
    }
}



#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Debug)]
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




#[derive(Clone, Ord,PartialOrd, PartialEq, Eq, Hash, Debug)]
pub struct SavedCard {
    card: Card,
    location: CardLocation,
    last_modified: Duration,
}



impl std::fmt::Display for SavedCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.front_text())
    }
}


impl From<SavedCard> for Card {
    fn from(value: SavedCard) -> Self {
        value.card
    }
}

impl VisitStuff for SavedCard {
    fn get_children(&self) -> Vec<Self> {
        SavedCard::from_ids(self.card.meta.dependencies.clone().into_iter().collect())
    }

    fn matches_predicate(&self) -> bool {
        !self.card.meta.finished
    }
}


pub enum ReviewType {
    Normal,
    Pending,
    Unfinished,
}



#[derive(Debug)]
pub struct CardInfo {
    pub recall_rate: f32,
    pub strength: f32,
    pub stability: f32,
    pub resolved: bool,
    pub suspended: bool,
    pub finished: bool,
}

impl CardInfo{
    fn new(card: &SavedCard, cache: &mut CardCache) -> Option<Self>{
        Self {
            recall_rate: card.recall_rate()?,
            strength: card.calculate_memory_left()?,
            stability: card.stability()?.as_secs_f32() / 86400.,
            resolved: card.is_resolved(cache),
            suspended: card.is_suspended(),
            finished: card.is_finished(),
        }.into()
    }
}



impl SavedCard {
    
    pub fn get_info(&self, cache: &mut CardCache) -> Option<CardInfo>{
        CardInfo::new(self, cache)
    }
    
    
    
    pub fn get_dependendents_cached<'a>(&'a self, cache: &'a CardCache) -> BTreeSet<&'a Self> {
        let mut dependencies = BTreeSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(self);

        while let Some(card) = stack.pop_back() {
            if !dependencies.contains(&card) {
                dependencies.insert(card);

                let card_dependencies = card.dependents(cache);
                
                for dependency in card_dependencies {
                    stack.push_back(dependency);
                }
            }
        }
        
        dependencies.remove(self);
        dependencies
    }
    
    pub fn get_dependencies_cached<'a>(&'a self, cache: &'a CardCache) -> BTreeSet<&'a Self> {
        let mut dependencies = BTreeSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(self);

        while let Some(card) = stack.pop_back() {
            if !dependencies.contains(&card) {
                dependencies.insert(card);

                let card_dependencies = card.dependencies(cache);
                
                for dependency in card_dependencies {
                    stack.push_back(dependency);
                }
            }
        }

        dependencies.remove(self);
        dependencies
    }

    
    pub fn reviews(&self) -> &Vec<Review> {
        &self.card.history.0
    }
    
pub fn calculate_memory_left(&self) -> Option<f32> {
    let (Some(stability), Some(time_passed)) = (self.stability(), self.time_since_last_review())  else {
        return None;
    };
    calculate_left_memory(time_passed, stability.to_owned()).into()
}

    pub fn new(card: Card, location: CardLocation, last_modified: Duration) -> Self {
        Self {
            card,
            location,
            last_modified,
        }
    }
    
    pub fn get_unfinished_dependent_qty(&self, cache: &mut CardCache) -> usize {
        self.get_dependendents_cached(cache).iter().filter(|card|card.is_finished()).count()
    }
    
    pub fn category(&self) -> &Category {
        &self.location.category
    }
    
    pub fn last_modified(&self) -> &Duration {
        &self.last_modified
    }
    
    pub fn front_text(&self) -> &str {
        &self.card.front.text
    }
    
    pub fn is_suspended(&self) -> bool {
        self.card.meta.suspended.is_suspended()
    }
    
    pub fn is_finished(&self) -> bool {
        self.card.meta.finished
    }
    
    pub fn recall_rate(&self) -> Option<f32> {
        self.card.history.recall_rate()
    }
    
    pub fn stability(&self) -> Option<Duration> {
        self.card.history.stability()
    }
    
    pub fn time_since_last_review(&self) -> Option<Duration> {
        self.card.time_passed_since_last_review()
    }
    
    pub fn set_suspended(&mut self, suspended: IsSuspended) {
        self.card.meta.suspended = suspended;
    }
    
    pub fn set_finished(&mut self, finished: bool) {
        self.card.meta.finished = finished;
    }
    
    pub fn back_text(&self) -> &str {
        &self.card.back.text
    }

    pub fn insert_tag(&mut self, tag: String)  {
        self.card.meta.tags.insert(tag);
    }
    
    pub fn contains_tag(&self, tag: &str) -> bool {
        self.card.meta.tags.contains(tag)
    }

    pub fn id(&self) -> &Id {
        &self.card.meta.id
    }
    
    pub fn dependent_ids(&self) -> &BTreeSet<Id> {
        &self.card.meta.dependents
    }

    pub fn dependency_ids(&self) -> &BTreeSet<Id> {
        &self.card.meta.dependencies
    }

    pub fn dependents<'a>(&'a self, cache: &'a  CardCache) -> BTreeSet<&'a Self> {
        let mut set = BTreeSet::new();
        for id in self.dependent_ids(){
                set.insert(cache.0.get(id).unwrap());
    }
    set
}

    pub fn dependencies<'a>(&'a self, cache: &'a  CardCache) -> BTreeSet<&'a Self> {
            let mut set = BTreeSet::new();
            for id in self.dependency_ids(){
                // this sucks
                set.insert(cache.0.get(id).unwrap());
            }
        set
    }


    
    pub fn set_dependent(&mut self, id: &Id) {
        self.card.meta.dependents.insert(*id);
        self.update_card();
        
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependencies.insert(self.card.meta.id);
        other_card.update_card();
    }
    
    /// a = span means foo
    /// b = change span desc by..
    /// inserted: c = what is a span desc?
    
    pub fn _insert_dependency_raw(dependent_id: &Id, dependency_id: &Id, insertion_id: &Id) {
        let mut dependent = Self::from_id(dependent_id).unwrap();
        let mut insertion = Self::from_id(insertion_id).unwrap();
        
        dependent._remove_dependency(dependency_id);
        dependent.set_dependency(insertion_id);
        insertion.set_dependency(dependency_id);
        
    }

    pub fn set_dependency(&mut self, id: &Id) {
        self.card.meta.dependencies.insert(*id);
        self.update_card();
        
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependents.insert(self.card.meta.id);
        other_card.update_card();
    }
    
    pub fn _remove_dependency(&mut self, id: &Id) {
        self.card.meta.dependencies.remove(id);
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependents.remove(self.id());
        other_card.update_card();
        self.update_card();
    }
    
    
    pub fn _remove_dependent(&mut self, id: &Id) {
        self.card.meta.dependencies.remove(id);
        let mut other_card = Self::from_id(id).unwrap();
        other_card.card.meta.dependencies.remove(self.id());
        other_card.update_card();
        self.update_card();
    }
    

    

    pub fn as_path(&self) -> PathBuf {
        self.location.as_path()
    }
    

    pub fn pending_filter(&self, cache: &mut CardCache) -> bool {
        self.card.history.is_empty()
            && !self.is_suspended()
            && self.is_finished()
            && self.is_confidently_resolved(cache)
    }

    pub fn unfinished_filter(&self, cache: &mut CardCache) -> bool {
        !self.is_finished() && !self.is_suspended() && self.is_resolved(cache)
    }



    pub fn review_filter(&self, cache: &mut CardCache) -> bool {
        match (self.stability(), self.card.history.time_since_last_review()) {
            (Some(stability), Some(last_review_time)) => {
                self.is_finished()
                    && !self.is_suspended()
                    && last_review_time > Duration::from_secs(60) // Lets not review if its less than a minute since last time
                    && stability < last_review_time
                    && self.is_confidently_resolved(cache)
            }
            (_, _) => false,
        }
    }

    /// Checks if corresponding file has been modified after this type got deserialized from the file.
    pub fn _is_outdated(&self) -> bool {
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


    pub fn is_resolved(&self, cache: &mut CardCache) -> bool {
    self.get_dependencies_cached(cache)
            .iter()
            .all(|card| card.is_finished())
    }

    /// Checks that its dependencies are not only marked finished, but they're also strong memories.
    pub fn is_confidently_resolved(&self, cache: &mut CardCache) -> bool {
        let min_stability = Duration::from_secs(86400 * 2);
        let min_recall: f32 = 0.95;
        let dependencies = self.get_dependencies_cached(cache);
        let mut dbgshit = vec![];
      //  dbg!("##############", self.front_text());
        for dep in dependencies{
            dbgshit.push(format!("{}: {}\t", dep.front_text(), dep.is_finished()));
        }
     //   dbg!(dbgshit);

        let x = self.get_dependencies_cached(cache).iter().all(|card| {
            let (Some(stability), Some(recall)) = (card.stability(), card.card.history.recall_rate()) else {return false};
            
            card.card.meta.finished && stability > min_stability && recall > min_recall
        });
    //    dbg!("$$", &x, "$$");
        x
    }

    /// Moves card by deleting it and then creating it again in a new location
    /// warning: will refresh file name
    pub fn move_card(self, destination: &Category, cache: &mut CardCache) -> Self {
        if self.location.category == *destination {
            return self;
        }
        assert!(self.as_path().exists());
        std::fs::remove_file(self.as_path()).unwrap();
        assert!(!self.as_path().exists());
        self.into_card().save_new_card(destination, cache)
    }


    pub fn get_review_type(&self) -> ReviewType {
        match (self.card.history.is_empty(), self.is_finished()) {
            (_, false) => ReviewType::Unfinished,
            (false, true) => ReviewType::Normal,
            (true, true) => ReviewType::Pending,
        }
    }

    pub fn delete(self, cache: &mut CardCache) {
        cache.0.remove(&self.card.meta.id);

        let path = self.as_path();
        std::fs::remove_file(path).unwrap();

        let self_id = self.card.meta.id;
        
        for dependency in self.card.meta.dependencies {
            let Some(mut dependency) = SavedCard::from_id(&dependency) else {continue};
            dependency.card.meta.dependents.remove(&self_id);
            dependency.update_card();
        }
        
        
        for dependent in self.card.meta.dependents {
            let Some(mut dependent) = SavedCard::from_id(&dependent) else {continue};
            dependent.card.meta.dependencies.remove(&self_id);
            dependent.update_card();
        }

    }

    pub fn get_cards_from_category_recursively(category: &Category) -> HashSet<Self> {
        let mut cards = HashSet::new();
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
    
        
    pub fn search_in_cards<'a>(input: &'a str, cards: &'a HashSet<SavedCard>) -> Vec<&'a SavedCard> {
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
    

    pub fn from_id(id: &Id) -> Option<Self> {
        Self::load_all()
            .into_iter()
            .find(|card| &card.card.meta.id == id)
    }
    pub fn load_all() -> HashSet<Self> {
        Self::get_cards_from_category_recursively(&Category::root())
    }


    pub fn sort_by_last_modified(vec: &mut [Self]) {
        vec.sort_by_key(|k| Reverse(k.last_modified));
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


    pub fn update_card(&self) -> Self {
        let path = self.as_path();
        if !path.exists() {
            let msg = format!("following path doesn't really exist: {}", path.display());
            panic!("{msg}");
        }

        let toml = toml::to_string(self.card_as_ref()).unwrap();

        std::fs::write(&path, toml).unwrap();

        SavedCard::from_path(path.as_path())
    }


    pub fn new_review(&mut self, grade: Grade) -> Self {
        let review = Review::new(grade);
        self.card.history.add_review(review);
        self.update_card()
    }
}



#[derive(Ord, PartialOrd, Eq, Hash, PartialEq, Deserialize, Serialize, Debug, Default, Clone)]
pub struct Card {
    pub front: Side,
    pub back: Side,
    pub meta: Meta,
    #[serde(default, skip_serializing_if = "Reviews::is_empty")]
    pub history: Reviews,
}


// public
impl Card {
    pub fn new(front: Side, back: Side, meta: Meta) -> Self {
        Card {
            front,
            back,
            meta,
            history: Reviews::default(),
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
    
    pub fn _review_priority(&self) -> Option<f32> {
        let recall_rate = self.history.recall_rate()?;
        let priority = &self.meta.priority;
        Some((recall_rate - 1.0) * priority.as_float())
    }



    pub fn save_new_card(self, category: &Category, cache: &mut CardCache) -> SavedCard {
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

        let full_card = SavedCard::from_path(path.as_path());
        cache.cache_one(full_card.clone());
        full_card
    }


    fn time_passed_since_last_review(&self) -> Option<Duration> {
        Some(current_time() - self.history.0.last()?.timestamp)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, Serialize, Debug, Default, Clone)]
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
};



#[derive(Ord, PartialOrd, Eq, Hash, PartialEq, Debug, Default, Clone)]
pub struct Reviews(Vec<Review>);

impl Reviews{
    pub fn is_empty(&self) -> bool{
        self.0.is_empty()
    }
    
    
    pub fn add_review(&mut self, review: Review) {
        self.0.push(review);
    }
    
    fn new_stability(grade: &Grade, time_passed: Option<Duration>) -> Duration {
        let grade_factor = grade.get_factor();
        let time_passed = time_passed.unwrap_or(Duration::from_secs(86400));
        time_passed.mul_f32(grade_factor)
    }


    pub fn stability(&self) -> Option<Duration> {
        let mut reviews = self.0.iter();
        let mut stability = Self::new_stability(&reviews.next()?.grade, None);
        
        for review in reviews {
           stability = Self::new_stability(&review.grade, review.time_passed().into());
        }
        stability.into()
    }

    

    pub fn recall_rate(&self) -> Option<RecallRate> {
        let days_passed = self.time_since_last_review()?;
        let stability = self.stability()?;
        Some(Self::calculate_strength(&days_passed, &stability))
    }

    pub fn calculate_strength(days_passed: &Duration, stability: &Duration) -> RecallRate {
        let base: f32 = 0.9;
        let ratio = days_passed.as_secs_f32() / stability.as_secs_f32();
        (base.ln() * ratio).exp()
    }

    pub fn time_since_last_review(&self) -> Option<Duration> {
        self.0.last().map(Review::time_passed)
    }
    
}

impl Serialize for Reviews {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Reviews {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut reviews = Vec::<Review>::deserialize(deserializer)?;
        reviews.sort_by_key(|review| review.timestamp);
        Ok(Reviews(reviews))

    }
}



#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, Clone, Serialize, Debug, Default)]
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
    
    fn time_passed(&self) -> Duration {
        let unix = self.timestamp;
        let current_unix = current_time();
        current_unix.checked_sub(unix).unwrap()
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, Clone, Serialize, Debug, Default)]
pub struct Side {
    pub text: String,
    #[serde(flatten)]
    pub audio: AudioSource,
    //#[serde(deserialize_with = "deserialize_image_path")]
    //pub image: ImagePath,
}


use serde::de::{Deserializer};

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Clone)]
pub enum IsSuspended{
    False,
    True,
    // Card is temporarily suspended, until contained unix time has passed.
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

impl Default for IsSuspended {
    fn default() -> Self {
        Self::False
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


fn default_finished() -> bool {
    true
}

// How important a given card is, where 0 is the least important, 100 is most important.
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Clone)]
pub struct Priority(u32);

impl Priority {
    pub fn as_float(&self) -> f32 {
        self.to_owned().into()
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self(50)
    }
}

impl From<Priority> for f32 {
    fn from(value: Priority) -> Self {
        value.0 as f32 / 100.
    }
}

impl Serialize for Priority {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Priority {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        if value > 100 {
            Err(serde::de::Error::custom("Invalid priority value"))
        } else {
            Ok(Priority(value))
        }
    }
}



#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, Serialize, Debug, Clone)]
pub struct Meta {
    pub id: Id,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dependencies: BTreeSet<Id>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dependents: BTreeSet<Id>,
    #[serde(default)]
    pub suspended: IsSuspended,
    #[serde(default = "default_finished")]
    pub finished: bool,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
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
            priority: Priority::default(),
            tags: BTreeSet::new(),
        }
    }
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
    fn foobar(){
        let vec : Vec<i32>= vec![];
        dbg!(vec.iter().all(|x|x==&0));
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
        let recall_rate = Reviews::calculate_strength(&days_passed, &stability);
        assert_eq!(recall_rate, 1.0);

        let days_passed = Duration::from_secs(86400);
        let recall_rate = Reviews::calculate_strength(&days_passed, &stability);
        assert_eq!(recall_rate, 0.9);
    }
}
