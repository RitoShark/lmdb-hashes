use lmdb_hash_builder::{read_source, write_tables, Universe, MAGIC};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Duration;

const SOURCE: &str = "https://raw.githubusercontent.com/CommunityDragon/Data/master/hashes/lol";
const ASSETS: [&str; 3] = [
    "lol-names-v2-combined.txt.zst",
    "lol-names-v2-wad.txt.zst",
    "lol-names-v2-bin.txt.zst",
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Usage: lmdb-hash-builder [OUTPUT [WORK]] [--offline]\n\nBuild version 2 name tables. --offline uses existing source files in WORK.");
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
    for universe in Universe::ALL {
        let names = read_source(
            BufReader::new(File::open(work.join(universe.source()))?),
            universe,
        )?;
        eprintln!("{}: {} names", universe.header(), names.len());
        tables.push((universe, names));
    }
    for (asset, selected) in [
        (ASSETS[0], tables.as_slice()),
        (ASSETS[1], &tables[..1]),
        (ASSETS[2], &tables[1..]),
    ] {
        let temporary = output.join(format!("{asset}.tmp"));
        let file = BufWriter::new(File::create(&temporary)?);
        let mut encoder = zstd::Encoder::new(file, 19)?;
        encoder.include_checksum(true)?;
        write_tables(&mut encoder, selected)?;
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
    for universe in Universe::ALL {
        let temporary = work.join(format!("{}.tmp", universe.source()));
        let mut file = BufWriter::new(File::create(&temporary)?);
        if universe == Universe::Game {
            for part in 0u32.. {
                let url = format!("{SOURCE}/{}.{part}", universe.source());
                eprintln!("Downloading {url}");
                let response = client.get(&url).send()?;
                if response.status() == reqwest::StatusCode::NOT_FOUND && part > 0 {
                    break;
                }
                copy_response(response, &mut file)?;
            }
        } else {
            let url = format!("{SOURCE}/{}", universe.source());
            eprintln!("Downloading {url}");
            copy_response(client.get(url).send()?, &mut file)?;
        }
        file.flush()?;
        drop(file);
        fs::rename(temporary, work.join(universe.source()))?;
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
    hasher.update(MAGIC);
    hasher.update(env!("CARGO_PKG_VERSION"));
    let mut buffer = [0u8; 64 * 1024];
    for universe in Universe::ALL {
        hasher.update(universe.source());
        let mut file = File::open(work.join(universe.source()))?;
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
