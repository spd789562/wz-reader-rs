pub struct VersionGen {
    encver: i32,

    pub max_version: i32,
    pub current: i32,
}

impl VersionGen {
    pub fn new(encver: i32, min_version: i32, max_version: i32) -> Self {
        Self {
            encver,
            max_version,
            current: min_version,
        }
    }
    #[inline]
    pub fn calculate_version_hash(patch_version: i32) -> u32 {
        let mut version_hash = 0_u32;

        let bind_version = &patch_version.to_string();

        for i in bind_version.chars() {
            let char_code = i.to_ascii_lowercase() as u32;
            version_hash = version_hash * 32 + char_code + 1;
        }
        version_hash as u32
    }
    #[inline]
    pub fn check_and_get_version_hash(&self) -> u32 {
        let version_hash = Self::calculate_version_hash(self.current);

        if self.encver == self.current {
            return version_hash;
        }
        let enc = 0xff
            ^ (version_hash >> 24) & 0xff
            ^ (version_hash >> 16) & 0xff
            ^ (version_hash >> 8) & 0xff
            ^ version_hash & 0xff;

        if enc == self.encver as u32 {
            version_hash
        } else {
            0
        }
    }
}

impl Iterator for VersionGen {
    type Item = (i32, u32); // version and hash

    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.max_version {
            return None;
        }

        let hash = self.check_and_get_version_hash();

        let item = (self.current, hash);
        self.current += 1;
        Some(item)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pkg1_version_gen() {
        let mut version_gen = VersionGen::new(0, 0, 5);
        assert_eq!(version_gen.next(), Some((0, 49)));
        assert_eq!(version_gen.next(), Some((1, 0)));
        assert_eq!(version_gen.next(), Some((2, 0)));
        assert_eq!(version_gen.next(), Some((3, 0)));
        assert_eq!(version_gen.next(), Some((4, 0)));
        assert_eq!(version_gen.next(), Some((5, 0)));
        assert_eq!(version_gen.next(), None);
    }
}
