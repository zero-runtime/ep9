//! `cargo xtask oxc-sites` (T-SITES, H01 layer 1): every recursion of the patched oxc_parser
//! passes a stack check. The task parses the oxc_parser sources with `syn`, builds a call graph
//! and fails when a cycle has no check site.
//!
//! ```text
//! parse every .rs file under --src (default third_party/oxc/crates/oxc_parser/src)
//!   pass 0  the `macro_rules!` of the sources; each item macro of the sources is expanded in
//!           place (matcher and transcriber, with repetitions), so the fns it writes with
//!           `fn $id` are items like the others (the lexer byte handlers: `ascii_byte_handler!`)
//!   pass 1  the named fields of every struct (field -> type name), the traits, the impls
//!   pass 2  one node per function: `Type::name` for a fn in an impl (or a trait default fn),
//!           `name` for a free fn; nested fns and local impls are their own nodes; one node per
//!           static or const with an initializer (a table of fn values is a static)
//!           an edge for every name in a body that names a node of the sources:
//!           a method call (`self.f(..)`, `p.f(..)`), a path call (`AssignmentTarget::cover(..)`),
//!           a function value (`self.parse_list(Self::parse_f)`), a static (`&NO_TOKENS`), and
//!           every name of the expansion of a macro of the sources (`byte_search! {..}`)
//!   a site = a top-level statement of a fn body in one of the 2 site forms of the patch:
//!     `if C::CHECKS_RECURSION && !R.config.may_recurse() { return R.recursion_stopped(); }`
//!     `if C::CHECKS_RECURSION && !R.config.may_recurse() { return Err(R.recursion_stopped_error()); }`
//!   (R the parser: `self` or a parameter; C a type parameter of the fn or its impl, never a
//!   fixed config type). The statements after it are behind the check, so the node keeps only
//!   the edges of the statements before it. Any other statement that calls
//!   `may_recurse` is a finding: a form that does not stop (no `return`), that calls more in its
//!   condition, or that is not at the top level of the body would cut the graph falsely
//!   -> the site table; a cycle (a strongly connected group, or a self call) that is left is a
//!      finding, unless its edges are in ALLOW (name noise: from, to, the use that makes the
//!      edge, and the reason; an entry that leaves out no edge is a finding)
//! ```
//!
//! The lexer byte handlers (`lexer/byte_handlers.rs`) are called through a table, and the graph
//! follows it: `Lexer::handle_byte` -> `self.config.byte_handlers()` (a generic receiver: each
//! `LexerConfig` impl) -> the static table (`NO_TOKENS`, `WITH_TOKENS`, `RUNTIME_TOKENS`, each
//! written by `byte_handlers!()`) -> each handler fn. So a handler that lexes again (calls
//! `next_token`) is a cycle.
//!
//! How a name becomes an edge (the graph has more edges than the code; the forms that it cannot
//! see are in the list after the table, and each one fails the run or is checked):
//!
//! | Use | Edge to |
//! |---|---|
//! | `recv.f(..)`, the type T of `recv` known (`self`, a typed parameter that no pattern of the body binds again, a field of a struct of the sources, the return type of a call of a fn of the sources; through `&`, `Box`, `Rc`, `Pin` and the other [`DEREF`] wrappers) | T a trait of the sources: its default `T::f` and every fn `f` of its impls; T a type: `T::f` if the sources define it; else every method named `f` if T has an impl in the sources; else (a foreign type such as `str` or `Vec`) only the methods named `f` of a blanket impl (`impl<X> Tr for X`) |
//! | `recv.f(..)`, the type not known, or a generic parameter (`config: C`) | every method named `f` |
//! | `Self::f`, `T::f`, `<X as T>::f` (T a type or trait) | the same as a receiver of type T (`Self` = the impl type); T a generic parameter (`C::f`): every method named `f` |
//! | `m::f` (m a module: a lower-case segment) | every free fn, static or const named `f` |
//! | `f`, `f(..)` (one segment: a local or a free fn in Rust) | every free fn, static or const named `f` |
//! | a call of a macro of the sources in a body | the edges of its expansion; if it does not expand, also every fn named by an identifier of its rules |
//! | an identifier in a macro body of another crate that does not parse as expressions | every fn named it |
//!
//! A type or trait T in this table is read by each of its names (review R2 B2-2): T, what T
//! stands for (a `use X as T` rename or a `type T = X` alias of the sources, to the end of a
//! chain), and each name that stands for T. So `R::go` with `use Tr as R` is `Tr::go`, and a
//! parameter `p: &mut P<C>` with `type P<C> = ParserImpl<C>` has the type `ParserImpl`.
//! A field type or a return type that two items of one name give two types (two structs `Ctx`
//! in two modules, each with a field `inner`) is not known (review R2 B2-5): one type for both
//! would narrow the edges of one of them to the fns of the other. Each narrowing rule has a
//! unit test whose cycle runs only through the narrowed edge (`each_narrowing_rule_keeps_its_cycle`).
//!
//! What the graph cannot see, and what the task does instead:
//!
//! - A call through a fn value that a struct or enum field holds (a fn pointer, a `dyn` trait
//!   object, a type parameter with an `Fn`, `FnMut` or `FnOnce` bound on the struct or enum or
//!   on an impl of it, also through a type alias): such a field fails the run (exit 2).
//! - The body of a fn that an item macro of another crate writes: a call of such a macro whose
//!   tokens hold `fn` fails the run (exit 2). The other item macros of other crates
//!   (`bitflags!`, `fieldless_enum!`) write no body from the tokens of the call.
//! - A call that the code does not write (an operator, `Drop`, `Deref`, a `for` loop, `?`,
//!   `format!`) into a fn of an impl of a trait of another crate (an implicit callee): an edge
//!   into it from each fn that names its self type outside a pattern (a type of the signature
//!   or the body, a struct literal, a path `T::..`, `Self`: [`Use::Type`]), so a recursion that
//!   such a call closes is a cycle (review R2 B2-1). The limit: a fn of the same self type gets
//!   no such edge (the code of `T` names `T` at each `Self::A`, and at 0.158.0 those edges give
//!   7 false cycles, `Token::fmt` and the `SourcePosition` compares among them), and code of
//!   another crate can make and drop a value with no fn of the sources that names its type.
//!   So also no implicit callee may reach a site (exit 1).
//!
//! Test code (`#[cfg(test)]` items, `#[test]` fns) is not part of the parser and is skipped.
//!
//! - Macro bodies of other crates are read as comma-separated expressions when they parse, else
//!   as tokens; `macro_rules!` bodies are read only as the rules of a macro of the sources.
//! - [`LOOPS`] are the walks that the patch turns into loops (DES section 1.3, C-1): each one must
//!   exist and must not be on a cycle.
//! - `--assume-site NAME` (repeatable; `name` or `Type::name`) treats that fn as a site at its
//!   first statement. A bump uses it on the new, unpatched sources to find the site list before
//!   the patch is ported. `--explain` prints each edge of a cycle with the use that made it;
//!   `--edges NAME` prints each edge out of one node.
//!
//! Exit codes: 0 no cycle; 1 a cycle, a loop that is missing or recursive, a fn of an impl of a
//! trait of another crate that reaches a site, a call of `may_recurse` that is not a site form,
//! or an ALLOW entry that leaves out no edge;
//! 2 usage error, a file that does not parse,
//! a macro call that does not expand, or a form that the graph cannot see (the list above).
//! Paths print relative to the source dir, with `/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::discouraged::Speculative;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Expr, FnArg, Pat, Stmt, Type};

const USAGE: &str = "\
usage: cargo xtask oxc-sites [--src DIR] [--assume-site NAME]... [--explain] [--edges NAME]...
  --src DIR            the oxc_parser sources (default: third_party/oxc/crates/oxc_parser/src
                       of this tree; sync it first: bash scripts/sync-oxc.sh)
  --assume-site NAME   treat the fn NAME (name or Type::name) as a site (a bump: find the
                       sites on the new sources first)
  --explain            print each edge of a cycle and the use that made it
  --edges NAME         print each edge out of the node NAME (name or Type::name) after the
                       cut, with the use that made it";

const DEFAULT_SRC: &str = "third_party/oxc/crates/oxc_parser/src";

/// The method that a site calls.
const CHECK: &str = "may_recurse";

/// The type whose methods print with no `Type::` prefix.
const PARSER: &str = "ParserImpl";

/// Wrapper types that a method call derefs through to the type inside (`Box<T>` -> `T`).
const DEREF: &[&str] = &[
    "Box",
    "ArenaBox",
    "Rc",
    "Arc",
    "Ref",
    "RefMut",
    "Pin",
    "ManuallyDrop",
];

/// The most nested macro expansions in one item or body (a macro that calls itself stops here).
const MAX_EXPANSION: usize = 64;

/// The tree walks that the patch makes loops, not checks (DES section 1.3, C-1): with a loop
/// a chain gives one result on every host. Each must exist and must not be on a cycle.
const LOOPS: &[(&str, &str)] = &[
    (
        "ParserImpl::set_pure_on_call_or_new_expr",
        "B4: the `/*@__PURE__*/` walk down a member chain",
    ),
    (
        "ParserImpl::jsx_member_expression_eq",
        "B5: the compare of 2 JSX member name chains",
    ),
];

/// An ALLOW entry: (from, to, the use that makes the edge as `--explain` prints it, reason).
type Allow = (&'static str, &'static str, &'static str, &'static str);

/// Edges that are name noise (a call whose receiver type the graph cannot see, joined with a
/// same-name fn of another type). An entry leaves out only the edge that its own use makes
/// (another use that makes the same edge stays), and an entry that leaves out no edge is a
/// finding (review R2 B2-4: after a bump it can hide a new call of the same name).
const ALLOW: &[Allow] = &[
    (
        "ParserImpl::parse_regex_pattern",
        "Parser::parse",
        r#"Method(None, "parse")"#,
        "`LiteralParser::new(..).parse()` is the oxc_regular_expression parser (another crate)",
    ),
    (
        "ParserImpl::parse_regex_pattern",
        "ParserImpl::parse",
        r#"Method(None, "parse")"#,
        "`LiteralParser::new(..).parse()` is the oxc_regular_expression parser (another crate)",
    ),
    (
        "parse_float_with_underscores",
        "Parser::parse",
        r#"Method(None, "parse")"#,
        "`s.cow_replace(..).parse::<f64>()` is `str::parse` (std)",
    ),
    (
        "parse_float_with_underscores",
        "ParserImpl::parse",
        r#"Method(None, "parse")"#,
        "`s.cow_replace(..).parse::<f64>()` is `str::parse` (std)",
    ),
    (
        "JSXClosing::dummy",
        "JSXClosing::dummy",
        r#"Path("Dummy", "dummy")"#,
        "`Dummy::dummy(allocator)` builds the `JSXClosingFragment` field (an oxc_ast type)",
    ),
    (
        "ModifierKind::from_usize_unchecked",
        "ModifierKinds::iter",
        r#"Method(None, "iter")"#,
        "`Self::VARIANTS.iter()` (a debug_assert) is the slice iter of the const array that the \
         `fieldless_enum!` macro (oxc_data_structures) writes",
    ),
    (
        "ModifierKind::from_usize_unchecked",
        "Modifiers::iter",
        r#"Method(None, "iter")"#,
        "`Self::VARIANTS.iter()` (a debug_assert) is the slice iter of the const array that the \
         `fieldless_enum!` macro (oxc_data_structures) writes",
    ),
    (
        "MODIFIER_LENGTHS",
        "ModifierKind::len",
        r#"Method(None, "len")"#,
        "the static initializer calls `ModifierKind::VARIANTS.len()` and `kind.as_str().len()`: \
         slice and str `len` (std), run once at compile time",
    ),
];

/// A call or a use of a name, before it is resolved to nodes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Use {
    /// A method call; the receiver type if the code shows it.
    Method(Option<String>, String),
    /// `Q::name` (Q = the type or module segment before the name).
    Path(String, String),
    /// A one-segment name.
    Bare(String),
    /// A name in macro tokens.
    Any(String),
    /// A type that the fn names outside a pattern (a type in its signature or body, a struct
    /// literal, the type of a path): an edge to each fn of an impl of a trait of another crate
    /// for that type (a call that the code does not write: `Drop`, `Display`, `?`, `for`).
    Type(String),
}

#[derive(Debug, Default)]
struct Func {
    key: String,
    name: String,
    file: String,
    line: usize,
    /// The line of the check statement, if the fn holds a site.
    check_line: Option<usize>,
    /// The statements that call [`CHECK`] and are not a site form: (line, why).
    bad_checks: Vec<(usize, String)>,
    /// The uses before the check (all uses when there is no check).
    uses: BTreeSet<Use>,
    /// A static or const (its initializer), not a fn.
    data: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) files: usize,
    pub(crate) funcs: usize,
    /// The statics and consts with an initializer.
    pub(crate) data: usize,
    /// (label, file, fn line, check line), sorted by file and line.
    pub(crate) sites: Vec<(String, String, usize, usize)>,
    /// (key, reason, found and not on a cycle).
    pub(crate) loops: Vec<(String, String, bool)>,
    /// Each cycle that is left: its node keys, sorted.
    pub(crate) cycles: Vec<Vec<String>>,
    /// The place of each node: (file, line) of its first fn.
    pub(crate) places: BTreeMap<String, (String, usize)>,
    /// For the nodes on a cycle: each edge inside the cycle -> the use that made it.
    pub(crate) why: BTreeMap<String, BTreeMap<String, String>>,
    /// (a fn of an impl of a trait of another crate, the path to the first site it reaches).
    pub(crate) implicit: Vec<(String, Vec<String>)>,
    /// (label, file, line, why) of each statement that calls [`CHECK`] and is not a site form.
    pub(crate) bad_checks: Vec<(String, String, usize, String)>,
    /// The graph after the cut: node -> the nodes it calls.
    pub(crate) edges: BTreeMap<String, BTreeMap<String, String>>,
    /// (from, to, use) of each ALLOW entry that leaves out no edge.
    pub(crate) unused_allow: Vec<(String, String, String)>,
}

impl Report {
    pub(crate) fn ok(&self) -> bool {
        self.cycles.is_empty()
            && self.loops.iter().all(|l| l.2)
            && self.implicit.is_empty()
            && self.bad_checks.is_empty()
            && self.unused_allow.is_empty()
    }
}

fn label(key: &str) -> &str {
    key.strip_prefix(PARSER)
        .and_then(|k| k.strip_prefix("::"))
        .unwrap_or(key)
}

pub fn main(args: &[String]) -> Result<bool, String> {
    let mut src: Option<PathBuf> = None;
    let mut assume = BTreeSet::new();
    let mut explain = false;
    let mut show = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--src" => src = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--assume-site" => {
                assume.insert(it.next().ok_or(USAGE)?.clone());
            }
            "--explain" => explain = true,
            "--edges" => show.push(it.next().ok_or(USAGE)?.clone()),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(true);
            }
            _ => return Err(format!("unknown argument {a:?}\n{USAGE}")),
        }
    }
    let src = match src {
        Some(s) => s,
        None => crate::exit::default_root()?.join(DEFAULT_SRC),
    };
    if !src.is_dir() {
        return Err(format!(
            "{}: no such directory (run bash scripts/sync-oxc.sh first)",
            src.display()
        ));
    }
    let mut files = Vec::new();
    rs_files(&src, &mut files)?;
    let mut sources = Vec::new();
    for f in &files {
        let rel = rel_path(&src, f);
        let text = fs::read_to_string(f).map_err(|e| format!("{rel}: {e}"))?;
        sources.push((rel, text));
    }
    let r = run(&sources, &assume, ALLOW)?;
    print_report(&r, &src, explain);
    for name in &show {
        let found: Vec<&String> = r
            .edges
            .keys()
            .filter(|k| *k == name || label(k) == name)
            .collect();
        if found.is_empty() {
            println!("oxc-sites: --edges {name}: no such node");
        }
        for k in found {
            for (to, u) in &r.edges[k] {
                println!("  edge {} -> {}: {u}", label(k), label(to));
            }
        }
    }
    Ok(r.ok())
}

fn print_report(r: &Report, src: &Path, explain: bool) {
    println!(
        "oxc-sites: {} files, {} functions, {} statics and consts under {}",
        r.files,
        r.funcs,
        r.data,
        src.display()
    );
    println!("| # | Site | Function at | Check at |");
    println!("|---|---|---|---|");
    for (i, (label, file, line, check)) in r.sites.iter().enumerate() {
        println!("| {} | `{label}` | {file}:{line} | :{check} |", i + 1);
    }
    for (key, why, ok) in &r.loops {
        let at = r
            .places
            .get(key)
            .map(|(f, l)| format!("{f}:{l}"))
            .unwrap_or_else(|| "missing".into());
        let state = if *ok { "loop" } else { "NOT a loop" };
        println!("| - | `{}` ({why}) | {at} | {state} |", label(key));
    }
    for c in &r.cycles {
        let at: Vec<String> = c
            .iter()
            .map(|n| match r.places.get(n) {
                Some((f, l)) => format!("{} ({f}:{l})", label(n)),
                None => label(n).to_string(),
            })
            .collect();
        println!(
            "oxc-sites: CYCLE with no site ({} fns): {}",
            c.len(),
            at.join(", ")
        );
        if explain {
            for n in c {
                for (to, u) in r.why.get(n).into_iter().flatten() {
                    println!("  edge {} -> {}: {u}", label(n), label(to));
                }
            }
        }
    }
    for (f, path) in &r.implicit {
        let path: Vec<&str> = path.iter().map(|n| label(n)).collect();
        println!(
            "oxc-sites: IMPLICIT CALLEE reaches a site: `{}` (an impl of a trait of another crate: \
             a call that the code does not write) -> {}",
            label(f),
            path.join(" -> ")
        );
    }
    for (from, to, u) in &r.unused_allow {
        println!(
            "oxc-sites: UNUSED ALLOW entry: `{from}` -> `{to}` by `{u}` leaves out no edge \
             (remove it, or fix it to the use that makes the edge now)"
        );
    }
    for (label, file, line, why) in &r.bad_checks {
        println!(
            "oxc-sites: NOT A SITE FORM at {file}:{line} in `{label}`: {why} (the 2 forms: \
             `if C::CHECKS_RECURSION && !R.config.may_recurse() {{ return R.recursion_stopped(); }}` \
             or `{{ return Err(R.recursion_stopped_error()); }}`)"
        );
    }
    let loops_ok = r.loops.iter().filter(|l| l.2).count();
    println!(
        "oxc-sites: {} sites = {} checks + {} loops; {} cycle{} with no site: {}",
        r.sites.len() + loops_ok,
        r.sites.len(),
        loops_ok,
        r.cycles.len(),
        if r.cycles.len() == 1 { "" } else { "s" },
        if r.ok() { "PASS" } else { "FAIL" }
    );
}

/// Builds the graph of `sources` (path, text) and finds the cycles that no site cuts; `allow`
/// is [`ALLOW`] (a test gives its own entries).
pub(crate) fn run(
    sources: &[(String, String)],
    assume: &BTreeSet<String>,
    allow: &[Allow],
) -> Result<Report, String> {
    let mut asts = Vec::new();
    for (rel, text) in sources {
        let ast = syn::parse_file(text).map_err(|e| {
            let at = e.span().start();
            format!("{rel}:{}:{}: does not parse: {e}", at.line, at.column + 1)
        })?;
        asts.push((rel, ast));
    }
    // Pass 0: the macros of the sources; item macros expand in place.
    let mut defs = Macros::default();
    for (_, ast) in &asts {
        Defs(&mut defs.0).visit_file(ast);
    }
    for (rel, ast) in &mut asts {
        let items = std::mem::take(&mut ast.items);
        ast.items = expand_items(items, &defs, rel, 0)?;
    }
    // Pass 1: struct fields, traits, impl types, return types.
    let mut types = Types::default();
    for (rel, ast) in &asts {
        types.file = (*rel).clone();
        types.visit_file(ast);
    }
    types.check_fields()?;
    // Pass 2: functions, statics and consts.
    let mut funcs = Vec::new();
    for (rel, ast) in &asts {
        let mut c = Collector {
            file: (*rel).clone(),
            impl_self: Vec::new(),
            generics: Vec::new(),
            types: &types,
            defs: &defs,
            out: &mut funcs,
        };
        c.visit_file(ast);
    }
    for f in &mut funcs {
        if f.check_line.is_none() && (assume.contains(&f.key) || assume.contains(label(&f.key))) {
            f.check_line = Some(f.line);
            f.uses.clear();
        }
    }
    // Name tables.
    let keys: BTreeSet<&str> = funcs.iter().map(|f| f.key.as_str()).collect();
    let mut by_name: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut methods: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut free: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for f in &funcs {
        by_name.entry(&f.name).or_default().insert(&f.key);
        if f.key.contains("::") {
            methods.entry(&f.name).or_default().insert(&f.key);
        } else {
            free.entry(&f.name).or_default().insert(&f.key);
        }
    }
    let empty = BTreeSet::new();
    let all = |m: &BTreeMap<&str, BTreeSet<&str>>, n: &str| -> Vec<String> {
        m.get(n)
            .unwrap_or(&empty)
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    // The other names of a type or trait (review R2 B2-2): a `use .. as` rename and a type alias
    // of the sources. `names_of(t)`: `t`, each name that `t` stands for (to the end of a chain
    // of renames and aliases), and each name that stands for `t`.
    let mut stands_for: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (new, old) in &types.other_names {
        if new != old {
            stands_for.entry(new).or_default().insert(old);
        }
    }
    let chain = |t: &str| -> BTreeSet<String> {
        let mut seen = BTreeSet::from([t.to_string()]);
        let mut todo = vec![t.to_string()];
        while let Some(x) = todo.pop() {
            for y in stands_for.get(x.as_str()).into_iter().flatten() {
                if seen.insert(y.to_string()) {
                    todo.push(y.to_string());
                }
            }
        }
        seen
    };
    let names_of = |t: &str| -> BTreeSet<String> {
        let mut names = chain(t);
        for x in stands_for.keys() {
            if chain(x).contains(t) {
                names.insert(x.to_string());
            }
        }
        names
    };
    // A use with the type or trait `t` shown, `t` by one of its names: see the module table.
    let typed_as = |t: &str, n: &str| -> Vec<String> {
        let k = format!("{t}::{n}");
        let is_trait = types.traits.contains(t) || types.trait_fns.contains_key(t);
        if is_trait && !types.blanket.contains(n) {
            let mut v: Vec<String> = types
                .trait_fns
                .get(t)
                .and_then(|fns| fns.get(n))
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            if keys.contains(k.as_str()) {
                v.push(k);
            }
            if v.is_empty() && types.traits.contains(t) {
                // A trait of the sources with no impl of `n` here: any method of that name.
                return all(&methods, n);
            }
            return v;
        }
        if keys.contains(k.as_str()) {
            vec![k]
        } else if types.impls.contains(t) || types.blanket.contains(n) {
            all(&methods, n)
        } else {
            Vec::new()
        }
    };
    let typed = |t: &str, n: &str| -> Vec<String> {
        let all: BTreeSet<String> = names_of(t).iter().flat_map(|x| typed_as(x, n)).collect();
        all.into_iter().collect()
    };
    // The fns of the impls of traits of other crates, by their self type.
    let mut implicit_by: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for k in &types.implicit {
        if let Some((t, _)) = k.rsplit_once("::") {
            implicit_by.entry(t).or_default().push(k.clone());
        }
    }
    let implicit_of = |t: &str| -> Vec<String> {
        let all: BTreeSet<String> = names_of(t)
            .iter()
            .flat_map(|x| implicit_by.get(x.as_str()).cloned().unwrap_or_default())
            .collect();
        all.into_iter().collect()
    };
    let mut edges: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut places = BTreeMap::new();
    let mut allow_used = vec![false; allow.len()];
    for f in &funcs {
        let out = edges.entry(f.key.clone()).or_default();
        let own_type = f.key.rsplit_once("::").map(|(t, _)| t);
        for u in &f.uses {
            let to: Vec<String> = match u {
                // Not from a fn of the same type: a method of T that names T (`Self::A`, `Self
                // { .. }`) is the code of T itself (the limit is in the module doc).
                Use::Type(t) if own_type == Some(t.as_str()) => Vec::new(),
                Use::Type(t) => implicit_of(t),
                Use::Method(Some(t), n) => typed(t, n),
                Use::Method(None, n) => all(&methods, n),
                Use::Path(q, n) if q.starts_with(|c: char| c.is_ascii_lowercase()) => all(&free, n),
                Use::Path(q, n) => typed(q, n),
                Use::Bare(n) => all(&free, n),
                Use::Any(n) => all(&by_name, n),
            };
            let text = format!("{u:?}");
            for k in to {
                match allow
                    .iter()
                    .position(|(a, b, w, _)| *a == f.key && *b == k && *w == text)
                {
                    Some(i) => allow_used[i] = true,
                    None => {
                        out.entry(k).or_insert_with(|| text.clone());
                    }
                }
            }
        }
        places
            .entry(f.key.clone())
            .or_insert_with(|| (f.file.clone(), f.line));
    }
    let edges: BTreeMap<String, BTreeMap<String, String>> = edges;
    let mut cycles = Vec::new();
    let mut why = BTreeMap::new();
    for comp in sccs(&edges) {
        if comp.len() > 1 || edges[&comp[0]].contains_key(&comp[0]) {
            let set: BTreeSet<&String> = comp.iter().collect();
            for n in &comp {
                let inside: BTreeMap<String, String> = edges[n]
                    .iter()
                    .filter(|(to, _)| set.contains(to))
                    .map(|(to, u)| (to.clone(), u.clone()))
                    .collect();
                why.insert(n.clone(), inside);
            }
            cycles.push(comp);
        }
    }
    cycles.sort();
    let on_cycle: BTreeSet<&String> = cycles.iter().flatten().collect();
    let loops = LOOPS
        .iter()
        .map(|(k, why)| {
            let k = k.to_string();
            let ok = keys.contains(k.as_str()) && !on_cycle.contains(&k);
            (k, why.to_string(), ok)
        })
        .collect();
    let mut sites: Vec<(String, String, usize, usize)> = funcs
        .iter()
        .filter_map(|f| {
            f.check_line
                .map(|c| (label(&f.key).to_string(), f.file.clone(), f.line, c))
        })
        .collect();
    sites.sort_by(|a, b| (&a.1, a.2).cmp(&(&b.1, b.2)));
    // The fns that code of another crate calls with no call in these sources: none may reach a site.
    let site_keys: BTreeSet<&str> = funcs
        .iter()
        .filter(|f| f.check_line.is_some())
        .map(|f| f.key.as_str())
        .collect();
    let mut implicit = Vec::new();
    for f in &types.implicit {
        if let Some(path) = path_to(&edges, f, &site_keys) {
            implicit.push((f.clone(), path));
        }
    }
    let mut bad_checks: Vec<(String, String, usize, String)> = funcs
        .iter()
        .flat_map(|f| {
            f.bad_checks.iter().map(|(line, why)| {
                (
                    label(&f.key).to_string(),
                    f.file.clone(),
                    *line,
                    why.clone(),
                )
            })
        })
        .collect();
    bad_checks.sort_by(|a, b| (&a.1, a.2).cmp(&(&b.1, b.2)));
    let data = funcs.iter().filter(|f| f.data).count();
    let unused_allow = allow
        .iter()
        .zip(&allow_used)
        .filter(|(_, used)| !**used)
        .map(|((a, b, w, _), _)| (a.to_string(), b.to_string(), w.to_string()))
        .collect();
    Ok(Report {
        files: sources.len(),
        funcs: funcs.len() - data,
        data,
        sites,
        loops,
        cycles,
        places,
        why,
        implicit,
        bad_checks,
        edges,
        unused_allow,
    })
}

/// The shortest path from `from` to a node of `to` (both ends included), if there is one.
fn path_to(
    edges: &BTreeMap<String, BTreeMap<String, String>>,
    from: &str,
    to: &BTreeSet<&str>,
) -> Option<Vec<String>> {
    let mut back: BTreeMap<&str, &str> = BTreeMap::new();
    let mut seen = BTreeSet::from([from]);
    let mut queue = std::collections::VecDeque::from([from]);
    while let Some(n) = queue.pop_front() {
        if to.contains(n) {
            let mut path = vec![n.to_string()];
            let mut at = n;
            while let Some(&p) = back.get(at) {
                path.push(p.to_string());
                at = p;
            }
            path.reverse();
            return Some(path);
        }
        for m in edges.get(n).into_iter().flat_map(|e| e.keys()) {
            if seen.insert(m.as_str()) {
                back.insert(m.as_str(), n);
                queue.push_back(m.as_str());
            }
        }
    }
    None
}

/// Tarjan's strongly connected groups, without recursion; each group sorted.
fn sccs(edges: &BTreeMap<String, BTreeMap<String, String>>) -> Vec<Vec<String>> {
    let names: Vec<&String> = edges.keys().collect();
    let index_of: BTreeMap<&String, usize> =
        names.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let adj: Vec<Vec<usize>> = names
        .iter()
        .map(|n| {
            edges[*n]
                .keys()
                .filter_map(|m| index_of.get(m).copied())
                .collect()
        })
        .collect();
    let n = names.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut out = Vec::new();
    for start in 0..n {
        if index[start] != usize::MAX {
            continue;
        }
        // (node, the next edge to look at)
        let mut work = vec![(start, 0usize)];
        index[start] = next;
        low[start] = next;
        next += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(top) = work.last_mut() {
            let (v, ei) = (top.0, top.1);
            if ei < adj[v].len() {
                top.1 += 1;
                let w = adj[v][ei];
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                work.pop();
                if let Some(&(u, _)) = work.last() {
                    low[u] = low[u].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        comp.push(names[w].clone());
                        if w == v {
                            break;
                        }
                    }
                    comp.sort();
                    out.push(comp);
                }
            }
        }
    }
    out
}

/// The last path segment of a type, through references, parens, groups and the [`DEREF`]
/// wrappers. A pointer, slice, array or tuple is a foreign type: `<builtin>`.
fn type_name(t: &Type) -> Option<String> {
    match t {
        Type::Path(p) => {
            let last = p.path.segments.last()?;
            let name = last.ident.to_string();
            if DEREF.contains(&name.as_str())
                && let syn::PathArguments::AngleBracketed(a) = &last.arguments
                && let Some(inner) = a.args.iter().find_map(|g| match g {
                    syn::GenericArgument::Type(t) => Some(t),
                    _ => None,
                })
            {
                return type_name(inner);
            }
            Some(name)
        }
        Type::Reference(r) => type_name(&r.elem),
        Type::Paren(p) => type_name(&p.elem),
        Type::Group(g) => type_name(&g.elem),
        Type::Ptr(_) | Type::Slice(_) | Type::Array(_) | Type::Tuple(_) => Some("<builtin>".into()),
        _ => None,
    }
}

/// Test code: a `#[test]` fn or a `#[cfg(test)]` item.
fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || (a.path().is_ident("cfg") && a.parse_args::<syn::Ident>().is_ok_and(|i| i == "test"))
    })
}

/// The names of the type parameters of `g`.
fn type_params(g: &syn::Generics) -> impl Iterator<Item = String> + '_ {
    g.type_params().map(|p| p.ident.to_string())
}

/// The type parameters of `g` with an `Fn`, `FnMut` or `FnOnce` bound (in the list or in the
/// `where` clause): a value of such a type is a fn value (review R2 B2-2 (c)).
fn fn_params(g: &syn::Generics) -> BTreeSet<String> {
    let is_fn = |b: &syn::TypeParamBound| {
        matches!(b, syn::TypeParamBound::Trait(t)
        if t.path.segments.last().is_some_and(|s| {
            matches!(s.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce")
        }))
    };
    let params: BTreeSet<String> = type_params(g).collect();
    let mut out: BTreeSet<String> = g
        .type_params()
        .filter(|p| p.bounds.iter().any(is_fn))
        .map(|p| p.ident.to_string())
        .collect();
    for w in g.where_clause.iter().flat_map(|w| &w.predicates) {
        if let syn::WherePredicate::Type(pt) = w
            && let Type::Path(tp) = &pt.bounded_ty
            && let Some(id) = tp.path.get_ident()
            && params.contains(&id.to_string())
            && pt.bounds.iter().any(is_fn)
        {
            out.insert(id.to_string());
        }
    }
    out
}

/// The names of the lifetime-free parameters of `g` (type and const), in their places.
fn param_places(g: &syn::Generics) -> Vec<String> {
    g.params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(t.ident.to_string()),
            syn::GenericParam::Const(c) => Some(c.ident.to_string()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect()
}

/// Does a value of this type hold a fn value that code can call (a fn pointer, a `dyn` trait
/// object, or a type alias in `aliases` of one)?
fn holds_fn_value(t: &Type, aliases: &BTreeSet<String>) -> bool {
    match t {
        Type::FnPtr(_) | Type::TraitObject(_) => true,
        Type::Path(p) => p.path.segments.iter().any(|s| {
            aliases.contains(&s.ident.to_string()) || match &s.arguments {
                syn::PathArguments::AngleBracketed(a) => a.args.iter().any(
                    |g| matches!(g, syn::GenericArgument::Type(t) if holds_fn_value(t, aliases)),
                ),
                _ => false,
            }
        }),
        Type::Reference(r) => holds_fn_value(&r.elem, aliases),
        Type::Ptr(p) => holds_fn_value(&p.elem, aliases),
        Type::Slice(s) => holds_fn_value(&s.elem, aliases),
        Type::Array(a) => holds_fn_value(&a.elem, aliases),
        Type::Paren(p) => holds_fn_value(&p.elem, aliases),
        Type::Group(g) => holds_fn_value(&g.elem, aliases),
        Type::Tuple(t) => t.elems.iter().any(|e| holds_fn_value(e, aliases)),
        _ => false,
    }
}

/// Adds a known type (or `None`, not known) for `key`: a second type that is not the first
/// makes it not known (review R2 B2-5: one name for two structs or two fns).
fn merge(map: &mut BTreeMap<String, Option<String>>, key: String, t: Option<String>) {
    match map.entry(key) {
        std::collections::btree_map::Entry::Vacant(v) => {
            v.insert(t);
        }
        std::collections::btree_map::Entry::Occupied(mut o) => {
            if *o.get() != t {
                o.insert(None);
            }
        }
    }
}

/// A struct or enum field for the fn value check: (place, type, the type parameters of the
/// struct or enum with an `Fn` bound, the struct or enum name, its parameter places as
/// [`param_places`] gives them).
type FieldType = (String, Type, BTreeSet<String>, String, Vec<String>);

/// Pass 1: what the sources say about types.
#[derive(Default)]
struct Types {
    /// The file that is visited now.
    file: String,
    /// struct name -> field name -> field type name (not for a field of a generic type).
    /// `None`: not known (a field of a generic type, or two structs of the name whose fields of
    /// the name have other types: review R2 B2-5).
    fields: BTreeMap<String, BTreeMap<String, Option<String>>>,
    /// The self type names of every impl.
    impls: BTreeSet<String>,
    /// The traits that the sources define.
    traits: BTreeSet<String>,
    /// The method names of blanket impls (`impl<X: B> Tr for X`).
    blanket: BTreeSet<String>,
    /// trait name -> fn name -> the keys of the impls of that trait in the sources.
    trait_fns: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// (trait name, fn key) of each fn of an impl of a trait.
    impl_fns: Vec<(String, String)>,
    /// The fns of the impls of traits of other crates (filled by `check_fields`).
    implicit: Vec<String>,
    /// fn or associated const key -> its type name (`Self` is the impl type).
    /// `None`: not known (two fns of the key with other return types, as for `fields`).
    rets: BTreeMap<String, Option<String>>,
    /// Type aliases (name -> type) and the struct and enum fields, for the check.
    aliases: Vec<(String, Type)>,
    field_types: Vec<FieldType>,
    /// type name -> the places (no lifetimes) of the arguments of its self type that an impl
    /// gives an `Fn` bound: `impl<F: Fn()> Hook<F>` gives `Hook` -> {0} (review R2 B2-2 (c)).
    impl_fn_places: BTreeMap<String, BTreeSet<usize>>,
    /// (a name, the name of the type or trait that it stands for): each `use .. as` rename and
    /// each type alias of the sources (review R2 B2-2).
    other_names: Vec<(String, String)>,
    impl_self: Vec<Option<String>>,
    /// The type parameters of each impl around the current item.
    generics: Vec<BTreeSet<String>>,
}

impl Types {
    /// A field that holds a fn value fails the run; then the implicit callees are known.
    fn check_fields(&mut self) -> Result<(), String> {
        let mut aliases = BTreeSet::new();
        loop {
            let before = aliases.len();
            for (name, t) in &self.aliases {
                if holds_fn_value(t, &aliases) {
                    aliases.insert(name.clone());
                }
            }
            if aliases.len() == before {
                break;
            }
        }
        for (place, t, fn_params, owner, params) in &self.field_types {
            let mut names = aliases.clone();
            names.extend(fn_params.iter().cloned());
            // An `Fn` bound on an impl of the owner, not on the owner itself.
            for &i in self.impl_fn_places.get(owner).into_iter().flatten() {
                names.extend(params.get(i).cloned());
            }
            if holds_fn_value(t, &names) {
                return Err(format!(
                    "{place}: the field holds a fn value (`{}`): a call through it is not an \
                     edge of the graph",
                    t.to_token_stream()
                ));
            }
        }
        self.implicit = self
            .impl_fns
            .iter()
            .filter(|(tr, _)| !self.traits.contains(tr))
            .map(|(_, k)| k.clone())
            .collect();
        Ok(())
    }

    /// `owner`: the struct, or `Enum::Variant`; `ty`: the struct or enum name.
    fn fields_of(&mut self, owner: &str, ty: &str, generics: &syn::Generics, fields: &syn::Fields) {
        let fn_params = fn_params(generics);
        let places = param_places(generics);
        for (i, f) in fields.iter().enumerate() {
            let id = f
                .ident
                .as_ref()
                .map_or_else(|| i.to_string(), |id| id.to_string());
            let at = f.ty.span().start().line;
            self.field_types.push((
                format!("{}:{at}: {owner}.{id}", self.file),
                f.ty.clone(),
                fn_params.clone(),
                ty.to_string(),
                places.clone(),
            ));
        }
    }
}

impl<'ast> Visit<'ast> for Types {
    fn visit_item_struct(&mut self, s: &'ast syn::ItemStruct) {
        let generics: BTreeSet<String> = type_params(&s.generics).collect();
        let name = s.ident.to_string();
        self.fields_of(&name, &name, &s.generics, &s.fields);
        let e = self.fields.entry(s.ident.to_string()).or_default();
        for f in &s.fields {
            if let Some(id) = &f.ident {
                let t = type_name(&f.ty).filter(|t| !generics.contains(t));
                merge(e, id.to_string(), t);
            }
        }
        visit::visit_item_struct(self, s);
    }

    fn visit_item_enum(&mut self, e: &'ast syn::ItemEnum) {
        for v in &e.variants {
            let owner = format!("{}::{}", e.ident, v.ident);
            self.fields_of(&owner, &e.ident.to_string(), &e.generics, &v.fields);
        }
        visit::visit_item_enum(self, e);
    }

    fn visit_item_type(&mut self, t: &'ast syn::ItemType) {
        self.aliases.push((t.ident.to_string(), (*t.ty).clone()));
        if let Some(target) = type_name(&t.ty) {
            self.other_names.push((t.ident.to_string(), target));
        }
    }

    fn visit_use_rename(&mut self, r: &'ast syn::UseRename) {
        if r.rename != "_" {
            self.other_names
                .push((r.rename.to_string(), r.ident.to_string()));
        }
    }

    fn visit_item_trait(&mut self, t: &'ast syn::ItemTrait) {
        self.traits.insert(t.ident.to_string());
        visit::visit_item_trait(self, t);
    }

    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if !is_test(&m.attrs) {
            visit::visit_item_mod(self, m);
        }
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if is_test(&i.attrs) {
            return;
        }
        let t = type_name(&i.self_ty);
        // The arguments of the self type that this impl gives an `Fn` bound (B2-2 (c)).
        let fns = fn_params(&i.generics);
        if let Type::Path(p) = &*i.self_ty
            && let Some(last) = p.path.segments.last()
            && let syn::PathArguments::AngleBracketed(a) = &last.arguments
        {
            let args = a
                .args
                .iter()
                .filter(|g| !matches!(g, syn::GenericArgument::Lifetime(_)));
            for (place, g) in args.enumerate() {
                if let syn::GenericArgument::Type(Type::Path(tp)) = g
                    && tp
                        .path
                        .get_ident()
                        .is_some_and(|id| fns.contains(&id.to_string()))
                {
                    self.impl_fn_places
                        .entry(last.ident.to_string())
                        .or_default()
                        .insert(place);
                }
            }
        }
        let generic = t
            .as_ref()
            .is_some_and(|t| i.generics.type_params().any(|p| p.ident == t.as_str()));
        if generic {
            for it in &i.items {
                if let syn::ImplItem::Fn(f) = it {
                    self.blanket.insert(f.sig.ident.to_string());
                }
            }
        } else if let Some(t) = &t {
            self.impls.insert(t.clone());
            if let Some((tr, _)) = &i.trait_
                && let Some(tr) = tr.segments.last()
            {
                let tr = tr.ident.to_string();
                let e = self.trait_fns.entry(tr.clone()).or_default();
                for it in &i.items {
                    if let syn::ImplItem::Fn(f) = it {
                        let k = format!("{t}::{}", f.sig.ident);
                        e.entry(f.sig.ident.to_string())
                            .or_default()
                            .insert(k.clone());
                        self.impl_fns.push((tr.clone(), k));
                    }
                }
            }
        }
        self.impl_self.push(t);
        self.generics.push(type_params(&i.generics).collect());
        visit::visit_item_impl(self, i);
        self.generics.pop();
        self.impl_self.pop();
    }

    fn visit_impl_item_const(&mut self, c: &'ast syn::ImplItemConst) {
        if let Some(Some(t)) = self.impl_self.last() {
            merge(
                &mut self.rets,
                format!("{t}::{}", c.ident),
                type_name(&c.ty),
            );
        }
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        let generic = |r: &str| {
            self.generics.last().is_some_and(|g| g.contains(r))
                || f.sig.generics.type_params().any(|p| p.ident == r)
        };
        if let Some(Some(t)) = self.impl_self.last() {
            let r = match &f.sig.output {
                syn::ReturnType::Type(_, ty) => type_name(ty).filter(|r| !generic(r)),
                syn::ReturnType::Default => None,
            };
            let r = r.map(|r| if r == "Self" { t.clone() } else { r });
            let key = format!("{t}::{}", f.sig.ident);
            merge(&mut self.rets, key, r);
        }
        // Local types and impls in the body.
        let saved = (
            std::mem::take(&mut self.impl_self),
            std::mem::take(&mut self.generics),
        );
        visit::visit_block(self, &f.block);
        (self.impl_self, self.generics) = saved;
    }
}

/// Pass 2: the functions of one file.
struct Collector<'o> {
    file: String,
    /// The self type of each impl or trait around the current item.
    impl_self: Vec<Option<String>>,
    /// The type parameters of each impl or trait around the current item.
    generics: Vec<BTreeSet<String>>,
    types: &'o Types,
    defs: &'o Macros,
    out: &'o mut Vec<Func>,
}

impl Collector<'_> {
    fn key(&self, name: &str) -> (Option<String>, String) {
        let self_ty = self.impl_self.last().cloned().flatten();
        let key = match &self_ty {
            Some(t) => format!("{t}::{name}"),
            None => name.to_string(),
        };
        (self_ty, key)
    }

    fn func(&mut self, sig: &syn::Signature, block: &Block) {
        let name = sig.ident.to_string();
        let (self_ty, key) = self.key(&name);
        let mut generics = self.generics.last().cloned().unwrap_or_default();
        generics.extend(type_params(&sig.generics));
        let mut params = BTreeMap::new();
        for a in &sig.inputs {
            if let FnArg::Typed(pt) = a
                && let Pat::Ident(pi) = &*pt.pat
                && let Some(t) = type_name(&pt.ty)
                && !generics.contains(&t)
            {
                params.insert(pi.ident.to_string(), t);
            }
        }
        // A name that a pattern of the body binds again (`let p = ..`, a closure or match arm)
        // has another type there: its type is not known (every method of the name is an edge).
        for name in bound_names(block) {
            params.remove(&name);
        }
        let mut f = Func {
            key,
            name,
            file: self.file.clone(),
            line: sig.ident.span().start().line,
            ..Func::default()
        };
        let mut nested = Vec::new();
        // The types of the signature (not the `self` receiver): the fn takes, makes or drops a
        // value of each one, so they count as uses before the check.
        let mut n = Uses {
            self_ty: self_ty.as_deref(),
            params: &params,
            generics: &generics,
            types: self.types,
            defs: self.defs,
            uses: &mut f.uses,
            nested: &mut nested,
            depth: 0,
            in_pat: false,
        };
        for a in &sig.inputs {
            if let FnArg::Typed(pt) = a {
                n.visit_type(&pt.ty);
            }
        }
        if let syn::ReturnType::Type(_, ty) = &sig.output {
            n.visit_type(ty);
        }
        let mut behind = BTreeSet::new();
        for stmt in &block.stmts {
            match check_stmt(stmt, &generics) {
                Some(Ok(())) if f.check_line.is_none() => {
                    // The check is the cut: its own uses are the check and the stop.
                    f.check_line = Some(stmt.span().start().line);
                    continue;
                }
                Some(Err(why)) => f.bad_checks.push((stmt.span().start().line, why)),
                _ => {}
            }
            // Behind the check only nested items count (they are their own nodes).
            let uses = if f.check_line.is_none() {
                &mut f.uses
            } else {
                &mut behind
            };
            let mut n = Uses {
                self_ty: self_ty.as_deref(),
                params: &params,
                generics: &generics,
                types: self.types,
                defs: self.defs,
                uses,
                nested: &mut nested,
                depth: 0,
                in_pat: false,
            };
            n.visit_stmt(stmt);
        }
        self.out.push(f);
        self.nested(nested);
    }

    /// A static or const with an initializer: a node with the uses of the initializer.
    fn data(&mut self, ident: &syn::Ident, expr: &Expr) {
        let name = ident.to_string();
        let (self_ty, key) = self.key(&name);
        let generics = self.generics.last().cloned().unwrap_or_default();
        let params = BTreeMap::new();
        let mut f = Func {
            key,
            name,
            file: self.file.clone(),
            line: ident.span().start().line,
            data: true,
            ..Func::default()
        };
        let mut nested = Vec::new();
        let mut n = Uses {
            self_ty: self_ty.as_deref(),
            params: &params,
            generics: &generics,
            types: self.types,
            defs: self.defs,
            uses: &mut f.uses,
            nested: &mut nested,
            depth: 0,
            in_pat: false,
        };
        n.visit_expr(expr);
        self.out.push(f);
        self.nested(nested);
    }

    /// The fns and impls inside a body: their own nodes, outside the impl around the body.
    fn nested(&mut self, nested: Vec<syn::Item>) {
        let saved = (
            std::mem::take(&mut self.impl_self),
            std::mem::take(&mut self.generics),
        );
        for item in &nested {
            self.visit_item(item);
        }
        (self.impl_self, self.generics) = saved;
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if !is_test(&m.attrs) {
            visit::visit_item_mod(self, m);
        }
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if is_test(&i.attrs) {
            return;
        }
        self.impl_self.push(type_name(&i.self_ty));
        self.generics.push(type_params(&i.generics).collect());
        visit::visit_item_impl(self, i);
        self.generics.pop();
        self.impl_self.pop();
    }

    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        self.impl_self.push(Some(i.ident.to_string()));
        self.generics.push(type_params(&i.generics).collect());
        visit::visit_item_trait(self, i);
        self.generics.pop();
        self.impl_self.pop();
    }

    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        if is_test(&i.attrs) {
            return;
        }
        let saved = (
            std::mem::take(&mut self.impl_self),
            std::mem::take(&mut self.generics),
        );
        self.func(&i.sig, &i.block);
        (self.impl_self, self.generics) = saved;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        if !is_test(&i.attrs) {
            self.func(&i.sig, &i.block);
        }
    }

    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        if let Some(b) = &i.default {
            self.func(&i.sig, b);
        }
    }

    fn visit_item_static(&mut self, s: &'ast syn::ItemStatic) {
        if !is_test(&s.attrs) {
            let saved = std::mem::take(&mut self.impl_self);
            self.data(&s.ident, &s.expr);
            self.impl_self = saved;
        }
    }

    fn visit_item_const(&mut self, c: &'ast syn::ItemConst) {
        if !is_test(&c.attrs) {
            let saved = std::mem::take(&mut self.impl_self);
            self.data(&c.ident, &c.expr);
            self.impl_self = saved;
        }
    }

    fn visit_impl_item_const(&mut self, c: &'ast syn::ImplItemConst) {
        self.data(&c.ident, &c.expr);
    }

    fn visit_trait_item_const(&mut self, c: &'ast syn::TraitItemConst) {
        if let Some((_, e)) = &c.default {
            self.data(&c.ident, e);
        }
    }

    fn visit_item_macro(&mut self, _: &'ast syn::ItemMacro) {
        // Left after pass 0: `macro_rules!` (read in pass 0) and item macros of other crates
        // whose tokens hold no fn (`bitflags!`, `fieldless_enum!`).
    }
}

/// A statement that calls [`CHECK`]: `Ok` if it is a site form, else why not. `None` if it does
/// not call it (a nested item is its own node and is not looked at here). `generics`: the type
/// parameters of the fn and its impl.
fn check_stmt(stmt: &Stmt, generics: &BTreeSet<String>) -> Option<Result<(), String>> {
    struct Find(usize);
    impl<'ast> Visit<'ast> for Find {
        fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
            if m.method == CHECK {
                self.0 += 1;
            }
            visit::visit_expr_method_call(self, m);
        }
        fn visit_item(&mut self, _: &'ast syn::Item) {}
    }
    let mut f = Find(0);
    f.visit_stmt(stmt);
    match f.0 {
        0 => None,
        1 => Some(site_form(stmt, generics)),
        n => Some(Err(format!("{n} calls of {CHECK} in one statement"))),
    }
}

/// Every name that a pattern in `block` binds (not in a nested item).
fn bound_names(block: &Block) -> BTreeSet<String> {
    struct Bind(BTreeSet<String>);
    impl<'ast> Visit<'ast> for Bind {
        fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
            self.0.insert(p.ident.to_string());
            visit::visit_pat_ident(self, p);
        }
        fn visit_item(&mut self, _: &'ast syn::Item) {}
    }
    let mut b = Bind(BTreeSet::new());
    b.visit_block(block);
    b.0
}

/// The one-segment path `R` (the parser) of an expression, if it is one.
fn bare_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Path(p) if p.qself.is_none() && p.path.segments.len() == 1 => {
            let seg = &p.path.segments[0];
            seg.arguments.is_none().then(|| seg.ident.to_string())
        }
        _ => None,
    }
}

/// `R.name()` with no arguments, R the receiver `recv`.
fn is_call_on(e: &Expr, recv: &str, name: &str) -> bool {
    matches!(e, Expr::MethodCall(m)
        if m.method == name
            && m.args.is_empty()
            && m.turbofish.is_none()
            && bare_ident(&m.receiver).as_deref() == Some(recv))
}

/// Is `stmt` one of the 2 site forms of the patch (the module doc)?
fn site_form(stmt: &Stmt, generics: &BTreeSet<String>) -> Result<(), String> {
    let Stmt::Expr(Expr::If(i), _) = stmt else {
        return Err("not an `if` statement at the top level of the body".into());
    };
    if i.else_branch.is_some() {
        return Err("the `if` has an `else`".into());
    }
    // The condition: `C::CHECKS_RECURSION && !R.config.may_recurse()`.
    let cond = match &*i.cond {
        Expr::Binary(b) if matches!(b.op, syn::BinOp::And(_)) => Some(b),
        _ => None,
    };
    let recv = cond.and_then(|b| {
        let gate = match &*b.left {
            Expr::Path(p) => {
                let segs = &p.path.segments;
                p.qself.is_none()
                    && segs.len() == 2
                    && segs.iter().all(|s| s.arguments.is_none())
                    && segs[1].ident == "CHECKS_RECURSION"
                    // The config of this parser (review R2 B2-3): a type parameter of the fn or
                    // its impl, never a fixed type (`NoTokensParserConfig` gives `false`).
                    && generics.contains(&segs[0].ident.to_string())
            }
            _ => false,
        };
        let Expr::Unary(u) = &*b.right else {
            return None;
        };
        let Expr::MethodCall(m) = &*u.expr else {
            return None;
        };
        let Expr::Field(field) = &*m.receiver else {
            return None;
        };
        let config = matches!(&field.member, syn::Member::Named(n) if n == "config");
        (gate
            && matches!(u.op, syn::UnOp::Not(_))
            && m.method == CHECK
            && m.args.is_empty()
            && m.turbofish.is_none()
            && config)
            .then(|| bare_ident(&field.base))
            .flatten()
    });
    let Some(recv) = recv else {
        return Err(format!(
            "the condition is not `C::CHECKS_RECURSION && !R.config.{CHECK}()` (C a type \
             parameter of the fn or its impl)"
        ));
    };
    // The body: one `return R.recursion_stopped()` or `return Err(R.recursion_stopped_error())`.
    let stop = match i.then_branch.stmts.as_slice() {
        [Stmt::Expr(Expr::Return(r), _)] => r.expr.as_deref(),
        _ => None,
    };
    let ok = stop.is_some_and(|e| {
        is_call_on(e, &recv, "recursion_stopped")
            || matches!(e, Expr::Call(c)
                if bare_ident(&c.func).as_deref() == Some("Err")
                    && c.args.len() == 1
                    && is_call_on(&c.args[0], &recv, "recursion_stopped_error"))
    });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "the body is not one `return {recv}.recursion_stopped()` or \
             `return Err({recv}.recursion_stopped_error())`"
        ))
    }
}

/// Collects the uses of a body; nested fn and impl items go to `nested`.
struct Uses<'n> {
    self_ty: Option<&'n str>,
    params: &'n BTreeMap<String, String>,
    /// The type parameters in scope: a receiver of such a type is not known.
    generics: &'n BTreeSet<String>,
    types: &'n Types,
    defs: &'n Macros,
    uses: &'n mut BTreeSet<Use>,
    nested: &'n mut Vec<syn::Item>,
    /// The macro expansions around the current expression.
    depth: usize,
    /// In a pattern: a path there matches a value and makes none (no [`Use::Type`]).
    in_pat: bool,
}

impl Uses<'_> {
    /// The type of a receiver, if the code shows it.
    fn recv_type(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Path(p) if p.qself.is_none() && p.path.segments.len() == 1 => {
                let id = p.path.segments[0].ident.to_string();
                if id == "self" {
                    self.self_ty.map(str::to_string)
                } else {
                    self.params.get(&id).cloned()
                }
            }
            // An associated const: `Self::VARIANTS`.
            Expr::Path(p) if p.qself.is_none() && p.path.segments.len() == 2 => {
                let q = p.path.segments[0].ident.to_string();
                let q = if q == "Self" {
                    self.self_ty?.to_string()
                } else {
                    q
                };
                let n = &p.path.segments[1].ident;
                self.types.rets.get(&format!("{q}::{n}")).cloned()?
            }
            Expr::Field(f) => {
                let base = self.recv_type(&f.base)?;
                let syn::Member::Named(m) = &f.member else {
                    return None;
                };
                self.types.fields.get(&base)?.get(&m.to_string()).cloned()?
            }
            Expr::Call(c) => {
                let Expr::Path(p) = &*c.func else {
                    return None;
                };
                let segs = &p.path.segments;
                if segs.len() < 2 {
                    return None;
                }
                let q = segs[segs.len() - 2].ident.to_string();
                let q = if q == "Self" {
                    self.self_ty?.to_string()
                } else {
                    q
                };
                let n = &segs[segs.len() - 1].ident;
                self.types.rets.get(&format!("{q}::{n}")).cloned()?
            }
            Expr::MethodCall(m) => {
                let t = self.recv_type(&m.receiver)?;
                self.types
                    .rets
                    .get(&format!("{t}::{}", m.method))
                    .cloned()?
            }
            Expr::Paren(p) => self.recv_type(&p.expr),
            Expr::Group(g) => self.recv_type(&g.expr),
            Expr::Reference(r) => self.recv_type(&r.expr),
            Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => self.recv_type(&u.expr),
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Uses<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.nested.push(syn::Item::Fn(i.clone()));
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        self.nested.push(syn::Item::Impl(i.clone()));
    }

    fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
        let t = self.recv_type(&m.receiver);
        self.uses.insert(Use::Method(t, m.method.to_string()));
        visit::visit_expr_method_call(self, m);
    }

    fn visit_pat(&mut self, p: &'ast Pat) {
        let saved = std::mem::replace(&mut self.in_pat, true);
        visit::visit_pat(self, p);
        self.in_pat = saved;
    }

    /// `let g: Guard = ..` and `|g: Guard| ..`: the type is not a pattern.
    fn visit_pat_type(&mut self, p: &'ast syn::PatType) {
        self.visit_pat(&p.pat);
        let saved = std::mem::replace(&mut self.in_pat, false);
        self.visit_type(&p.ty);
        self.in_pat = saved;
    }

    /// Each type that a path names (`Guard { .. }`, `Guard::new`, `Vec<Guard>`, `-> Guard`):
    /// an upper-case segment, `Self` as the impl type.
    fn visit_path(&mut self, p: &'ast syn::Path) {
        if !self.in_pat {
            for s in &p.segments {
                let id = s.ident.to_string();
                if id == "Self" {
                    if let Some(t) = self.self_ty {
                        self.uses.insert(Use::Type(t.to_string()));
                    }
                } else if id.starts_with(|c: char| c.is_ascii_uppercase()) {
                    self.uses.insert(Use::Type(id));
                }
            }
        }
        visit::visit_path(self, p);
    }

    fn visit_expr_path(&mut self, p: &'ast syn::ExprPath) {
        let segs = &p.path.segments;
        if let Some(last) = segs.last() {
            let name = last.ident.to_string();
            let qual = if let Some(q) = &p.qself {
                type_name(&q.ty)
            } else if segs.len() >= 2 {
                Some(segs[segs.len() - 2].ident.to_string())
            } else {
                None
            };
            match qual {
                Some(q) if q == "Self" => match self.self_ty {
                    Some(t) => self.uses.insert(Use::Path(t.to_string(), name)),
                    None => self.uses.insert(Use::Any(name)),
                },
                Some(q) if self.generics.contains(&q) => self.uses.insert(Use::Method(None, name)),
                Some(q) => self.uses.insert(Use::Path(q, name)),
                None => self.uses.insert(Use::Bare(name)),
            };
        }
        visit::visit_expr_path(self, p);
    }

    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        if m.path.is_ident("macro_rules") {
            return;
        }
        if let Some(rules) = self.defs.rules(m) {
            // A macro of the sources: the uses of its expansion.
            if self.depth < MAX_EXPANSION
                && let Ok(Some(ts)) = self.defs.expand(m)
                && let Ok(b) =
                    syn::parse2::<Block>(TokenTree::Group(Group::new(Delimiter::Brace, ts)).into())
            {
                self.depth += 1;
                self.visit_block(&b);
                self.depth -= 1;
                return;
            }
            // It does not expand here: every name of its rules is an edge.
            for (_, t) in rules {
                idents(t, self.uses);
            }
        }
        if let Ok(exprs) = m.parse_body_with(Punctuated::<Expr, syn::Token![,]>::parse_terminated) {
            for e in &exprs {
                self.visit_expr(e);
            }
        } else {
            idents(&m.tokens, self.uses);
        }
    }
}

fn idents(ts: &TokenStream, out: &mut BTreeSet<Use>) {
    for t in ts.clone() {
        match t {
            TokenTree::Ident(i) => {
                out.insert(Use::Any(i.to_string()));
            }
            TokenTree::Group(g) => idents(&g.stream(), out),
            _ => {}
        }
    }
}

// ---- The `macro_rules!` of the sources: a small expander ----
//
// It reads the rules of each macro (matcher => transcriber), matches a call with the first rule
// that takes all of its tokens (literal tokens, groups, `$name:kind` fragments by the syn
// parser of that kind, `$( .. ) sep op` repetitions, greedy, no backtracking) and writes the
// transcriber with the bound tokens. That is enough for the 8 macros of oxc_parser 0.158.0;
// a call that no rule takes is an error for an item macro (exit 2) and an over-count for a
// macro in a body (every identifier of its rules is an edge).

/// A part of a matcher.
#[derive(Debug, Clone)]
enum Mt {
    /// A token that must be equal (an ident, a punct or a literal, by its text).
    Tok(String),
    /// A delimited group.
    Group(Delimiter, Vec<Mt>),
    /// `$name:kind`.
    Frag(String, String),
    /// `$( .. ) sep op`: the parts, the separator, the op (`*`, `+` or `?`).
    Rep(Vec<Mt>, Option<String>, char),
}

/// The tokens that a fragment took, or one per round of a repetition.
#[derive(Debug, Clone)]
enum Bind {
    One(TokenStream),
    Many(Vec<Bind>),
}

type Binds = BTreeMap<String, Bind>;

/// The macros of the sources by name: the rules of each, in order.
#[derive(Default)]
struct Macros(BTreeMap<String, Vec<(Vec<Mt>, TokenStream)>>);

fn macro_name(mac: &syn::Macro) -> String {
    mac.path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

impl Macros {
    /// The rules of the macro of this call, if the sources define it.
    fn rules(&self, mac: &syn::Macro) -> Option<&Vec<(Vec<Mt>, TokenStream)>> {
        self.0.get(&macro_name(mac))
    }

    /// The expansion of a call: `None` for a macro of another crate.
    fn expand(&self, mac: &syn::Macro) -> Result<Option<TokenStream>, String> {
        let Some(rules) = self.rules(mac) else {
            return Ok(None);
        };
        for (m, t) in rules {
            let parse = |input: ParseStream| -> syn::Result<Binds> {
                let mut b = Binds::new();
                match_seq(m, input, &mut b)?;
                Ok(b)
            };
            if let Ok(b) = parse.parse2(mac.tokens.clone()) {
                let mut out = TokenStream::new();
                transcribe(t, &b, &mut out)?;
                return Ok(Some(out));
            }
        }
        Err(format!(
            "{}!: no rule of the macro takes the call",
            macro_name(mac)
        ))
    }
}

/// Pass 0: the `macro_rules!` definitions, anywhere in a file.
struct Defs<'m>(&'m mut BTreeMap<String, Vec<(Vec<Mt>, TokenStream)>>);

impl<'ast> Visit<'ast> for Defs<'_> {
    fn visit_item_macro(&mut self, m: &'ast syn::ItemMacro) {
        if m.mac.path.is_ident("macro_rules")
            && let Some(id) = &m.ident
        {
            self.0
                .entry(id.to_string())
                .or_default()
                .extend(rules(&m.mac.tokens));
        }
    }
}

/// The rules of a `macro_rules!` body: `(matcher) => {transcriber};` ...
fn rules(ts: &TokenStream) -> Vec<(Vec<Mt>, TokenStream)> {
    let toks: Vec<TokenTree> = ts.clone().into_iter().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while let (
        Some(TokenTree::Group(m)),
        Some(TokenTree::Punct(a)),
        Some(TokenTree::Punct(b)),
        Some(TokenTree::Group(t)),
    ) = (
        toks.get(i),
        toks.get(i + 1),
        toks.get(i + 2),
        toks.get(i + 3),
    ) && a.as_char() == '='
        && b.as_char() == '>'
    {
        out.push((matcher(&m.stream()), t.stream()));
        i += 4;
        if matches!(toks.get(i), Some(TokenTree::Punct(p)) if p.as_char() == ';') {
            i += 1;
        }
    }
    out
}

fn matcher(ts: &TokenStream) -> Vec<Mt> {
    let toks: Vec<TokenTree> = ts.clone().into_iter().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let TokenTree::Punct(p) = &toks[i]
            && p.as_char() == '$'
        {
            match (toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
                (
                    Some(TokenTree::Ident(name)),
                    Some(TokenTree::Punct(c)),
                    Some(TokenTree::Ident(kind)),
                ) if c.as_char() == ':' => {
                    out.push(Mt::Frag(name.to_string(), kind.to_string()));
                    i += 4;
                    continue;
                }
                (Some(TokenTree::Ident(name)), _, _) => {
                    // `$crate`
                    out.push(Mt::Tok(name.to_string()));
                    i += 2;
                    continue;
                }
                (Some(TokenTree::Group(g)), _, _) if g.delimiter() == Delimiter::Parenthesis => {
                    let (sep, op, used) = rep_tail(&toks, i + 2);
                    out.push(Mt::Rep(matcher(&g.stream()), sep, op));
                    i += 2 + used;
                    continue;
                }
                _ => {}
            }
        }
        match &toks[i] {
            TokenTree::Group(g) => out.push(Mt::Group(g.delimiter(), matcher(&g.stream()))),
            t => out.push(Mt::Tok(t.to_string())),
        }
        i += 1;
    }
    out
}

/// After `$( .. )`: an optional separator and the op: (separator, op, tokens used).
fn rep_tail(toks: &[TokenTree], i: usize) -> (Option<String>, char, usize) {
    let op = |t: Option<&TokenTree>| match t {
        Some(TokenTree::Punct(p)) if matches!(p.as_char(), '*' | '+' | '?') => Some(p.as_char()),
        _ => None,
    };
    if let Some(c) = op(toks.get(i)) {
        return (None, c, 1);
    }
    if let (Some(sep), Some(c)) = (toks.get(i), op(toks.get(i + 1))) {
        return (Some(sep.to_string()), c, 2);
    }
    (None, '*', 0)
}

/// One token that is not a group, with this text.
fn tok(input: ParseStream, s: &str) -> syn::Result<()> {
    input.step(|c| match c.token_tree() {
        Some((t, rest)) if !matches!(t, TokenTree::Group(_)) && t.to_string() == s => {
            Ok(((), rest))
        }
        _ => Err(c.error(format!("expected `{s}`"))),
    })
}

fn match_seq(m: &[Mt], input: ParseStream, b: &mut Binds) -> syn::Result<()> {
    for mt in m {
        match mt {
            Mt::Tok(s) => tok(input, s)?,
            Mt::Group(d, inner) => {
                let content;
                match d {
                    Delimiter::Parenthesis => {
                        syn::parenthesized!(content in input);
                    }
                    Delimiter::Brace => {
                        syn::braced!(content in input);
                    }
                    Delimiter::Bracket => {
                        syn::bracketed!(content in input);
                    }
                    Delimiter::None => return Err(input.error("a group with no delimiter")),
                }
                match_seq(inner, &content, b)?;
                if !content.is_empty() {
                    return Err(content.error("a token that the matcher does not take"));
                }
            }
            Mt::Frag(name, kind) => {
                let ts = frag(kind, input)?;
                b.insert(name.clone(), Bind::One(ts));
            }
            Mt::Rep(inner, sep, op) => {
                let mut rounds: Vec<Binds> = Vec::new();
                loop {
                    if *op == '?' && !rounds.is_empty() {
                        break;
                    }
                    let fork = input.fork();
                    if !rounds.is_empty()
                        && let Some(s) = sep
                        && tok(&fork, s).is_err()
                    {
                        break;
                    }
                    let mut rb = Binds::new();
                    if match_seq(inner, &fork, &mut rb).is_err() {
                        break;
                    }
                    let moved = fork.cursor() != input.cursor();
                    input.advance_to(&fork);
                    rounds.push(rb);
                    if !moved {
                        break;
                    }
                }
                if *op == '+' && rounds.is_empty() {
                    return Err(input.error("a repetition with no round"));
                }
                for name in frag_names(inner) {
                    let all = rounds
                        .iter()
                        .map(|r| r.get(&name).cloned().unwrap_or(Bind::Many(Vec::new())))
                        .collect();
                    b.insert(name, Bind::Many(all));
                }
            }
        }
    }
    Ok(())
}

/// The tokens of one fragment of this kind.
fn frag(kind: &str, input: ParseStream) -> syn::Result<TokenStream> {
    Ok(match kind {
        "ident" => syn::Ident::parse_any(input)?.into_token_stream(),
        "tt" => input.parse::<TokenTree>()?.into(),
        "lifetime" => input.parse::<syn::Lifetime>()?.into_token_stream(),
        "literal" => input.parse::<syn::Lit>()?.into_token_stream(),
        "expr" | "expr_2021" => input.parse::<Expr>()?.into_token_stream(),
        "ty" => input.parse::<Type>()?.into_token_stream(),
        "path" => input.parse::<syn::Path>()?.into_token_stream(),
        "pat" => Pat::parse_multi_with_leading_vert(input)?.into_token_stream(),
        "pat_param" => Pat::parse_single(input)?.into_token_stream(),
        "block" => input.parse::<Block>()?.into_token_stream(),
        "stmt" => input.parse::<Stmt>()?.into_token_stream(),
        "item" => input.parse::<syn::Item>()?.into_token_stream(),
        "vis" => input.parse::<syn::Visibility>()?.into_token_stream(),
        "meta" => input.parse::<syn::Meta>()?.into_token_stream(),
        _ => return Err(input.error(format!("the fragment kind `{kind}`"))),
    })
}

fn frag_names(m: &[Mt]) -> Vec<String> {
    let mut out = Vec::new();
    for mt in m {
        match mt {
            Mt::Frag(n, _) => out.push(n.clone()),
            Mt::Group(_, inner) | Mt::Rep(inner, _, _) => out.extend(frag_names(inner)),
            Mt::Tok(_) => {}
        }
    }
    out
}

/// The `$name`s in a transcriber part.
fn used_names(ts: &TokenStream, out: &mut BTreeSet<String>) {
    let toks: Vec<TokenTree> = ts.clone().into_iter().collect();
    for (i, t) in toks.iter().enumerate() {
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => {
                if let Some(TokenTree::Ident(id)) = toks.get(i + 1) {
                    out.insert(id.to_string());
                }
            }
            TokenTree::Group(g) => used_names(&g.stream(), out),
            _ => {}
        }
    }
}

fn transcribe(t: &TokenStream, b: &Binds, out: &mut TokenStream) -> Result<(), String> {
    let toks: Vec<TokenTree> = t.clone().into_iter().collect();
    let mut i = 0;
    while i < toks.len() {
        if let TokenTree::Punct(p) = &toks[i]
            && p.as_char() == '$'
        {
            match toks.get(i + 1) {
                Some(TokenTree::Ident(id)) => {
                    match b.get(&id.to_string()) {
                        Some(Bind::One(ts)) => out.extend(ts.clone()),
                        Some(Bind::Many(_)) => {
                            return Err(format!("`${id}` is a repetition at this depth"));
                        }
                        // `$crate`
                        None => out.extend([TokenTree::Ident(id.clone())]),
                    }
                    i += 2;
                    continue;
                }
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    let (sep, _, used) = rep_tail(&toks, i + 2);
                    let mut names = BTreeSet::new();
                    used_names(&g.stream(), &mut names);
                    let reps: Vec<(&String, &Vec<Bind>)> = names
                        .iter()
                        .filter_map(|n| match b.get(n) {
                            Some(Bind::Many(v)) => Some((n, v)),
                            _ => None,
                        })
                        .collect();
                    let rounds = reps.iter().map(|r| r.1.len()).max().unwrap_or(0);
                    for k in 0..rounds {
                        if k > 0
                            && let Some(s) = &sep
                        {
                            out.extend(s.parse::<TokenStream>().map_err(|e| e.to_string())?);
                        }
                        let mut bk = b.clone();
                        for (name, v) in &reps {
                            if let Some(x) = v.get(k) {
                                bk.insert((*name).clone(), x.clone());
                            }
                        }
                        transcribe(&g.stream(), &bk, out)?;
                    }
                    i += 2 + used;
                    continue;
                }
                _ => {}
            }
        }
        match &toks[i] {
            TokenTree::Group(g) => {
                let mut inner = TokenStream::new();
                transcribe(&g.stream(), b, &mut inner)?;
                let mut ng = Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.extend([TokenTree::Group(ng)]);
            }
            t => out.extend([t.clone()]),
        }
        i += 1;
    }
    Ok(())
}

/// Does a token stream hold the keyword `fn`?
fn has_fn(ts: &TokenStream) -> bool {
    ts.clone().into_iter().any(|t| match t {
        TokenTree::Ident(i) => i == "fn",
        TokenTree::Group(g) => has_fn(&g.stream()),
        _ => false,
    })
}

/// Pass 0: each item macro of the sources (in a file, a mod or an impl) is replaced by the items
/// of its expansion. A call of an item macro of another crate whose tokens hold `fn` fails.
fn expand_items(
    items: Vec<syn::Item>,
    defs: &Macros,
    rel: &str,
    depth: usize,
) -> Result<Vec<syn::Item>, String> {
    let at = |mac: &syn::Macro| format!("{rel}:{}", mac.path.span().start().line);
    let too_deep = |mac: &syn::Macro| {
        format!(
            "{}: {}!: more than {MAX_EXPANSION} nested expansions",
            at(mac),
            macro_name(mac)
        )
    };
    let foreign = |mac: &syn::Macro| {
        format!(
            "{}: {}! is a macro of another crate and its tokens hold a fn: the graph cannot \
             see that body",
            at(mac),
            macro_name(mac)
        )
    };
    let mut out = Vec::new();
    for item in items {
        match item {
            syn::Item::Macro(m) if m.ident.is_none() && !is_test(&m.attrs) => {
                match defs
                    .expand(&m.mac)
                    .map_err(|e| format!("{}: {e}", at(&m.mac)))?
                {
                    Some(_) if depth >= MAX_EXPANSION => return Err(too_deep(&m.mac)),
                    Some(ts) => {
                        let f: syn::File = syn::parse2(ts).map_err(|e| {
                            format!(
                                "{}: the expansion of {}! is not items: {e}",
                                at(&m.mac),
                                macro_name(&m.mac)
                            )
                        })?;
                        out.extend(expand_items(f.items, defs, rel, depth + 1)?);
                    }
                    None if has_fn(&m.mac.tokens) => return Err(foreign(&m.mac)),
                    None => out.push(syn::Item::Macro(m)),
                }
            }
            syn::Item::Mod(mut m) => {
                if let Some((brace, inner)) = m.content.take() {
                    m.content = Some((brace, expand_items(inner, defs, rel, depth)?));
                }
                out.push(syn::Item::Mod(m));
            }
            syn::Item::Impl(mut i) => {
                let mut todo = std::mem::take(&mut i.items);
                let mut rounds = 0;
                while todo
                    .iter()
                    .any(|it| matches!(it, syn::ImplItem::Macro(m) if !is_test(&m.attrs)))
                {
                    rounds += 1;
                    let mut next = Vec::new();
                    for it in todo {
                        match it {
                            syn::ImplItem::Macro(m) if !is_test(&m.attrs) => {
                                match defs
                                    .expand(&m.mac)
                                    .map_err(|e| format!("{}: {e}", at(&m.mac)))?
                                {
                                    Some(_) if depth + rounds > MAX_EXPANSION => {
                                        return Err(too_deep(&m.mac));
                                    }
                                    Some(ts) => {
                                        let parse = |input: ParseStream| {
                                            let mut v = Vec::new();
                                            while !input.is_empty() {
                                                v.push(input.parse::<syn::ImplItem>()?);
                                            }
                                            Ok(v)
                                        };
                                        next.extend(parse.parse2(ts).map_err(|e| {
                                            format!(
                                                "{}: the expansion of {}! is not impl items: {e}",
                                                at(&m.mac),
                                                macro_name(&m.mac)
                                            )
                                        })?);
                                    }
                                    None if has_fn(&m.mac.tokens) => return Err(foreign(&m.mac)),
                                    // Kept, and not expanded again.
                                    None => {
                                        next.push(syn::ImplItem::Verbatim(m.into_token_stream()))
                                    }
                                }
                            }
                            other => next.push(other),
                        }
                    }
                    todo = next;
                }
                i.items = todo;
                out.push(syn::Item::Impl(i));
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

fn rel_path(root: &Path, f: &Path) -> String {
    let rel = f.strip_prefix(root).unwrap_or(f);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(files: &[(&str, &str)]) -> Report {
        let s: Vec<(String, String)> = files
            .iter()
            .map(|(p, t)| (p.to_string(), t.to_string()))
            .collect();
        run(&s, &BTreeSet::new(), &[]).expect("fixture parses")
    }

    fn keys(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Fixture 1: a mutual recursion by method calls; a site at the start of one fn cuts it.
    /// A same-name wrapper on another type (`checkpoint` -> `self.lexer.checkpoint()`) is no
    /// self call, and the 2 loops are found.
    #[test]
    fn a_site_cuts_a_method_cycle() {
        let src = r#"
            struct ParserImpl<C> { lexer: Lexer, config: C }
            struct Lexer { pos: u32 }
            impl Lexer { fn checkpoint(&self) -> u32 { self.pos } }
            impl<C> ParserImpl<C> {
                fn parse_a(&mut self) -> u32 {
                    if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }
                    self.parse_b()
                }
                fn parse_b(&mut self) -> u32 { if self.more() { self.parse_a() } else { 0 } }
                fn more(&mut self) -> bool { self.checkpoint() > 0 }
                fn checkpoint(&mut self) -> u32 { self.lexer.checkpoint() }
                fn recursion_stopped(&mut self) -> u32 { 0 }
                fn set_pure_on_call_or_new_expr() {}
                fn jsx_member_expression_eq() {}
            }"#;
        let r = report(&[("a.rs", src)]);
        assert!(r.cycles.is_empty(), "{:?}", r.cycles);
        assert_eq!(r.sites.len(), 1);
        assert_eq!(r.sites[0].0, "parse_a");
        assert!(r.ok());
        // Without the site the same code is a cycle.
        let bare = src.replace("may_recurse", "other_check");
        let r = report(&[("a.rs", &bare)]);
        assert_eq!(
            r.cycles,
            vec![keys(&["ParserImpl::parse_a", "ParserImpl::parse_b"])]
        );
        assert!(!r.ok());
    }

    /// Fixture 2: a cycle through type path calls (`Target::cover(..)`, `Self::cover`) and a
    /// function value (`self.list(Self::parse_item)`), the edge forms that a method-name graph
    /// misses. A loop that is missing is a fail too.
    #[test]
    fn path_calls_and_function_values_are_edges() {
        let parser = r#"
            impl<C> ParserImpl<C> {
                fn parse_item(&mut self) -> u32 { Target::cover(0, self) }
                fn parse_items(&mut self) -> u32 { self.list(Self::parse_item) }
                fn list(&mut self, f: fn(&mut Self) -> u32) -> u32 { f(self) }
            }"#;
        let grammar = r#"
            impl Target {
                fn cover<C>(n: u32, p: &mut ParserImpl<C>) -> u32 { Inner::cover(n, p) }
            }
            impl Inner {
                fn cover<C>(n: u32, p: &mut ParserImpl<C>) -> u32 {
                    if n > 0 { Self::cover(n - 1, p) } else { p.parse_items() }
                }
            }"#;
        let r = report(&[("p.rs", parser), ("g.rs", grammar)]);
        assert_eq!(
            r.cycles,
            vec![keys(&[
                "Inner::cover",
                "ParserImpl::parse_item",
                "ParserImpl::parse_items",
                "Target::cover",
            ])]
        );
        assert_eq!(r.places["Target::cover"].0, "g.rs");
        assert!(r.loops.iter().all(|l| !l.2));
    }

    /// Fixture 3: the statements before the check stay edges (a check after a recursive call
    /// cuts nothing), a self call is a cycle, and a name in a macro body is an edge.
    #[test]
    fn a_late_check_and_a_self_call_are_cycles() {
        let src = r#"
            impl<C> ParserImpl<C> {
                fn parse_x(&mut self) -> u32 {
                    let a = self.parse_y();
                    if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }
                    a
                }
                fn parse_y(&mut self) -> u32 { debug_assert!(self.ok()); self.parse_x() }
                fn ok(&self) -> bool { matches!(self.walk(), 0) }
                fn walk(&self) -> u32 { self.walk() }
            }"#;
        let r = report(&[("x.rs", src)]);
        assert_eq!(
            r.cycles,
            vec![
                keys(&["ParserImpl::parse_x", "ParserImpl::parse_y"]),
                keys(&["ParserImpl::walk"]),
            ]
        );
        assert_eq!(r.sites.len(), 1, "the check still counts as a site");
    }

    /// Is `to` reachable from `from` in the graph after the cut?
    fn reaches(r: &Report, from: &str, to: &str) -> bool {
        path_to(&r.edges, from, &BTreeSet::from([to])).is_some()
    }

    /// The lexer of oxc_parser 0.158.0 in small (lexer/byte_handlers.rs, config.rs).
    const LEXER: &str = r#"
        macro_rules! byte_handlers {
            () => { [ERR, SPS, SLH, UNI] };
        }
        macro_rules! ascii_byte_handler {
            ($id:ident($lex:ident) $body:expr) => {
                #[expect(non_snake_case)]
                fn $id<C: Config>($lex: &mut Lexer<'_, C>) -> Kind {
                    unsafe { assert_unchecked!(!$lex.source.is_eof()); }
                    $body
                }
            };
        }
        pub mod byte_handler_tables {
            use super::*;
            pub static NO_TOKENS: ByteHandlers<NoTokensLexerConfig> = byte_handlers!();
        }
        pub trait LexerConfig: Default {
            fn byte_handlers(&self) -> &Self::ByteHandlers;
        }
        impl LexerConfig for NoTokensLexerConfig {
            fn byte_handlers(&self) -> &Self::ByteHandlers { &byte_handler_tables::NO_TOKENS }
        }
        pub struct Lexer<'a, C: Config> { config: C, source: Source<'a> }
        impl<C: Config> Lexer<'_, C> {
            pub fn next_token(&mut self) -> Token { let k = self.read_next_token(); self.finish(k) }
            fn read_next_token(&mut self) -> Kind { let b = self.source.peek(); unsafe { self.handle_byte(b) } }
            unsafe fn handle_byte(&mut self, byte: u8) -> Kind {
                let byte_handlers = self.config.byte_handlers();
                unsafe { byte_handlers[byte as usize](self) }
            }
            fn finish(&mut self, k: Kind) -> Token { Token(k) }
            fn skip_comment(&mut self) -> Kind { Kind::Skip }
        }
        ascii_byte_handler!(ERR(lexer) {
            lexer.skip_comment()
        });
        ascii_byte_handler!(SPS(lexer) {
            #[expect(clippy::needless_return)]
            return lexer.skip_comment();
        });
        ascii_byte_handler!(SLH(lexer) {
            lexer.skip_comment()
        });
        #[expect(non_snake_case)]
        fn UNI<C: Config>(lexer: &mut Lexer<'_, C>) -> Kind { lexer.skip_comment() }
        impl<C> ParserImpl<C> {
            fn set_pure_on_call_or_new_expr() {}
            fn jsx_member_expression_eq() {}
        }"#;

    /// Fixture 4 (role check 1 F1): the byte handlers are fns that an item macro writes and that
    /// `Lexer::handle_byte` calls through a static table. Each is a node, `handle_byte` reaches
    /// each one, and a handler that lexes again (calls `next_token`) is a cycle.
    #[test]
    fn a_byte_handler_that_lexes_again_is_a_cycle() {
        let r = report(&[("lexer.rs", LEXER)]);
        assert!(r.ok(), "{:?}", r.cycles);
        for h in ["ERR", "SPS", "SLH", "UNI"] {
            assert!(r.places.contains_key(h), "{h} is a node");
            assert!(
                reaches(&r, "Lexer::handle_byte", h),
                "handle_byte reaches {h}"
            );
        }
        // The node of a macro-written fn is at the line of the macro call.
        let line = LEXER
            .lines()
            .position(|l| l.contains("(SLH(lexer)"))
            .unwrap()
            + 1;
        assert_eq!(r.places["SLH"], ("lexer.rs".to_string(), line));
        // An attribute in a macro body is not a use (`#[expect(..)]` is no edge to `expect`).
        assert!(
            r.edges["SPS"]
                .keys()
                .all(|k| k == "Lexer::skip_comment" || k.starts_with("Source::"))
        );
        let again = LEXER.replace(
            "ascii_byte_handler!(SLH(lexer) {\n            lexer.skip_comment()",
            "ascii_byte_handler!(SLH(lexer) {\n            lexer.next_token(); lexer.skip_comment()",
        );
        assert_ne!(again, LEXER);
        let r = report(&[("lexer.rs", &again)]);
        assert!(!r.ok());
        assert_eq!(
            r.cycles,
            vec![keys(&[
                "Lexer::handle_byte",
                "Lexer::next_token",
                "Lexer::read_next_token",
                "NO_TOKENS",
                "NoTokensLexerConfig::byte_handlers",
                "SLH",
            ])]
        );
    }

    /// Review R2 B2-2 (a) in the lexer form: `handle_byte` calls the table by the path
    /// `Config::byte_handlers(&self.config)`, `Config` the trait `LexerConfig` by `use .. as`
    /// (13 files of 0.158.0 have that rename). A handler that lexes again is still a cycle.
    #[test]
    fn a_path_call_by_a_renamed_trait_keeps_the_byte_table() {
        let again = LEXER
            .replace(
                "let byte_handlers = self.config.byte_handlers();",
                "let byte_handlers = Config::byte_handlers(&self.config);",
            )
            .replace(
                "ascii_byte_handler!(SLH(lexer) {\n            lexer.skip_comment()",
                "ascii_byte_handler!(SLH(lexer) {\n            lexer.next_token(); lexer.skip_comment()",
            );
        let again = format!("use crate::lexer::LexerConfig as Config;\n{again}");
        let r = report(&[("lexer.rs", &again)]);
        assert_eq!(
            r.cycles,
            vec![keys(&[
                "Lexer::handle_byte",
                "Lexer::next_token",
                "Lexer::read_next_token",
                "NO_TOKENS",
                "NoTokensLexerConfig::byte_handlers",
                "SLH",
            ])]
        );
        // With no rename `Config` is not known: no edge, no cycle (the form before B2-2).
        let bare = again.replace("use crate::lexer::LexerConfig as Config;\n", "");
        assert!(report(&[("lexer.rs", &bare)]).cycles.is_empty());
    }

    /// Review R2 B2-2 (c): a type parameter with an `Fn` bound in the `where` clause, also inside
    /// an `Option`, is a fn value field; a parameter with another bound is not.
    #[test]
    fn a_field_of_an_fn_bound_parameter_fails() {
        let src = "struct S<F, G: Clone> where F: FnMut() -> u32 { f: Option<F>, g: G }";
        let e = run(&[("w.rs".into(), src.into())], &BTreeSet::new(), &[]).unwrap_err();
        assert!(
            e.starts_with("w.rs:1: S.f: the field holds a fn value"),
            "{e}"
        );
        let ok = "struct S<G: Clone> { g: G }";
        assert!(run(&[("w.rs".into(), ok.into())], &BTreeSet::new(), &[]).is_ok());
        // The `Fn` bound on an impl of the type (the usual form), not on the struct: the same
        // fn value field. Lifetimes do not count in the place of a parameter, const ones do.
        let on_impl = "struct H<'a, const N: usize, F> { r: &'a u32, f: F }
                       impl<'a, const N: usize, F> H<'a, N, F> where F: Fn(u32) -> u32 {
                           fn call(&self) -> u32 { (self.f)(*self.r) } }";
        let e = run(&[("h.rs".into(), on_impl.into())], &BTreeSet::new(), &[]).unwrap_err();
        assert!(
            e.starts_with("h.rs:1: H.f: the field holds a fn value"),
            "{e}"
        );
        let on_trait_impl =
            "enum E<F> { A(F) } impl<F: FnOnce()> Drop for E<F> { fn drop(&mut self) {} }";
        let e = run(
            &[("e.rs".into(), on_trait_impl.into())],
            &BTreeSet::new(),
            &[],
        )
        .unwrap_err();
        assert!(
            e.starts_with("e.rs:1: E::A.0: the field holds a fn value"),
            "{e}"
        );
        let ok = "struct H<F, G> { f: F, g: G } impl<F: Clone, G: Fn()> H<F, u32> {}";
        assert!(run(&[("h.rs".into(), ok.into())], &BTreeSet::new(), &[]).is_ok());
    }

    /// Review R2 B2-4: an ALLOW entry leaves out only the edge of its own use; an entry that
    /// leaves out no edge is a finding.
    #[test]
    fn an_allow_entry_holds_only_its_own_use() {
        const ENTRY: Allow = (
            "JSXClosing::dummy",
            "JSXClosing::dummy",
            r#"Path("Dummy", "dummy")"#,
            "the oxc_ast impl of the same name",
        );
        let src = "struct JSXClosing; impl Dummy for JSXClosing { fn dummy(a: u32) -> Self { CALL } }
                   impl<C> ParserImpl<C> { fn set_pure_on_call_or_new_expr() {} fn jsx_member_expression_eq() {} }";
        let run_with = |call: &str| {
            let s = vec![("d.rs".to_string(), src.replace("CALL", call))];
            run(&s, &BTreeSet::new(), &[ENTRY]).expect("fixture parses")
        };
        // The use of the entry: the edge is left out, and the entry is used.
        let r = run_with("Dummy::dummy(a)");
        assert!(r.ok(), "{:?} {:?}", r.cycles, r.unused_allow);
        // Another use of the same edge: a cycle, and the entry is unused.
        let r = run_with("Self::dummy(a)");
        assert_eq!(r.cycles, vec![keys(&["JSXClosing::dummy"])]);
        assert_eq!(
            r.unused_allow,
            vec![(
                ENTRY.0.to_string(),
                ENTRY.1.to_string(),
                ENTRY.2.to_string()
            )]
        );
        // Both uses: the edge of `Self::dummy` stays.
        let r = run_with("{ Dummy::dummy(a); Self::dummy(a) }");
        assert_eq!(r.cycles, vec![keys(&["JSXClosing::dummy"])]);
        assert!(r.unused_allow.is_empty());
    }

    /// Review R2 B2-5: each rule that narrows an edge to fewer fns has a cycle that runs only
    /// through the narrowed edge. A rule that gives a wrong type loses the edge, and its case
    /// fails here (for example `DEREF = &[]` loses the `Box<Lexer>` case). Each case also has a
    /// same-name fn of another type, so an edge to every fn of the name is not the only edge.
    #[test]
    fn each_narrowing_rule_keeps_its_cycle() {
        const DECOY: &str = "struct Other; impl Other { fn f(&mut self) -> u32 { 0 } \
                             fn a(&mut self) -> u32 { 0 } fn c(&mut self) -> u32 { 0 } \
                             fn go(&mut self) -> u32 { 0 } fn step(&mut self) -> u32 { 0 } }";
        let cases: &[(&str, &str, &[&str])] = &[
            (
                "a field type",
                "struct P { lexer: Lexer } struct Lexer;
                 impl P { fn a(&mut self) -> u32 { self.lexer.f(self) } }
                 impl Lexer { fn f(&mut self, p: &mut P) -> u32 { p.a() } }",
                &["Lexer::f", "P::a"],
            ),
            (
                "a DEREF wrapper",
                "struct P { lexer: Box<Lexer> } struct Lexer;
                 impl P { fn a(&mut self) -> u32 { self.lexer.f(self) } }
                 impl Lexer { fn f(&mut self, p: &mut P) -> u32 { p.a() } }",
                &["Lexer::f", "P::a"],
            ),
            (
                "a return type chain",
                "struct P; struct Q;
                 impl P { fn b(&self) -> Q { Q } fn a(&mut self) -> u32 { self.b().c(self) } }
                 impl P { fn c(&self, p: &mut P) -> u32 { 0 } }
                 impl Q { fn c(&self, p: &mut P) -> u32 { p.a() } }",
                &["P::a", "Q::c"],
            ),
            (
                "an associated const receiver",
                "struct K; struct W;
                 impl K { const ALL: W = W; fn a(&mut self) -> u32 { Self::ALL.go(self) } }
                 impl W { fn go(&self, k: &mut K) -> u32 { k.a() } }",
                &["K::a", "W::go"],
            ),
            (
                "a qualified path",
                "trait Walk { fn step(&mut self) -> u32; } struct A;
                 impl Walk for A { fn step(&mut self) -> u32 { run(self) } }
                 fn run(a: &mut A) -> u32 { <A as Walk>::step(a) }",
                &["A::step", "run"],
            ),
            (
                "a module path",
                "mod m { pub fn down(n: u32) -> u32 { super::up(n) } }
                 fn up(n: u32) -> u32 { m::down(n) }",
                &["down", "up"],
            ),
            (
                "a blanket impl",
                "trait Ext { fn ext(&self) -> u32; }
                 impl<X: AsRef<str>> Ext for X { fn ext(&self) -> u32 { look(self.as_ref()) } }
                 fn look(s: &str) -> u32 { s.ext() }",
                &["X::ext", "look"],
            ),
            (
                "a nested fn",
                "fn outer(n: u32) -> u32 { fn inner(n: u32) -> u32 { outer(n) } inner(n) }",
                &["inner", "outer"],
            ),
        ];
        for (rule, src, cycle) in cases {
            let r = report(&[("n.rs", &format!("{src}\n{DECOY}"))]);
            assert_eq!(r.cycles, vec![keys(cycle)], "{rule}");
        }
        // A LOOP fn on a cycle is not a loop.
        let src = "impl<C> ParserImpl<C> { fn set_pure_on_call_or_new_expr(&mut self) { \
                   self.set_pure_on_call_or_new_expr() } fn jsx_member_expression_eq() {} }";
        let r = report(&[("l.rs", src)]);
        assert_eq!(
            r.loops
                .iter()
                .map(|(k, _, ok)| (label(k), *ok))
                .collect::<Vec<_>>(),
            vec![
                ("set_pure_on_call_or_new_expr", false),
                ("jsx_member_expression_eq", true)
            ]
        );
    }

    /// Review R2 B2-5: two fns of one key (two types of one name) with other return types:
    /// the return type is not known, never the type of one of them.
    #[test]
    fn a_return_type_of_two_fns_of_one_key_is_not_known() {
        let a = "struct Ctx; struct Walker;
                 impl Ctx { fn inner(&self) -> Walker { Walker } fn go(&mut self) -> u32 { self.inner().step() } }
                 impl Walker { fn step(&mut self) -> u32 { again() } }
                 fn again() -> u32 { let mut c = Ctx; c.go() }";
        let b = "mod b { struct Ctx; struct Plain;
                 impl Ctx { fn inner(&self) -> Plain { Plain } }
                 impl Plain { fn step(&mut self) -> u32 { 0 } } }";
        let r = report(&[("a.rs", a), ("b.rs", b)]);
        assert_eq!(r.cycles, vec![keys(&["Ctx::go", "Walker::step", "again"])]);
    }

    /// Fixture 5: the macro expander. A macro with nested repetitions writes an enum, an impl
    /// and one fn per entry (the `parser_diagnostics!` form); a macro in a body gives the edges
    /// of its expansion, also when it calls itself with another rule.
    #[test]
    fn macros_of_the_sources_expand() {
        let src = r#"
            macro_rules! diags {
                ( $( $name:ident ( $($p:ident : $t:ty),* $(,)? ) => $body:expr );* $(;)? ) => {
                    pub enum Diag { $( $name { $($p : $t),* }, )* }
                    impl Diag {
                        pub fn build(self) -> u32 { match self { $( Self::$name { $($p),* } => $body, )* } }
                    }
                    $( pub fn $name($($p : $t),*) -> Diag { Diag::$name { $($p),* } } )*
                };
            }
            diags! {
                first(a: u32, b: u32,) => { a + b };
                second() => { helper() };
            }
            fn helper() -> u32 { second().build() }
            macro_rules! again {
                (p: $p:ident) => { again!(go: $p, 1) };
                (go: $p:ident, $n:literal) => { $p.parse_x() + $n };
            }
            impl<C> ParserImpl<C> {
                fn parse_x(&mut self) -> u32 { again!(p: self) }
            }"#;
        let r = report(&[("m.rs", src)]);
        for f in ["first", "second", "Diag::build"] {
            assert!(r.places.contains_key(f), "{f} is a node");
        }
        let line = src.lines().position(|l| l.contains("first(a:")).unwrap() + 1;
        assert_eq!(r.places["first"].1, line);
        assert_eq!(
            r.cycles,
            vec![
                keys(&["Diag::build", "helper"]),
                keys(&["ParserImpl::parse_x"]),
            ]
        );
    }

    /// Fixture 6: a receiver of a generic type and a call in a trait default fn are edges to
    /// every fn that the call can dispatch to; a static initializer is a node.
    #[test]
    fn generic_and_trait_receivers_are_edges() {
        let src = r#"
            trait Walk { fn step(&self) -> u32; fn twice(&self) -> u32 { self.step() } }
            struct A; struct B;
            impl Walk for A { fn step(&self) -> u32 { self.twice() } }
            impl Walk for B { fn step(&self) -> u32 { walk(self) + TABLE[0](0) } }
            fn walk<T: Walk>(t: &T) -> u32 { t.step() }
            fn entry(n: u32) -> u32 { walk(&B) + n }
            static TABLE: [fn(u32) -> u32; 1] = [entry];
            struct Holder<W: Walk> { w: W }
            impl<W: Walk> Holder<W> { fn go(&self) -> u32 { self.w.twice() } }"#;
        let r = report(&[("g.rs", src)]);
        assert_eq!(
            r.cycles,
            vec![keys(&[
                "A::step",
                "B::step",
                "TABLE",
                "Walk::twice",
                "entry",
                "walk"
            ]),]
        );
        assert!(reaches(&r, "Holder::go", "A::step"));
        assert_eq!(r.data, 1);
    }

    /// Fixture 7: what the graph cannot see fails. A field that holds a fn value and an item
    /// macro of another crate that writes a fn stop the run; a fn of an impl of a trait of
    /// another crate (`Drop`) that reaches a site is a finding.
    #[test]
    fn what_the_graph_cannot_see_fails() {
        let field = "type Handler = fn(u32) -> u32;\nstruct S { h: Option<Handler> }";
        let e = run(&[("f.rs".into(), field.into())], &BTreeSet::new(), &[]).unwrap_err();
        assert!(
            e.starts_with("f.rs:2: S.h: the field holds a fn value"),
            "{e}"
        );
        let foreign = "other_crate::make_fns! { fn hidden() { parse_a() } }";
        let e = run(&[("i.rs".into(), foreign.into())], &BTreeSet::new(), &[]).unwrap_err();
        assert!(e.contains("make_fns! is a macro of another crate"), "{e}");
        let drop = r#"
            struct Guard<C> { p: ParserImpl<C> }
            impl<C> Drop for Guard<C> { fn drop(&mut self) { self.p.reset() } }
            impl<C> ParserImpl<C> {
                fn reset(&mut self) { self.parse_a(); }
                fn parse_a(&mut self) -> u32 {
                    if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }
                    self.parse_a()
                }
                fn set_pure_on_call_or_new_expr() {}
                fn jsx_member_expression_eq() {}
            }"#;
        let r = report(&[("d.rs", drop)]);
        assert!(r.cycles.is_empty(), "{:?}", r.cycles);
        assert_eq!(
            r.implicit,
            vec![(
                "Guard::drop".to_string(),
                keys(&["Guard::drop", "ParserImpl::reset", "ParserImpl::parse_a"])
            )]
        );
        assert!(!r.ok());
    }

    /// Fixture 8 (review V6): only the 2 site forms of the patch are sites. Each other statement
    /// that calls `may_recurse` is a finding, and it cuts nothing: one that does not stop, one
    /// that calls more in its condition, one in a nested block, one with an `else`.
    #[test]
    fn only_the_2_site_forms_are_sites() {
        const FORM: &str = "if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }";
        let src = r#"
            impl<C> ParserImpl<C> {
                fn parse_a(&mut self) -> u32 {
                    if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }
                    self.parse_a()
                }
                fn parse_e(&self, p: &Self) -> Result<u32, Diag> {
                    if C::CHECKS_RECURSION && !p.config.may_recurse() { return Err(p.recursion_stopped_error()); }
                    self.parse_e(p)
                }
                fn f(&mut self) -> Option<u32> { None }
                fn error(&mut self) {}
                fn recursion_stopped(&mut self) -> u32 { 0 }
                fn recursion_stopped_error(&self) -> Diag { Diag }
                fn set_pure_on_call_or_new_expr() {}
                fn jsx_member_expression_eq() {}
            }"#;
        let r = report(&[("s.rs", src)]);
        assert!(r.ok(), "{:?} {:?}", r.cycles, r.bad_checks);
        assert_eq!(r.sites.len(), 2, "both forms are sites");
        for (bad, why) in [
            (
                "if C::CHECKS_RECURSION && !self.config.may_recurse() { self.error(); }",
                "the body is not one",
            ),
            (
                "if self.f().is_some() && !self.config.may_recurse() { return self.recursion_stopped(); }",
                "the condition is not",
            ),
            (
                "if true { if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); } }",
                "the condition is not",
            ),
            (
                "if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); } else { self.error(); }",
                "has an `else`",
            ),
            (
                "let _go = self.config.may_recurse();",
                "not an `if` statement",
            ),
            // Review R2 B2-3: the gate of a fixed config type is no site.
            (
                "if NoTokensParserConfig::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }",
                "the condition is not",
            ),
        ] {
            let text = src.replacen(FORM, bad, 1);
            assert_ne!(text, src);
            let r = report(&[("s.rs", &text)]);
            assert!(!r.ok(), "{bad}");
            assert_eq!(r.bad_checks.len(), 1, "{bad}");
            let (label, file, line, got) = &r.bad_checks[0];
            assert_eq!(
                (label.as_str(), file.as_str(), *line),
                ("parse_a", "s.rs", 4),
                "{bad}"
            );
            assert!(got.contains(why), "{bad}: {got}");
            // It cuts nothing: the recursion of parse_a is a cycle.
            assert_eq!(r.cycles, vec![keys(&["ParserImpl::parse_a"])], "{bad}");
        }
    }

    /// Review R2 B2-1: a fn that names a type outside a pattern has an edge to each implicit
    /// callee of that type, so a recursion that a `?` (`From`), a `format!` (`Display`) or a
    /// `for` (`Iterator`) closes is a cycle. A pattern names no value, and a fn of the same type
    /// gets no such edge.
    #[test]
    fn an_implicit_call_closes_a_cycle() {
        const LOOPS: &str = "impl<C> ParserImpl<C> { fn set_pure_on_call_or_new_expr() {} \
                             fn jsx_member_expression_eq() {} }";
        for (src, cycle) in [
            (
                "struct E; impl From<u32> for E { fn from(_: u32) -> E { parse_e().unwrap_or(E) } }
                 fn parse_e() -> Result<E, E> { let n: u32 = 1; Err(n)? }",
                &["E::from", "parse_e"][..],
            ),
            (
                "struct D(u32); impl std::fmt::Display for D { fn fmt(&self, f: &mut F) -> R { show() } }
                 fn show() -> R { format!(\"{}\", make()) } fn make() -> D { D(0) }",
                &["D::fmt", "make", "show"][..],
            ),
            (
                "struct W; impl Iterator for W { type Item = u32; fn next(&mut self) -> Option<u32> { walk() } }
                 fn walk() -> Option<u32> { let w: W = W; for x in w { return Some(x); } None }",
                &["W::next", "walk"][..],
            ),
        ] {
            let r = report(&[("a.rs", &format!("{src}\n{LOOPS}"))]);
            assert_eq!(r.cycles, vec![keys(cycle)], "{src}");
        }
        // A match on `K::A` names no value; `K::name` names `K` but is a fn of `K`.
        let pat = "enum K { A } impl std::fmt::Display for K { fn fmt(&self, f: &mut F) -> R { self.name(); see(self) } }
                   impl K { fn name(&self) -> u32 { let k = Self::A; 0 } }
                   fn see(k: &u32) -> R { match k { K::A => ok() } }";
        let r = report(&[("p.rs", &format!("{pat}\n{LOOPS}"))]);
        assert!(r.cycles.is_empty(), "{:?}", r.cycles);
    }

    /// Review V7: a call in the body of a `macro_rules!` of the sources is an edge where the
    /// macro is used (the edges of its expansion), so a recursion through it is a cycle.
    #[test]
    fn a_call_in_a_macro_body_is_an_edge_at_the_use() {
        let src = r#"
            macro_rules! m { ($s:ident) => { $s.f() } }
            impl<C> ParserImpl<C> {
                fn f(&mut self) -> u32 { m!(self) }
                fn set_pure_on_call_or_new_expr() {}
                fn jsx_member_expression_eq() {}
            }"#;
        let r = report(&[("m.rs", src)]);
        assert_eq!(r.cycles, vec![keys(&["ParserImpl::f"])]);
        assert!(!r.ok());
    }

    /// Review V8: a call of a trait method that has no default, in a trait default fn, is an
    /// edge to each impl of that method, so the recursion through the trait is a cycle.
    #[test]
    fn a_trait_call_with_no_default_is_an_edge_to_each_impl() {
        let src = r#"
            trait T { fn a(&mut self) { self.g() } fn g(&mut self); }
            struct X;
            impl T for X { fn g(&mut self) { self.a() } }"#;
        let r = report(&[("t.rs", src)]);
        assert_eq!(r.cycles, vec![keys(&["T::a", "X::g"])]);
        assert!(
            r.edges["T::a"].contains_key("X::g"),
            "{:?}",
            r.edges["T::a"]
        );
    }

    /// Review V8 (b): a typed parameter that a `let` binds again has another type after it, so
    /// a call on that name is an edge to every method of the name, not only to the one of the
    /// parameter type (the edge to `B::f` closes the cycle).
    #[test]
    fn a_parameter_bound_again_has_no_known_type() {
        let src = r#"
            struct P { b: B }
            struct A; struct B;
            impl A { fn f(&mut self, p: &mut P) -> u32 { 0 } }
            impl B { fn f(&mut self, p: &mut P) -> u32 { p.g(&mut A) } }
            impl P {
                fn g(&mut self, a: &mut A) -> u32 { let a = &mut self.b; a.f(self) }
            }"#;
        let r = report(&[("p.rs", src)]);
        assert_eq!(r.cycles, vec![keys(&["B::f", "P::g"])]);
        // With no `let`, the parameter type holds: only `A::f`, no cycle.
        let typed = src.replace("let a = &mut self.b; ", "");
        assert!(report(&[("p.rs", &typed)]).cycles.is_empty());
    }
}
