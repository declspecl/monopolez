use std::fs::{
    File,
    OpenOptions,
};
use std::hash::Hasher;
use std::io::{
    BufRead,
    BufReader,
    Read,
    Seek,
    SeekFrom,
    Write,
};
use std::path::Path;

use anyhow::{
    Context,
    Result,
    ensure,
};
use serde_json::Value;

pub struct Checkpoint {
    file: File,
    pub entries: Vec<Value>,
    pub discarded_tail_bytes: u64,
}

pub fn executable_fingerprint() -> Result<String> {
    let mut executable = File::open(std::env::current_exe()?)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = executable.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.write(&buffer[..count]);
    }
    Ok(format!("default_hasher_64:{:016x}", hasher.finish()))
}

impl Checkpoint {
    pub fn open(
        path: &Path,
        header: &Value,
    ) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
        file.try_lock().context("checkpoint is already in use or cannot be locked")?;
        if file.metadata()?.len() == 0 {
            let mut bytes = serde_json::to_vec(header)?;
            bytes.push(b'\n');
            file.write_all(&bytes)?;
            file.sync_data()?;
        }
        file.seek(SeekFrom::Start(0))?;
        let mut reader = BufReader::new(&mut file);
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line)?;
        ensure!(line.last() == Some(&b'\n'), "checkpoint header is incomplete");
        let saved_header: Value = serde_json::from_slice(&line).context("invalid checkpoint header")?;
        ensure!(&saved_header == header, "checkpoint inputs or executable differ from this run");
        let mut valid_bytes = line.len() as u64;
        let mut entries = Vec::new();
        let discarded_tail_bytes = loop {
            line.clear();
            let count = reader.read_until(b'\n', &mut line)?;
            if count == 0 {
                break 0;
            }
            if line.last() != Some(&b'\n') {
                break count as u64;
            }
            entries.push(serde_json::from_slice(&line).context("invalid completed checkpoint record")?);
            valid_bytes += count as u64;
        };
        drop(reader);
        file.seek(SeekFrom::Start(valid_bytes))?;
        Ok(Self { file, entries, discarded_tail_bytes })
    }

    pub fn recover_tail(&mut self) -> Result<()> {
        if self.discarded_tail_bytes > 0 {
            let valid_bytes = self.file.stream_position()?;
            self.file.set_len(valid_bytes)?;
            self.file.sync_data()?;
            self.discarded_tail_bytes = 0;
        }
        Ok(())
    }

    pub fn append(
        &mut self,
        entry: &Value,
    ) -> Result<()> {
        ensure!(self.discarded_tail_bytes == 0, "recover the checkpoint tail before appending");
        let mut bytes = serde_json::to_vec(entry)?;
        bytes.push(b'\n');
        self.file.write_all(&bytes)?;
        self.file.sync_data()?;
        Ok(())
    }
}
