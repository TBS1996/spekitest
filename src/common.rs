use std::fmt::{Debug, Display};

use std::io::{self, ErrorKind};

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::card::CardLocationCache;

pub fn current_time() -> Duration {
    system_time_as_unix_time(SystemTime::now())
}

pub fn system_time_as_unix_time(time: SystemTime) -> Duration {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
}

/// Safe way to truncate string.
pub fn truncate_string(input: String, max_len: usize) -> String {
    let mut graphemes = input.chars();
    let mut result = String::new();

    for _ in 0..max_len {
        if let Some(c) = graphemes.next() {
            result.push(c);
        } else {
            break;
        }
    }

    result
}

pub mod serde_duration_as_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = duration.as_secs();
        serializer.serialize_u64(secs)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

pub fn open_file_with_vim(path: &Path) -> io::Result<()> {
    let status = Command::new("vim").arg(path).status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::Other,
            "Failed to open file with vim",
        ))
    }
}

pub trait MenuItem: Display {
    fn action(&self) -> Box<dyn FnMut() -> bool>;
}

/// Randomizing a vector.
/// Not importing rand cause im trying to keep dependency-count low.
pub fn randvec<T>(mut v: Vec<T>) -> Vec<T> {
    let veclen = v.len();
    let mut randomized = Vec::with_capacity(veclen);

    for i in 0..veclen {
        let now = current_time();
        let veclen = v.len();
        let micros = now.as_micros();
        let popped = v.remove(i * micros as usize % veclen);
        randomized.push(popped);
    }
    randomized
}

type GetChildren<T> = Box<dyn FnMut(&T) -> Vec<T>>;

type GetKids<T> = Box<dyn FnMut(T) -> Vec<T>>;

pub fn visit_check_any_match_predicate<T: Sized>(
    ty: &T,
    predicate: &mut Box<dyn FnMut(&T) -> bool>,
    get_children: &mut GetChildren<T>,
) -> bool {
    let kids = get_children(ty);
    for kid in &kids {
        if predicate(kid) || visit_check_any_match_predicate(kid, predicate, get_children) {
            return true;
        }
    }
    false
}

pub fn visit_collect_all_descendants<T: Sized + Clone + PartialEq + Debug>(
    ty: T,
    get_children: &mut Box<dyn FnMut(T, &mut CardLocationCache) -> Vec<T>>,
    cache: &mut CardLocationCache,
) -> Result<Vec<T>, T> {
    let mut descendants = vec![];
    visit_collect_all_descendants_inner(ty, &mut descendants, get_children, cache)?;
    Ok(descendants)
}

fn visit_collect_all_descendants_inner<T: Sized + Clone + PartialEq + Debug>(
    ty: T,
    descendants: &mut Vec<T>,
    get_children: &mut Box<dyn FnMut(T, &mut CardLocationCache) -> Vec<T>>,
    cache: &mut CardLocationCache,
) -> Result<(), T> {
    let kids = get_children(ty, cache);
    for kid in &kids {
        if !descendants.contains(kid) {
            descendants.push(kid.to_owned());
            visit_collect_all_descendants_inner(kid.clone(), descendants, get_children, cache)?;
        } else {
            //return Err(kid.to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {
        let input_vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let randomized = randvec(input_vec);
        dbg!(randomized);
    }
}
