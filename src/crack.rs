use crate::dictionary::{enroll, load_candidates};
use crate::hash::{fnv1a, xxh64};
use crate::scanner::ScanReport;
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Kind {
    Bin,
    Wad,
}

pub fn run(
    targets_path: &Path,
    dictionaries: &[PathBuf],
    meta: Option<&Path>,
    report_path: Option<&Path>,
    depth: usize,
    output: &Path,
) -> Result<()> {
    let targets = load_targets(targets_path)?;
    if targets.is_empty() {
        bail!("no targets found in {}", targets_path.display());
    }
    let mut candidates = load_candidates(dictionaries, meta)?;
    let report = report_path.map(read_report).transpose()?;
    if let Some(report) = &report {
        for string in &report.strings {
            enroll(&string.value, &mut candidates);
        }
    }
    let mut results = BTreeMap::new();
    let mut remaining = targets.clone();
    test_candidates(&candidates, &mut remaining, &mut results);
    let tokens = tokens(&candidates);
    for level in 1..=depth.min(4) {
        if remaining.is_empty() {
            break;
        }
        let generated = generate_level(&candidates, &tokens, level);
        eprintln!(
            "Guess pass {level}: {} candidates, {} targets remain",
            generated.len(),
            remaining.len()
        );
        test_candidates(&generated, &mut remaining, &mut results);
    }
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for (target, value) in &results {
        text.push_str(&format!("{} {}\n", target_text(*target), value));
    }
    fs::write(output, text)?;
    eprintln!("Cracked {} of {} targets", results.len(), targets.len());
    Ok(())
}

fn read_report(path: &Path) -> Result<ScanReport> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn load_targets(path: &Path) -> Result<HashSet<(Kind, u64)>> {
    let mut targets = HashSet::new();
    for line in fs::read_to_string(path)?.lines() {
        let token = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches("0x");
        if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        match token.len() {
            1..=8 => {
                targets.insert((Kind::Bin, u32::from_str_radix(token, 16)? as u64));
            }
            9..=16 => {
                targets.insert((Kind::Wad, u64::from_str_radix(token, 16)?));
            }
            _ => {}
        }
    }
    Ok(targets)
}

fn test_candidates(
    candidates: &BTreeSet<String>,
    remaining: &mut HashSet<(Kind, u64)>,
    results: &mut BTreeMap<(Kind, u64), String>,
) {
    for candidate in candidates {
        let bin = (Kind::Bin, fnv1a(candidate) as u64);
        if remaining.remove(&bin) {
            results.insert(bin, candidate.clone());
        }
        let wad = (Kind::Wad, xxh64(candidate));
        if remaining.remove(&wad) {
            results.insert(wad, candidate.clone());
        }
    }
}

fn tokens(candidates: &BTreeSet<String>) -> Vec<String> {
    let mut counts = HashMap::new();
    for candidate in candidates {
        for token in split_words(candidate) {
            if (2..=40).contains(&token.len()) {
                *counts.entry(token).or_insert(0usize) += 1;
            }
        }
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(512)
        .map(|(token, _)| token)
        .collect()
}

fn generate_level(base: &BTreeSet<String>, tokens: &[String], level: usize) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    let prefixes = [
        "m", "is", "has", "use", "enable", "disable", "default", "base",
    ];
    let suffixes = [
        "data",
        "name",
        "id",
        "path",
        "map",
        "list",
        "table",
        "key",
        "type",
        "value",
        "index",
        "count",
        "enabled",
        "properties",
    ];
    if level == 1 {
        for value in base.iter().take(200_000) {
            for variant in case_variants(value) {
                output.insert(variant);
            }
            for prefix in prefixes {
                output.insert(format!("{prefix}{}", upper_first(value)));
            }
            for suffix in suffixes {
                output.insert(format!("{}{}", value, upper_first(suffix)));
            }
        }
    } else if level == 2 {
        for left in tokens.iter().take(256) {
            for right in tokens.iter().take(256) {
                if left.eq_ignore_ascii_case(right) {
                    continue;
                }
                output.insert(format!("{}{}", left, upper_first(right)));
                output.insert(format!("{left}_{right}"));
                output.insert(format!("{left}/{right}"));
            }
        }
    } else if level == 3 {
        let roots = ["assets", "data", "characters", "maps", "ux", "clientstates"];
        let exts = [
            "bin", "dds", "tex", "skn", "skl", "anm", "troy", "lua", "py",
        ];
        for root in roots {
            for left in tokens.iter().take(128) {
                for right in tokens.iter().take(64) {
                    for ext in exts {
                        output.insert(format!("{root}/{left}/{right}.{ext}"));
                    }
                }
            }
        }
    } else {
        for a in tokens.iter().take(64) {
            for b in tokens.iter().take(64) {
                for c in tokens.iter().take(32) {
                    output.insert(format!("{}{}{}", a, upper_first(b), upper_first(c)));
                }
            }
        }
    }
    output
}

fn split_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = value.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        let boundary = !current.is_empty()
            && (matches!(ch, '/' | '\\' | '.' | '_' | '-' | ':' | ' ')
                || (ch.is_ascii_uppercase()
                    && chars
                        .get(index.wrapping_sub(1))
                        .is_some_and(|prev| prev.is_ascii_lowercase())));
        if boundary {
            words.push(current.to_ascii_lowercase());
            current.clear();
        }
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current.to_ascii_lowercase());
    }
    words
}

fn case_variants(value: &str) -> Vec<String> {
    let words = split_words(value);
    if words.is_empty() {
        return Vec::new();
    }
    let camel = words[0].clone()
        + &words[1..]
            .iter()
            .map(|word| upper_first(word))
            .collect::<String>();
    let pascal = words
        .iter()
        .map(|word| upper_first(word))
        .collect::<String>();
    vec![
        value.to_ascii_lowercase(),
        camel,
        pascal,
        words.join("_"),
        words.join("/"),
    ]
}

fn upper_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn target_text(target: (Kind, u64)) -> String {
    match target.0 {
        Kind::Bin => format!("{:08x}", target.1 as u32),
        Kind::Wad => format!("{:016x}", target.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_split_handles_camel_and_paths() {
        assert_eq!(
            split_words("assets/VfxEmitterDefinitionData.bin"),
            ["assets", "vfx", "emitter", "definition", "data", "bin"]
        );
    }

    #[test]
    fn variants_include_riot_styles() {
        let values = case_variants("particle color texture");
        assert!(values.contains(&"particleColorTexture".to_owned()));
        assert!(values.contains(&"ParticleColorTexture".to_owned()));
    }
}
