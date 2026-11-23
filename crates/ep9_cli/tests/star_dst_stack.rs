//! STAR-DST (rule O13, DL M11-P3-PICK, M11-P3-L16-VERDICT; T4): the fused set is the only
//! form of the front end since the M11-P3 merge, and the `ep9` binary has no arm to pick. The
//! old probe switch `EP9_STAR_DST` not set, `0`, `1` and `2` gives the same output in each run
//! (the binary reads no switch): the stack traces of the base binary, the fused golden
//! bytecode, and the same facts.
//!
//! ```text
//! ep9 t3.js            (not set; 0, 1, 2)  ->  stdout = M10P5 (15 lines)
//! ep9 extra.js         (not set; 0, 1, 2)  ->  stdout = the base binary output (6 lines)
//! ep9 --print-bytecode (not set; 0, 1, 2)  ->  the call-forms golden (fused)
//! ep9 --facts-out      (not set; 0, 1, 2)  ->  equal facts (unit hash zeroed)
//! ```
//!
//! A fused instruction has the position entry of its load and stores `dst` on the success exit
//! only, so a throw in it reports the line and column of the pair (INV-S4) and no stack string
//! changes against the base binary.

use std::process::Command;

/// The 15 `this` and parameter cases of `ldar_elide_stack.rs` (T3 there).
const T3: &str = r#""use strict";
function show(tag, f) { try { f(); print(tag, "no throw"); } catch (e) { var s = String(e && e.stack).split("\n"); print(tag, "|", s[0], "|", s[1], "|", s[2]); } }
function c1() {
  this.a;
}
function c2() {
  return this.a.b;
}
function c3() {
  this.m(1);
}
function c4() {
  this.a = 1;
}
function c5() {
  var k = 1;
  this
    .a;
}
function c6() {
  try { this.a; } finally { k6 = 1; }
}
var k6;
function c7(p) {
  p.x;
}
function c8(p) {
  p.m();
}
class G { get g() { throw new Error("getter"); } c9() { this.g; } c10() { let v = this.g + 1; return v; } c11(p) { p; this.g; } }
function* c12() { this.a; yield 1; }
function c13(p) { if (p) { this.q.r = 2; } }
function c14(p) { for (var i = 0; i < 1; i++) this.a; }
function c15(p) { p.x = this; this.a; }
show("c1 this.a; (this undefined)", c1);
show("c2 return this.a.b", c2);
show("c3 this.m(1)", c3);
show("c4 this.a = 1", c4);
show("c5 this\\n.a", c5);
show("c6 try this.a", c6);
show("c7 p.x; (param)", c7);
show("c8 p.m(); (param)", c8);
show("c9 this.g getter", () => new G().c9());
show("c10 let v = this.g + 1", () => new G().c10());
show("c11 p; this.g;", () => new G().c11(1));
show("c12 generator this.a", () => c12().next());
show("c13 this.q.r = 2", () => c13.call({}, 1));
show("c14 loop this.a", c14);
show("c15 p.x = this; this.a", () => c15({}));
"#;

/// The m10-p5 output of `ep9 t3.js` (`ldar_elide_stack.rs`).
const M10P5: &str = r#"c1 this.a; (this undefined) | TypeError: Cannot read properties of undefined (reading 'a') |     at c1 (t3.js:4:8) |     at show (t3.js:2:31)
c2 return this.a.b | TypeError: Cannot read properties of undefined (reading 'a') |     at c2 (t3.js:7:15) |     at show (t3.js:2:31)
c3 this.m(1) | TypeError: Cannot read properties of undefined (reading 'm') |     at c3 (t3.js:10:8) |     at show (t3.js:2:31)
c4 this.a = 1 | TypeError: Cannot set properties of undefined (setting 'a') |     at c4 (t3.js:13:3) |     at show (t3.js:2:31)
c5 this\n.a | TypeError: Cannot read properties of undefined (reading 'a') |     at c5 (t3.js:18:6) |     at show (t3.js:2:31)
c6 try this.a | TypeError: Cannot read properties of undefined (reading 'a') |     at c6 (t3.js:21:14) |     at show (t3.js:2:31)
c7 p.x; (param) | TypeError: Cannot read properties of undefined (reading 'x') |     at c7 (t3.js:25:3) |     at show (t3.js:2:31)
c8 p.m(); (param) | TypeError: Cannot read properties of undefined (reading 'm') |     at c8 (t3.js:28:5) |     at show (t3.js:2:31)
c9 this.g getter | Error: getter |     at get g (t3.js:30:27) |     at c9 (t3.js:30:62)
c10 let v = this.g + 1 | Error: getter |     at get g (t3.js:30:27) |     at c10 (t3.js:30:88)
c11 p; this.g; | Error: getter |     at get g (t3.js:30:27) |     at c11 (t3.js:30:124)
c12 generator this.a | TypeError: Cannot read properties of undefined (reading 'a') |     at c12 (t3.js:31:24) |     at t3.js:46:42
c13 this.q.r = 2 | TypeError: Cannot set properties of undefined (setting 'r') |     at c13 (t3.js:32:28) |     at t3.js:47:36
c14 loop this.a | TypeError: Cannot read properties of undefined (reading 'a') |     at c14 (t3.js:33:52) |     at show (t3.js:2:31)
c15 p.x = this; this.a | TypeError: Cannot read properties of undefined (reading 'a') |     at c15 (t3.js:34:36) |     at t3.js:49:38
"#;

/// The STAR-DST cases (PK section 4.2 test (5), SC b1): `f(a.b)` with `a` undefined, `this.a.b`
/// with `this.a` null, a getter that throws, a context load in a closure, a context load 2
/// hops out, and a chain of named loads into bindings.
const EXTRA: &str = r#""use strict";
function show(tag, f) { try { f(); print(tag, "no throw"); } catch (e) { var s = String(e && e.stack).split("\n"); print(tag, "|", s[0], "|", s[1], "|", s[2]); } }
function id(v) { return v; }
function d1(a) {
  id(a.b);
}
function d2() {
  return this.a.b;
}
var H = { get g() { throw new TypeError("getter " + 1); } };
function d3(o) {
  var v = o.g;
  return v;
}
function d4() {
  var o = null;
  function inner() { var t = o; return t.p; }
  return inner();
}
function d5() {
  var o = undefined;
  function m1() { var w1 = 1; function m2() { var w2 = 2; function deep() { var t = o; return w1 + w2 + t.q; } return deep(); } return m2(); }
  return m1();
}
function d6(p) {
  var k = p.a;
  var m = k.b;
  return m.c;
}
show("d1 f(a.b) a undefined", () => d1(undefined));
show("d2 this.a.b this.a null", () => d2.call({ a: null }));
show("d3 getter throws", () => d3(H));
show("d4 closure context load", d4);
show("d5 context load 2 hops", d5);
show("d6 chain", () => d6({ a: { b: null } }));
"#;

/// The output of `ep9 extra.js` from the base binary (0814321a).
const EXTRA_BASE: &str = r#"d1 f(a.b) a undefined | TypeError: Cannot read properties of undefined (reading 'b') |     at d1 (extra.js:5:8) |     at extra.js:30:37
d2 this.a.b this.a null | TypeError: Cannot read properties of null (reading 'b') |     at d2 (extra.js:8:17) |     at extra.js:31:42
d3 getter throws | TypeError: getter 1 |     at get g (extra.js:10:27) |     at d3 (extra.js:12:7)
d4 closure context load | TypeError: Cannot read properties of null (reading 'p') |     at inner (extra.js:17:33) |     at d4 (extra.js:18:10)
d5 context load 2 hops | TypeError: Cannot read properties of undefined (reading 'q') |     at deep (extra.js:22:107) |     at m2 (extra.js:22:119)
d6 chain | TypeError: Cannot read properties of null (reading 'c') |     at d6 (extra.js:28:3) |     at extra.js:35:24
"#;

/// The `ep9` command with the old probe switch `EP9_STAR_DST` not set (`None`) or set to a
/// value (the binary must not read it).
fn ep9(switch: Option<&str>) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ep9"));
    match switch {
        None => cmd.env_remove("EP9_STAR_DST"),
        Some(v) => cmd.env("EP9_STAR_DST", v),
    };
    cmd
}

const ARMS: [Option<&str>; 4] = [None, Some("0"), Some("1"), Some("2")];

/// `ep9 <file>` in a new test dir, in each run: the stdout must be `want`.
fn same_stdout(dir: &str, file: &str, src: &str, want: &str) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(dir);
    std::fs::create_dir_all(&dir).expect("make the test dir");
    std::fs::write(dir.join(file), src).expect("write the script");
    let mut failures = Vec::new();
    for arm in ARMS {
        let out = ep9(arm)
            .arg(file)
            .current_dir(&dir)
            .output()
            .expect("run ep9");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if out.status.code() != Some(0) || stdout != want {
            failures.push(format!(
                "switch {arm:?}: status {:?}\n{stdout}\nstderr {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T4: `ep9 t3.js` prints the 15 m10-p5 strings in each run.
#[test]
fn stack_traces_keep_the_m10p5_strings() {
    same_stdout("star-dst-t4", "t3.js", T3, M10P5);
}

/// T4: `ep9 extra.js` prints the base binary output in each run (the throw line and column of
/// a fused named load and of a fused context load).
#[test]
fn stack_traces_of_fused_loads_are_the_base_strings() {
    same_stdout("star-dst-t4-extra", "extra.js", EXTRA, EXTRA_BASE);
}

/// The committed golden of `call-forms` (6 `GetNamedProperty` + `Star` pairs in arm 0; the
/// default fuses them).
const CALL_FORMS: &str = include_str!("../../ep9_frontend/src/bytecodegen/golden/call-forms.txt");

/// `ep9 --print-bytecode` prints the committed golden text (with `GetNamedPropertyStar`) in each
/// run. The script then throws at `f()` (`f` is undefined): exit 1 and the same error text in
/// each run.
#[test]
fn the_binary_prints_the_golden_bytecode() {
    let mut parts = CALL_FORMS.splitn(3, '\n');
    let src = parts
        .nth(1)
        .and_then(|l| l.strip_prefix("# | "))
        .expect("the golden source line");
    let want = parts.next().expect("the golden body");
    assert!(want.contains("GetNamedPropertyStar"), "{want}");
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("star-dst-arm0");
    std::fs::create_dir_all(&dir).expect("make the test dir");
    std::fs::write(dir.join("calls.js"), src).expect("write calls.js");
    let mut stderr = Vec::new();
    for arm in ARMS {
        let out = ep9(arm)
            .args(["--print-bytecode", "calls.js"])
            .current_dir(&dir)
            .output()
            .expect("run ep9");
        assert_eq!(out.status.code(), Some(1), "switch {arm:?}");
        stderr.push(String::from_utf8_lossy(&out.stderr).into_owned());
        let got = String::from_utf8_lossy(&out.stdout);
        assert_eq!(got.strip_suffix('\n'), Some(want), "switch {arm:?}");
    }
    assert!(
        stderr[0].contains("calls.js:1:17: Uncaught TypeError"),
        "{}",
        stderr[0]
    );
    for (k, e) in stderr.iter().enumerate().skip(1) {
        assert_eq!(e, &stderr[0], "switch {:?}", ARMS[k]);
    }
}

/// T7: constructors with `this.<name> =` stores (`ldar_elide_stack.rs`).
const SLACK: &str = r#"function P(x, y) { this.x = x; this.y = y; }
function Q(p) { this.a = 1; this.b = { c: 2 }; p; this.c = p; }
function R(p) { this.a = p; this.m(); }
R.prototype.m = function () { this.z = 3; };
function S(p) { p.x = this; this.k = 1; }
var s = 0;
for (var i = 0; i < 200; i++) {
  var o = new P(i, i + 1), q = new Q(i), r = new R(i), t = new S({});
  s += o.x + q.c + r.z + t.k;
}
print(s);
"#;

/// The facts file with each copy of its unit hash (header bytes 16..48) set to zero.
fn facts_without_unit_hash(bytes: &[u8]) -> Vec<u8> {
    let hash = bytes[16..48].to_vec();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(&hash) {
            out.extend(std::iter::repeat_n(0, hash.len()));
            i += hash.len();
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// T7: `ep9 --facts-out slack.js` prints 40600 and writes the same facts (unit hash zeroed) in
/// each run (the binary reads no switch, so this checks only that a run is repeatable). That
/// a fused load is the same feedback site as its pair, with the same records, is the VM test
/// T3 (`ep9_vm` `tests::star_dst`: the facts of the pair unit and of the fused unit are
/// equal, for each dispatch, each feedback mode and with and without GC stress).
#[test]
fn slack_facts_are_the_same_in_each_arm() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("star-dst-t7");
    std::fs::create_dir_all(&dir).expect("make the test dir");
    std::fs::write(dir.join("slack.js"), SLACK).expect("write slack.js");
    let mut facts = Vec::new();
    for (k, arm) in ARMS.into_iter().enumerate() {
        let file = format!("facts{k}.bin");
        let out = ep9(arm)
            .args(["--facts-out", &file, "slack.js"])
            .current_dir(&dir)
            .output()
            .expect("run ep9");
        assert_eq!(out.status.code(), Some(0), "switch {arm:?}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "40600\n",
            "switch {arm:?}"
        );
        let bytes = std::fs::read(dir.join(&file)).expect("read the facts");
        facts.push(facts_without_unit_hash(&bytes));
    }
    for (k, f) in facts.iter().enumerate().skip(1) {
        assert!(*f == facts[0], "switch {:?}: the facts differ", ARMS[k]);
    }
}
