



use std::io::{self, ErrorKind};

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

pub fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
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

pub fn open_file_with_vim(path: PathBuf) -> io::Result<()> {
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
