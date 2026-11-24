//! Layer 1 of the depth guard (H01, H23-Q10): a stack check at each recursion entry of the oxc
//! parse, and the ep9 side of it.
//!
//! ```text
//! worker thread start (worker.rs)  sp_entry = stack_address(); limit = sp_entry - (W - RED)
//! inline entry (Stack::Inline)     the same line at the front-end entry; inline only if
//!                                  limit >= low + RED, else the worker
//! each oxc parse (regex_check)     L1Config { limit }: at each recursion entry of the patched
//!                                  oxc_parser (third_party/patches/oxc/0002), sp >= limit, else a
//!                                  sticky stop --> ParserReturn::stack_limit_hit
//!                                  --> RangeError(StackLimit)
//! each regex literal (regex_check) groups x REGEX_PER_GROUP x M > sp - limit --> StackLimit
//! after layer 2 (lib.rs check_depth) depth x PASS_PER_LEVEL x M > sp - limit --> StackLimit
//! ep9 walks (backstop)             layer 2, rewrite_completion, scope pass, BytecodeGen:
//!                                  sp < limit at a depth step --> RangeError
//! ```
//!
//! - W is the requested stack size of the thread (`stack::WORKER_STACK_BYTES` for every
//!   production job), RED is `stack::RED`. The room above the limit is `W - RED` =
//!   `stack::BUDGET_BYTES` on both paths, so the result does not depend on the caller's stack.
//!   Within the budget the check never fires, and the caps decide (H23).
//! - The stop is sticky twice. oxc keeps its flag outside the checkpoint state (a rewind drops a
//!   fatal error), and [`L1Config`] sets its limit to `usize::MAX` at the first stop, so after a
//!   rewind every later site stops at once.
//! - Fail closed: a thread with no limit (not a front-end worker and not in a limit scope) parses
//!   with `limit = usize::MAX`, so its first site stops. That is a StackLimit, never an abort.
//!   The budget checks and the backstops act only where a limit is set: a pass runs on a parse
//!   result, and a thread with no limit has none.
//! - The budget checks (M = the margin of `stack.rs`, 1.5) guard the recursion that has no check
//!   of its own: the regex parser, oxc_semantic and the oxc `Visit` walks of the eval path. On the
//!   full worker they never fire (the budget holds the caps with M); on a smaller thread they stop
//!   before such a walk starts.
//! - [`stack_address`] reads the stack pointer with asm on aarch64 and x86-64 (the 4 tier-1
//!   targets): the fuzz build uses ASan, and there a local can live off the real stack. Other
//!   targets use the address of a local.

use std::cell::Cell;

#[cfg(debug_assertions)]
use oxc_parser::config::NoTokensLexerConfig;
use oxc_parser::config::{ParserConfig, TokensLexerConfig};

use crate::stack::{
    PASS_PER_LEVEL, RED, REGEX_PER_GROUP, SAFETY_DEN, SAFETY_NUM, WORKER_STACK_BYTES,
};
use crate::worker::{self, SpawnError};

thread_local! {
    /// The stack limit of this thread: 0 when none is set.
    static LIMIT: Cell<usize> = const { Cell::new(0) };
}

/// The stack pointer at the call site (the form of `ep9_platform::stack::stack_pointer`).
#[inline(always)]
pub(crate) fn stack_address() -> usize {
    #[cfg(target_arch = "aarch64")]
    {
        let sp: usize;
        // SAFETY: the asm only copies the stack pointer into a register.
        unsafe {
            core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
        }
        sp
    }
    #[cfg(target_arch = "x86_64")]
    {
        let sp: usize;
        // SAFETY: the asm only copies the stack pointer into a register.
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) sp, options(nomem, nostack, preserves_flags));
        }
        sp
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let marker = 0u8;
        std::hint::black_box(&raw const marker) as usize
    }
}

/// The limit of a stack of `stack_bytes` whose job starts at `sp_entry`: `stack_bytes - RED` of
/// room above it. Never 0 (0 means "no limit").
const fn limit_below(sp_entry: usize, stack_bytes: usize) -> usize {
    let room = stack_bytes.saturating_sub(RED);
    let limit = sp_entry.saturating_sub(room);
    if limit == 0 { 1 } else { limit }
}

/// The worker thread start (worker.rs): the limit of this thread, from its own requested stack
/// size. A nested job on the thread keeps it.
pub(crate) fn start_thread(stack_bytes: usize) {
    LIMIT.with(|l| l.set(limit_below(stack_address(), stack_bytes)));
}

/// The stack limit of this thread, 0 when none is set.
pub(crate) fn thread_limit() -> usize {
    LIMIT.with(Cell::get)
}

/// While it lives, this thread has the limit that it set; the old value comes back when it
/// drops (also on a panic).
pub(crate) struct LimitScope(usize);

impl LimitScope {
    fn set(limit: usize) -> Self {
        Self(LIMIT.with(|l| l.replace(limit)))
    }

    /// The limit of a thread stack of `stack_bytes` that starts here. For a thread that parses
    /// and is not a front-end worker (the probe threads, the test helpers that parse on the test
    /// thread): without it, every parse there stops at its first site.
    #[cfg(test)]
    pub(crate) fn of(stack_bytes: usize) -> Self {
        Self::set(limit_below(stack_address(), stack_bytes))
    }
}

impl Drop for LimitScope {
    fn drop(&mut self) {
        LIMIT.with(|l| l.set(self.0));
    }
}

/// The stack is below this thread's limit (never when it has none).
#[inline]
pub(crate) fn below_limit() -> bool {
    stack_address() < thread_limit()
}

/// AST levels between 2 stack reads of a pass backstop. The frames of that many levels lie in
/// RED (BytecodeGen, the largest pass, is below 16 KiB per level in debug).
pub(crate) const BACKSTOP_STEP: u32 = 16;

/// The backstop of an ep9 walk at its depth step (layer 2, the scope pass, BytecodeGen): true
/// when the walk must stop, because the stack is below this thread's limit. It reads the stack
/// at every [`BACKSTOP_STEP`]-th level only. On the full worker within the caps it never fires;
/// it matters on a thread smaller than the budget.
#[inline]
pub(crate) fn backstop(depth: u32) -> bool {
    depth.is_multiple_of(BACKSTOP_STEP) && below_limit()
}

/// `levels` x `per_level` with the margin fits the stack above this thread's limit (always,
/// when it has none).
fn fits(levels: usize, per_level: usize) -> bool {
    let limit = thread_limit();
    if limit == 0 {
        return true;
    }
    let need = levels.saturating_mul(per_level).saturating_mul(SAFETY_NUM) / SAFETY_DEN;
    stack_address().saturating_sub(limit) >= need
}

/// The run-time budget check, once after layer 2 (DES section 3.3): the walks that follow
/// (oxc_semantic, the oxc `Visit` walks of the eval path, the scope pass) fit `depth` AST levels
/// at `PASS_PER_LEVEL` with the margin. oxc_semantic and the oxc walks have no backstop of their
/// own, so on a thread smaller than the budget this is their stop.
pub(crate) fn passes_fit(depth: u32) -> bool {
    fits(depth as usize, PASS_PER_LEVEL)
}

/// The per-literal regex budget check (`regex_check`): the regex parser recurses `groups` levels
/// at `REGEX_PER_GROUP`, and it has no check of its own (the regex crate is not patched).
pub(crate) fn regex_fits(groups: u32) -> bool {
    fits(groups as usize, REGEX_PER_GROUP)
}

/// Run `f` with the limit of [`LimitScope::of`]`(stack_bytes)`.
#[cfg(test)]
pub(crate) fn with_limit<R>(stack_bytes: usize, f: impl FnOnce() -> R) -> R {
    let _scope = LimitScope::of(stack_bytes);
    f()
}

/// The oxc parser config of every production parse (`regex_check::parse`, the shim reparse):
/// the static lexer config of `TOKENS` and the stack check of layer 1.
///
/// One type with a const parameter, not a run-time `tokens` field: a field would need oxc's
/// `RuntimeLexerConfig`, a run-time token test in the release lexer. Release links only
/// `L1Config<true>`: one parser copy (a second copy is about +337 KB). Debug also links
/// `L1Config<false>` for pass 1 of `regex_check::parse`.
pub(crate) struct L1Config<const TOKENS: bool> {
    limit: Cell<usize>,
}

impl<const TOKENS: bool> Default for L1Config<TOKENS> {
    /// Fails closed: the first site stops. oxc 0.158 builds no default config, but a later
    /// release may.
    fn default() -> Self {
        Self {
            limit: Cell::new(usize::MAX),
        }
    }
}

impl<const TOKENS: bool> L1Config<TOKENS> {
    /// The config of one parse on this thread: this thread's limit, or the fail-closed default
    /// when it has none.
    pub(crate) fn for_this_thread() -> Self {
        match thread_limit() {
            0 => Self::default(),
            limit => Self {
                limit: Cell::new(limit),
            },
        }
    }

    /// `may_recurse`: the stack is at or above the limit. The first `false` is sticky.
    #[inline(always)]
    fn above_limit(&self) -> bool {
        if stack_address() >= self.limit.get() {
            return true;
        }
        self.limit.set(usize::MAX);
        false
    }
}

impl ParserConfig for L1Config<true> {
    type LexerConfig = TokensLexerConfig;

    const CHECKS_RECURSION: bool = true;

    #[inline(always)]
    fn lexer_config(&self) -> TokensLexerConfig {
        TokensLexerConfig
    }

    #[inline(always)]
    fn may_recurse(&self) -> bool {
        self.above_limit()
    }
}

/// Debug pass 1 of `regex_check::parse` only: a release build cannot use it.
#[cfg(debug_assertions)]
impl ParserConfig for L1Config<false> {
    type LexerConfig = NoTokensLexerConfig;

    const CHECKS_RECURSION: bool = true;

    #[inline(always)]
    fn lexer_config(&self) -> NoTokensLexerConfig {
        NoTokensLexerConfig
    }

    #[inline(always)]
    fn may_recurse(&self) -> bool {
        self.above_limit()
    }
}

/// Where a front-end job runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stack {
    /// On this thread's front-end worker, with a stack of this size: `WORKER_STACK_BYTES` for
    /// every public entry, a small one in the small-stack tests.
    Worker(usize),
    /// DES section 7: on the caller's thread when its stack has the room of a worker, else on the
    /// worker (`WORKER_STACK_BYTES`). `low` is the lowest address of the caller's stack
    /// (`ep9_platform::stack::current_thread_stack_bounds`).
    #[expect(
        dead_code,
        reason = "no public entry yet (H01 B1); T-INLINE comes with the tests"
    )]
    Inline { low: usize },
}

/// The stack of every public entry.
pub(crate) const WORKER: Stack = Stack::Worker(WORKER_STACK_BYTES);

/// Run the front-end job `job` where `stack` says.
///
/// The inline rule: `limit = sp_entry - (W - RED)` with `sp_entry` here and W =
/// `WORKER_STACK_BYTES`, the line of a worker thread start. The job runs inline only if
/// `limit >= low + RED`, so the room above the limit is `W - RED` on both paths, never more. A
/// thread that has a limit already (a worker, or an inline job) hops or runs nested as before.
///
/// # Errors
///
/// Returns [`SpawnError`] if the worker thread cannot be created.
pub(crate) fn run<R, F>(stack: Stack, job: F) -> Result<R, SpawnError>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    match stack {
        Stack::Worker(stack_bytes) => worker::run(stack_bytes, job),
        Stack::Inline { low } => {
            if thread_limit() == 0 {
                let limit = limit_below(stack_address(), WORKER_STACK_BYTES);
                if limit >= low.saturating_add(RED) {
                    let _scope = LimitScope::set(limit);
                    return Ok(job());
                }
            }
            worker::run(WORKER_STACK_BYTES, job)
        }
    }
}
