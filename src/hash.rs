pub fn fnv1a(value: &str) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in value.bytes().map(|byte| byte.to_ascii_lowercase()) {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

pub fn xxh64(value: &str) -> u64 {
    ritoshark::hash::xxh64(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_is_ascii_case_insensitive() {
        assert_eq!(fnv1a("mParticleName"), fnv1a("mparticlename"));
        assert_eq!(fnv1a("a"), 0xe40c292c);
    }
}
