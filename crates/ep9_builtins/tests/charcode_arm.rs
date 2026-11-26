//! The `charCodeAt` arm of `CallProperty1` (M15-P1 CHARCODE; the design's tests T1 and T1h).
//!
//! `slow::call::call_property::<1>` answers `s.charCodeAt(i)` with no host call when the
//! callee runs the builtin's host function (`ep9_vm::internals::STRING_CHAR_CODE_AT`), the
//! human fast paths are on, the receiver is a flat string and `i` is an int32 below its length;
//! every other call takes the builtin. The arm is a part of the FASTDIFF row
//! `host-call-property` (the same function, the same switch), so `Cx::set_human_fast_paths`
//! turns it off.
//!
//! ```text
//! T1   each case runs with the fast paths on (the arm) and off (the builtin), each under GC
//!      stress (a collection at each allocation, then the heap verifier) and without; the
//!      printed lines and the uncaught exception of the 4 runs must be equal (0 mismatches).
//!      The recursion to the stack limit runs without GC stress only (it fills the VM stack,
//!      and each collection would scan it: the FASTDIFF rule)
//! T1h  the hits of "charcode-arm" (ep9_vm feature `fastdiff-hits`): with the fast paths on,
//!      0.9 x N <= hits <= N for the cases that call the builtin on flat strings, where N is
//!      the number of calls with an int32 index in range; 0 with them off; exactly 0 for the
//!      cases whose calls must never take the arm (a new rope at each call, the index forms,
//!      the other receivers, a changed method)
//! ```
//!
//! Each case first runs 300 empty loop iterations, so its calls are outside the call record
//! window of x86-64 (`profile::CALL_WINDOW`, 64 budget units after a poll), where a call takes
//! `slow::call` and not the arm; the loops of the hit cases run 30,000 calls or more.

use std::cell::RefCell;

use ep9_frontend::{CompileOptions, SourceOpts, Strictness};
use ep9_heap::HeapConfig;
use ep9_vm::{Args, Cx, Isolate, IsolateConfig, JsResult, Local};

/// The FASTDIFF hit counter of the arm (`fast_hits::NAMES` in `ep9_vm/src/call.rs`).
const HIT: &str = "charcode-arm";

/// The prelude of each case: the warm-up loop (see the module doc).
const WARM: &str = "for (var warm_ = 0; warm_ < 300; warm_++) {}\n";

/// How a case's hits are checked with the fast paths on (T1h).
#[derive(Clone, Copy, Debug)]
enum Hits {
    /// `0.9 x N <= hits <= N`, N from the line the case prints last: `n=<N>`.
    InRange,
    /// Exactly 0.
    Never,
    /// Not checked (the case has calls of both kinds).
    Any,
}

/// One differential case.
struct Case {
    name: &'static str,
    src: &'static str,
    hits: Hits,
    /// Also under GC stress (every case but the recursion to the stack limit).
    stress: bool,
}

const CASES: &[Case] = &[
    // (a) the flat forms, each index from -2 to length + 2.
    Case {
        name: "(a) one-byte, two-byte, a slice, a thin string, a flattened rope",
        src: "var one = 'abcdefghijklmnopq';\n\
              var two = '\\u0100bcdefghij\\u20acklm';\n\
              var slice = '0123456789abcdefghijklmnopqrstuvwxyz'.substring(2, 20);\n\
              var thin = 'thin-rope-' + 'part-two!'; var keys = {}; keys[thin] = 1;\n\
              var rope = 'flat-rope-' + 'abcdefgh'; rope.indexOf('z');\n\
              var forms = [one, two, slice, thin, rope], R = 300, n = 0;\n\
              for (var f = 0; f < forms.length; f++) {\n\
                var s = forms[f], first = [], sum = 0, nan = 0;\n\
                for (var k = 0; k < R; k++) {\n\
                  for (var i = -2; i < s.length + 3; i++) {\n\
                    var v = s.charCodeAt(i);\n\
                    if (v !== v) nan++; else sum += v;\n\
                    if (k === 0) first.push(v);\n\
                  }\n\
                }\n\
                n += R * s.length;\n\
                print(s.length, sum, nan, first.join());\n\
              }\n\
              print('n=' + n);",
        hits: Hits::InRange,
        stress: true,
    },
    // (b) a new rope at each call: the builtin flattens it; the arm never takes it.
    Case {
        name: "(b) a new rope at each call",
        src: "var a = 'abcdefghij', b = 'klmnop\\u0101', out = [];\n\
              for (var k = 0; k < 40; k++) {\n\
                for (var i = -1; i < 19; i += 3) out.push((a + b).charCodeAt(i));\n\
              }\n\
              print(out.length, out.slice(0, 7).join());",
        hits: Hits::Never,
        stress: true,
    },
    // (c) the index forms that are not an int32 in range.
    Case {
        name: "(c) the index forms",
        src: "var log = [], s = 'abcdef', u = '\\u0100bc', e = '';\n\
              var forms = [-1, 6, 2147483647, -2147483648, 1.5, -0.5, -0, NaN, Infinity,\n\
                -Infinity, 2 ** 31, 4294967296 + 1, '1', '2.9', 'x', undefined, null, true, false,\n\
                { valueOf() { log.push('v'); return 1; } }, { toString() { log.push('s'); return '2'; } }];\n\
              for (var k = 0; k < 20; k++) {\n\
                var row = [];\n\
                for (var j = 0; j < forms.length; j++) {\n\
                  row.push(s.charCodeAt(forms[j]), u.charCodeAt(forms[j]), e.charCodeAt(forms[j]));\n\
                }\n\
                if (k === 0) print(row.join());\n\
              }\n\
              print(log.join(''));\n\
              try { s.charCodeAt(Symbol()); } catch (x) { print(x.name); }\n\
              try { s.charCodeAt(1n); } catch (x) { print(x.name); }",
        hits: Hits::Never,
        stress: true,
    },
    // (d) receivers that are not a String value.
    Case {
        name: "(d) other receivers",
        src: "var log = [], cc = String.prototype.charCodeAt;\n\
              var w = new String('abc');\n\
              var o = { charCodeAt: cc, toString() { log.push('t'); return 'xyz'; } };\n\
              Number.prototype.charCodeAt = cc;\n\
              var row = [];\n\
              for (var k = 0; k < 20; k++) {\n\
                row = [w.charCodeAt(1), w.charCodeAt(3), o.charCodeAt(2), (12.5).charCodeAt(2), (7).charCodeAt(0), true.charCodeAt ? 1 : 0];\n\
              }\n\
              print(row.join(), log.length);\n\
              delete Number.prototype.charCodeAt;\n\
              Symbol.prototype.charCodeAt = cc;\n\
              try { Symbol('q').charCodeAt(0); } catch (x) { print(x.name); }\n\
              try { cc.call(null, 0); } catch (x) { print(x.name); }\n\
              try { cc.call(undefined, 0); } catch (x) { print(x.name); }",
        hits: Hits::Never,
        stress: true,
    },
    // (e) a changed method: a bytecode function, a getter, then gone.
    Case {
        name: "(e) a changed String.prototype.charCodeAt",
        src: "var p = String.prototype.charCodeAt, s = 'abcdef', out = [];\n\
              String.prototype.charCodeAt = function (i) { return 'p' + i; };\n\
              for (var k = 0; k < 20; k++) out.push(s.charCodeAt(k % 8));\n\
              Object.defineProperty(String.prototype, 'charCodeAt', { get: function () { return function (i) { return 'g' + i; }; }, configurable: true });\n\
              for (var k = 0; k < 20; k++) out.push(s.charCodeAt(k % 8));\n\
              Object.defineProperty(String.prototype, 'charCodeAt', { get: function () { return p.bind('xyz'); }, configurable: true });\n\
              for (var k = 0; k < 20; k++) out.push(s.charCodeAt(k % 4));\n\
              delete String.prototype.charCodeAt;\n\
              try { s.charCodeAt(1); } catch (x) { out.push(x.name); }\n\
              print(out.length, out.join());",
        hits: Hits::Never,
        stress: true,
    },
    Case {
        name: "(e) the method changed, then the builtin back",
        src: "var p = String.prototype.charCodeAt, s = 'abcdefghijklm', out = [];\n\
              String.prototype.charCodeAt = function (i) { return 'p' + i; };\n\
              for (var k = 0; k < 50; k++) out.push(s.charCodeAt(k % 16));\n\
              String.prototype.charCodeAt = p;\n\
              for (var k = 0; k < 50; k++) out.push(s.charCodeAt(k % 16 - 1));\n\
              print(out.length, out.join());",
        hits: Hits::Any,
        stress: true,
    },
    // (f) the builtin under another name is the builtin.
    Case {
        name: "(f) the builtin under another name",
        src: "String.prototype.cc = String.prototype.charCodeAt;\n\
              var o = { m: String.prototype.charCodeAt };\n\
              var s = '\\u00e9bcdefghijkl', R = 2000, sum = 0, nan = 0, n = 0;\n\
              for (var k = 0; k < R; k++) {\n\
                for (var i = -2; i < s.length + 3; i++) {\n\
                  var v = s.cc(i);\n\
                  if (v !== v) nan++; else sum += v;\n\
                }\n\
              }\n\
              n = R * s.length;\n\
              print(sum, nan, o.m(1), s.cc());\n\
              print('n=' + n);",
        hits: Hits::InRange,
        stress: true,
    },
    // (g) the recursion to the stack limit: the RangeError (no GC stress).
    Case {
        name: "(g) the recursion to the stack limit",
        src: "function r(s) { s.charCodeAt(0); return [s].map(r); }\n\
              try { r('abc'); } catch (e) { print(e.name); }\n\
              print('abc'.charCodeAt(2));",
        hits: Hits::Any,
        stress: false,
    },
    // (h) a Wide call: the argument register is above 127 (the FASTDIFF form).
    Case {
        name: "(h) a Wide call",
        src: "function w(k) {\n\
              var a0 = 0, a1 = 1, a2 = 2, a3 = 3, a4 = 4, a5 = 5, a6 = 6, a7 = 7, a8 = 8, a9 = 9, a10 = 10, a11 = 11, a12 = 12, a13 = 13,\n\
                  a14 = 14, a15 = 15, a16 = 16, a17 = 17, a18 = 18, a19 = 19, a20 = 20, a21 = 21, a22 = 22, a23 = 23, a24 = 24, a25 = 25, a26 = 26, a27 = 27,\n\
                  a28 = 28, a29 = 29, a30 = 30, a31 = 31, a32 = 32, a33 = 33, a34 = 34, a35 = 35, a36 = 36, a37 = 37, a38 = 38, a39 = 39, a40 = 40, a41 = 41,\n\
                  a42 = 42, a43 = 43, a44 = 44, a45 = 45, a46 = 46, a47 = 47, a48 = 48, a49 = 49, a50 = 50, a51 = 51, a52 = 52, a53 = 53, a54 = 54, a55 = 55,\n\
                  a56 = 56, a57 = 57, a58 = 58, a59 = 59, a60 = 60, a61 = 61, a62 = 62, a63 = 63, a64 = 64, a65 = 65, a66 = 66, a67 = 67, a68 = 68, a69 = 69,\n\
                  a70 = 70, a71 = 71, a72 = 72, a73 = 73, a74 = 74, a75 = 75, a76 = 76, a77 = 77, a78 = 78, a79 = 79, a80 = 80, a81 = 81, a82 = 82, a83 = 83,\n\
                  a84 = 84, a85 = 85, a86 = 86, a87 = 87, a88 = 88, a89 = 89, a90 = 90, a91 = 91, a92 = 92, a93 = 93, a94 = 94, a95 = 95, a96 = 96, a97 = 97,\n\
                  a98 = 98, a99 = 99, a100 = 100, a101 = 101, a102 = 102, a103 = 103, a104 = 104, a105 = 105, a106 = 106, a107 = 107, a108 = 108, a109 = 109, a110 = 110, a111 = 111,\n\
                  a112 = 112, a113 = 113, a114 = 114, a115 = 115, a116 = 116, a117 = 117, a118 = 118, a119 = 119, a120 = 120, a121 = 121, a122 = 122, a123 = 123, a124 = 124, a125 = 125,\n\
                  a126 = 126, a127 = 127, a128 = 128, a129 = 129, a130 = 130, a131 = 131, a132 = 132, a133 = 133, a134 = 134, a135 = 135, a136 = 136, a137 = 137, a138 = 138, a139 = 139;\n\
                return 'abc\\u0100'.charCodeAt(a1 + a0 + k % 5 - 1) + '|' + 'xyz'.charCodeAt(a139 % 3);\n\
              }\n\
              var out = [];\n\
              for (var k = 0; k < 50; k++) out.push(w(k));\n\
              print(out.join());",
        hits: Hits::Any,
        stress: true,
    },
];

thread_local! {
    static OUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn print<'s>(
    cx: &mut Cx<'s>,
    _this: Local<'s>,
    args: Args<'s>,
    _nt: Local<'s>,
) -> JsResult<Local<'s>> {
    let mut parts = Vec::new();
    for v in args.iter() {
        parts.push(cx.to_rust_string(v)?);
    }
    OUT.with(|o| o.borrow_mut().push(parts.join(" ")));
    Ok(Local::undefined())
}

/// What a run gives: the printed lines and the uncaught exception.
type RunOut = (Vec<String>, Option<String>);

/// Runs the sloppy script `WARM + src` on a fresh isolate with the human fast paths `on` or
/// off; `stress`: a collection at each allocation, then the heap verifier. Gives the output
/// and the hits of the arm. On its own thread with a big stack (the counters are per thread;
/// debug builds and the recursion case). The runner of `fastpath_diff.rs`.
fn run(src: &'static str, on: bool, stress: bool) -> (RunOut, u64) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let text = format!("{WARM}{src}");
            let unit = ep9_frontend::compile(
                &text,
                SourceOpts::script(Strictness::Sloppy),
                CompileOptions::default(),
            )
            .expect("the case compiles");
            let mut iso = Isolate::new(IsolateConfig {
                heap: HeapConfig {
                    gc_stress: u32::from(stress),
                    verify_after_gc: stress,
                    ..HeapConfig::default()
                },
                builtins: Some(&ep9_builtins::TABLE),
                ..IsolateConfig::default()
            })
            .expect("an isolate");
            iso.register_host_function("print", 1, print);
            OUT.with(|o| o.borrow_mut().clear());
            let _ = ep9_vm::internals::take_fastdiff_hits();
            let r = iso.with_cx_checkpoint(|cx| {
                cx.set_human_fast_paths(on);
                cx.run_script(unit, "case.js").map(|_| ())
            });
            let out = OUT.with(|o| core::mem::take(&mut *o.borrow_mut()));
            let hits = ep9_vm::internals::take_fastdiff_hits();
            iso.collect_garbage();
            iso.verify_heap().expect("a valid heap");
            // The switch is process-wide and keyed by the isolate id: turn it on again, so a
            // dropped isolate leaves no id behind.
            let _ = iso.with_cx(|cx| {
                cx.set_human_fast_paths(true);
                Ok(())
            });
            let hit = hits.iter().find(|(n, _)| *n == HIT).map_or(0, |(_, n)| *n);
            ((out, r.err().map(|e| e.message)), hit)
        })
        .expect("a thread")
        .join()
        .expect("no panic")
}

/// N of a case that prints `n=<N>` last: its calls with an int32 index in range.
fn in_range_calls(out: &RunOut) -> u64 {
    let last = out.0.last().expect("the case prints n=<N> last");
    last.strip_prefix("n=")
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("the last line is not n=<N>: {last:?}"))
}

/// Runs case `c` (T1, then T1h): its mismatches.
fn check(c: &Case) -> Vec<String> {
    let mut bad = Vec::new();
    let (on, hits_on) = run(c.src, true, false);
    let (off, hits_off) = run(c.src, false, false);
    if on.1.is_some() {
        bad.push(format!("{}: uncaught {:?}", c.name, on.1));
    }
    if on != off {
        bad.push(format!("{}: on {on:?}\n  off {off:?}", c.name));
    }
    if c.stress {
        for fast in [true, false] {
            let (st, _) = run(c.src, fast, true);
            if st != on {
                bad.push(format!(
                    "{}: GC stress, fast paths {fast}: {st:?}\n  want {on:?}",
                    c.name
                ));
            }
        }
    }
    if hits_off != 0 {
        bad.push(format!(
            "{}: {hits_off} hits with the fast paths off",
            c.name
        ));
    }
    match c.hits {
        Hits::InRange => {
            let n = in_range_calls(&on);
            if !(hits_on * 10 >= n * 9 && hits_on <= n) {
                bad.push(format!("{}: {hits_on} hits for {n} calls in range", c.name));
            }
        }
        Hits::Never if hits_on != 0 => {
            bad.push(format!("{}: {hits_on} hits, want 0", c.name));
        }
        Hits::Never | Hits::Any => {}
    }
    eprintln!("{}: hits on {hits_on}, off {hits_off}", c.name);
    bad
}

/// T1 and T1h: every case gives the same output with the arm and with the builtin (and under
/// GC stress), and the arm takes the calls it should and no other.
#[test]
fn the_char_code_arm_equals_the_builtin_in_every_case() {
    let bad: Vec<String> = std::thread::scope(|s| {
        let runs: Vec<_> = CASES.iter().map(|c| s.spawn(move || check(c))).collect();
        runs.into_iter()
            .flat_map(|r| r.join().expect("no panic"))
            .collect()
    });
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// The table of `ep9_builtins` holds the VM's host function for `charCodeAt` (the pointer
/// that the arm tests): the const of `internals`, under the builtin's name.
#[test]
fn the_table_names_the_vm_host_function() {
    let d = &ep9_builtins::string::MODULE.defs[2];
    assert_eq!(d.name, "charCodeAt");
    assert!(
        core::ptr::fn_addr_eq(d.f, ep9_vm::internals::STRING_CHAR_CODE_AT),
        "the table entry is not the VM host function"
    );
}
