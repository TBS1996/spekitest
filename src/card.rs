use serde::{Deserialize, Serialize, de, Serializer};
use toml::Value;

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use std::time::Duration;
use uuid::Uuid;


use std::sync::Arc;

use crate::categories::Category;
use crate::media::AudioSource;
use crate::{common::current_time, Id};

pub type RecallRate = f32;



#[derive(Default)]
pub struct CardCache(HashMap<Id, Arc<SavedCard>>);

impl CardCache{
    pub fn maybe_update(&mut self, id: &Id) {
    let card_needs_update = match self.0.get(id) {
        Some(cached_card) => {
            // Get the file's last_modified time
            let metadata = std::fs::metadata(cached_card.as_path()).unwrap();
            let last_modified_time = system_time_as_unix_time( metadata.modified().unwrap());

            // Check if the file has been modified since we cached it
            Some(last_modified_time > cached_card.last_modified)
        },
        None => None, // If card isn't in the cache, then we definitely need to update
    };

    match card_needs_update {
        Some(true) => {
            let path = self.0.get(id).unwrap().as_path();
            let updated_card = SavedCard::from_path(path.as_path());
            self.0.insert(*id, updated_card.into());

    }
    // if you find the card, and it's up to date, then no need to do anything.
    Some(false) => {},
    None => {
        
        // Read the card from the disk
        // expensive! it'll comb through all the cards linearly.
        let card = SavedCard::from_id(id).unwrap();
        self.0.insert(*id, card.into());
    }
    };
}
    
    pub fn exists(&self, id: &Id) -> bool {
        self.0.get(id).is_some()
    }
    
    pub fn insert(&mut self, id: &Id, card: SavedCard) {
        self.0.insert(*id, card.into());
    }
    
    #[allow(dead_code)]
    pub fn remove(&mut self, id: &Id) {
        self.0.remove(id);
    }

    pub fn dependencies(&mut self, id: &Id) -> BTreeSet<Id>{
        self.get_ref(id).dependency_ids().iter().map(|id| id.to_owned()).collect()
    }


    pub fn dependents(&mut self, id: &Id) -> BTreeSet<Id>{
        self.get_ref(id).dependent_ids().iter().map(|id| id.to_owned()).collect()
    }


pub fn recursive_dependencies(&mut self, id: &Id) -> BTreeSet<Id> {
        let mut dependencies = BTreeSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(*id);

        while let Some(card) = stack.pop_back() {
            if !dependencies.contains(&card) {
                dependencies.insert(card);

                let card_dependencies = self.dependencies(&card);
                
                for dependency in card_dependencies {
                    stack.push_back(dependency);
                }
            }
        }

        dependencies.remove(id);
        dependencies
}

pub fn recursive_dependents(&mut self, id: &Id) -> BTreeSet<Id> {
        let mut dependencies = BTreeSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(*id);

        while let Some(card) = stack.pop_back() {
            if !dependencies.contains(&card) {
                dependencies.insert(card);

                let card_dependencies = self.dependents(&card);
                
                for dependency in card_dependencies {
                    stack.push_back(dependency);
                }
            }
        }

        dependencies.remove(id);
        dependencies
}

    pub fn get_owned(&mut self, id: &Id) -> SavedCard {
        self.maybe_update(id);
        (*self.get_ref(id)).clone()
    }

    pub fn get_ref(&mut self, id: &Id) -> Arc<SavedCard> {
        self.maybe_update(id);
        self.0.get(id).unwrap().to_owned()
}

   pub fn new() -> Self {
       let mut cache = Self::default();
       cache.cache_all();
       cache
   } 

    pub fn refresh(&mut self) {
        *self = Self::new();
    }

    fn cache_all(&mut self) {
        let all_cards = SavedCard::load_all();
        for card in all_cards{
            self.cache_one(card);
        }
    }
    
    pub fn cache_one(&mut self, card: SavedCard) {
        self.0.insert(card.card.meta.id, card.into());
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




/// Represents a card that has been saved as a toml file, which is basically anywhere in the codebase
/// except for when youre constructing a new card. 
/// Don't save this in containers or pass to functions, rather use the Id, and get new instances of SavedCard from the cache. 
/// Also, every time you mutate it, call the persist() function.
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


pub enum ReviewType {
    Normal,
    Pending,
    Unfinished,
}



#[derive(Debug, Default)]
pub struct CardInfo {
    pub recall_rate: f32,
    pub strength: f32,
    pub stability: f32,
    pub resolved: bool,
    pub suspended: bool,
    pub finished: bool,
    pub change: f32,
}

impl CardInfo{
    fn new(card: &SavedCard, cache: &mut CardCache) -> Option<Self>{
        Self {
            recall_rate: card.recall_rate()?,
            strength: card.strength()?.as_secs_f32() / 86400.,
            stability: card.stability()?.as_secs_f32() / 86400.,
            change: card.card.history.expected_gain()?,
            resolved: card.is_resolved(cache),
            suspended: card.is_suspended(),
            finished: card.is_finished(),

        }.into()
    }
}



impl SavedCard {
    pub fn set_priority(&mut self, priority: Priority) {
        self.card.meta.priority = priority;
        self.persist();
    } 
    
    #[allow(dead_code)]
    pub fn priority(&self) -> &Priority {
        &self.card.meta.priority
    }
    
    pub fn expected_gain(&self) -> Option<f32> {
        self.card.history.expected_gain()
    }
    
    pub fn get_info(&self, cache: &mut CardCache) -> Option<CardInfo>{
        CardInfo::new(self, cache)
    }
    
    
    pub fn reviews(&self) -> &Vec<Review> {
        &self.card.history.0
    }
    

    pub fn new(card: Card, location: CardLocation, last_modified: Duration) -> Self {
        Self {
            card,
            location,
            last_modified,
        }
    }
    
    pub fn get_unfinished_dependent_qty(&self, cache: &mut CardCache) -> usize {
       let dependents = cache.recursive_dependents(self.id());
       let mut unfinished = 0;
       for dependent in dependents {
        unfinished += cache.get_ref(&dependent).is_finished() as usize;
       }
       unfinished
    }
    
    pub fn category(&self) -> &Category {
        &self.location.category
    }
    
    pub fn front_text(&self) -> &str {
        &self.card.front.text
    }
    
    #[allow(dead_code)]
    pub fn is_pending(&self) -> bool {
        self.card.history.is_empty()
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

    pub fn strength(&self) -> Option<Duration> {
        self.card.history.strength()
    }
    
    pub fn time_since_last_review(&self) -> Option<Duration> {
        self.card.time_passed_since_last_review()
    }
    
    pub fn set_suspended(&mut self, suspended: IsSuspended) {
        self.card.meta.suspended = suspended;
        self.persist();
    }
    
    pub fn set_finished(&mut self, finished: bool) {
        self.card.meta.finished = finished;
        self.persist();
    }
    
    pub fn back_text(&self) -> &str {
        &self.card.back.text
    }

    pub fn insert_tag(&mut self, tag: String)  {
        self.card.meta.tags.insert(tag);
        self.persist();
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

    pub fn set_dependent(&mut self, id: &Id, cache: &mut CardCache) {
        self.card.meta.dependents.insert(*id);
        *self = self.persist();
        
        let mut other_card = cache.get_owned(id);
        other_card.card.meta.dependencies.insert(self.card.meta.id);
        other_card.persist();
    }
    
    /// a = span means foo
    /// b = change span desc by..
    /// inserted: c = what is a span desc?
    
    pub fn _insert_dependency_raw(dependent_id: &Id, dependency_id: &Id, insertion_id: &Id, cache: &mut CardCache) {
        let mut dependent = Self::from_id(dependent_id).unwrap();
        let _insertion = Self::from_id(insertion_id).unwrap();
        
        dependent.remove_dependency(dependency_id, cache);
        //dependent.set_dependency(insertion_id);
        //insertion.set_dependency(dependency_id);
        
    }

    pub fn set_dependency(&mut self, id: &Id, cache: &mut CardCache) {
        self.card.meta.dependencies.insert(*id);
        *self = self.persist();
        
        let mut other_card = cache.get_owned(id);
        other_card.card.meta.dependents.insert(self.card.meta.id);
        other_card.persist();
    }
    
    pub fn remove_dependency(&mut self, id: &Id, _cache: &mut CardCache) {
        self.card.meta.dependencies.remove(id);
        self.persist();
        
        if let Some(mut other_card) = Self::from_id(id) {
            other_card.card.meta.dependents.remove(self.id());
            other_card.persist();
        }
    }
    
    
    pub fn remove_dependent(&mut self, id: &Id, _cache: &mut CardCache) {
        self.card.meta.dependencies.remove(id);
        self.persist();
        

        if let Some(mut other_card) = Self::from_id(id) {
            other_card.card.meta.dependents.remove(self.id());
            other_card.persist();
        }
        
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
        cache.recursive_dependencies(self.id()).iter().all(|id| cache.get_ref(id).is_finished())
    }

    /// Checks that its dependencies are not only marked finished, but they're also strong memories.
    pub fn is_confidently_resolved(&self, cache: &mut CardCache) -> bool {
        let min_stability = Duration::from_secs(86400 * 2);
        let min_recall: f32 = 0.95;
        let dependencies = cache.recursive_dependencies(self.id());
      //  dbg!("##############", self.front_text());
        for _dep in dependencies{
         //   dbgshit.push(format!("{}: {}\t", dep.front_text(), dep.is_finished()));
        }
     //   dbg!(dbgshit);

        let x = cache.recursive_dependencies(self.id()).iter().all(|id| {
            let card = cache.get_ref(id);
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

        let path = self.as_path();
        std::fs::remove_file(path).unwrap();

        let self_id = self.card.meta.id;
        
        for dependency in self.card.meta.dependencies {
            let Some(mut dependency) = SavedCard::from_id(&dependency) else {continue};
            dependency.card.meta.dependents.remove(&self_id);
            dependency.persist();
        }
        
        
        for dependent in self.card.meta.dependents {
            let Some(mut dependent) = SavedCard::from_id(&dependent) else {continue};
            dependent.card.meta.dependencies.remove(&self_id);
            dependent.persist();
        }
        cache.refresh();
    }

    pub fn get_cards_from_category_recursively(category: &Category) -> HashSet<Self> {
        let mut cards = HashSet::new();
        let cats = category.get_following_categories();
        for cat in cats {
            cards.extend(cat.get_containing_cards());
        }
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

    fn persist(&self) -> Self {
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
        self.persist()
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
        cache.refresh();
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
        match self {
            Grade::None => 0.1,
            Grade::Late => 0.25,
            Grade::Some => 2.,
            Grade::Perfect => 3.,
        }
        //factor * Self::randomize_factor()
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
    pub fn strength(&self) -> Option<Duration> {
        let days_passed = self.time_since_last_review()?;
        let stability = self.stability()?;
        let strength = calculate_left_memory(days_passed, stability);
        //dbg!(days_passed.as_secs_f32() / 86400., stability.as_secs_f32() / 86400., strength);
          Duration::from_secs_f32(strength * 86400.).into()
       
    }
    
    
    pub fn next_strength(&self, grade: Grade) -> Duration {
        let mut myself = self.clone();
        let new_review = Review::new(grade);
        myself.add_review(new_review);
        myself.strength().unwrap()
    }

    
    // Expected gain in memory strength after a review.
    pub fn expected_gain(&self) -> Option<f32> {

        let recall_rate = self.recall_rate()?;

        let current_strength = self.strength()?.as_secs_f32() / 86400. ;
        let failstrength = self.next_strength(Grade::Late).as_secs_f32() / 86400. ;
        let winstrength = self.next_strength(Grade::Some).as_secs_f32() / 86400.;
        
        
        let expected_win = (winstrength) * recall_rate;
        let expected_loss = (failstrength) * (1. - recall_rate);
        
        let expected_strength = expected_win + expected_loss;
        
        //dbg!(recall_rate, current_strength, expected_loss, expected_win, failstrength, winstrength);
        

      //  Some((expected_strength - current_strength) / 1.)
        (expected_strength / current_strength).into()
    }

    pub fn is_empty(&self) -> bool{
        self.0.is_empty()
    }
    
    
    pub fn add_review(&mut self, review: Review) {
        self.0.push(review);
    }
    
    
pub fn new_stability(grade: &Grade, time_passed: Option<Duration>, current_stability: Duration) -> Duration {
    let grade_factor = grade.get_factor();
    let time_passed = time_passed.unwrap_or(Duration::from_secs(86400));

    if grade_factor < 1.0 { // the grade is wrong
        time_passed.mul_f32(grade_factor).min(current_stability)
    } else { // the grade is correct
        let alternative_stability = time_passed.mul_f32(grade_factor);
        if alternative_stability > current_stability {
             alternative_stability
        } else {
            let interpolation_ratio = time_passed.as_secs_f32() / current_stability.as_secs_f32() * grade_factor;
            current_stability + Duration::from_secs_f32(current_stability.as_secs_f32() * interpolation_ratio)
        }
    }
}

pub fn stability(&self) -> Option<Duration> {
    let reviews = &self.0;
    if reviews.is_empty() {
        return None;
    }

    let mut stability = Self::new_stability(&reviews[0].grade, None, Duration::from_secs(86400));
    let mut prev_timestamp = reviews[0].timestamp;

    for review in &reviews[1..] {
        let time_passed = review.timestamp - prev_timestamp; // Calculate the time passed since the previous review
        stability = Self::new_stability(&review.grade, Some(time_passed), stability);
        prev_timestamp = review.timestamp; // Update the timestamp for the next iteration
    }

    Some(stability)
}


    pub fn recall_rate(&self) -> Option<RecallRate> {
        let days_passed = self.time_since_last_review()?;
        let stability = self.stability()?;
        Some(Self::calculate_recall_rate(&days_passed, &stability))
    }

    pub fn calculate_recall_rate(days_passed: &Duration, stability: &Duration) -> RecallRate {
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
     !matches!(self, IsSuspended::False)
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

impl TryFrom<char> for Priority{
    type Error = ();

    fn try_from(value: char) -> Result<Self, Self::Error> {
        let pri = match value  {
            '1' => 16,
            '2' => 33,
            '3' => 66,
            '4' => 83,
            _ => return Err(()),
        };
        Ok(Self(pri))
    }
}

impl From<u32> for Priority {
    fn from(value: u32) -> Self {
        Self(value.clamp(0, 100))
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
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
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


pub fn calculate_left_memory(time_passed: Duration, stability: Duration) -> f32 {
    let t = time_passed.as_secs_f64() / 86400.;
    let ln0_9 = 0.9_f64.ln();
    let value = -1.0 / ln0_9 + (1.0 - f64::exp(t * ln0_9)) / ln0_9;
    value as f32 * stability.as_secs_f32() / 86400.
}




#[cfg(test)]
mod tests {
    use super::*;
    
        #[test]
    fn test_stability() {
        let reviews = vec![
            Review {
                timestamp: Duration::from_secs(1687124756),
                grade: Grade::None,
                time_spent: Duration::default(),
            },
            Review {
                timestamp: Duration::from_secs(1687158818),
                grade: Grade::Some,
                time_spent: Duration::default(),
            },
            Review {
                timestamp: Duration::from_secs(1687248985),
                grade: Grade::Some,
                time_spent: Duration::default(),
            },
            Review {
                timestamp: Duration::from_secs(1687439802),
                grade: Grade::Some,
                time_spent: Duration::default(),
            },
            Review {
                timestamp: Duration::from_secs(1687853599),
                grade: Grade::Late,
                time_spent: Duration::default(),
            },

            Review {
                timestamp: Duration::from_secs(1687853599),
                grade: Grade::Some,
                time_spent: Duration::default(),
            },

        ];
        
        //reviews.pop();

        let reviews = Reviews(reviews);
        let x = reviews.stability().unwrap().as_secs_f32() / 86400.;
        dbg!(x);

    }

    
    fn debug_review(passed: f32, success: bool) -> Review{
        Review {
            timestamp: Duration::default() + Duration::from_secs_f32(passed * 86400.),
            grade: if success { Grade::Some} else {Grade::Late},
            time_spent: Duration::default()
        }
    }
    
    #[test]
    fn test_expected_gain() {
        let reviews = vec![
            debug_review(0., true),
            debug_review(1., false),
            debug_review(10., false),
        ];
        let reviews = Reviews(reviews);
        dbg!(reviews.expected_gain());
        
    }
    
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
    fn debug_strength(){
        let stability = Duration::from_secs_f32(100. * 86400.);
        let time_passed = Duration::from_secs_f32(0. * 86400.);
        let x = calculate_left_memory(time_passed, stability);
        dbg!(x);
        
    }

    #[test]
    fn test_strength() {
        let stability = Duration::from_secs(86400);
        let days_passed = Duration::default();
        let recall_rate = Reviews::calculate_recall_rate(&days_passed, &stability);
        assert_eq!(recall_rate, 1.0);

        let days_passed = Duration::from_secs(86400);
        let recall_rate = Reviews::calculate_recall_rate(&days_passed, &stability);
        assert_eq!(recall_rate, 0.9);
    }
}
