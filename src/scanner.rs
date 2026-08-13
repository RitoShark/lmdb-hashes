use crate::dictionary::KnownHashes;
use anyhow::{Context, Result};
use ritoshark::bin::{Bin, BinValue};
use ritoshark::io::Parse;
use ritoshark::wad::Wad;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanReport {
    pub league_root: String,
    pub wad_files: usize,
    pub wad_chunks: u64,
    pub bin_files: u64,
    pub parse_failures: u64,
    pub bin_hashes: Vec<Hash32Record>,
    pub wad_hashes: Vec<Hash64Record>,
    pub strings: Vec<StringRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hash32Record {
    pub hash: String,
    pub resolved: Option<String>,
    pub count: u64,
    pub kinds: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hash64Record {
    pub hash: String,
    pub resolved: Option<String>,
    pub count: u64,
    pub kinds: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StringRecord {
    pub value: String,
    pub count: u64,
    pub examples: Vec<String>,
}

#[derive(Default)]
struct Use {
    count: u64,
    kinds: BTreeSet<String>,
    examples: BTreeSet<String>,
}

#[derive(Default)]
struct ScanState {
    wad_files: usize,
    wad_chunks: u64,
    bin_files: u64,
    parse_failures: u64,
    bin: HashMap<u32, Use>,
    wad: HashMap<u64, Use>,
    strings: BTreeMap<String, Use>,
}

pub fn scan(root: &Path, known: &KnownHashes, scan_unknown_chunks: bool) -> Result<ScanReport> {
    let root = root
        .canonicalize()
        .with_context(|| format!("find League directory {}", root.display()))?;
    let mut state = ScanState::default();
    let files: Vec<PathBuf> = WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect();
    let wads: Vec<&PathBuf> = files.iter().filter(|path| is_wad(path)).collect();
    for (index, path) in wads.iter().enumerate() {
        eprintln!("[{}/{}] {}", index + 1, wads.len(), path.display());
        if let Err(error) = scan_wad(path, known, scan_unknown_chunks, &mut state) {
            eprintln!("  skipped: {error:#}");
            state.parse_failures += 1;
        }
    }
    for path in files.iter().filter(|path| is_bin(path)) {
        let data = match fs::read(path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        scan_bin(&data, &path.display().to_string(), &mut state);
    }
    Ok(finish(root, state, known))
}

fn scan_wad(
    path: &Path,
    known: &KnownHashes,
    scan_unknown: bool,
    state: &mut ScanState,
) -> Result<()> {
    let mut file = File::open(path)?;
    let wad = Wad::from_reader_toc(&mut file)?;
    state.wad_files += 1;
    state.wad_chunks += wad.chunks.len() as u64;
    for chunk in &wad.chunks {
        let path_name = known.wad.get(&chunk.path_hash);
        let context = format!("{}!{:016x}", path.display(), chunk.path_hash);
        use64(&mut state.wad, chunk.path_hash, "wad_chunk", &context);
        let resolved_bin =
            path_name.is_some_and(|name| name.to_ascii_lowercase().ends_with(".bin"));
        if !resolved_bin && !scan_unknown {
            continue;
        }
        let data = match wad.chunk_data_from(&mut file, chunk) {
            Ok(data) => data,
            Err(_) => continue,
        };
        if is_prop(&data) {
            let label = path_name.map(String::as_str).unwrap_or(&context);
            scan_bin(&data, label, state);
        }
    }
    Ok(())
}

fn scan_bin(data: &[u8], context: &str, state: &mut ScanState) {
    let bin = match Bin::from_bytes(data) {
        Ok(bin) => bin,
        Err(_) => {
            state.parse_failures += 1;
            return;
        }
    };
    state.bin_files += 1;
    for linked in &bin.linked {
        string_use(state, linked, context);
    }
    for entry in &bin.entries {
        use32(&mut state.bin, entry.path_hash, "entry", context);
        use32(&mut state.bin, entry.class_hash, "class", context);
        for (field, value) in &entry.fields {
            use32(&mut state.bin, *field, "field", context);
            scan_value(value, context, state);
        }
    }
    for patch in &bin.patches {
        use32(&mut state.bin, patch.key_hash, "patch_key", context);
        string_use(state, &patch.path, context);
        scan_value(&patch.value, context, state);
    }
}

fn scan_value(value: &BinValue, context: &str, state: &mut ScanState) {
    match value {
        BinValue::String(value) => string_use(state, value, context),
        BinValue::Hash(hash) => use32(&mut state.bin, *hash, "hash_value", context),
        BinValue::Link(hash) => use32(&mut state.bin, *hash, "link", context),
        BinValue::File(hash) => use64(&mut state.wad, *hash, "file", context),
        BinValue::List { items, .. } => {
            for item in items {
                scan_value(item, context, state);
            }
        }
        BinValue::Map { entries, .. } => {
            for (key, value) in entries {
                scan_value(key, context, state);
                scan_value(value, context, state);
            }
        }
        BinValue::Pointer { class, fields } | BinValue::Embed { class, fields } => {
            if *class != 0 {
                use32(&mut state.bin, *class, "class", context);
            }
            for (field, value) in fields {
                use32(&mut state.bin, *field, "field", context);
                scan_value(value, context, state);
            }
        }
        BinValue::Option {
            value: Some(value), ..
        } => scan_value(value, context, state),
        _ => {}
    }
}

fn use32(map: &mut HashMap<u32, Use>, hash: u32, kind: &str, context: &str) {
    if hash == 0 {
        return;
    }
    add_use(map.entry(hash).or_default(), kind, context);
}

fn use64(map: &mut HashMap<u64, Use>, hash: u64, kind: &str, context: &str) {
    if hash == 0 {
        return;
    }
    add_use(map.entry(hash).or_default(), kind, context);
}

fn string_use(state: &mut ScanState, value: &str, context: &str) {
    if value.is_empty() {
        return;
    }
    add_use(
        state.strings.entry(value.to_owned()).or_default(),
        "string",
        context,
    );
}

fn add_use(record: &mut Use, kind: &str, context: &str) {
    record.count += 1;
    record.kinds.insert(kind.to_owned());
    if record.examples.len() < 5 {
        record.examples.insert(context.to_owned());
    }
}

fn finish(root: PathBuf, state: ScanState, known: &KnownHashes) -> ScanReport {
    let mut bin_hashes: Vec<_> = state
        .bin
        .into_iter()
        .map(|(hash, value)| Hash32Record {
            hash: format!("{hash:08x}"),
            resolved: known.bin.get(&hash).cloned(),
            count: value.count,
            kinds: value.kinds.into_iter().collect(),
            examples: value.examples.into_iter().collect(),
        })
        .collect();
    bin_hashes.sort_by(|a, b| a.hash.cmp(&b.hash));
    let mut wad_hashes: Vec<_> = state
        .wad
        .into_iter()
        .map(|(hash, value)| Hash64Record {
            hash: format!("{hash:016x}"),
            resolved: known.wad.get(&hash).cloned(),
            count: value.count,
            kinds: value.kinds.into_iter().collect(),
            examples: value.examples.into_iter().collect(),
        })
        .collect();
    wad_hashes.sort_by(|a, b| a.hash.cmp(&b.hash));
    let strings = state
        .strings
        .into_iter()
        .map(|(value, usage)| StringRecord {
            value,
            count: usage.count,
            examples: usage.examples.into_iter().collect(),
        })
        .collect();
    ScanReport {
        league_root: root.display().to_string(),
        wad_files: state.wad_files,
        wad_chunks: state.wad_chunks,
        bin_files: state.bin_files,
        parse_failures: state.parse_failures,
        bin_hashes,
        wad_hashes,
        strings,
    }
}

pub fn write_report(path: &Path, report: &ScanReport) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(report)?)?;
    let unresolved_bin = report
        .bin_hashes
        .iter()
        .filter(|entry| entry.resolved.is_none())
        .count();
    let unresolved_wad = report
        .wad_hashes
        .iter()
        .filter(|entry| entry.resolved.is_none())
        .count();
    eprintln!(
        "Scanned {} BINs in {} WADs: {} strings, {} unresolved BIN hashes, {} unresolved WAD hashes",
        report.bin_files,
        report.wad_files,
        report.strings.len(),
        unresolved_bin,
        unresolved_wad
    );
    Ok(())
}

pub fn write_unresolved(path: &Path, report: &ScanReport) -> Result<()> {
    let mut output = String::new();
    for entry in report
        .bin_hashes
        .iter()
        .filter(|entry| entry.resolved.is_none())
    {
        output.push_str(&entry.hash);
        output.push('\n');
    }
    fs::write(path, output)?;
    Ok(())
}

fn is_wad(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.ends_with(".wad") || name.ends_with(".wad.client")
        })
}

fn is_bin(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
}

fn is_prop(data: &[u8]) -> bool {
    data.starts_with(b"PROP") || data.starts_with(b"PTCH")
}
