use lmdb_hash_builder::{build_lmdb, read_source, Table, SCHEMA};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Duration;

const SOURCE: &str = "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol";
const ASSETS: [&str; 3] = [
    "lol-hashes-v2-combined.zst",
    "lol-hashes-v2-wad.zst",
    "lol-hashes-v2-bin.zst",
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Usage: lmdb-hash-builder [OUTPUT [WORK]] [--offline]\n\nBuild LMDB databases with separate BIN tables. --offline uses existing source files in WORK.");
        return Ok(());
    }
    let offline = args.iter().any(|arg| arg == "--offline");
    let positional: Vec<_> = args
        .iter()
        .filter(|arg| arg.as_str() != "--offline")
        .collect();
    if positional.len() > 2 || positional.iter().any(|arg| arg.starts_with('-')) {
        return Err("expected [OUTPUT [WORK]] [--offline]; use --help".into());
    }
    let output = Path::new(positional.first().map_or("output", |arg| arg.as_str()));
    let work = Path::new(positional.get(1).map_or("work", |arg| arg.as_str()));
    fs::create_dir_all(output)?;
    fs::create_dir_all(work)?;
    if !offline {
        download_sources(work)?;
    }
    let fingerprint = fingerprint(work)?;
    let fingerprint_path = output.join("fingerprint-v2.sha256");
    if fs::read_to_string(&fingerprint_path).ok().as_deref() == Some(&fingerprint)
        && ASSETS.iter().all(|asset| output.join(asset).is_file())
    {
        eprintln!("Sources and format unchanged; all outputs present.");
        return Ok(());
    }
    let mut tables = Vec::new();
    for table in Table::ALL {
        let entries = read_source(
            BufReader::new(File::open(work.join(table.source()))?),
            table,
        )?;
        eprintln!("{}: {} records", table.name(), entries.len());
        tables.push((table, entries));
    }
    for (asset, selected) in [
        (ASSETS[0], tables.as_slice()),
        (ASSETS[1], &tables[..1]),
        (ASSETS[2], &tables[1..]),
    ] {
        let database_dir = tempfile::tempdir_in(output)?;
        build_lmdb(database_dir.path(), selected)?;
        let temporary = output.join(format!("{asset}.tmp"));
        let file = BufWriter::new(File::create(&temporary)?);
        let mut encoder = zstd::Encoder::new(file, 19)?;
        encoder.include_checksum(true)?;
        let mut data = BufReader::new(File::open(database_dir.path().join("data.mdb"))?);
        io::copy(&mut data, &mut encoder)?;
        let mut file = encoder.finish()?;
        file.flush()?;
        file.get_ref().sync_all()?;
        drop(file);
        fs::rename(temporary, output.join(asset))?;
        eprintln!("{asset}: {} bytes", fs::metadata(output.join(asset))?.len());
    }
    fs::write(fingerprint_path, fingerprint)?;
    Ok(())
}

fn download_sources(work: &Path) -> Result<(), Box<dyn Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;
    for table in Table::ALL {
        let temporary = work.join(format!("{}.tmp", table.source()));
        let mut file = BufWriter::new(File::create(&temporary)?);
        if table == Table::Wad {
            for part in 0u32.. {
                let url = format!("{SOURCE}/{}.{part}", table.source());
                eprintln!("Downloading {url}");
                let response = client.get(&url).send()?;
                if response.status() == reqwest::StatusCode::NOT_FOUND && part > 0 {
                    break;
                }
                copy_response(response, &mut file)?;
            }
        } else {
            let url = format!("{SOURCE}/{}", table.source());
            eprintln!("Downloading {url}");
            copy_response(client.get(url).send()?, &mut file)?;
        }
        file.flush()?;
        drop(file);
        fs::rename(temporary, work.join(table.source()))?;
    }
    Ok(())
}

fn copy_response(
    response: reqwest::blocking::Response,
    writer: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let bytes = response.error_for_status()?.bytes()?;
    if bytes.is_empty() {
        return Err("empty source response".into());
    }
    writer.write_all(&bytes)?;
    if !bytes.ends_with(b"\n") {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn fingerprint(work: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA);
    hasher.update(env!("CARGO_PKG_VERSION"));
    let mut buffer = [0u8; 64 * 1024];
    for table in Table::ALL {
        hasher.update(table.source());
        let mut file = File::open(work.join(table.source()))?;
        hasher.update(file.metadata()?.len().to_le_bytes());
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            hasher.update(&buffer[..length]);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}
