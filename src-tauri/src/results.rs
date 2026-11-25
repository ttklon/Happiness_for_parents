//! JSONL result storage with optional AES-256-GCM encryption.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitRecord {
    pub id: String,
    pub time: String,
    pub coin: String,
    pub address: String,
    pub phrase: String,
    pub source: String,
    pub balance: u64,
    pub received: u64,
}

#[derive(Debug, Serialize)]
pub struct EmptyRecord {
    pub time: String,
    pub coin: String,
    pub address: String,
    pub phrase: String,
}

fn write_line(path: &Path, line: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes())
        .and_then(|_| writeln!(file))
        .map_err(|e| e.to_string())
}

fn encode_line<T: Serialize>(value: &T, password: Option<&str>) -> Result<String, String> {
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    match password {
        Some(p) if !p.is_empty() => crate::crypto::encrypt_string(p, &json),
        _ => Ok(json),
    }
}

pub fn append_hit(path: &Path, record: &HitRecord, password: Option<&str>) -> Result<(), String> {
    write_line(path, &encode_line(record, password)?)
}

pub fn append_empty(path: &Path, record: &EmptyRecord, password: Option<&str>) -> Result<(), String> {
    write_line(path, &encode_line(record, password)?)
}

pub fn read_hits(path: &Path, password: Option<&str>) -> Result<Vec<HitRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let decoded = match password {
            Some(p) if !p.is_empty() => crate::crypto::decrypt_string(p, line)?,
            _ => line.to_string(),
        };
        if let Ok(record) = serde_json::from_str::<HitRecord>(&decoded) {
            out.push(record);
        }
    }
    Ok(out)
}

pub fn clear_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::write(path, "").map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
