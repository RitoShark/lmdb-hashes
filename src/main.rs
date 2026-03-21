use heed::types::{Bytes, Str};
use heed::{Database, EnvOpenOptions};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const CDRAGON_BASE: &str =
    "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol/";

const GAME_HASH_PARTS: &[&str] = &[
    "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol/hashes.game.txt.0",
    "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol/hashes.game.txt.1",
];

const BIN_HASH_FILES: &[&str] = &[
    "hashes.binhashes.txt",
    "hashes.binentries.txt",
    "hashes.binfields.txt",
    "hashes.bintypes.txt",
];

const LMDB_MAP_SIZE: usize = 2 * 1024 * 1024 * 1024; // 2 GB virtual

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output_dir = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("output"));
    let work_dir = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("work"));

    fs::create_dir_all(&work_dir).expect("Failed to create work directory");
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    eprintln!("=== LMDB Hash Builder ===");

    eprintln!("\n[1/5] Downloading hash files from CommunityDragon...");
    download_game_hashes(&work_dir);
    download_bin_hashes(&work_dir);

    eprintln!("\n[2/5] Computing fingerprint...");
    let fingerprint = compute_fingerprint(&work_dir);
    let fingerprint_path = output_dir.join("fingerprint.sha256");
    if fingerprint_path.exists() {
        let old = fs::read_to_string(&fingerprint_path).unwrap_or_default();
        if old.trim() == fingerprint {
            eprintln!("Hashes unchanged — skipping LMDB build.");
            std::process::exit(0);
        }
    }
    eprintln!("Fingerprint: {}", fingerprint);

    eprintln!("\n[3/5] Parsing hash files...");
    let wad_entries = {
        let game_path = work_dir.join("hashes.game.txt");
        let mut entries = parse_hash_file(&game_path, 16);
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.dedup_by(|a, b| a.0 == b.0);
        entries
    };
    let bin_entries = {
        let mut all: Vec<(Vec<u8>, String)> = Vec::new();
        for filename in BIN_HASH_FILES {
            let path = work_dir.join(filename);
            if path.exists() {
                all.extend(parse_hash_file(&path, 8));
            }
        }
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all.dedup_by(|a, b| a.0 == b.0);
        all
    };
    eprintln!("Parsed {} WAD hashes, {} BIN hashes", wad_entries.len(), bin_entries.len());

    eprintln!("\n[4/5] Building LMDBs...");

    // 1) Combined — both WAD + BIN
    let combined_dir = output_dir.join("lol-hashes-combined.lmdb");
    build_lmdb_combined(&combined_dir, &wad_entries, &bin_entries);
    eprintln!("  Combined: {} WAD + {} BIN hashes", wad_entries.len(), bin_entries.len());

    // 2) WAD-only
    let wad_dir = output_dir.join("lol-hashes-wad.lmdb");
    build_lmdb_single(&wad_dir, "wad", &wad_entries);
    eprintln!("  WAD-only: {} hashes", wad_entries.len());

    // 3) BIN-only
    let bin_dir = output_dir.join("lol-hashes-bin.lmdb");
    build_lmdb_single(&bin_dir, "bin", &bin_entries);
    eprintln!("  BIN-only: {} hashes", bin_entries.len());

    eprintln!("\n[5/5] Compressing with zstd...");

    let variants = [
        (&combined_dir, "lol-hashes-combined.zst"),
        (&wad_dir, "lol-hashes-wad.zst"),
        (&bin_dir, "lol-hashes-bin.zst"),
    ];

    for (lmdb_dir, zst_name) in &variants {
        let data_mdb = lmdb_dir.join("data.mdb");
        let compressed_path = output_dir.join(zst_name);
        compress_zstd(&data_mdb, &compressed_path);

        let size = fs::metadata(&compressed_path).map(|m| m.len()).unwrap_or(0);
        eprintln!("  {} → {:.1} MB", zst_name, size as f64 / 1024.0 / 1024.0);
    }

    fs::write(&fingerprint_path, &fingerprint).expect("Failed to write fingerprint");

    eprintln!("\nDone!");
}

fn download_game_hashes(work_dir: &Path) {
    let output = work_dir.join("hashes.game.txt");
    let mut out_file = fs::File::create(&output).expect("Failed to create hashes.game.txt");

    for (i, url) in GAME_HASH_PARTS.iter().enumerate() {
        eprint!("  Downloading part {}/{}...", i + 1, GAME_HASH_PARTS.len());
        let response = reqwest::blocking::get(*url)
            .unwrap_or_else(|e| panic!("Failed to download {}: {}", url, e));

        if !response.status().is_success() {
            panic!("HTTP {} for {}", response.status(), url);
        }

        let bytes = response.bytes().expect("Failed to read response body");
        out_file.write_all(&bytes).expect("Failed to write hash data");
        eprintln!(" {:.1} MB", bytes.len() as f64 / 1024.0 / 1024.0);
    }
}

fn download_bin_hashes(work_dir: &Path) {
    for filename in BIN_HASH_FILES {
        let url = format!("{}{}", CDRAGON_BASE, filename);
        let output = work_dir.join(filename);

        eprint!("  Downloading {}...", filename);
        let response = reqwest::blocking::get(&url)
            .unwrap_or_else(|e| panic!("Failed to download {}: {}", url, e));

        if !response.status().is_success() {
            panic!("HTTP {} for {}", response.status(), url);
        }

        let bytes = response.bytes().expect("Failed to read response body");
        fs::write(&output, &bytes).expect("Failed to write hash file");
        eprintln!(" {:.1} KB", bytes.len() as f64 / 1024.0);
    }
}

fn compute_fingerprint(work_dir: &Path) -> String {
    let mut hasher = Sha256::new();
    let mut files: Vec<PathBuf> = fs::read_dir(work_dir)
        .expect("Failed to read work dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "txt").unwrap_or(false))
        .collect();
    files.sort();

    for path in &files {
        let data = fs::read(path).expect("Failed to read hash file");
        hasher.update(&data);
    }

    hex::encode(hasher.finalize())
}

fn prepare_lmdb_dir(lmdb_dir: &Path) {
    if lmdb_dir.exists() {
        fs::remove_dir_all(lmdb_dir).expect("Failed to clean old LMDB");
    }
    fs::create_dir_all(lmdb_dir).expect("Failed to create LMDB dir");
}

fn build_lmdb_combined(lmdb_dir: &Path, wad_entries: &[(Vec<u8>, String)], bin_entries: &[(Vec<u8>, String)]) {
    prepare_lmdb_dir(lmdb_dir);

    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(LMDB_MAP_SIZE)
            .max_dbs(2)
            .open(lmdb_dir)
            .expect("Failed to open LMDB environment")
    };

    let mut wtxn = env.write_txn().expect("Failed to start write txn");

    let wad_db: Database<Bytes, Str> = env
        .create_database(&mut wtxn, Some("wad"))
        .expect("Failed to create wad database");
    for (key, value) in wad_entries {
        wad_db.put(&mut wtxn, key, value).expect("Failed to insert WAD hash");
    }

    let bin_db: Database<Bytes, Str> = env
        .create_database(&mut wtxn, Some("bin"))
        .expect("Failed to create bin database");
    for (key, value) in bin_entries {
        bin_db.put(&mut wtxn, key, value).expect("Failed to insert BIN hash");
    }

    wtxn.commit().expect("Failed to commit transaction");
    env.force_sync().expect("Failed to sync LMDB");
}

fn build_lmdb_single(lmdb_dir: &Path, db_name: &str, entries: &[(Vec<u8>, String)]) {
    prepare_lmdb_dir(lmdb_dir);

    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(LMDB_MAP_SIZE)
            .max_dbs(1)
            .open(lmdb_dir)
            .expect("Failed to open LMDB environment")
    };

    let mut wtxn = env.write_txn().expect("Failed to start write txn");
    let db: Database<Bytes, Str> = env
        .create_database(&mut wtxn, Some(db_name))
        .expect("Failed to create database");

    for (key, value) in entries {
        db.put(&mut wtxn, key, value).expect("Failed to insert hash");
    }

    wtxn.commit().expect("Failed to commit transaction");
    env.force_sync().expect("Failed to sync LMDB");
}

/// Parse a hash text file into (key_bytes, path_string) pairs.
/// `hex_len` is 16 for u64 WAD hashes, 8 for u32 BIN hashes.
fn parse_hash_file(path: &Path, hex_len: usize) -> Vec<(Vec<u8>, String)> {
    let file = fs::File::open(path).unwrap_or_else(|e| panic!("Failed to open {}: {}", path.display(), e));
    let reader = BufReader::with_capacity(1024 * 1024, file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.len() < hex_len + 2 {
            continue;
        }
        let hex_part = &line[..hex_len];
        let value = line[hex_len..].trim();
        if value.is_empty() {
            continue;
        }

        let key_bytes = match hex_len {
            16 => {
                match u64::from_str_radix(hex_part, 16) {
                    Ok(v) => v.to_be_bytes().to_vec(),
                    Err(_) => continue,
                }
            }
            8 => {
                match u32::from_str_radix(hex_part, 16) {
                    Ok(v) => v.to_be_bytes().to_vec(),
                    Err(_) => continue,
                }
            }
            _ => continue,
        };

        entries.push((key_bytes, value.to_string()));
    }

    entries
}

fn compress_zstd(input: &Path, output: &Path) {
    let data = fs::read(input).expect("Failed to read LMDB data.mdb");
    let compressed = zstd::encode_all(data.as_slice(), 19).expect("Failed to compress with zstd");
    fs::write(output, &compressed).expect("Failed to write compressed file");
}
