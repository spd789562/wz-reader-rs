use crate::util::string_decryptor::pkg2_decryptor::mix_kmst1199;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

/* a special key for decryption */
mod keys {
    pub const VERIFY_KEY: u32 = 0x1A2B3C4D;

    pub const VERIFY_KEY_V4: u32 = 0x6D4C3B2A;

    pub const VERIFY_KEY_V5: u32 = 0x2A2C818B;
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Pkg2VersionGen {
    V1,
    V2,
    V3,
    V4,
    V5,
    #[default]
    Unknown,
}
impl Pkg2VersionGen {
    pub fn get_generator(&self, hash1: u32, hash2: u32) -> Box<dyn VersionGenerator> {
        match self {
            Pkg2VersionGen::V1 => Box::new(Pkg2VersionGenV1::new(hash1, hash2)),
            Pkg2VersionGen::V2 => Box::new(Pkg2VersionGenV2::new(hash1, hash2)),
            Pkg2VersionGen::V3 => Box::new(Pkg2VersionGenV3::new(hash1, hash2)),
            Pkg2VersionGen::V4 => Box::new(Pkg2VersionGenV4::new(hash1, hash2)),
            Pkg2VersionGen::V5 => Box::new(Pkg2VersionGenV5::new(hash1, hash2)),
            _ => unreachable!(),
        }
    }
}

pub trait VersionGenerator {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>>;
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool>;
}

pub struct VersionGen {
    pub hash1: u32,
    pub hash2: u32,
    current_iter: Option<Box<dyn Iterator<Item = u32>>>,
    generators: Vec<Box<dyn VersionGenerator>>,
}

pub struct Pkg2VersionGenV1 {
    hash1: u32,
    hash2: u32,
}
impl Pkg2VersionGenV1 {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self { hash1, hash2 }
    }
    #[inline]
    pub fn verify_hash(hash1: u32, hash2: u32, target_hash: u32) -> bool {
        let lt = hash1.rotate_left(7) ^ target_hash;
        lt == hash2
    }
    fn calc_hash(&self) -> Vec<u32> {
        vec![self.hash1.rotate_left(7) ^ self.hash2]
    }
}
impl VersionGenerator for Pkg2VersionGenV1 {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>> {
        Box::new(self.calc_hash().into_iter())
    }
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool> {
        Box::new(Self::verify_hash)
    }
}

pub struct Pkg2VersionGenV2 {
    hash1: u32,
    hash2: u32,
}
impl Pkg2VersionGenV2 {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self { hash1, hash2 }
    }
    #[inline]
    pub fn verify_hash(hash1: u32, hash2: u32, target_hash: u32) -> bool {
        let rotate_base = hash1 ^ (target_hash.wrapping_add(keys::VERIFY_KEY));
        let lt = rotate_base.rotate_left((target_hash & 0x1f) as u32);
        (lt ^ target_hash) == hash2
    }
    fn calc_hash(&self) -> Vec<u32> {
        let mut results: Vec<u32> = Vec::new();
        let mut carries = [0; 33];
        let mut lhs_bits = [0; 32];
        let mut info = HashGenInfo::new(
            self.hash1,
            self.hash2,
            5,
            self.hash2,
            &mut carries,
            &mut lhs_bits,
            &mut results,
            &Self::verify_hash,
        );

        for s_candidate in 0..32 {
            info.carries.fill(0);
            info.lhs_bits.fill(0);
            info.s = s_candidate;
            info.low_bits = s_candidate;
            backtrack(0, 0, &mut info);
        }

        results
    }
}
impl VersionGenerator for Pkg2VersionGenV2 {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>> {
        Box::new(self.calc_hash().into_iter())
    }
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool> {
        Box::new(Self::verify_hash)
    }
}

pub struct Pkg2VersionGenV3 {
    hash1: u32,
    hash2: u32,
}
impl Pkg2VersionGenV3 {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self { hash1, hash2 }
    }
    #[inline]
    pub fn verify_hash(hash1: u32, hash2: u32, target_hash: u32) -> bool {
        let rotate_base = hash1 ^ (target_hash.wrapping_add(keys::VERIFY_KEY));
        let rotate_amount = (target_hash & 0xf) + (hash1 & 0xf);

        let lt = rotate_base.rotate_left(rotate_amount);

        !(lt ^ target_hash ^ hash1) == hash2
    }
    fn calc_hash(&self) -> Vec<u32> {
        let mut results: Vec<u32> = Vec::new();
        let mut carries = [0; 33];
        let mut lhs_bits = [0; 32];
        let mut info = HashGenInfo::new(
            self.hash1,
            self.hash2,
            4,
            (!self.hash2) ^ self.hash1,
            &mut carries,
            &mut lhs_bits,
            &mut results,
            &Pkg2VersionGenV3::verify_hash,
        );
        for s_candidate in 0_u32..16_u32 {
            info.carries.fill(0);
            info.lhs_bits.fill(0);
            info.s = s_candidate.wrapping_add(self.hash1 & 0xf) as i32 as u32;
            info.low_bits = s_candidate;
            backtrack(0, 0, &mut info);
        }

        results
    }
}
impl VersionGenerator for Pkg2VersionGenV3 {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>> {
        Box::new(self.calc_hash().into_iter())
    }
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool> {
        Box::new(Self::verify_hash)
    }
}

pub struct Pkg2VersionGenV4 {
    hash1: u32,
    hash2: u32,
}
impl Pkg2VersionGenV4 {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self { hash1, hash2 }
    }
    #[inline]
    pub fn verify_hash(hash1: u32, hash2: u32, target_hash: u32) -> bool {
        let mix_base = hash1 ^ target_hash;
        let mixed_hash = mix_kmst1199(mix_base ^ keys::VERIFY_KEY_V4) ^ 0x91E10DA5;

        let rotate_base = hash1
            ^ (mixed_hash as u16 as u32)
                .wrapping_add(target_hash)
                .wrapping_add(keys::VERIFY_KEY);
        let rotate_amount = ((mixed_hash ^ target_hash) & 0xF) + (hash1 & 0xF);

        let lt = rotate_base.rotate_left(rotate_amount);

        (lt ^ mix_base.wrapping_add(mixed_hash)) == !hash2
    }
    fn calc_hash(&self) -> Vec<u32> {
        brute_force_hash(self.hash1, self.hash2, &Self::verify_hash)
    }
}
impl VersionGenerator for Pkg2VersionGenV4 {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>> {
        Box::new(self.calc_hash().into_iter())
    }
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool> {
        Box::new(Self::verify_hash)
    }
}

pub struct Pkg2VersionGenV5 {
    hash1: u32,
    hash2: u32,
}
impl Pkg2VersionGenV5 {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self { hash1, hash2 }
    }
    #[inline]
    pub fn verify_hash(hash1: u32, hash2: u32, target_hash: u32) -> bool {
        let mix_base = hash1 ^ target_hash;
        let mixed_hash = mix_kmst1199(mix_base ^ keys::VERIFY_KEY_V4) ^ 0x91E10DA5;

        let rotate_base = hash1
            ^ (mixed_hash as u16 as u32)
                .wrapping_add(target_hash)
                .wrapping_add(keys::VERIFY_KEY);
        let rotate_amount = ((mixed_hash ^ target_hash) & 0xF) + (hash1 & 0xF);

        let lt = rotate_base.rotate_left(rotate_amount);

        (lt ^ (mix_base.wrapping_add(mixed_hash)) ^ keys::VERIFY_KEY_V5) == hash2
    }
    fn calc_hash(&self) -> Vec<u32> {
        brute_force_hash(self.hash1, self.hash2, &Self::verify_hash)
    }
}
impl VersionGenerator for Pkg2VersionGenV5 {
    fn get_iter(self: Box<Self>) -> Box<dyn Iterator<Item = u32>> {
        Box::new(self.calc_hash().into_iter())
    }
    fn get_verifier(&self) -> Box<dyn Fn(u32, u32, u32) -> bool> {
        Box::new(Self::verify_hash)
    }
}

#[derive(Debug)]
pub struct HashGenInfo<'a, F: Fn(u32, u32, u32) -> bool> {
    hash1: u32,
    hash2: u32,
    s: u32,
    low_bit_len: u32,
    low_bits: u32,
    target: u32,
    carries: &'a mut [u32],
    lhs_bits: &'a mut [u32],
    validator: &'a F,
    results: &'a mut Vec<u32>,
}

impl<'a, F: Fn(u32, u32, u32) -> bool> HashGenInfo<'a, F> {
    pub fn new(
        hash1: u32,
        hash2: u32,
        low_bit_len: u32,
        target: u32,
        carries: &'a mut [u32],
        lhs_bits: &'a mut [u32],
        results: &'a mut Vec<u32>,
        validator: &'a F,
    ) -> Self {
        Self {
            hash1,
            hash2,
            s: 0,
            low_bits: 0,
            low_bit_len,
            target,
            carries,
            lhs_bits,
            results,
            validator,
        }
    }
}

impl VersionGen {
    pub fn new(hash1: u32, hash2: u32) -> Self {
        Self {
            hash1,
            hash2,
            current_iter: None,
            generators: vec![
                Box::new(Pkg2VersionGenV1::new(hash1, hash2)),
                Box::new(Pkg2VersionGenV2::new(hash1, hash2)),
                Box::new(Pkg2VersionGenV3::new(hash1, hash2)),
            ],
        }
    }
}

impl Iterator for VersionGen {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        // use the current iterator if it exists
        if let Some(current_iter) = self.current_iter.as_mut() {
            if let Some(result) = current_iter.next() {
                return Some(result);
            } else {
                self.current_iter = None;
            }
        }

        // try using the next generator
        if self.current_iter.is_none() {
            if let Some(generator) = self.generators.pop() {
                self.current_iter = Some(generator.get_iter());
                return self.current_iter.as_mut().unwrap().next();
            }
        }

        // no more generators to try
        None
    }
}

// ported code from WzComparerR2(C#) and it's generated by google AI
// @link https://github.com/Kagamia/WzComparerR2/blob/2b23c99325569815c9abbca10d91a6baefbf1b86/WzComparerR2.WzLib/Wz_Header.cs#L263
fn backtrack<'a, F: Fn(u32, u32, u32) -> bool>(
    bit_idx: u32,
    v_hash: u32,
    info: &mut HashGenInfo<'a, F>,
) {
    if bit_idx == 32 {
        if (info.validator)(info.hash1, info.hash2, v_hash) {
            info.results.push(v_hash);
        }
        return;
    }

    let start = if bit_idx < info.low_bit_len {
        (info.low_bits >> bit_idx) & 1
    } else {
        0
    };
    let end = if bit_idx < info.low_bit_len {
        (info.low_bits >> bit_idx) & 1
    } else {
        1
    };

    for v_bit in start..=end {
        let prev_lhs_idx = (bit_idx.wrapping_sub(info.s).wrapping_add(32)) & 0x1f;
        if prev_lhs_idx < bit_idx {
            let v_xor_h2 = v_bit ^ ((info.target >> bit_idx) & 1);
            if v_xor_h2 != info.lhs_bits[prev_lhs_idx as usize] {
                continue;
            }
        }

        let sum = v_bit + ((keys::VERIFY_KEY >> bit_idx) & 1) + info.carries[bit_idx as usize];
        let current_lhs_bit = (sum ^ (info.hash1 >> bit_idx)) & 1;

        let future_v_idx = (bit_idx.wrapping_add(info.s)) & 0x1f;
        if future_v_idx <= bit_idx {
            let known_v_bit = (v_hash >> future_v_idx) & 1;
            let target_v_xor_h2 = known_v_bit ^ ((info.target >> future_v_idx) & 1);
            if current_lhs_bit != target_v_xor_h2 {
                continue;
            }
        } else if future_v_idx < info.low_bit_len {
            let known_v_bit = (info.low_bits >> future_v_idx) & 1;
            let target_v_xor_h2 = known_v_bit ^ ((info.target >> future_v_idx) & 1);
            if current_lhs_bit != target_v_xor_h2 {
                continue;
            }
        }

        info.lhs_bits[bit_idx as usize] = current_lhs_bit;
        info.carries[bit_idx as usize + 1] = sum >> 1;

        backtrack(bit_idx + 1, v_hash | (v_bit << bit_idx), info);
    }
}

fn brute_force_hash<F: Fn(u32, u32, u32) -> bool + Sync>(
    hash1: u32,
    hash2: u32,
    verifier: &F,
) -> Vec<u32> {
    #[cfg(feature = "rayon")]
    return (0_u32..u32::MAX)
        .into_par_iter()
        .filter(|target_hash| verifier(hash1, hash2, *target_hash))
        .collect();
    #[cfg(not(feature = "rayon"))]
    return (0_u32..u32::MAX)
        .into_iter()
        .filter(|target_hash| verifier(hash1, hash2, *target_hash))
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH1: u32 = 0x0000abcd;
    const HASH2: u32 = 0x12340000;

    #[test]
    fn test_pkg2_version_gen_v1() {
        let version_gen = Pkg2VersionGenV1::new(HASH1, HASH2);
        let results = version_gen.calc_hash();
        assert!(results.len() > 0);
        assert!(Pkg2VersionGenV1::verify_hash(
            version_gen.hash1,
            version_gen.hash2,
            results[0]
        ));
    }

    #[test]
    fn test_pkg2_version_gen_v2() {
        let version_gen = Pkg2VersionGenV2::new(HASH1, HASH2);

        let results = version_gen.calc_hash();
        assert!(results.len() > 0);
        for result in results {
            assert!(Pkg2VersionGenV2::verify_hash(
                version_gen.hash1,
                version_gen.hash2,
                result
            ));
        }
    }

    #[test]
    fn test_pkg2_version_gen_v3() {
        let version_gen = Pkg2VersionGenV3::new(HASH1, HASH2);
        let results = version_gen.calc_hash();
        assert!(results.len() > 0);
        for result in results {
            assert!(Pkg2VersionGenV3::verify_hash(
                version_gen.hash1,
                version_gen.hash2,
                result
            ));
        }
    }

    /* the v4 and v5 calculation is pretty heavy and pretty rely on has1/hash2 so no need to run the test */
    // #[test]
    // fn test_pkg2_version_gen_v4() {
    //     let version_gen = Pkg2VersionGenV4::new(HASH1, HASH2);
    //     let results = version_gen.calc_hash();
    //     assert!(results.len() > 0);
    // }

    // #[test]
    // fn test_pkg2_version_gen_v5() {
    //     let version_gen = Pkg2VersionGenV5::new(HASH1, HASH2);
    //     let results = version_gen.calc_hash();
    //     assert!(results.len() > 0);
    // }

    #[test]
    fn test_pkg2_version_gen_iterator() {
        let mut version_gen = VersionGen::new(HASH1, HASH2);
        let v = version_gen.next().unwrap();

        assert!(Pkg2VersionGenV3::verify_hash(HASH1, HASH2, v));

        while let Some(v) = version_gen.next() {
            // should pass one of it
            assert!(
                Pkg2VersionGenV1::verify_hash(version_gen.hash1, version_gen.hash2, v)
                    || Pkg2VersionGenV2::verify_hash(version_gen.hash1, version_gen.hash2, v)
                    || Pkg2VersionGenV3::verify_hash(version_gen.hash1, version_gen.hash2, v)
            )
        }
    }
}
