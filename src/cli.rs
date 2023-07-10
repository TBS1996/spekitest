use std::error::Error;

use crate::{
    card::{Card, CardCache, Meta, Side},
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
