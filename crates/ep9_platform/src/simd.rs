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

// ---- the sweep bitmaps (SWEEP-LEAN; ALLOC-SET design 1.3; FD L3 (b)) -----------------------
//
// ```text
// SWEEP     scalar, sse2, avx2, avx512-popcnt, neon, simd128    the bodies: here; the caller:
//           ep9_heap heap.rs Sweeper::sweep_page_with (the lean walk: no zap, no CHECK)
// sweep()   the pick of this table: once per process, from the bitset and EP9_SIMD (the forms of
//           the module doc; this table has no old name)
// sweep_report()   "SIMD-REPORT-SWEEP target=<arch> sweep=<body>"
// ```
//
// One call is one small page of `ep9_heap`: SWEEP_WORDS words of the VO bitmap and of the mark
// bitmap (64 granules a word), SWEEP_LINES lines (one line = 32 granules = one u32 half of a
// word, the low half first). For each word w, with v = vo[w], m = mk[w], a = v & m (the live
// cells) and, per half h (line l = 2w + h), x = the half of a and y = the half of v:
//
//   live += popcount(a)          dead += popcount(v & !m)
//   lines[l] = (x != 0)          a live cell starts in line l
//   ends[l]  = ((y & !x) < x)    no cell (live or dead) starts after the last live one of line l
//                                (x != 0 is implied; unsigned compare)
//   vo[w] = a                    mk[w] = a & keep if v != 0, else mk[w] (left as it was)
//
// So every body is 3 pure passes over the words, and the store-backs. The caller then sizes the
// last live cell of each line l with ends[l] and no live start in line l + 1 (it may reach into
// the next lines): that fix stays scalar and reads live cells only.

/// Words of one bitmap of a small page (`ep9_heap`: 256 KiB pages, 8-byte granules).
pub const SWEEP_WORDS: usize = 512;
/// Lines of a small page: 2 per bitmap word.
pub const SWEEP_LINES: usize = 2 * SWEEP_WORDS;

/// What one sweep bitmap pass counted.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SweepCounts {
    /// Bits of `vo & mk`: the cells kept.
    pub live: u32,
    /// Bits of `vo & !mk`: the cells freed.
    pub dead: u32,
}

/// A sweep bitmap body (the pass of the section comment above).
///
/// # Safety
///
/// `vo` and `mk` point to [`SWEEP_WORDS`] u64 each (8-aligned), `lines` and `ends` to
/// [`SWEEP_LINES`] bytes each; the 4 ranges do not overlap, and no other thread reads or writes
/// them during the call. The CPU check is not the caller's: [`SweepBody::bitmaps`] hands out a
/// body only when this CPU and build have it.
pub type SweepBitmaps =
    unsafe fn(vo: *mut u64, mk: *mut u64, keep: u64, lines: *mut u8, ends: *mut u8) -> SweepCounts;

/// The bodies of the sweep bitmap pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SweepBody {
    /// The reference: one word at a time.
    Scalar = 1,
    /// x86-64 base: 2 words per step.
    Sse2 = 2,
    /// x86-64 AVX2: 4 words per step (population count by nibble table).
    Avx2 = 3,
    /// x86-64 AVX-512 VL + VPOPCNTDQ on 256-bit lanes: 4 words per step, mask compares.
    Avx512Popcnt = 4,
    /// aarch64 NEON: 4 words per step (`cnt`).
    Neon = 5,
    /// wasm32 simd128: 4 words per step (`i8x16.popcnt`); a build fact.
    Simd128 = 6,
}

impl SweepBody {
    /// Every body, in order.
    pub const ALL: [SweepBody; 6] = [
        SweepBody::Scalar,
        SweepBody::Sse2,
        SweepBody::Avx2,
        SweepBody::Avx512Popcnt,
        SweepBody::Neon,
        SweepBody::Simd128,
    ];

    /// The `EP9_SIMD` name of the body.
    pub const fn name(self) -> &'static str {
        match self {
            SweepBody::Scalar => "scalar",
            SweepBody::Sse2 => "sse2",
            SweepBody::Avx2 => "avx2",
            SweepBody::Avx512Popcnt => "avx512-popcnt",
            SweepBody::Neon => "neon",
            SweepBody::Simd128 => "simd128",
        }
    }

    /// Whether this build and the running CPU can run the body.
    pub fn available(self) -> bool {
        match self {
            SweepBody::Scalar => true,
            // SSE2 is part of the x86-64 base.
            SweepBody::Sse2 => cfg!(target_arch = "x86_64"),
            SweepBody::Avx2 => cfg!(target_arch = "x86_64") && has(AVX2),
            SweepBody::Avx512Popcnt => cfg!(target_arch = "x86_64") && has(AVX512_POPCNT),
            SweepBody::Neon => cfg!(target_arch = "aarch64") && has(NEON),
            SweepBody::Simd128 => {
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")) && has(SIMD128)
            }
        }
    }

    /// The pass of the body, if this CPU and build have it (the only way to get a SIMD body:
    /// it is handed out only after the check).
    pub fn bitmaps(self) -> Option<SweepBitmaps> {
        if !self.available() {
            return None;
        }
        Some(match self {
            #[cfg(target_arch = "x86_64")]
            SweepBody::Sse2 => sweep_x86::sse2,
            #[cfg(target_arch = "x86_64")]
            SweepBody::Avx2 => sweep_x86::avx2,
            #[cfg(target_arch = "x86_64")]
            SweepBody::Avx512Popcnt => sweep_x86::avx512,
            #[cfg(target_arch = "aarch64")]
            SweepBody::Neon => sweep_arm::neon,
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            SweepBody::Simd128 => sweep_wasm::simd128,
            _ => sweep_scalar,
        })
    }
}

impl Body for SweepBody {
    const TABLE: &'static str = "sweep";
    fn all() -> &'static [Self] {
        &SweepBody::ALL
    }
    fn best_order() -> &'static [Self] {
        &[
            SweepBody::Avx512Popcnt,
            SweepBody::Avx2,
            SweepBody::Sse2,
            SweepBody::Neon,
            SweepBody::Simd128,
        ]
    }
    fn fallback() -> Self {
        SweepBody::Scalar
    }
    fn name(self) -> &'static str {
        SweepBody::name(self)
    }
    fn available(self) -> bool {
        SweepBody::available(self)
    }
}

/// The picked body of the SWEEP table (`SweepBody as u8`), 0 before the first pick.
static SWEEP: AtomicU8 = AtomicU8::new(0);

/// The body of the SWEEP table for this process.
#[inline]
pub fn sweep() -> SweepBody {
    match SWEEP.load(Ordering::Relaxed) {
        1 => SweepBody::Scalar,
        2 => SweepBody::Sse2,
        3 => SweepBody::Avx2,
        4 => SweepBody::Avx512Popcnt,
        5 => SweepBody::Neon,
        6 => SweepBody::Simd128,
        _ => pick_sweep(),
    }
}

/// Picks the SWEEP body from the bitset and `EP9_SIMD`, once per process (two threads may both
/// pick; they store the same value).
#[cold]
#[inline(never)]
fn pick_sweep() -> SweepBody {
    let b: SweepBody = pick(std::env::var("EP9_SIMD").ok().as_deref(), None);
    SWEEP.store(b as u8, Ordering::Relaxed);
    b
}

/// The pass of the SWEEP pick of this process.
#[inline]
pub fn sweep_bitmaps() -> SweepBitmaps {
    sweep().bitmaps().unwrap_or(sweep_scalar)
}

/// One line: the target and the SWEEP body of this process (`SIMD-REPORT-SWEEP target=aarch64
/// sweep=neon`). A run that forces the body prints it (never inside a timed run).
pub fn sweep_report() -> String {
    format!(
        "SIMD-REPORT-SWEEP target={} sweep={}",
        std::env::consts::ARCH,
        sweep().name()
    )
}

/// The scalar body (the reference of every other body).
///
/// # Safety
///
/// The contract of [`SweepBitmaps`].
pub unsafe fn sweep_scalar(
    vo: *mut u64,
    mk: *mut u64,
    keep: u64,
    lines: *mut u8,
    ends: *mut u8,
) -> SweepCounts {
    let mut c = SweepCounts::default();
    for w in 0..SWEEP_WORDS {
        // SAFETY: w < SWEEP_WORDS and 2w + 1 < SWEEP_LINES: inside the 4 ranges of the contract.
        unsafe {
            let v = vo.add(w).read();
            let m = mk.add(w).read();
            let a = v & m;
            c.live += a.count_ones();
            c.dead += (v & !m).count_ones();
            for h in 0..2 {
                let x = (a >> (32 * h)) as u32;
                let y = (v >> (32 * h)) as u32;
                lines.add(2 * w + h).write(u8::from(x != 0));
                ends.add(2 * w + h).write(u8::from(y & !x < x));
            }
            vo.add(w).write(a);
            if v != 0 {
                mk.add(w).write(a & keep);
            }
        }
    }
    c
}

/// Byte i of `SPREAD[m]` is bit i of `m` (8 mask bits to 8 line bytes).
#[cfg(target_arch = "x86_64")]
static SPREAD: [u64; 256] = {
    let mut t = [0u64; 256];
    let mut m = 0;
    while m < 256 {
        let mut i = 0;
        while i < 8 {
            if m >> i & 1 != 0 {
                t[m] |= 1 << (8 * i);
            }
            i += 1;
        }
        m += 1;
    }
    t
};

#[cfg(target_arch = "x86_64")]
mod sweep_x86 {
    use super::{SPREAD, SWEEP_WORDS, SweepCounts};
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_and_si128, _mm_andnot_si128, _mm_castsi128_ps, _mm_cmpeq_epi32,
        _mm_cmpgt_epi32, _mm_loadu_si128, _mm_movemask_ps, _mm_or_si128, _mm_set1_epi32,
        _mm_set1_epi64x, _mm_setzero_si128, _mm_shuffle_epi32, _mm_storeu_si128, _mm_xor_si128,
        _mm256_add_epi8, _mm256_add_epi64, _mm256_and_si256, _mm256_andnot_si256,
        _mm256_blendv_epi8, _mm256_castsi256_ps, _mm256_cmpeq_epi32, _mm256_cmpeq_epi64,
        _mm256_cmpgt_epi32, _mm256_cmplt_epu32_mask, _mm256_extract_epi64, _mm256_loadu_si256,
        _mm256_mask_blend_epi64, _mm256_movemask_ps, _mm256_popcnt_epi64, _mm256_sad_epu8,
        _mm256_set1_epi8, _mm256_set1_epi32, _mm256_set1_epi64x, _mm256_setr_epi8,
        _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_srli_epi16, _mm256_storeu_si256,
        _mm256_test_epi32_mask, _mm256_test_epi64_mask, _mm256_xor_si256,
    };

    /// Writes the 8 line bytes of mask `m` (bit i = line i) at `p`.
    ///
    /// # Safety
    ///
    /// `p` is valid for 8 byte writes.
    #[inline(always)]
    unsafe fn spread8(p: *mut u8, m: u32) {
        // SAFETY: m < 256 (a movemask or mask of 8 lanes); the caller gives 8 bytes at `p`.
        unsafe { p.cast::<u64>().write_unaligned(SPREAD[m as usize & 0xff]) };
    }

    /// The byte population counts of `x` (nibble table).
    #[target_feature(enable = "avx2")]
    fn popcnt8(x: __m256i) -> __m256i {
        let low = _mm256_set1_epi8(0x0f);
        let lut = _mm256_setr_epi8(
            0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2,
            3, 3, 4,
        );
        let lo = _mm256_and_si256(x, low);
        let hi = _mm256_and_si256(_mm256_srli_epi16::<4>(x), low);
        _mm256_add_epi8(_mm256_shuffle_epi8(lut, lo), _mm256_shuffle_epi8(lut, hi))
    }

    /// The sum of the 4 u64 lanes of `x` (a count of at most 32768 bits).
    #[target_feature(enable = "avx2")]
    fn sum4(x: __m256i) -> u32 {
        (_mm256_extract_epi64::<0>(x)
            + _mm256_extract_epi64::<1>(x)
            + _mm256_extract_epi64::<2>(x)
            + _mm256_extract_epi64::<3>(x)) as u32
    }

    /// The SSE2 body: 2 words (4 lines) per step; scalar population counts.
    ///
    /// # Safety
    ///
    /// The contract of [`super::SweepBitmaps`] (SSE2 is part of the x86-64 base).
    #[target_feature(enable = "sse2")]
    pub(super) unsafe fn sse2(
        vo: *mut u64,
        mk: *mut u64,
        keep: u64,
        lines: *mut u8,
        ends: *mut u8,
    ) -> SweepCounts {
        let mut c = SweepCounts::default();
        let keepv = _mm_set1_epi64x(keep as i64);
        let sign = _mm_set1_epi32(i32::MIN);
        let zero = _mm_setzero_si128();
        let mut w = 0;
        while w < SWEEP_WORDS {
            // SAFETY: w + 2 <= SWEEP_WORDS and 2w + 4 <= SWEEP_LINES: inside the 4 ranges of
            // the contract; unaligned loads and stores.
            unsafe {
                let v = _mm_loadu_si128(vo.add(w).cast::<__m128i>());
                let m = _mm_loadu_si128(mk.add(w).cast::<__m128i>());
                let a = _mm_and_si128(v, m);
                for i in w..w + 2 {
                    let (vi, mi) = (vo.add(i).read(), mk.add(i).read());
                    c.live += (vi & mi).count_ones();
                    c.dead += (vi & !mi).count_ones();
                }
                let nz = _mm_cmpeq_epi32(_mm_cmpeq_epi32(a, zero), zero);
                let rest = _mm_andnot_si128(a, v);
                let lt = _mm_cmpgt_epi32(_mm_xor_si128(a, sign), _mm_xor_si128(rest, sign));
                let lm = _mm_movemask_ps(_mm_castsi128_ps(nz)) as u32;
                let em = _mm_movemask_ps(_mm_castsi128_ps(lt)) as u32;
                lines
                    .add(2 * w)
                    .cast::<u32>()
                    .write_unaligned(SPREAD[lm as usize] as u32);
                ends.add(2 * w)
                    .cast::<u32>()
                    .write_unaligned(SPREAD[em as usize] as u32);
                // v == 0 per 64-bit lane: both 32-bit halves equal 0.
                let z32 = _mm_cmpeq_epi32(v, zero);
                let z64 = _mm_and_si128(z32, _mm_shuffle_epi32::<0xB1>(z32));
                let mk_new = _mm_or_si128(
                    _mm_and_si128(z64, m),
                    _mm_andnot_si128(z64, _mm_and_si128(a, keepv)),
                );
                _mm_storeu_si128(vo.add(w).cast::<__m128i>(), a);
                _mm_storeu_si128(mk.add(w).cast::<__m128i>(), mk_new);
            }
            w += 2;
        }
        c
    }

    /// The AVX2 body: 4 words (8 lines) per step; population counts by nibble table.
    ///
    /// # Safety
    ///
    /// The contract of [`super::SweepBitmaps`], on a CPU with AVX2.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn avx2(
        vo: *mut u64,
        mk: *mut u64,
        keep: u64,
        lines: *mut u8,
        ends: *mut u8,
    ) -> SweepCounts {
        let keepv = _mm256_set1_epi64x(keep as i64);
        let sign = _mm256_set1_epi32(i32::MIN);
        let zero = _mm256_setzero_si256();
        let mut live = zero;
        let mut dead = zero;
        let mut w = 0;
        while w < SWEEP_WORDS {
            // SAFETY: w + 4 <= SWEEP_WORDS and 2w + 8 <= SWEEP_LINES: inside the 4 ranges of
            // the contract; unaligned loads and stores.
            unsafe {
                let v = _mm256_loadu_si256(vo.add(w).cast::<__m256i>());
                let m = _mm256_loadu_si256(mk.add(w).cast::<__m256i>());
                let a = _mm256_and_si256(v, m);
                let d = _mm256_andnot_si256(m, v);
                live = _mm256_add_epi64(live, _mm256_sad_epu8(popcnt8(a), zero));
                dead = _mm256_add_epi64(dead, _mm256_sad_epu8(popcnt8(d), zero));
                let z = _mm256_cmpeq_epi32(a, zero);
                let rest = _mm256_andnot_si256(a, v);
                let lt =
                    _mm256_cmpgt_epi32(_mm256_xor_si256(a, sign), _mm256_xor_si256(rest, sign));
                let lm = !(_mm256_movemask_ps(_mm256_castsi256_ps(z)) as u32) & 0xff;
                let em = _mm256_movemask_ps(_mm256_castsi256_ps(lt)) as u32;
                spread8(lines.add(2 * w), lm);
                spread8(ends.add(2 * w), em);
                let z64 = _mm256_cmpeq_epi64(v, zero);
                let mk_new = _mm256_blendv_epi8(_mm256_and_si256(a, keepv), m, z64);
                _mm256_storeu_si256(vo.add(w).cast::<__m256i>(), a);
                _mm256_storeu_si256(mk.add(w).cast::<__m256i>(), mk_new);
            }
            w += 4;
        }
        SweepCounts {
            live: sum4(live),
            dead: sum4(dead),
        }
    }

    /// The AVX-512 VL + VPOPCNTDQ body on 256-bit lanes: 4 words (8 lines) per step; mask
    /// compares.
    ///
    /// # Safety
    ///
    /// The contract of [`super::SweepBitmaps`], on a CPU with AVX-512 F, VL and VPOPCNTDQ.
    #[target_feature(enable = "avx512f,avx512vl,avx512vpopcntdq,avx2")]
    pub(super) unsafe fn avx512(
        vo: *mut u64,
        mk: *mut u64,
        keep: u64,
        lines: *mut u8,
        ends: *mut u8,
    ) -> SweepCounts {
        let keepv = _mm256_set1_epi64x(keep as i64);
        let zero = _mm256_setzero_si256();
        let mut live = zero;
        let mut dead = zero;
        let mut w = 0;
        while w < SWEEP_WORDS {
            // SAFETY: w + 4 <= SWEEP_WORDS and 2w + 8 <= SWEEP_LINES: inside the 4 ranges of
            // the contract; unaligned loads and stores.
            unsafe {
                let v = _mm256_loadu_si256(vo.add(w).cast::<__m256i>());
                let m = _mm256_loadu_si256(mk.add(w).cast::<__m256i>());
                let a = _mm256_and_si256(v, m);
                let d = _mm256_andnot_si256(m, v);
                live = _mm256_add_epi64(live, _mm256_popcnt_epi64(a));
                dead = _mm256_add_epi64(dead, _mm256_popcnt_epi64(d));
                let lm = _mm256_test_epi32_mask(a, a) as u32;
                let em = _mm256_cmplt_epu32_mask(_mm256_andnot_si256(a, v), a) as u32;
                spread8(lines.add(2 * w), lm);
                spread8(ends.add(2 * w), em);
                let nz64 = _mm256_test_epi64_mask(v, v);
                let mk_new = _mm256_mask_blend_epi64(nz64, m, _mm256_and_si256(a, keepv));
                _mm256_storeu_si256(vo.add(w).cast::<__m256i>(), a);
                _mm256_storeu_si256(mk.add(w).cast::<__m256i>(), mk_new);
            }
            w += 4;
        }
        SweepCounts {
            live: sum4(live),
            dead: sum4(dead),
        }
    }
}

#[cfg(target_arch = "aarch64")]
mod sweep_arm {
    use super::{SWEEP_WORDS, SweepCounts};
    use core::arch::aarch64::{
        uint8x8_t, uint32x4_t, vaddvq_u8, vand_u8, vandq_u64, vbicq_u32, vbicq_u64, vbslq_u64,
        vceqzq_u64, vcltq_u32, vcntq_u8, vcombine_u16, vdup_n_u8, vdupq_n_u32, vdupq_n_u64,
        vld1q_u64, vmovn_u16, vmovn_u32, vreinterpretq_u8_u64, vreinterpretq_u32_u64, vst1_u8,
        vst1q_u64, vtstq_u32,
    };

    /// 8 lane masks (0 or all ones; lines 8k..8k+8) to 8 bytes 0 or 1.
    #[target_feature(enable = "neon")]
    fn bytes8(lo: uint32x4_t, hi: uint32x4_t) -> uint8x8_t {
        vand_u8(
            vmovn_u16(vcombine_u16(vmovn_u32(lo), vmovn_u32(hi))),
            vdup_n_u8(1),
        )
    }

    /// The NEON body: 4 words (8 lines) per step; `cnt` population counts.
    ///
    /// # Safety
    ///
    /// The contract of [`super::SweepBitmaps`], on a CPU with NEON.
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn neon(
        vo: *mut u64,
        mk: *mut u64,
        keep: u64,
        lines: *mut u8,
        ends: *mut u8,
    ) -> SweepCounts {
        let keepv = vdupq_n_u64(keep);
        let mut c = SweepCounts::default();
        let mut w = 0;
        while w < SWEEP_WORDS {
            let mut lm = [vdupq_n_u32(0); 2];
            let mut em = lm;
            for (k, (lmk, emk)) in lm.iter_mut().zip(em.iter_mut()).enumerate() {
                let i = w + 2 * k;
                // SAFETY: i + 2 <= SWEEP_WORDS: inside the 2 bitmaps of the contract.
                let (v, m) = unsafe { (vld1q_u64(vo.add(i)), vld1q_u64(mk.add(i))) };
                let a = vandq_u64(v, m);
                let d = vbicq_u64(v, m);
                // Each byte count is at most 8, so 16 of them fit a u8.
                c.live += u32::from(vaddvq_u8(vcntq_u8(vreinterpretq_u8_u64(a))));
                c.dead += u32::from(vaddvq_u8(vcntq_u8(vreinterpretq_u8_u64(d))));
                let x = vreinterpretq_u32_u64(a);
                let y = vreinterpretq_u32_u64(v);
                *lmk = vtstq_u32(x, x);
                *emk = vcltq_u32(vbicq_u32(y, x), x);
                let mk_new = vbslq_u64(vceqzq_u64(v), m, vandq_u64(a, keepv));
                // SAFETY: as for the loads.
                unsafe {
                    vst1q_u64(vo.add(i), a);
                    vst1q_u64(mk.add(i), mk_new);
                }
            }
            // SAFETY: 2w + 8 <= SWEEP_LINES: inside the 2 line ranges of the contract.
            unsafe {
                vst1_u8(lines.add(2 * w), bytes8(lm[0], lm[1]));
                vst1_u8(ends.add(2 * w), bytes8(em[0], em[1]));
            }
            w += 4;
        }
        c
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod sweep_wasm {
    use super::{SWEEP_WORDS, SweepCounts};
    use core::arch::wasm32::{
        i8x16_narrow_i16x8, i8x16_popcnt, i16x8_narrow_i32x4, i32x4_add, i64x2_eq,
        i64x2_extract_lane, i64x2_splat, u16x8_extadd_pairwise_u8x16, u32x4_extadd_pairwise_u16x8,
        u32x4_extract_lane, u32x4_lt, u32x4_ne, u32x4_splat, v128, v128_and, v128_andnot,
        v128_bitselect, v128_load, v128_store,
    };

    /// The simd128 body: 4 words (8 lines) per step; `i8x16.popcnt`.
    ///
    /// # Safety
    ///
    /// The contract of [`super::SweepBitmaps`] (simd128 is on for the whole build).
    pub(super) unsafe fn simd128(
        vo: *mut u64,
        mk: *mut u64,
        keep: u64,
        lines: *mut u8,
        ends: *mut u8,
    ) -> SweepCounts {
        let keepv = i64x2_splat(keep as i64);
        let zero = i64x2_splat(0);
        let one = u32x4_splat(1);
        let count =
            |x: v128| u32x4_extadd_pairwise_u16x8(u16x8_extadd_pairwise_u8x16(i8x16_popcnt(x)));
        let mut live = zero;
        let mut dead = zero;
        let mut w = 0;
        while w < SWEEP_WORDS {
            let mut lm = [zero; 2];
            let mut em = [zero; 2];
            for k in 0..2 {
                let i = w + 2 * k;
                // SAFETY: i + 2 <= SWEEP_WORDS: inside the 2 bitmaps of the contract.
                let (v, m) = unsafe {
                    (
                        v128_load(vo.add(i).cast::<v128>()),
                        v128_load(mk.add(i).cast::<v128>()),
                    )
                };
                let a = v128_and(v, m);
                let d = v128_andnot(v, m);
                live = i32x4_add(live, count(a));
                dead = i32x4_add(dead, count(d));
                lm[k] = v128_and(u32x4_ne(a, zero), one);
                em[k] = v128_and(u32x4_lt(v128_andnot(v, a), a), one);
                let mk_new = v128_bitselect(m, v128_and(a, keepv), i64x2_eq(v, zero));
                // SAFETY: as for the loads.
                unsafe {
                    v128_store(vo.add(i).cast::<v128>(), a);
                    v128_store(mk.add(i).cast::<v128>(), mk_new);
                }
            }
            let lb = i8x16_narrow_i16x8(i16x8_narrow_i32x4(lm[0], lm[1]), zero);
            let eb = i8x16_narrow_i16x8(i16x8_narrow_i32x4(em[0], em[1]), zero);
            // SAFETY: 2w + 8 <= SWEEP_LINES: inside the 2 line ranges of the contract.
            unsafe {
                lines
                    .add(2 * w)
                    .cast::<i64>()
                    .write_unaligned(i64x2_extract_lane::<0>(lb));
                ends.add(2 * w)
                    .cast::<i64>()
                    .write_unaligned(i64x2_extract_lane::<0>(eb));
            }
            w += 4;
        }
        let sum = |x: v128| {
            u32x4_extract_lane::<0>(x)
                + u32x4_extract_lane::<1>(x)
                + u32x4_extract_lane::<2>(x)
                + u32x4_extract_lane::<3>(x)
        };
        SweepCounts {
            live: sum(live),
            dead: sum(dead),
        }
    }
}
