use anyhow::Result;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Deserializer, Serialize};

use super::OPLOG_FILE_NAME;

/// SystemTime used to be serialized as a u64 of seconds, but is now a propper SystemTime struct.
/// This function will handle the old format gracefully.
fn unfailing_system_time_deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(SystemTime::deserialize(deserializer).unwrap_or(SystemTime::UNIX_EPOCH))
}

fn unix_epoch() -> SystemTime {
    SystemTime::UNIX_EPOCH
}

/// This tracks the head of the oplog, persisted in operations-log.toml.  
#[derive(Serialize, Deserialize, Debug)]
pub struct Oplog {
    /// This is the sha of the last oplog commit
    pub head_sha: Option<String>,
    /// The time when the last snapshot was created. Seconds since Epoch
    #[serde(
        deserialize_with = "unfailing_system_time_deserialize",
        default = "unix_epoch"
    )]
    pub modified_at: SystemTime,
}

impl Default for Oplog {
    fn default() -> Self {
        Self {
            head_sha: None,
            modified_at: SystemTime::UNIX_EPOCH,
        }
    }
}

pub struct OplogHandle {
    /// The path to the file containing the oplog head state.
    file_path: PathBuf,
}

impl OplogHandle {
    /// Creates a new concurrency-safe handle to the state of the oplog.
    pub fn new(base_path: &Path) -> Self {
        let file_path = base_path.join(OPLOG_FILE_NAME);
        Self { file_path }
    }

    /// Persists the oplog head for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_oplog_head(&self, sha: String) -> Result<()> {
        let mut oplog = self.read_file()?;
        oplog.head_sha = Some(sha);
        self.write_file(oplog)?;
        Ok(())
    }

    /// Gets the oplog head sha for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_oplog_head(&self) -> anyhow::Result<Option<String>> {
        let oplog = self.read_file()?;
        Ok(oplog.head_sha)
    }

    /// Gets the time when the last snapshot was created.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_modified_at(&self) -> anyhow::Result<SystemTime> {
        let oplog = self.read_file()?;
        Ok(oplog.modified_at)
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<Oplog> {
        if !self.file_path.exists() {
            return Ok(Oplog::default());
        }
        let mut file: File = File::open(self.file_path.as_path())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let oplog: Oplog =
            toml::from_str(&contents).map_err(|e| crate::reader::Error::ParseError {
                path: self.file_path.clone(),
                source: e,
            })?;

        Ok(oplog)
    }

    fn write_file(&self, oplog: Oplog) -> anyhow::Result<()> {
        let mut oplog = oplog;
        let now = std::time::SystemTime::now();
        oplog.modified_at = now;
        write(self.file_path.as_path(), &oplog)
    }
}

fn write<P: AsRef<Path>>(file_path: P, oplog: &Oplog) -> anyhow::Result<()> {
    let contents = toml::to_string(&oplog)?;
    crate::fs::write(file_path, contents)
}
