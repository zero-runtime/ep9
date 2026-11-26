//! SIMD dispatch (rule 116; plan v19 §5 SIMD-DISPATCH): one CPU detection per process, one
//! body table per hot loop, and the `EP9_SIMD=` force for test runs.
//!
//! ```text
//! features()  one detection per process into an AtomicU8 bitset
//!               x86-64   AVX2 | SHA_NI (SHA + SSE4.1 + SSSE3) | AVX512_POPCNT (AVX-512 F + VL + VPOPCNTDQ)
//!               aarch64  NEON | SHA2
//!               wasm32   SIMD128: a build fact (cfg(target_feature = "simd128")), not a run-time test:
//!                        a module with SIMD opcodes does not validate on an engine without them,
//!                        so "detect and dispatch" on wasm = 2 builds and the loader picks one
//! a table     one AtomicU8 per table holds the picked body (0 before the first pick);
//!             the first read of any table picks all of them, once, from the bitset and the env
//!   SEARCH    scalar, sse2, avx2, neon, simd128    the bodies: ep9_vm string/search.rs
//!   JSON      scalar, swar, sse2, avx2, neon, simd128    the bodies: ep9_vm string/json_quote.rs
//!   SHA256    scalar, sha-ni, armv8-sha2           the bodies: here; the slot: ep9_bytecode sha256.rs
//!             (sha256_slot_body: the value to set, None for scalar = leave the slot unset)
//! report()    one line: the target, each feature bit and the body of every table (never inside a
//!             timed run): "SIMD-REPORT target=<arch> avx2=yes|no ... search=<body> json=<body>
//!             sha256=<body>"
//! ```
//!
//! Each SIMD body is a leaf: it has no GC point, takes no lock and calls nothing but its own
//! helpers and the scalar body. No SIMD body is called from T1 or AOT code (`ep9_masm` and
//! `ep9_unit` do not name this module). A body that the CPU or the build does not have is
//! never picked: [`Body::available`] reads the bitset, and the pick falls back to the best
//! body that is there. That check is what makes the `unsafe` calls of the bodies sound.
//!
//! **The force.** `EP9_SIMD` is a comma list, read once at the first pick:
//!
//! | item | meaning |
//! |---|---|
//! | `<table>=<body>` | that table takes that body (`search=sse2`, `json=swar`, `sha256=sha-ni`) |
//! | `<body>` | every table that has a body of that name takes it (`scalar` forces every table) |
//!
//! For each table the first of these that is set decides: a `<table>=<body>` item of
//! `EP9_SIMD`, then the old name of the table (`EP9_SIMD_SEARCH`, `EP9_SIMD_JSON`: they keep
//! their meaning), then the first bare `<body>` item that names a body of the table. A name
//! that the table has but the CPU or the build does not, or a name that the table does not
//! have, gives the best body (as the old names did). So a force is proved only by
//! [`report`]: each run that forces a body prints that line.
//!
//! The file uses only `core` and `std` (no crate item), so a standalone build of it proves the
//! wasm32 forms: `ep9_platform` itself does not build for wasm32.
//!
//! The SHA-NI and ARMv8 SHA2 bodies follow the round and message-schedule order of the
//! RustCrypto `sha2` 0.11.1 backends (`src/sha256/x86_sha.rs`, `src/sha256/aarch64_sha2.rs`);
//! learned, not copied (M6-ORCH-858).

use core::sync::atomic::{AtomicU8, Ordering};

// ---- the features --------------------------------------------------------------------------

/// x86-64: AVX2.
pub const AVX2: u8 = 1 << 0;
/// x86-64: the SHA extensions with SSE4.1 and SSSE3 (the SHA-NI body needs all three).
pub const SHA_NI: u8 = 1 << 1;
/// x86-64: AVX-512 F, VL and VPOPCNTDQ (the sweep bitmap bodies of SWEEP-LEAN).
pub const AVX512_POPCNT: u8 = 1 << 2;
/// aarch64: Advanced SIMD (NEON).
pub const NEON: u8 = 1 << 3;
/// aarch64: the SHA-256 instructions (FEAT_SHA256).
pub const SHA2: u8 = 1 << 4;
/// wasm32: simd128, on for the whole build.
pub const SIMD128: u8 = 1 << 5;
/// Set in [`FEATURES`] once the detection ran.
const READY: u8 = 1 << 7;

/// Every feature bit with its report key.
pub const FEATURE_NAMES: [(u8, &str); 6] = [
    (AVX2, "avx2"),
    (SHA_NI, "sha_ni"),
    (AVX512_POPCNT, "avx512_popcnt"),
    (NEON, "neon"),
    (SHA2, "sha2"),
    (SIMD128, "simd128"),
];

/// The detected feature bits, with [`READY`]; 0 before the first detection.
static FEATURES: AtomicU8 = AtomicU8::new(0);

/// The feature bits of this CPU and build, detected once per process.
#[inline]
pub fn features() -> u8 {
    let f = FEATURES.load(Ordering::Relaxed);
    if f & READY != 0 {
        f & !READY
    } else {
        detect_features()
    }
}

/// Whether this CPU and build have every feature of `bits`.
#[inline]
pub fn has(bits: u8) -> bool {
    features() & bits == bits
}

#[cold]
#[inline(never)]
fn detect_features() -> u8 {
    let f = cpu_features();
    // Two threads may both detect; they store the same value.
    FEATURES.store(f | READY, Ordering::Relaxed);
    f
}

#[cfg(target_arch = "x86_64")]
fn cpu_features() -> u8 {
    let mut f = 0;
    if std::is_x86_feature_detected!("avx2") {
        f |= AVX2;
    }
    if std::is_x86_feature_detected!("sha")
        && std::is_x86_feature_detected!("sse4.1")
        && std::is_x86_feature_detected!("ssse3")
    {
        f |= SHA_NI;
    }
    if std::is_x86_feature_detected!("avx512f")
        && std::is_x86_feature_detected!("avx512vl")
        && std::is_x86_feature_detected!("avx512vpopcntdq")
    {
        f |= AVX512_POPCNT;
    }
    f
}

#[cfg(target_arch = "aarch64")]
fn cpu_features() -> u8 {
    let mut f = 0;
    if std::arch::is_aarch64_feature_detected!("neon") {
        f |= NEON;
        if std::arch::is_aarch64_feature_detected!("sha2") {
            f |= SHA2;
        }
    }
    f
}

#[cfg(target_arch = "wasm32")]
fn cpu_features() -> u8 {
    if cfg!(target_feature = "simd128") {
        SIMD128
    } else {
        0
    }
}

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "wasm32"
)))]
fn cpu_features() -> u8 {
    0
}

/// True where the NEON search bodies of `ep9_vm` build: `neon` on for the whole build and a
/// little-endian target (their mask puts unit `i` at bits `4i..4i+4` only there).
const NEON_SEARCH_BUILD: bool = cfg!(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
));

// ---- the tables ----------------------------------------------------------------------------

/// A body of one table.
pub trait Body: Copy + Eq + core::fmt::Debug + 'static {
    /// The table name in `EP9_SIMD` (`search`, `json`, `sha256`).
    const TABLE: &'static str;
    /// Every body of the table, scalar first.
    fn all() -> &'static [Self];
    /// The bodies the pick prefers, widest first.
    fn best_order() -> &'static [Self];
    /// The body when none of [`Body::best_order`] is there.
    fn fallback() -> Self;
    /// The `EP9_SIMD` name of the body.
    fn name(self) -> &'static str;
    /// Whether this build and the running CPU can run the body.
    fn available(self) -> bool;
}

/// The bodies of the first-unit scan of the string search (`ep9_vm` `string/search.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SearchBody {
    Scalar = 1,
    Sse2 = 2,
    Avx2 = 3,
    Neon = 4,
    Simd128 = 5,
}

impl SearchBody {
    /// Every body, in order.
    pub const ALL: [SearchBody; 5] = [
        SearchBody::Scalar,
        SearchBody::Sse2,
        SearchBody::Avx2,
        SearchBody::Neon,
        SearchBody::Simd128,
    ];

    /// The `EP9_SIMD` and `EP9_SIMD_SEARCH` name of the body.
    pub const fn name(self) -> &'static str {
        match self {
            SearchBody::Scalar => "scalar",
            SearchBody::Sse2 => "sse2",
            SearchBody::Avx2 => "avx2",
            SearchBody::Neon => "neon",
            SearchBody::Simd128 => "simd128",
        }
    }

    /// Whether this build and the running CPU can run the body.
    pub fn available(self) -> bool {
        match self {
            SearchBody::Scalar => true,
            // SSE2 is part of the x86-64 base.
            SearchBody::Sse2 => cfg!(target_arch = "x86_64"),
            SearchBody::Avx2 => has(AVX2),
            SearchBody::Neon => NEON_SEARCH_BUILD && has(NEON),
            SearchBody::Simd128 => cfg!(target_arch = "wasm32") && has(SIMD128),
        }
    }
}

impl Body for SearchBody {
    const TABLE: &'static str = "search";
    fn all() -> &'static [Self] {
        &SearchBody::ALL
    }
    fn best_order() -> &'static [Self] {
        &[
            SearchBody::Avx2,
            SearchBody::Sse2,
            SearchBody::Neon,
            SearchBody::Simd128,
        ]
    }
    fn fallback() -> Self {
        SearchBody::Scalar
    }
    fn name(self) -> &'static str {
        SearchBody::name(self)
    }
    fn available(self) -> bool {
        SearchBody::available(self)
    }
}

/// The bodies of QuoteJSONString (`ep9_vm` `string/json_quote.rs`); `Scalar` is the reference
/// per-unit loop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum JsonBody {
    /// The reference: the per-unit loop.
    Scalar = 1,
    /// u64 lanes: 8 bytes or 4 UTF-16 units per step.
    Swar = 2,
    Sse2 = 3,
    Avx2 = 4,
    Neon = 5,
    Simd128 = 6,
}

impl JsonBody {
    /// Every body, in order.
    pub const ALL: [JsonBody; 6] = [
        JsonBody::Scalar,
        JsonBody::Swar,
        JsonBody::Sse2,
        JsonBody::Avx2,
        JsonBody::Neon,
        JsonBody::Simd128,
    ];

    /// The `EP9_SIMD` and `EP9_SIMD_JSON` name of the body.
    pub const fn name(self) -> &'static str {
        match self {
            JsonBody::Scalar => "scalar",
            JsonBody::Swar => "swar",
            JsonBody::Sse2 => "sse2",
            JsonBody::Avx2 => "avx2",
            JsonBody::Neon => "neon",
            JsonBody::Simd128 => "simd128",
        }
    }

    /// Whether this build and the running CPU can run the body.
    pub fn available(self) -> bool {
        match self {
            JsonBody::Scalar | JsonBody::Swar => true,
            // SSE2 is part of the x86-64 base.
            JsonBody::Sse2 => cfg!(target_arch = "x86_64"),
            JsonBody::Avx2 => has(AVX2),
            JsonBody::Neon => cfg!(target_arch = "aarch64") && has(NEON),
            JsonBody::Simd128 => cfg!(target_arch = "wasm32") && has(SIMD128),
        }
    }
}

impl Body for JsonBody {
    const TABLE: &'static str = "json";
    fn all() -> &'static [Self] {
        &JsonBody::ALL
    }
    fn best_order() -> &'static [Self] {
        &[
            JsonBody::Avx2,
            JsonBody::Sse2,
            JsonBody::Neon,
            JsonBody::Simd128,
        ]
    }
    fn fallback() -> Self {
        JsonBody::Swar
    }
    fn name(self) -> &'static str {
        JsonBody::name(self)
    }
    fn available(self) -> bool {
        JsonBody::available(self)
    }
}

/// The SHA-256 block bodies (FIPS 180-4 compression over whole 64-byte blocks).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Sha256Body {
    /// Portable Rust, the loop of `ep9_bytecode` `sha256.rs` `compress`.
    Scalar = 1,
    /// x86-64 SHA extensions (`sha256rnds2`, `sha256msg1`, `sha256msg2`).
    ShaNi = 2,
    /// aarch64 FEAT_SHA256 (`sha256h`, `sha256h2`, `sha256su0`, `sha256su1`).
    ArmSha2 = 3,
}

impl Sha256Body {
    /// Every body, in order.
    pub const ALL: [Sha256Body; 3] = [Sha256Body::Scalar, Sha256Body::ShaNi, Sha256Body::ArmSha2];

    /// The `EP9_SIMD` name of the body.
    pub const fn name(self) -> &'static str {
        match self {
            Sha256Body::Scalar => "scalar",
            Sha256Body::ShaNi => "sha-ni",
            Sha256Body::ArmSha2 => "armv8-sha2",
        }
    }

    /// Whether this build and the running CPU can run the body.
    pub fn available(self) -> bool {
        match self {
            Sha256Body::Scalar => true,
            Sha256Body::ShaNi => cfg!(target_arch = "x86_64") && has(SHA_NI),
            Sha256Body::ArmSha2 => cfg!(target_arch = "aarch64") && has(SHA2),
        }
    }

    /// The block function of the body, if this CPU and build have it. This is the only way to
    /// get a SIMD body as a safe function: it is handed out only after the check.
    pub fn blocks(self) -> Option<Sha256Blocks> {
        if !self.available() {
            return None;
        }
        Some(match self {
            #[cfg(target_arch = "x86_64")]
            Sha256Body::ShaNi => sha256_sha_ni,
            #[cfg(target_arch = "aarch64")]
            Sha256Body::ArmSha2 => sha256_armv8,
            _ => sha256_scalar,
        })
    }
}

impl Body for Sha256Body {
    const TABLE: &'static str = "sha256";
    fn all() -> &'static [Self] {
        &Sha256Body::ALL
    }
    fn best_order() -> &'static [Self] {
        &[Sha256Body::ShaNi, Sha256Body::ArmSha2]
    }
    fn fallback() -> Self {
        Sha256Body::Scalar
    }
    fn name(self) -> &'static str {
        Sha256Body::name(self)
    }
    fn available(self) -> bool {
        Sha256Body::available(self)
    }
}

/// The body of table `B` for the values of `EP9_SIMD` (`simd`) and of the table's old name
/// (`old`): the forced body if this CPU and build have it, else the best one that they have
/// (the module doc gives the order).
pub fn pick<B: Body>(simd: Option<&str>, old: Option<&str>) -> B {
    let best = B::best_order()
        .iter()
        .copied()
        .find(|b| b.available())
        .unwrap_or_else(B::fallback);
    forced_name::<B>(simd, old)
        .and_then(|v| B::all().iter().copied().find(|b| b.name() == v))
        .filter(|b| b.available())
        .unwrap_or(best)
}

/// The body name that `EP9_SIMD` or the old name forces for table `B`, if any.
fn forced_name<'a, B: Body>(simd: Option<&'a str>, old: Option<&'a str>) -> Option<&'a str> {
    let items = || {
        simd.into_iter()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|i| !i.is_empty())
    };
    let pair = items().find_map(|i| {
        let (t, b) = i.split_once('=')?;
        (t.trim() == B::TABLE).then_some(b.trim())
    });
    pair.or(old.map(str::trim)).or_else(|| {
        items()
            .filter(|i| !i.contains('='))
            .find(|i| B::all().iter().any(|b| b.name() == *i))
    })
}

/// The picked body of each table (`Body as u8`), 0 before the first pick.
static SEARCH: AtomicU8 = AtomicU8::new(0);
static JSON: AtomicU8 = AtomicU8::new(0);
static SHA256: AtomicU8 = AtomicU8::new(0);

/// The body of each table, picked at once.
#[derive(Clone, Copy, Debug)]
struct Picks {
    search: SearchBody,
    json: JsonBody,
    sha256: Sha256Body,
}

/// Picks the body of every table from the bitset and the env, once per process (two threads
/// may both pick; they store the same values).
#[cold]
#[inline(never)]
fn pick_tables() -> Picks {
    let simd = std::env::var("EP9_SIMD").ok();
    let search_old = std::env::var("EP9_SIMD_SEARCH").ok();
    let json_old = std::env::var("EP9_SIMD_JSON").ok();
    let p = Picks {
        search: pick(simd.as_deref(), search_old.as_deref()),
        json: pick(simd.as_deref(), json_old.as_deref()),
        sha256: pick(simd.as_deref(), None),
    };
    SEARCH.store(p.search as u8, Ordering::Relaxed);
    JSON.store(p.json as u8, Ordering::Relaxed);
    SHA256.store(p.sha256 as u8, Ordering::Relaxed);
    p
}

/// The body of the SEARCH table for this process.
#[inline]
pub fn search() -> SearchBody {
    match SEARCH.load(Ordering::Relaxed) {
        2 => SearchBody::Sse2,
        3 => SearchBody::Avx2,
        4 => SearchBody::Neon,
        5 => SearchBody::Simd128,
        1 => SearchBody::Scalar,
        _ => pick_tables().search,
    }
}

/// The body of the JSON table for this process.
#[inline]
pub fn json() -> JsonBody {
    match JSON.load(Ordering::Relaxed) {
        1 => JsonBody::Scalar,
        2 => JsonBody::Swar,
        3 => JsonBody::Sse2,
        4 => JsonBody::Avx2,
        5 => JsonBody::Neon,
        6 => JsonBody::Simd128,
        _ => pick_tables().json,
    }
}

/// The body of the SHA256 table for this process.
#[inline]
pub fn sha256() -> Sha256Body {
    match SHA256.load(Ordering::Relaxed) {
        1 => Sha256Body::Scalar,
        2 => Sha256Body::ShaNi,
        3 => Sha256Body::ArmSha2,
        _ => pick_tables().sha256,
    }
}

/// The value for the SHA-256 body slot of `ep9_bytecode` (set by `ep9_vm`; ALLOC-SET design
/// 6.2): the block function of the SHA256 pick of this process, or `None` when the pick is the
/// scalar body, so that the slot stays unset and `ep9_bytecode` runs its own scalar loop.
pub fn sha256_slot_body() -> Option<Sha256Blocks> {
    match sha256() {
        Sha256Body::Scalar => None,
        b => b.blocks(),
    }
}

/// One line: the target, each feature bit and the body of every table, as `key=value` words
/// (`SIMD-REPORT target=aarch64 avx2=no sha_ni=no avx512_popcnt=no neon=yes sha2=yes simd128=no
/// search=neon json=neon sha256=armv8-sha2`). A run that forces a body prints it (never inside
/// a timed run): a force that the CPU cannot take is not an error, so only this line proves it.
pub fn report() -> String {
    let f = features();
    let mut line = format!("SIMD-REPORT target={}", std::env::consts::ARCH);
    for (bit, key) in FEATURE_NAMES {
        line.push_str(&format!(
            " {key}={}",
            if f & bit != 0 { "yes" } else { "no" }
        ));
    }
    line.push_str(&format!(
        " search={} json={} sha256={}",
        search().name(),
        json().name(),
        sha256().name()
    ));
    line
}

// ---- SHA-256 -------------------------------------------------------------------------------

/// A SHA-256 block function: the compression function over each 64-byte block, in order.
pub type Sha256Blocks = fn(&mut [u32; 8], &[[u8; 64]]);

/// The FIPS 180-4 round constants.
static K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The scalar body: the loop of `ep9_bytecode` `sha256.rs` `compress`, once per block.
pub fn sha256_scalar(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    for block in blocks {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }
}

/// The SHA-NI body as a safe function. Private: only [`Sha256Body::blocks`] hands it out, after
/// the [`SHA_NI`] check.
#[cfg(target_arch = "x86_64")]
fn sha256_sha_ni(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    // SAFETY: this function is reachable only through `Sha256Body::blocks`, which returns it
    // only when the CPU has SHA, SSE4.1 and SSSE3 (`SHA_NI`).
    unsafe { x86::sha256_blocks(state, blocks) }
}

/// The ARMv8 SHA2 body as a safe function. Private: only [`Sha256Body::blocks`] hands it out,
/// after the [`SHA2`] check.
#[cfg(target_arch = "aarch64")]
fn sha256_armv8(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    // SAFETY: this function is reachable only through `Sha256Body::blocks`, which returns it
    // only when the CPU has NEON and FEAT_SHA256 (`SHA2`).
    unsafe { arm::sha256_blocks(state, blocks) }
}

/// x86-64 SHA-NI. The state is kept as 2 vectors in the order that `sha256rnds2` wants: ABEF
/// and CDGH (high lane first). Each `sha256rnds2` runs 2 rounds with the 2 low lanes of its
/// W + K vector, so 4 rounds are 2 calls (the second on the high half, `pshufd 0x0E`).
#[cfg(target_arch = "x86_64")]
mod x86 {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_alignr_epi8, _mm_blend_epi16, _mm_loadu_si128, _mm_set_epi64x,
        _mm_sha256msg1_epu32, _mm_sha256msg2_epu32, _mm_sha256rnds2_epu32, _mm_shuffle_epi8,
        _mm_shuffle_epi32, _mm_storeu_si128,
    };

    /// 4 rounds: `w` holds W[4r..4r+4] (lane 0 first).
    macro_rules! rounds4 {
        ($abef:ident, $cdgh:ident, $w:expr, $r:expr) => {{
            // SAFETY: $r < 16, so K[4r..4r + 4] is inside the table of 64 words.
            let k = unsafe { _mm_loadu_si128(super::K.as_ptr().add(4 * $r).cast::<__m128i>()) };
            let wk = _mm_add_epi32($w, k);
            $cdgh = _mm_sha256rnds2_epu32($cdgh, $abef, wk);
            $abef = _mm_sha256rnds2_epu32($abef, $cdgh, _mm_shuffle_epi32::<0x0E>(wk));
        }};
    }

    /// The next 4 schedule words from the 4 before (`w0` oldest), then 4 rounds with them.
    /// `w0` becomes W[4r..4r+4].
    macro_rules! schedule4 {
        ($abef:ident, $cdgh:ident, $w0:ident, $w1:ident, $w2:ident, $w3:ident, $r:expr) => {{
            let t = _mm_add_epi32(
                _mm_sha256msg1_epu32($w0, $w1),
                _mm_alignr_epi8::<4>($w3, $w2),
            );
            $w0 = _mm_sha256msg2_epu32(t, $w3);
            rounds4!($abef, $cdgh, $w0, $r);
        }};
    }

    /// SHA-256 over whole blocks with the SHA extensions.
    ///
    /// # Safety
    /// The CPU has SHA, SSE4.1 and SSSE3 (`has(SHA_NI)`).
    #[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
    pub(super) unsafe fn sha256_blocks(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
        // Big-endian words: reverse the bytes of each 32-bit lane.
        let bswap = _mm_set_epi64x(0x0c0d_0e0f_0809_0a0b, 0x0405_0607_0001_0203);
        let sp = state.as_mut_ptr().cast::<__m128i>();
        // SAFETY: the state is 8 words = 2 vectors; `loadu` needs no alignment.
        let (dcba, hgfe) = unsafe { (_mm_loadu_si128(sp), _mm_loadu_si128(sp.add(1))) };
        let cdab = _mm_shuffle_epi32::<0xB1>(dcba);
        let efgh = _mm_shuffle_epi32::<0x1B>(hgfe);
        let mut abef = _mm_alignr_epi8::<8>(cdab, efgh);
        let mut cdgh = _mm_blend_epi16::<0xF0>(efgh, cdab);
        for block in blocks {
            let (abef0, cdgh0) = (abef, cdgh);
            let bp = block.as_ptr().cast::<__m128i>();
            // SAFETY: a block is 64 bytes = 4 vectors; `loadu` needs no alignment.
            let (mut w0, mut w1, mut w2, mut w3) = unsafe {
                (
                    _mm_shuffle_epi8(_mm_loadu_si128(bp), bswap),
                    _mm_shuffle_epi8(_mm_loadu_si128(bp.add(1)), bswap),
                    _mm_shuffle_epi8(_mm_loadu_si128(bp.add(2)), bswap),
                    _mm_shuffle_epi8(_mm_loadu_si128(bp.add(3)), bswap),
                )
            };
            rounds4!(abef, cdgh, w0, 0);
            rounds4!(abef, cdgh, w1, 1);
            rounds4!(abef, cdgh, w2, 2);
            rounds4!(abef, cdgh, w3, 3);
            schedule4!(abef, cdgh, w0, w1, w2, w3, 4);
            schedule4!(abef, cdgh, w1, w2, w3, w0, 5);
            schedule4!(abef, cdgh, w2, w3, w0, w1, 6);
            schedule4!(abef, cdgh, w3, w0, w1, w2, 7);
            schedule4!(abef, cdgh, w0, w1, w2, w3, 8);
            schedule4!(abef, cdgh, w1, w2, w3, w0, 9);
            schedule4!(abef, cdgh, w2, w3, w0, w1, 10);
            schedule4!(abef, cdgh, w3, w0, w1, w2, 11);
            schedule4!(abef, cdgh, w0, w1, w2, w3, 12);
            schedule4!(abef, cdgh, w1, w2, w3, w0, 13);
            schedule4!(abef, cdgh, w2, w3, w0, w1, 14);
            schedule4!(abef, cdgh, w3, w0, w1, w2, 15);
            abef = _mm_add_epi32(abef, abef0);
            cdgh = _mm_add_epi32(cdgh, cdgh0);
        }
        let feba = _mm_shuffle_epi32::<0x1B>(abef);
        let dchg = _mm_shuffle_epi32::<0xB1>(cdgh);
        let dcba = _mm_blend_epi16::<0xF0>(feba, dchg);
        let hgef = _mm_alignr_epi8::<8>(dchg, feba);
        // SAFETY: as for the loads.
        unsafe {
            _mm_storeu_si128(sp, dcba);
            _mm_storeu_si128(sp.add(1), hgef);
        }
    }
}

/// aarch64 FEAT_SHA256. The state is ABCD and EFGH (lane 0 first); `sha256h` and `sha256h2`
/// run 4 rounds with one W + K vector.
#[cfg(target_arch = "aarch64")]
mod arm {
    use core::arch::aarch64::{
        vaddq_u32, vld1q_u8, vld1q_u32, vreinterpretq_u32_u8, vrev32q_u8, vsha256h2q_u32,
        vsha256hq_u32, vsha256su0q_u32, vsha256su1q_u32, vst1q_u32,
    };

    /// 4 rounds: `w` holds W[4r..4r+4] (lane 0 first).
    macro_rules! rounds4 {
        ($abcd:ident, $efgh:ident, $w:expr, $r:expr) => {{
            // SAFETY: $r < 16, so K[4r..4r + 4] is inside the table of 64 words.
            let wk = vaddq_u32($w, unsafe { vld1q_u32(super::K.as_ptr().add(4 * $r)) });
            let prev = $abcd;
            $abcd = vsha256hq_u32(prev, $efgh, wk);
            $efgh = vsha256h2q_u32($efgh, prev, wk);
        }};
    }

    /// The next 4 schedule words from the 4 before (`w0` oldest), then 4 rounds with them.
    macro_rules! schedule4 {
        ($abcd:ident, $efgh:ident, $w0:ident, $w1:ident, $w2:ident, $w3:ident, $r:expr) => {{
            $w0 = vsha256su1q_u32(vsha256su0q_u32($w0, $w1), $w2, $w3);
            rounds4!($abcd, $efgh, $w0, $r);
        }};
    }

    /// SHA-256 over whole blocks with the ARMv8 SHA2 instructions.
    ///
    /// # Safety
    /// The CPU has NEON and FEAT_SHA256 (`has(SHA2)`).
    #[target_feature(enable = "neon,sha2")]
    pub(super) unsafe fn sha256_blocks(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
        let sp = state.as_mut_ptr();
        // SAFETY: the state is 8 words = 2 vectors of 4.
        let (mut abcd, mut efgh) = unsafe { (vld1q_u32(sp), vld1q_u32(sp.add(4))) };
        for block in blocks {
            let (abcd0, efgh0) = (abcd, efgh);
            let bp = block.as_ptr();
            // SAFETY: a block is 64 bytes = 4 vectors of 16; big-endian words: reverse the
            // bytes of each 32-bit lane.
            let (mut w0, mut w1, mut w2, mut w3) = unsafe {
                (
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(bp))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(bp.add(16)))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(bp.add(32)))),
                    vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(bp.add(48)))),
                )
            };
            rounds4!(abcd, efgh, w0, 0);
            rounds4!(abcd, efgh, w1, 1);
            rounds4!(abcd, efgh, w2, 2);
            rounds4!(abcd, efgh, w3, 3);
            schedule4!(abcd, efgh, w0, w1, w2, w3, 4);
            schedule4!(abcd, efgh, w1, w2, w3, w0, 5);
            schedule4!(abcd, efgh, w2, w3, w0, w1, 6);
            schedule4!(abcd, efgh, w3, w0, w1, w2, 7);
            schedule4!(abcd, efgh, w0, w1, w2, w3, 8);
            schedule4!(abcd, efgh, w1, w2, w3, w0, 9);
            schedule4!(abcd, efgh, w2, w3, w0, w1, 10);
            schedule4!(abcd, efgh, w3, w0, w1, w2, 11);
            schedule4!(abcd, efgh, w0, w1, w2, w3, 12);
            schedule4!(abcd, efgh, w1, w2, w3, w0, 13);
            schedule4!(abcd, efgh, w2, w3, w0, w1, 14);
            schedule4!(abcd, efgh, w3, w0, w1, w2, 15);
            abcd = vaddq_u32(abcd, abcd0);
            efgh = vaddq_u32(efgh, efgh0);
        }
        // SAFETY: as for the loads.
        unsafe {
            vst1q_u32(sp, abcd);
            vst1q_u32(sp.add(4), efgh);
        }
    }
}
