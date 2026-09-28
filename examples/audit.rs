use lmdb_hash_builder::Universe;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let work = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "work".into()));
    let mut total_mismatches = 0;
    for universe in Universe::ALL {
        let mut count = 0;
        let mut mismatches = 0;
        for (index, line) in BufReader::new(File::open(work.join(universe.source()))?)
            .lines()
            .enumerate()
        {
            let line = line?;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (hash, name) = line.split_once(' ').ok_or("invalid source line")?;
            let expected = u64::from_str_radix(hash, 16)?;
            let actual = universe.hash(name);
            count += 1;
            if actual != expected {
                mismatches += 1;
                if mismatches <= 10 {
                    eprintln!(
                        "{}:{}: {expected:016x} != {actual:016x} {name:?}",
                        universe.source(),
                        index + 1
                    );
                }
            }
        }
        println!(
            "{}: {count} records, {mismatches} mismatches",
            universe.header()
        );
        total_mismatches += mismatches;
    }
    if total_mismatches > 0 {
        return Err(format!("{total_mismatches} hashes cannot be reconstructed").into());
    }
    Ok(())
}
