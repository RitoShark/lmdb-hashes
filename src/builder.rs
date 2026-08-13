use anyhow::{bail, Context, Result};
use heed::types::{Bytes, Str};
use heed::{Database, EnvOpenOptions};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const BASE: &str = "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol/";
const BIN_FILES: &[&str] = &[
    "hashes.binhashes.txt",
    "hashes.binentries.txt",
    "hashes.binfields.txt",
    "hashes.bintypes.txt",
];

pub fn run(output: &Path, work: &Path) -> Result<()> {
    fs::create_dir_all(work)?;
    fs::create_dir_all(output)?;
    download_game(work)?;
    for name in BIN_FILES {
        download(&format!("{BASE}{name}"), &work.join(name))?;
    }
    let fingerprint = fingerprint(work)?;
    let fingerprint_path = output.join("fingerprint.sha256");
    if fs::read_to_string(&fingerprint_path)
        .ok()
        .is_some_and(|old| old.trim() == fingerprint)
    {
        eprintln!("Hashes unchanged; skipping LMDB build");
        return Ok(());
    }
    let mut wad = parse(&work.join("hashes.game.txt"), 16)?;
    let mut bin = Vec::new();
    for name in BIN_FILES {
        bin.extend(parse(&work.join(name), 8)?);
    }
    normalize(&mut wad);
    normalize(&mut bin);
    build(
        &output.join("lol-hashes-combined.lmdb"),
        Some(&wad),
        Some(&bin),
    )?;
    build(&output.join("lol-hashes-wad.lmdb"), Some(&wad), None)?;
    build(&output.join("lol-hashes-bin.lmdb"), None, Some(&bin))?;
    for (dir, name) in [
        ("lol-hashes-combined.lmdb", "lol-hashes-combined.zst"),
        ("lol-hashes-wad.lmdb", "lol-hashes-wad.zst"),
        ("lol-hashes-bin.lmdb", "lol-hashes-bin.zst"),
    ] {
        let data = fs::read(output.join(dir).join("data.mdb"))?;
        fs::write(output.join(name), zstd::encode_all(data.as_slice(), 19)?)?;
    }
    fs::write(fingerprint_path, fingerprint)?;
    eprintln!("Built {} WAD and {} BIN hashes", wad.len(), bin.len());
    Ok(())
}

fn download_game(work: &Path) -> Result<()> {
    let mut output = fs::File::create(work.join("hashes.game.txt"))?;
    for part in 0u32.. {
        let url = format!("{BASE}hashes.game.txt.{part}");
        let response = reqwest::blocking::get(&url).with_context(|| format!("download {url}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            if part == 0 {
                bail!("no CommunityDragon game hash parts found");
            }
            return Ok(());
        }
        response.error_for_status_ref()?;
        output.write_all(&response.bytes()?)?;
    }
    unreachable!()
}

fn download(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)?.error_for_status()?;
    fs::write(path, response.bytes()?)?;
    Ok(())
}

fn fingerprint(work: &Path) -> Result<String> {
    let mut files: Vec<PathBuf> = fs::read_dir(work)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();
    let mut hasher = Sha256::new();
    for path in files {
        hasher.update(fs::read(path)?);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn parse(path: &Path, width: usize) -> Result<Vec<(Vec<u8>, String)>> {
    let reader = BufReader::new(fs::File::open(path)?);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.starts_with('#') || line.len() <= width {
            continue;
        }
        let value = line[width..].trim();
        let key = match width {
            8 => u32::from_str_radix(&line[..width], 16)
                .ok()
                .map(|v| v.to_be_bytes().to_vec()),
            16 => u64::from_str_radix(&line[..width], 16)
                .ok()
                .map(|v| v.to_be_bytes().to_vec()),
            _ => None,
        };
        if let Some(key) = key.filter(|_| !value.is_empty()) {
            entries.push((key, value.to_owned()));
        }
    }
    Ok(entries)
}

fn normalize(entries: &mut Vec<(Vec<u8>, String)>) {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|a, b| a.0 == b.0);
}

fn build(
    path: &Path,
    wad: Option<&[(Vec<u8>, String)]>,
    bin: Option<&[(Vec<u8>, String)]>,
) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(2 * 1024 * 1024 * 1024)
            .max_dbs(2)
            .open(path)?
    };
    let mut txn = env.write_txn()?;
    if let Some(entries) = wad {
        let db: Database<Bytes, Str> = env.create_database(&mut txn, Some("wad"))?;
        for (key, value) in entries {
            db.put(&mut txn, key, value)?;
        }
    }
    if let Some(entries) = bin {
        let db: Database<Bytes, Str> = env.create_database(&mut txn, Some("bin"))?;
        for (key, value) in entries {
            db.put(&mut txn, key, value)?;
        }
    }
    txn.commit()?;
    env.force_sync()?;
    Ok(())
}
