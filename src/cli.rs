use csv::Reader;
use std::error::Error;

use crate::{
    card::{Card, CardLocationCache, Meta, Side},
    categories::Category,
    media::AudioSource,
    paths::{get_import_csv, get_share_path},
};

fn empty_str_optional(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

struct CsvFields {
    front: String,
    back: String,
    front_url: Option<String>,
    back_url: Option<String>,
    front_img: Option<String>,
    back_img: Option<String>,
}

pub fn read_csv() -> Result<(), Box<dyn Error>> {
    let path = crate::paths::get_import_csv();
    if !path.exists() {
        return Ok(());
    }
    let mut reader = Reader::from_path(path)?;
    let mut data: Vec<Vec<String>> = Vec::new();

    for record in reader.records() {
        let record = record?;
        data.push(record.iter().map(|s| s.to_string()).collect());
    }

    for row in &data {
        let front = row[0].clone();
        let back = row[1].clone();
        let front_url = empty_str_optional(row[2].clone());
        let back_url = empty_str_optional(row[3].clone());
        let front_local = empty_str_optional(row[4].clone());
        let back_local = empty_str_optional(row[5].clone());

        let front_side = {
            let audio = AudioSource::new(front_local, front_url);
            Side { text: front, audio }
        };

        let back_side = {
            let audio = AudioSource::new(back_local, back_url);
            Side { text: back, audio }
        };

        let card = Card::new(front_side, back_side, Meta::default());
        card.save_new_card(
            &Category::import_category(),
            &mut CardLocationCache::default(),
        );
    }

    std::fs::rename(get_import_csv(), get_share_path().join("imported.csv")).unwrap();
    Ok(())
}
