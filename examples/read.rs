use lmdb_hash_builder::{visit_names, Universe};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected a .txt.zst file")?;
    let reader = BufReader::new(zstd::Decoder::new(File::open(path)?)?);
    let mut counts = BTreeMap::<Universe, usize>::new();
    visit_names(reader, |universe, name| {
        let _hash = universe.hash(name);
        *counts.entry(universe).or_default() += 1;
        Ok(())
    })?;
    for (universe, count) in counts {
        println!("{}: {count} names", universe.header());
    }
    Ok(())
}
