//! `ep9_platform::simd` (rule 116; plan v19 §5 SIMD-DISPATCH; the SRC-HASH lines of §4.3).
//!
//! Every SHA-256 body that this CPU has is called by its function, not through a slot, on the
//! NIST vectors, on every length up to 1100 bytes and at every split of an incremental hash.
//! The pick follows `EP9_SIMD`: a run with a forced body checks that the process took it and
//! prints the `SIMD-REPORT` line, and `EP9_REQUIRE_SIMD_PATHS` (a comma list, for the x86 box:
//! `sse2,avx2,sha-ni`) makes a missing body a failure instead of a skip.
//!
//! The 4 tests that the x86 box runs by name (`--ignored --exact <name> --nocapture`; the
//! SIMD-BOX `build-cmd.txt`). `EP9_SHA256_BODY=<name>` names one SHA-256 body (`scalar`,
//! `sha-ni`, `armv8-sha2`; a body this CPU lacks fails); unset = every body this CPU has.
//!
//! ```text
//! simd_report          "SIMD-REPORT target=.. search=<body> json=<body> sha256=<body>"  (not ignored)
//! simd_digest          "SIMD-DIGEST <body> <vector> <64 hex>" per NIST vector
//! simd_corpus_files    "SIMD-CORPUS-FILES <body> <64 hex> <files> <bytes>": the digest of the
//!                      digests of every corpus source file (the decoded-source hash input)
//! simd_bench           "SIMD-BENCH <body> <bytes> <ns>": 5 samples of 64 MiB of whole blocks,
//!                      then "simd-speed sha256 <body> <MB/s> x<ratio>" when it ran 2 bodies or more
//! ```

use ep9_platform::simd::{
    self, AVX2, AVX512_POPCNT, Body, JsonBody, NEON, SHA_NI, SHA2, SIMD128, SearchBody,
    Sha256Blocks, Sha256Body,
};

// ---- a SHA-256 driver over a block function -----------------------------------------------

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// An incremental SHA-256 (FIPS 180-4 padding) that runs `body` on whole blocks.
struct Hasher {
    body: Sha256Blocks,
    state: [u32; 8],
    buf: Vec<u8>,
    total: u64,
}

impl Hasher {
    fn new(body: Sha256Blocks) -> Hasher {
        Hasher {
            body,
            state: H0,
            buf: Vec::new(),
            total: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.total += data.len() as u64;
        self.buf.extend_from_slice(data);
        let whole = self.buf.len() / 64 * 64;
        let (blocks, _) = self.buf[..whole].as_chunks::<64>();
        (self.body)(&mut self.state, blocks);
        self.buf.drain(..whole);
    }

    fn finish(mut self) -> [u8; 32] {
        let bits = self.total * 8;
        let mut pad = vec![0x80u8];
        while (self.buf.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        pad.extend_from_slice(&bits.to_be_bytes());
        self.update(&pad);
        assert!(self.buf.is_empty());
        let mut out = [0u8; 32];
        for (o, w) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *o = w.to_be_bytes();
        }
        out
    }
}

fn digest(body: Sha256Blocks, data: &[u8]) -> [u8; 32] {
    let mut h = Hasher::new(body);
    h.update(data);
    h.finish()
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// xorshift64*: reproducible test data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

/// The names that `EP9_REQUIRE_SIMD_PATHS` lists.
fn required() -> Vec<String> {
    std::env::var("EP9_REQUIRE_SIMD_PATHS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The SHA-256 bodies this CPU has, scalar first; prints the ones it lacks. A body that
/// `EP9_REQUIRE_SIMD_PATHS` names must be there.
fn sha_bodies() -> Vec<(Sha256Body, Sha256Blocks)> {
    let mut v = Vec::new();
    for b in Sha256Body::ALL {
        match b.blocks() {
            Some(f) => v.push((b, f)),
            None => eprintln!("simd: sha256 body {} not on this CPU or build", b.name()),
        }
    }
    assert_eq!(v[0].0, Sha256Body::Scalar);
    for name in required() {
        if Sha256Body::ALL.iter().any(|b| b.name() == name) {
            assert!(
                v.iter().any(|(b, _)| b.name() == name),
                "sha256 body {name} is missing"
            );
        }
    }
    if cfg!(target_arch = "aarch64") && simd::has(SHA2) {
        assert!(v.iter().any(|(b, _)| *b == Sha256Body::ArmSha2));
    }
    v
}

/// The body that `EP9_SHA256_BODY` names (it must be on this CPU), else every body this CPU
/// has (scalar first).
fn named_bodies() -> Vec<(Sha256Body, Sha256Blocks)> {
    let Ok(name) = std::env::var("EP9_SHA256_BODY") else {
        return sha_bodies();
    };
    let b = Sha256Body::ALL
        .into_iter()
        .find(|b| b.name() == name.trim())
        .unwrap_or_else(|| panic!("EP9_SHA256_BODY={name}: no such sha256 body"));
    let f = b
        .blocks()
        .unwrap_or_else(|| panic!("EP9_SHA256_BODY={name}: not on this CPU or build"));
    vec![(b, f)]
}

// ---- SHA-256: the SRC-HASH lines ----------------------------------------------------------

/// FIPS 180-4 examples and the NIST SHA-256 sample and additional messages: (name, message,
/// digest). The names are the vector names of the `SIMD-DIGEST` lines.
fn nist_vectors() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    vec![
        (
            "empty",
            b"".to_vec(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "448-bit",
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            "896-bit",
            b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu".to_vec(),
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
        ),
        (
            "1-byte-bd",
            vec![0xbd],
            "68325720aabd7c82f30f554b313d0570c95accbb7dc4b5aae11204c08ffe732b",
        ),
        (
            "4-byte-c98c8e55",
            vec![0xc9, 0x8c, 0x8e, 0x55],
            "7abc22c0ae5af26ce93dbb94433a0e0b2e119d014f8e7f65bd56c61ccccd9504",
        ),
        (
            "55-zero",
            vec![0; 55],
            "02779466cdec163811d078815c633f21901413081449002f24aa3e80f0b88ef7",
        ),
        (
            "56-zero",
            vec![0; 56],
            "d4817aa5497628e7c77e6b606107042bbba3130888c5f47a375e6179be789fbb",
        ),
        (
            "1000-zero",
            vec![0; 1000],
            "541b3e9daa09b20bf85fa273e5cbd3e80185aa4ec298e765db87742b70138a53",
        ),
        (
            "1000-A",
            vec![b'A'; 1000],
            "c2e686823489ced2017f6059b8b239318b6364f6dcd835d0a519105a1eadd6e4",
        ),
        (
            "1005-U",
            vec![b'U'; 1005],
            "f4d62ddec0f3dd90ea1380fa16a5ff8dc4c54b21740650f24afc4120903552b0",
        ),
        (
            "million-a",
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ]
}

#[test]
fn every_sha256_body_gives_the_nist_digests() {
    let bodies = sha_bodies();
    for (name, msg, want) in nist_vectors() {
        for (b, f) in &bodies {
            assert_eq!(hex(&digest(*f, &msg)), want, "body {} on {name}", b.name());
        }
    }
}

/// The NIST vectors by the body that `EP9_SHA256_BODY` names (else every body this CPU has),
/// the body function called by its name: one `SIMD-DIGEST <body> <vector> <64 hex>` line each.
#[test]
#[ignore = "the x86 box form of every_sha256_body_gives_the_nist_digests"]
fn simd_digest() {
    for (b, f) in named_bodies() {
        for (name, msg, want) in nist_vectors() {
            let got = hex(&digest(f, &msg));
            println!("\nSIMD-DIGEST {} {name} {got}", b.name());
            assert_eq!(got, want, "body {} on {name}", b.name());
        }
    }
}

#[test]
fn every_sha256_body_matches_scalar_at_every_length_and_split() {
    let bodies = sha_bodies();
    let mut r = Rng(0x5eed_0f5a_2560_0001);
    let data = r.bytes(1100);
    for n in 0..=data.len() {
        let want = digest(simd::sha256_scalar, &data[..n]);
        for (b, f) in &bodies[1..] {
            assert_eq!(digest(*f, &data[..n]), want, "body {} len {n}", b.name());
        }
    }
    // Incremental equals one shot at every split (the split test of ep9_bytecode sha256.rs).
    let whole = digest(simd::sha256_scalar, &data[..300]);
    for (b, f) in &bodies {
        for split in 0..=300 {
            let mut h = Hasher::new(*f);
            h.update(&data[..split]);
            h.update(&data[split..300]);
            assert_eq!(h.finish(), whole, "body {} split {split}", b.name());
        }
    }
}

#[test]
fn every_sha256_body_runs_many_blocks_as_one_block_at_a_time() {
    let mut r = Rng(0x5eed_0f5a_2560_0002);
    let data = r.bytes(64 * 37);
    let (blocks, _) = data.as_chunks::<64>();
    for (b, f) in sha_bodies() {
        for start in [H0, [0; 8], [u32::MAX; 8], [1, 2, 3, 4, 5, 6, 7, 8]] {
            let mut one = start;
            f(&mut one, blocks);
            let mut each = start;
            for block in blocks {
                simd::sha256_scalar(&mut each, core::slice::from_ref(block));
            }
            assert_eq!(one, each, "body {}", b.name());
            // No block: the state does not change.
            let mut none = start;
            f(&mut none, &[]);
            assert_eq!(none, start, "body {}", b.name());
        }
    }
}

/// The source files of the corpus digest (`ep9_frontend` `bytecodegen/corpus.rs`: test262
/// test and harness, the 3 benchmark pins), sorted.
fn corpus_files() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party");
    let mut out = Vec::new();
    for dir in [
        "test262/test",
        "test262/harness",
        "bench-octane",
        "bench-v8v7",
        "bench-jetstream",
    ] {
        let mut stack = vec![root.join(dir)];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for path in entries.flatten().map(|e| e.path()) {
                if path.is_dir() {
                    if !path
                        .file_name()
                        .is_some_and(|n| n == ".git" || n == "node_modules")
                    {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|e| e == "js" || e == "mjs") {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

/// Each body hashes the bytes of every corpus source file (the decoded-source hash input of
/// each unit) to the scalar digest; one line per body with the digest of the digests. It needs
/// the test262 and the 3 bench checkouts (`scripts/sync-test262.sh`, `scripts/sync-bench.sh`).
/// The unit content hashes and the corpus digest itself (`expectations/bytecode-corpus.digest`)
/// come from `ep9_frontend` `bytecodegen/corpus.rs`, which runs the scalar body (it does not
/// set the slot).
#[test]
#[ignore = "needs the third_party corpus checkouts"]
fn simd_corpus_files() {
    let files = corpus_files();
    assert!(
        !files.is_empty(),
        "no corpus checkout under third_party: run scripts/sync-test262.sh and scripts/sync-bench.sh"
    );
    let bodies = named_bodies();
    let mut sums: Vec<Hasher> = bodies.iter().map(|(_, f)| Hasher::new(*f)).collect();
    let mut bytes = 0usize;
    let mut read = 0usize;
    for path in &files {
        let Ok(src) = std::fs::read(path) else {
            continue;
        };
        bytes += src.len();
        read += 1;
        let want = digest(simd::sha256_scalar, &src);
        for (i, (b, f)) in bodies.iter().enumerate() {
            let got = digest(*f, &src);
            assert_eq!(got, want, "body {} on {}", b.name(), path.display());
            sums[i].update(&got);
        }
    }
    let sums: Vec<String> = sums.into_iter().map(|h| hex(&h.finish())).collect();
    for (i, (b, _)) in bodies.iter().enumerate() {
        println!(
            "\nSIMD-CORPUS-FILES {} {} {read} {bytes}",
            b.name(),
            sums[i]
        );
        assert_eq!(sums[i], sums[0]);
    }
}

// ---- the detection and the pick ------------------------------------------------------------

#[test]
fn the_features_match_the_std_detection() {
    let f = simd::features();
    assert_eq!(f, simd::features(), "one detection per process");
    #[cfg(target_arch = "x86_64")]
    {
        use std::is_x86_feature_detected as d;
        assert_eq!(f & AVX2 != 0, d!("avx2"));
        assert_eq!(f & SHA_NI != 0, d!("sha") && d!("sse4.1") && d!("ssse3"));
        assert_eq!(
            f & AVX512_POPCNT != 0,
            d!("avx512f") && d!("avx512vl") && d!("avx512vpopcntdq")
        );
        assert_eq!(f & (NEON | SHA2 | SIMD128), 0);
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected as d;
        assert_eq!(f & NEON != 0, d!("neon"));
        assert_eq!(f & SHA2 != 0, d!("neon") && d!("sha2"));
        assert_eq!(f & (AVX2 | SHA_NI | AVX512_POPCNT | SIMD128), 0);
    }
    for (bits, name) in [
        (AVX2, "avx2"),
        (SHA_NI, "sha-ni"),
        (AVX512_POPCNT, "avx512vl+vpopcntdq"),
        (NEON, "neon"),
        (SHA2, "sha2"),
    ] {
        assert_eq!(simd::has(bits), f & bits != 0, "{name}");
    }
}

fn best<B: Body>() -> B {
    simd::pick::<B>(None, None)
}

#[test]
fn the_pick_is_the_widest_body_or_the_forced_one() {
    // No force: the widest body this CPU has.
    assert!(best::<SearchBody>().available());
    assert!(best::<JsonBody>().available());
    assert!(best::<Sha256Body>().available());
    assert_ne!(best::<JsonBody>(), JsonBody::Scalar);
    if cfg!(target_arch = "aarch64") {
        assert_eq!(best::<SearchBody>(), SearchBody::Neon);
        assert_eq!(best::<JsonBody>(), JsonBody::Neon);
    }
    if cfg!(target_arch = "x86_64") {
        let want = if simd::has(AVX2) {
            SearchBody::Avx2
        } else {
            SearchBody::Sse2
        };
        assert_eq!(best::<SearchBody>(), want);
    }
    let sha = if Sha256Body::ShaNi.available() {
        Sha256Body::ShaNi
    } else if Sha256Body::ArmSha2.available() {
        Sha256Body::ArmSha2
    } else {
        Sha256Body::Scalar
    };
    assert_eq!(best::<Sha256Body>(), sha);

    // A bare name forces every table that has a body of that name.
    assert_eq!(
        simd::pick::<SearchBody>(Some("scalar"), None),
        SearchBody::Scalar
    );
    assert_eq!(
        simd::pick::<JsonBody>(Some("scalar"), None),
        JsonBody::Scalar
    );
    assert_eq!(
        simd::pick::<Sha256Body>(Some("scalar"), None),
        Sha256Body::Scalar
    );
    assert_eq!(simd::pick::<JsonBody>(Some("swar"), None), JsonBody::Swar);
    assert_eq!(simd::pick::<SearchBody>(Some("swar"), None), best());
    assert_eq!(simd::pick::<Sha256Body>(Some("swar"), None), best());
    // A table item wins over the old name, and the old name over a bare item.
    assert_eq!(
        simd::pick::<JsonBody>(Some("json=swar"), Some("scalar")),
        JsonBody::Swar
    );
    assert_eq!(
        simd::pick::<JsonBody>(Some("scalar"), Some("swar")),
        JsonBody::Swar
    );
    assert_eq!(
        simd::pick::<JsonBody>(Some(" search=scalar , json = swar "), None),
        JsonBody::Swar
    );
    assert_eq!(
        simd::pick::<SearchBody>(Some("search=scalar,json=swar"), None),
        SearchBody::Scalar
    );
    assert_eq!(
        simd::pick::<Sha256Body>(Some("search=scalar,json=swar"), None),
        best()
    );
    assert_eq!(
        simd::pick::<Sha256Body>(Some("sha256=scalar"), None),
        Sha256Body::Scalar
    );
    // The first bare item that the table has decides.
    assert_eq!(
        simd::pick::<Sha256Body>(Some("swar,scalar"), None),
        Sha256Body::Scalar
    );
    assert_eq!(
        simd::pick::<JsonBody>(Some("swar,scalar"), None),
        JsonBody::Swar
    );
    // Unknown or missing names give the best body.
    assert_eq!(simd::pick::<SearchBody>(Some("avx512"), None), best());
    assert_eq!(simd::pick::<SearchBody>(None, Some("no-such-path")), best());
    assert_eq!(simd::pick::<SearchBody>(Some(""), None), best());
    assert_eq!(simd::pick::<SearchBody>(Some("search="), None), best());
    for b in SearchBody::ALL {
        let want = if b.available() { b } else { best() };
        assert_eq!(simd::pick::<SearchBody>(None, Some(b.name())), want);
        assert_eq!(
            simd::pick::<SearchBody>(Some(&format!("search={}", b.name())), None),
            want
        );
    }
    for b in JsonBody::ALL {
        let want = if b.available() { b } else { best() };
        assert_eq!(simd::pick::<JsonBody>(None, Some(b.name())), want);
    }
    for b in Sha256Body::ALL {
        let want = if b.available() { b } else { best() };
        assert_eq!(simd::pick::<Sha256Body>(Some(b.name()), None), want);
        assert_eq!(
            simd::pick::<Sha256Body>(Some(&format!("sha256={}", b.name())), None),
            want
        );
        assert_eq!(b.blocks().is_some(), b.available());
    }
}

/// The process pick is the pick of the env of this run: a run with `EP9_SIMD=<body>` (or a
/// table item, or an old name) runs that body. Prints the `SIMD-REPORT` line (on a line of its
/// own: the test harness prints "test simd_report ... " first).
#[test]
fn simd_report() {
    let var = |n: &str| std::env::var(n).ok();
    let (all, search, json) = (
        var("EP9_SIMD"),
        var("EP9_SIMD_SEARCH"),
        var("EP9_SIMD_JSON"),
    );
    let s: SearchBody = simd::pick(all.as_deref(), search.as_deref());
    let j: JsonBody = simd::pick(all.as_deref(), json.as_deref());
    let h: Sha256Body = simd::pick(all.as_deref(), None);
    assert_eq!(simd::search(), s);
    assert_eq!(simd::json(), j);
    assert_eq!(simd::sha256(), h);
    assert_eq!(simd::search(), s, "the second read");
    let line = simd::report();
    println!("\n{line}");
    assert!(line.starts_with(&format!("SIMD-REPORT target={} ", std::env::consts::ARCH)));
    assert!(line.ends_with(&format!(
        " search={} json={} sha256={}",
        s.name(),
        j.name(),
        h.name()
    )));
    for (bit, key) in simd::FEATURE_NAMES {
        let v = if simd::has(bit) { "yes" } else { "no" };
        assert!(line.contains(&format!(" {key}={v} ")), "{key} in {line}");
    }
    // The slot value of the table: the picked body's function, none for the scalar body.
    match simd::sha256_slot_body() {
        None => assert_eq!(h, Sha256Body::Scalar),
        Some(f) => {
            assert_ne!(h, Sha256Body::Scalar);
            assert_eq!(
                hex(&digest(f, b"abc")),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        }
    }
    // Each required name is a body that this CPU has, in some table.
    for name in required() {
        let have = SearchBody::ALL
            .iter()
            .any(|b| b.name() == name && b.available())
            || JsonBody::ALL
                .iter()
                .any(|b| b.name() == name && b.available())
            || Sha256Body::ALL
                .iter()
                .any(|b| b.name() == name && b.available());
        assert!(have, "EP9_REQUIRE_SIMD_PATHS: no body {name} on this CPU");
    }
}

// ---- the speed of each body -----------------------------------------------------------------

/// SRC-HASH speed (plan v19 §4.3: each body >= 2.0 x the scalar body in bytes per second).
/// For each body (`EP9_SHA256_BODY`, else every body this CPU has): one warm-up call, then 5
/// samples of 64 MiB of whole blocks, one line each: `SIMD-BENCH <body> <bytes> <ns>`. With 2
/// bodies or more it also prints each body's median against the scalar median.
#[test]
#[ignore = "timing only"]
fn simd_bench() {
    const SAMPLES: usize = 5;
    let data = Rng(0x5eed_0f5a_2560_0003).bytes(64 << 20);
    let (blocks, _) = data.as_chunks::<64>();
    let bytes = blocks.len() * 64;
    let mut medians = Vec::new();
    for (b, f) in named_bodies() {
        // Warm-up: the pages of `data` and the code are hot before the first sample.
        let mut state = H0;
        f(
            &mut state,
            std::hint::black_box(&blocks[..blocks.len() / 8]),
        );
        std::hint::black_box(state);
        let mut ns = Vec::new();
        for _ in 0..SAMPLES {
            let mut state = H0;
            let t = std::time::Instant::now();
            f(&mut state, std::hint::black_box(blocks));
            let d = t.elapsed().as_nanos();
            std::hint::black_box(state);
            println!("\nSIMD-BENCH {} {bytes} {d}", b.name());
            ns.push(d);
        }
        ns.sort_unstable();
        medians.push((b, bytes as f64 / ns[SAMPLES / 2] as f64 * 1e9));
    }
    if medians.len() < 2 {
        return;
    }
    let base = medians
        .iter()
        .find(|(b, _)| *b == Sha256Body::Scalar)
        .map(|m| m.1);
    for (b, v) in &medians {
        let ratio = base.map_or(f64::NAN, |s| v / s);
        println!(
            "simd-speed sha256 {:<10} {:>9.1} MB/s  x{ratio:.2} vs scalar (median of {SAMPLES})",
            b.name(),
            v / 1e6
        );
    }
}
