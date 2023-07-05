use std::fmt::Display;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use crate::paths::get_cards_path;
use std::io::{self, ErrorKind};

use std::time::SystemTime;

pub fn current_time() -> Duration {
    system_time_as_unix_time(SystemTime::now()) + Duration::from_secs(86400 * 1)
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

pub fn view_cards_in_explorer() {
    open_folder_in_explorer(&get_cards_path()).unwrap()
}

fn open_folder_in_explorer(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).status()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).status()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(path).status()?;
    }

    Ok(())
}

pub fn get_last_modified(path: PathBuf) -> Duration {
    let metadata = std::fs::metadata(path).unwrap();
    let modified_time = metadata.modified().unwrap();
    let secs = modified_time
        .duration_since(UNIX_EPOCH)
        .map(|s| s.as_secs())
        .unwrap();
    Duration::from_secs(secs)
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
