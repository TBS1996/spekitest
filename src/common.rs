use std::fmt::{Debug, Display};

use std::io::{self, ErrorKind};

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::frontend::move_far_left;

pub fn current_time() -> Duration {
    system_time_as_unix_time(SystemTime::now()) // + Duration::from_secs(8644000)
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

pub fn double_vec<T: Clone>(vec: Vec<T>) -> Vec<T> {
    let mut output = vec![];
    for v in vec {
        output.push(v.clone());
        output.push(v);
    }
    output
}

pub fn interpolate(input: Vec<f64>) -> Vec<f64> {
    if input.len() < 2 {
        return input.to_vec();
    }

    let mut result = Vec::new();

    for window in input.windows(2) {
        let start = window[0];
        let end = window[1];
        let diff = end - start;

        // For this window, generate the interpolated values
        for i in 0..=diff as usize {
            result.push(start + i as f64);
        }
    }

    // Push the last value, as the window iteration will miss it
    result.push(*input.last().unwrap());

    result
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
