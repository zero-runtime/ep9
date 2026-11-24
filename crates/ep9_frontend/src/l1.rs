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

    /// Test only: a limit `room` bytes below this point, to reach the backstops of the passes on
    /// a big stack.
    #[cfg(test)]
    pub(crate) fn room(room: usize) -> Self {
        Self::set(stack_address().saturating_sub(room).max(1))
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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no public entry yet (H01 B1); T-INLINE tests it")
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

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::bytecodegen::{CodegenError, CompileOptions};
    use crate::forms::FORMS;
    use crate::stack::BUDGET_BYTES;
    use crate::{CompileError, RangeErrorKind, SourceOpts, Strictness};

    fn opts(module: bool) -> SourceOpts {
        if module {
            SourceOpts::module()
        } else {
            SourceOpts::script(Strictness::Sloppy)
        }
    }

    /// The unit bytes of `source` compiled with the job on `stack`, and whether the job ran on
    /// the calling thread.
    fn compile_on(
        source: &str,
        opts: SourceOpts,
        stack: Stack,
    ) -> (Result<Vec<u8>, CodegenError>, bool) {
        let caller = thread::current().id();
        let result = crate::with_front_end_at(source, opts, stack, move |fe| {
            let here = thread::current().id() == caller;
            let analysis = crate::scope::analyze(fe)?;
            Ok((
                crate::bytecodegen::generate(fe, &analysis, CompileOptions::default()),
                here,
            ))
        });
        match result {
            Ok((unit, here)) => (unit.map(|u| u.unit().to_bytes()), here),
            Err(e) => (Err(CodegenError::Compile(e)), false),
        }
    }

    /// The stack bounds of this thread (T-RED, T-INLINE).
    fn low() -> usize {
        ep9_platform::stack::current_thread_stack_bounds()
            .expect("the stack bounds of this thread")
            .low
    }

    /// A thread whose stack can hold the inline job: W of room above the entry and RED more.
    fn on_big_thread<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
        thread::Builder::new()
            .stack_size(WORKER_STACK_BYTES + 2 * RED)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn a_thread_with_no_limit_stops_at_the_first_site() {
        assert_eq!(thread_limit(), 0, "a test thread has no limit");
        let allocator = oxc_allocator::Allocator::default();
        let result = crate::regex_check::parse(&allocator, "[[0]]", oxc_span::SourceType::script());
        assert!(
            matches!(result, Err(crate::regex_check::Stopped::StackLimit(_))),
            "{:?}",
            result.err()
        );
        // With a limit the same parse runs.
        let _scope = LimitScope::of(2 << 20);
        assert!(
            crate::regex_check::parse(&allocator, "[[0]]", oxc_span::SourceType::script()).is_ok()
        );
    }

    /// T-INLINE (DES section 7): the inline path and the worker path give one result. (1) The
    /// same bytecode for a sample (every form at a small size, both goals). (2) One form above
    /// the budget, on a thread larger than W: StackLimit on both paths (the room above the limit
    /// is `W - RED` on both, so a bigger caller stack gives no deeper parse).
    #[test]
    fn inline_and_worker_give_one_result() {
        on_big_thread(|| {
            let low = low();
            let mut sample: Vec<(String, SourceOpts)> = FORMS
                .iter()
                .map(|f| ((f.build)(7), opts(f.module)))
                .collect();
            sample.push((
                "function f(a, ...b) { return [a, ...b].map(x => x * 2); } f(1, 2)".into(),
                opts(false),
            ));
            sample.push((
                "export const x = class { #p = 1; m() { return this.#p; } };".into(),
                opts(true),
            ));
            for (src, opts) in &sample {
                let (inline, here) = compile_on(src, *opts, Stack::Inline { low });
                assert!(here, "the job did not run inline: {src}");
                let (worker, there) = compile_on(src, *opts, WORKER);
                assert!(!there, "the worker job ran on the caller: {src}");
                match (&inline, &worker) {
                    (Ok(a), Ok(b)) => assert!(a == b, "inline and worker bytecode differ: {src}"),
                    (a, b) => assert_eq!(format!("{a:?}"), format!("{b:?}"), "{src}"),
                }
            }
            // Above the budget: a pre-scan hole (PCHK K2, the TS type sites from a Script
            // parse), 2,000,000 levels, which needs far more than W - RED.
            let deep = format!(
                "x as {}B{}",
                "A<B!,".repeat(2_000_000),
                ">,B".repeat(2_000_000)
            );
            for stack in [Stack::Inline { low }, WORKER] {
                let result = crate::with_front_end_at(&deep, opts(false), stack, |_| Ok(()));
                let kind = result
                    .as_ref()
                    .err()
                    .and_then(CompileError::as_range)
                    .map(|r| r.kind);
                assert_eq!(
                    kind,
                    Some(RangeErrorKind::StackLimit),
                    "{stack:?}: {result:?}"
                );
            }
        });
    }

    /// The T-RED inputs: forms that stop on a small worker within the caps (the pre-scan lets
    /// them reach oxc), each with its level count. The T-A worst forms of the 4 targets
    /// (`spread-args` debug; `bracket-function` and `function-body-list-first` release), an
    /// array nest, and a class method nest (a class body, a method and a function body per
    /// level: the largest step between 2 checks).
    fn red_forms() -> Vec<(&'static str, String)> {
        let form = |name: &str, n: usize| {
            let f = FORMS.iter().find(|f| f.name == name).expect("a form");
            (f.name, (f.build)(n))
        };
        vec![
            (
                "bracket-array",
                format!("{}0{}", "[".repeat(20_000), "]".repeat(20_000)),
            ),
            form("spread-args", 20_000),
            form("bracket-function", 20_000),
            form("function-body-list-first", 20_000),
            (
                "class-method",
                format!("{}{}", "class A{m(){".repeat(5_000), "}}".repeat(5_000)),
            ),
        ]
    }

    /// T-RED: (1) the limit lies in the real stack with room: `limit - low` on the worker and
    /// on the inline path is at least RED / 2. (2) The deepest point that a stopped parse
    /// touches below the limit (the leaf calls after the last check that passed, and the
    /// unwind) is at most RED / 2, for each form of [`red_forms`].
    #[test]
    fn the_red_zone_holds() {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        // (1) The worker of every public entry.
        let (limit, low_w) = worker::run(WORKER_STACK_BYTES, || (thread_limit(), low())).unwrap();
        println!(
            "T-RED {target} {profile}: worker limit - low = {} B (RED / 2 = {} B)",
            limit as isize - low_w as isize,
            RED / 2
        );
        assert!(
            limit >= low_w + RED / 2,
            "worker: the limit is not RED / 2 above low"
        );
        // (1) The inline path on a thread of W + 2 RED.
        let (inline_limit, low_i, ran_inline) = on_big_thread(|| {
            let low = low();
            let caller = thread::current().id();
            let (limit, here) = run(Stack::Inline { low }, move || {
                (thread_limit(), thread::current().id() == caller)
            })
            .unwrap();
            (limit, low, here)
        });
        assert!(ran_inline, "the inline job hopped to the worker");
        println!(
            "T-RED {target} {profile}: inline limit - low = {} B (RED / 2 = {} B)",
            inline_limit as isize - low_i as isize,
            RED / 2
        );
        assert!(
            inline_limit >= low_i + RED / 2,
            "inline: the limit is not RED / 2 above low"
        );
        // (2) A small worker (a new caller thread, so the worker is new and has this size); a
        // parse of each form that stops (below the pre-scan cap).
        let small = 4 << 20;
        let mut deepest_below = isize::MIN;
        for (name, deep) in red_forms() {
            let job = move || {
                let limit = thread_limit();
                let low = low();
                // Paint from here down to 256 KiB above the end (the guard pages lie below).
                let bytes = stack_address() - (low + (256 << 10));
                let (deepest, result) = crate::probe::deepest_point(bytes, || {
                    let allocator = oxc_allocator::Allocator::default();
                    crate::parse(&allocator, &deep, SourceOpts::script(Strictness::Sloppy))
                        .map(|_| ())
                });
                let kind = result.err().and_then(|e| e.as_range().map(|r| r.kind));
                (limit as isize - deepest as isize, kind)
            };
            let (below, kind) = thread::spawn(move || worker::run(small, job).unwrap())
                .join()
                .unwrap();
            assert_eq!(
                kind,
                Some(RangeErrorKind::StackLimit),
                "{name}: the parse did not stop"
            );
            println!(
                "T-RED {target} {profile}: {name}: deepest point below the limit = {below} B (RED / 2 = {} B)",
                RED / 2
            );
            deepest_below = deepest_below.max(below);
        }
        println!(
            "T-RED {target} {profile}: deepest point below the limit, all forms = {deepest_below} B"
        );
        assert!(
            deepest_below <= (RED / 2) as isize,
            "a stopped parse went {deepest_below} B below the limit (RED / 2 = {} B)",
            RED / 2
        );
        // The budget is a share of the reservation without RED (T-BUDGET).
        assert_eq!(BUDGET_BYTES + RED, WORKER_STACK_BYTES);
    }
}
