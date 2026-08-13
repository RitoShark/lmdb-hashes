use anyhow::{Context, Result};
use heed::types::{Bytes, Str};
use heed::{Database, EnvFlags, EnvOpenOptions};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Default)]
pub struct KnownHashes {
    pub bin: HashMap<u32, String>,
    pub wad: HashMap<u64, String>,
}

pub fn load_known_hashes(paths: &[PathBuf], meta: Option<&Path>) -> Result<KnownHashes> {
    let mut known = KnownHashes::default();
    for path in expand_files(paths) {
        load_hash_file(&path, &mut known)?;
    }
    for path in expand_lmdb_dirs(paths) {
        load_lmdb(&path, &mut known)?;
    }
    if let Some(path) = meta {
        let value: Value = serde_json::from_slice(&fs::read(path)?)?;
        collect_meta_hashes(&value, &mut known.bin);
    }
    Ok(known)
}

pub fn load_candidates(paths: &[PathBuf], meta: Option<&Path>) -> Result<BTreeSet<String>> {
    let mut values = BTreeSet::new();
    for path in expand_files(paths) {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("read dictionary {}", path.display()))?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let value = hash_line_value(line).unwrap_or(line);
            enroll(value, &mut values);
        }
    }
    if let Some(path) = meta {
        let value: Value = serde_json::from_slice(&fs::read(path)?)?;
        collect_meta_names(&value, &mut values);
    }
    Ok(values)
}

fn expand_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir() {
            files.extend(
                WalkDir::new(path)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_file())
                    .map(|entry| entry.into_path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
                    }),
            );
        }
    }
    files
}

fn expand_lmdb_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = BTreeSet::new();
    for path in paths {
        if path.is_dir() && path.join("data.mdb").is_file() {
            dirs.insert(path.clone());
        }
        if path.is_dir() {
            for entry in WalkDir::new(path)
                .max_depth(4)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file() && entry.file_name() == "data.mdb" {
                    if let Some(parent) = entry.path().parent() {
                        dirs.insert(parent.to_path_buf());
                    }
                }
            }
        }
    }
    dirs.into_iter().collect()
}

fn load_lmdb(path: &Path, known: &mut KnownHashes) -> Result<()> {
    let mut options = EnvOpenOptions::new();
    options.max_dbs(2);
    unsafe {
        options.flags(EnvFlags::READ_ONLY | EnvFlags::NO_LOCK);
    }
    let env =
        unsafe { options.open(path) }.with_context(|| format!("open LMDB {}", path.display()))?;
    let txn = env.read_txn()?;
    for name in ["bin", "wad"] {
        let Some(db): Option<Database<Bytes, Str>> = env.open_database(&txn, Some(name))? else {
            continue;
        };
        for entry in db.iter(&txn)? {
            let (key, value) = entry?;
            match (name, key.len()) {
                ("bin", 4) => {
                    let hash = u32::from_be_bytes(key.try_into().expect("four-byte key"));
                    known.bin.entry(hash).or_insert_with(|| value.to_owned());
                }
                ("wad", 8) => {
                    let hash = u64::from_be_bytes(key.try_into().expect("eight-byte key"));
                    known.wad.entry(hash).or_insert_with(|| value.to_owned());
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn load_hash_file(path: &Path, known: &mut KnownHashes) -> Result<()> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read hashes {}", path.display()))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((hex, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let hex = hex.trim_start_matches("0x");
        if hex.len() <= 8 {
            if let Ok(hash) = u32::from_str_radix(hex, 16) {
                known
                    .bin
                    .entry(hash)
                    .or_insert_with(|| value.trim().to_owned());
            }
        } else if hex.len() <= 16 {
            if let Ok(hash) = u64::from_str_radix(hex, 16) {
                known
                    .wad
                    .entry(hash)
                    .or_insert_with(|| value.trim().to_owned());
            }
        }
    }
    Ok(())
}

fn collect_meta_hashes(value: &Value, output: &mut HashMap<u32, String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if let Some(hash) = parse_hash(key) {
                    if let Some(name) = child.get("name").and_then(Value::as_str) {
                        output.entry(hash).or_insert_with(|| name.to_owned());
                    }
                }
                collect_meta_hashes(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_meta_hashes(child, output);
            }
        }
        _ => {}
    }
}

fn collect_meta_names(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(Value::as_str) {
                enroll(name, output);
            }
            for child in map.values() {
                collect_meta_names(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_meta_names(child, output);
            }
        }
        _ => {}
    }
}

fn parse_hash(value: &str) -> Option<u32> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    (value.len() <= 8)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

fn hash_line_value(line: &str) -> Option<&str> {
    let (head, tail) = line.split_once(char::is_whitespace)?;
    let head = head.trim_start_matches("0x");
    (matches!(head.len(), 8 | 16) && head.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(tail.trim())
}

pub fn enroll(value: &str, output: &mut BTreeSet<String>) {
    let value = value.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if value.is_empty() || value.len() > 512 {
        return;
    }
    output.insert(value.to_owned());
    let normalized = value.replace('\\', "/");
    output.insert(normalized.clone());
    for part in normalized.split(['/', '.', ':']) {
        if !part.is_empty() {
            output.insert(part.to_owned());
            if let Some(stem) = part.rsplit_once('.').map(|v| v.0) {
                output.insert(stem.to_owned());
            }
        }
    }
}
