//! `cargo xtask oxc-sites` (T-SITES, H01 layer 1): every recursion of the patched oxc_parser
//! passes a stack check. The task parses the oxc_parser sources with `syn`, builds a call graph
//! and fails when a cycle has no check site.
//!
//! ```text
//! parse every .rs file under --src (default third_party/oxc/crates/oxc_parser/src)
//!   pass 1  the named fields of every struct (field -> type name)
//!   pass 2  one node per function: `Type::name` for a fn in an impl (or a trait default fn),
//!           `name` for a free fn; nested fns are their own nodes
//!           an edge for every name in a body that names a function of the sources:
//!           a method call (`self.f(..)`, `p.f(..)`), a path call (`AssignmentTarget::cover(..)`),
//!           a function value (`self.parse_list(Self::parse_f)`), a name inside a macro body
//!   a site = a top-level statement of a fn body that holds a call of `may_recurse`
//!   (`if C::CHECKS_RECURSION && !self.config.may_recurse() { return .. }`): the statements
//!   after it are behind the check, so the node keeps only the edges of the statements before it
//!   -> the site table; a cycle (a strongly connected group, or a self call) that is left is a
//!      finding, unless its edges are in ALLOW (name noise, each with its reason)
//! ```
//!
//! How a name becomes an edge (the graph has more edges than the code, never fewer):
//!
//! | Use | Edge to |
//! |---|---|
//! | `recv.f(..)`, the type T of `recv` known (`self`, a typed parameter, a field of a struct of the sources, the return type of a call of a fn of the sources) | `T::f` if the sources define it; else every method named `f` if T has an impl in the sources; else (a foreign type such as `str` or `Vec`) only the methods named `f` of a blanket impl (`impl<X> Tr for X`) |
//! | `recv.f(..)`, the type not known | every method named `f` |
//! | `Self::f`, `T::f`, `<X as T>::f` (T a type or trait) | `T::f` (`Self` = the impl type) if defined; else, T a trait that the sources implement, the fns `f` of those impls; else every method named `f` if T has an impl in the sources; else (a foreign type or trait: `Vec::new`, `Box::new`) only the blanket-impl methods named `f` |
//! | `m::f` (m a module: a lower-case segment) | every free fn named `f` |
//! | `f`, `f(..)` (one segment: a local or a free fn in Rust) | every free fn named `f` |
//! | an identifier in a macro body that does not parse as expressions | every fn named it |
//!
//! Test code (`#[cfg(test)]` items, `#[test]` fns) is not part of the parser and is skipped.
//!
//! - Macro bodies are read as comma-separated expressions when they parse, else as tokens;
//!   `macro_rules!` bodies are skipped.
//! - [`LOOPS`] are the walks that the patch turns into loops (DES section 1.3, C-1): each one must
//!   exist and must not be on a cycle.
//! - `--assume-site NAME` (repeatable; `name` or `Type::name`) treats that fn as a site at its
//!   first statement. A bump uses it on the new, unpatched sources to find the site list before
//!   the patch is ported. `--explain` prints each edge of a cycle with the use that made it.
//!
//! Exit codes: 0 no cycle, 1 a cycle or a loop that is missing or recursive, 2 usage error or a
//! file that does not parse. Paths print relative to the source dir, with `/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Expr, FnArg, Pat, Stmt, Type};

const USAGE: &str = "\
usage: cargo xtask oxc-sites [--src DIR] [--assume-site NAME]... [--explain]
  --src DIR            the oxc_parser sources (default: third_party/oxc/crates/oxc_parser/src
                       of this tree; sync it first: bash scripts/sync-oxc.sh)
  --assume-site NAME   treat the fn NAME (name or Type::name) as a site (a bump: find the
                       sites on the new sources first)
  --explain            print each edge of a cycle and the use that made it";

const DEFAULT_SRC: &str = "third_party/oxc/crates/oxc_parser/src";

/// The method that a site calls.
const CHECK: &str = "may_recurse";

/// The type whose methods print with no `Type::` prefix.
const PARSER: &str = "ParserImpl";

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

/// Edges that are name noise (a call whose receiver type the graph cannot see, joined with a
/// same-name fn of another type): `(from, to, reason)`. An edge here is left out of the graph.
const ALLOW: &[(&str, &str, &str)] = &[
    (
        "ParserImpl::parse_regex_pattern",
        "Parser::parse",
        "`LiteralParser::new(..).parse()` is the oxc_regular_expression parser (another crate)",
    ),
    (
        "ParserImpl::parse_regex_pattern",
        "ParserImpl::parse",
        "`LiteralParser::new(..).parse()` is the oxc_regular_expression parser (another crate)",
    ),
    (
        "parse_float_with_underscores",
        "Parser::parse",
        "`s.cow_replace(..).parse::<f64>()` is `str::parse` (std)",
    ),
    (
        "parse_float_with_underscores",
        "ParserImpl::parse",
        "`s.cow_replace(..).parse::<f64>()` is `str::parse` (std)",
    ),
    (
        "JSXClosing::dummy",
        "JSXClosing::dummy",
        "`Dummy::dummy(allocator)` builds the `JSXClosingFragment` field (an oxc_ast type)",
    ),
    (
        "ModifierKind::from_usize_unchecked",
        "ModifierKinds::iter",
        "`Self::VARIANTS.iter()` (a debug_assert) is the slice iter of the const array that the \
         `fieldless_enum!` macro (oxc_data_structures) writes",
    ),
    (
        "ModifierKind::from_usize_unchecked",
        "Modifiers::iter",
        "`Self::VARIANTS.iter()` (a debug_assert) is the slice iter of the const array that the \
         `fieldless_enum!` macro (oxc_data_structures) writes",
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
}

#[derive(Debug, Default)]
struct Func {
    key: String,
    name: String,
    file: String,
    line: usize,
    /// The line of the check statement, if the fn holds a site.
    check_line: Option<usize>,
    /// The uses before the check (all uses when there is no check).
    uses: BTreeSet<Use>,
}

#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) files: usize,
    pub(crate) funcs: usize,
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
}

impl Report {
    pub(crate) fn ok(&self) -> bool {
        self.cycles.is_empty() && self.loops.iter().all(|l| l.2)
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
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--src" => src = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--assume-site" => {
                assume.insert(it.next().ok_or(USAGE)?.clone());
            }
            "--explain" => explain = true,
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
    let r = run(&sources, &assume)?;
    print_report(&r, &src, explain);
    Ok(r.ok())
}

fn print_report(r: &Report, src: &Path, explain: bool) {
    println!(
        "oxc-sites: {} files, {} functions under {}",
        r.files,
        r.funcs,
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

/// Builds the graph of `sources` (path, text) and finds the cycles that no site cuts.
pub(crate) fn run(
    sources: &[(String, String)],
    assume: &BTreeSet<String>,
) -> Result<Report, String> {
    let mut asts = Vec::new();
    for (rel, text) in sources {
        let ast = syn::parse_file(text).map_err(|e| {
            let at = e.span().start();
            format!("{rel}:{}:{}: does not parse: {e}", at.line, at.column + 1)
        })?;
        asts.push((rel, ast));
    }
    // Pass 1: struct fields, impl types, return types.
    let mut types = Types::default();
    for (_, ast) in &asts {
        types.visit_file(ast);
    }
    // Pass 2: functions.
    let mut funcs = Vec::new();
    for (rel, ast) in &asts {
        let mut c = Collector {
            file: (*rel).clone(),
            impl_self: Vec::new(),
            types: &types,
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
    let mut edges: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut places = BTreeMap::new();
    for f in &funcs {
        let out = edges.entry(f.key.clone()).or_default();
        for u in &f.uses {
            let to: Vec<String> = match u {
                Use::Method(Some(t), n) => {
                    let k = format!("{t}::{n}");
                    if keys.contains(k.as_str()) {
                        vec![k]
                    } else if types.impls.contains(t) || types.blanket.contains(n) {
                        all(&methods, n)
                    } else {
                        Vec::new()
                    }
                }
                Use::Method(None, n) => all(&methods, n),
                Use::Path(q, n) if q.starts_with(|c: char| c.is_ascii_lowercase()) => all(&free, n),
                Use::Path(q, n) => {
                    let k = format!("{q}::{n}");
                    if keys.contains(k.as_str()) {
                        vec![k]
                    } else if let Some(fns) = types.trait_fns.get(q)
                        && !types.blanket.contains(n)
                    {
                        fns.get(n).into_iter().flatten().cloned().collect()
                    } else if types.impls.contains(q) || types.blanket.contains(n) {
                        all(&methods, n)
                    } else {
                        Vec::new()
                    }
                }
                Use::Bare(n) => all(&free, n),
                Use::Any(n) => all(&by_name, n),
            };
            for k in to {
                if !ALLOW.iter().any(|(a, b, _)| *a == f.key && *b == k) {
                    out.entry(k).or_insert_with(|| format!("{u:?}"));
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
    Ok(Report {
        files: sources.len(),
        funcs: funcs.len(),
        sites,
        loops,
        cycles,
        places,
        why,
    })
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

/// The last path segment of a type, through references, parens and groups.
/// A pointer, slice, array or tuple is a foreign type: `<builtin>`.
fn type_name(t: &Type) -> Option<String> {
    match t {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
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

/// Pass 1: what the sources say about types.
#[derive(Default)]
struct Types {
    /// struct name -> field name -> field type name.
    fields: BTreeMap<String, BTreeMap<String, String>>,
    /// The self type names of every impl.
    impls: BTreeSet<String>,
    /// The method names of blanket impls (`impl<X: B> Tr for X`).
    blanket: BTreeSet<String>,
    /// trait name -> fn name -> the keys of the impls of that trait in the sources.
    trait_fns: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// fn or associated const key -> its type name (`Self` is the impl type).
    rets: BTreeMap<String, String>,
    impl_self: Vec<Option<String>>,
}

impl<'ast> Visit<'ast> for Types {
    fn visit_item_struct(&mut self, s: &'ast syn::ItemStruct) {
        let e = self.fields.entry(s.ident.to_string()).or_default();
        for f in &s.fields {
            if let (Some(id), Some(t)) = (&f.ident, type_name(&f.ty)) {
                e.insert(id.to_string(), t);
            }
        }
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
                let e = self.trait_fns.entry(tr.ident.to_string()).or_default();
                for it in &i.items {
                    if let syn::ImplItem::Fn(f) = it {
                        e.entry(f.sig.ident.to_string())
                            .or_default()
                            .insert(format!("{t}::{}", f.sig.ident));
                    }
                }
            }
        }
        self.impl_self.push(t);
        visit::visit_item_impl(self, i);
        self.impl_self.pop();
    }

    fn visit_impl_item_const(&mut self, c: &'ast syn::ImplItemConst) {
        if let (Some(Some(t)), Some(r)) = (self.impl_self.last(), type_name(&c.ty)) {
            self.rets.insert(format!("{t}::{}", c.ident), r);
        }
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        if let (Some(Some(t)), syn::ReturnType::Type(_, ty)) =
            (self.impl_self.last(), &f.sig.output)
            && let Some(r) = type_name(ty)
        {
            let r = if r == "Self" { t.clone() } else { r };
            self.rets.insert(format!("{t}::{}", f.sig.ident), r);
        }
    }
}

/// Pass 2: the functions of one file.
struct Collector<'o> {
    file: String,
    /// The self type of each impl or trait around the current item.
    impl_self: Vec<Option<String>>,
    types: &'o Types,
    out: &'o mut Vec<Func>,
}

impl Collector<'_> {
    fn func(&mut self, sig: &syn::Signature, block: &Block) {
        let name = sig.ident.to_string();
        let self_ty = self.impl_self.last().cloned().flatten();
        let key = match &self_ty {
            Some(t) => format!("{t}::{name}"),
            None => name.clone(),
        };
        let mut params = BTreeMap::new();
        for a in &sig.inputs {
            if let FnArg::Typed(pt) = a
                && let Pat::Ident(pi) = &*pt.pat
                && let Some(t) = type_name(&pt.ty)
            {
                params.insert(pi.ident.to_string(), t);
            }
        }
        let mut f = Func {
            key,
            name,
            file: self.file.clone(),
            line: sig.ident.span().start().line,
            ..Func::default()
        };
        let mut nested = Vec::new();
        let mut behind = BTreeSet::new();
        for stmt in &block.stmts {
            if f.check_line.is_none() && stmt_has_check(stmt) {
                f.check_line = Some(stmt.span().start().line);
                continue;
            }
            // Behind the check only nested fns count (they are their own nodes).
            let uses = if f.check_line.is_none() {
                &mut f.uses
            } else {
                &mut behind
            };
            let mut n = Uses {
                self_ty: self_ty.as_deref(),
                params: &params,
                types: self.types,
                uses,
                nested: &mut nested,
            };
            n.visit_stmt(stmt);
        }
        self.out.push(f);
        let saved = std::mem::take(&mut self.impl_self);
        for item in nested {
            self.visit_item_fn(&item);
        }
        self.impl_self = saved;
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
        visit::visit_item_impl(self, i);
        self.impl_self.pop();
    }

    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        self.impl_self.push(Some(i.ident.to_string()));
        visit::visit_item_trait(self, i);
        self.impl_self.pop();
    }

    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        if is_test(&i.attrs) {
            return;
        }
        let saved = std::mem::take(&mut self.impl_self);
        self.func(&i.sig, &i.block);
        self.impl_self = saved;
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

    fn visit_item_macro(&mut self, _: &'ast syn::ItemMacro) {
        // `macro_rules!` and item macros at file level define no parser fn in these sources.
    }
}

/// Does this statement hold a call of [`CHECK`]?
fn stmt_has_check(stmt: &Stmt) -> bool {
    struct Find(bool);
    impl<'ast> Visit<'ast> for Find {
        fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
            if m.method == CHECK {
                self.0 = true;
            }
            visit::visit_expr_method_call(self, m);
        }
    }
    let mut f = Find(false);
    f.visit_stmt(stmt);
    f.0
}

/// Collects the uses of a body; nested fn items go to `nested`.
struct Uses<'n> {
    self_ty: Option<&'n str>,
    params: &'n BTreeMap<String, String>,
    types: &'n Types,
    uses: &'n mut BTreeSet<Use>,
    nested: &'n mut Vec<syn::ItemFn>,
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
                self.types.rets.get(&format!("{q}::{n}")).cloned()
            }
            Expr::Field(f) => {
                let base = self.recv_type(&f.base)?;
                let syn::Member::Named(m) = &f.member else {
                    return None;
                };
                self.types.fields.get(&base)?.get(&m.to_string()).cloned()
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
                self.types.rets.get(&format!("{q}::{n}")).cloned()
            }
            Expr::MethodCall(m) => {
                let t = self.recv_type(&m.receiver)?;
                self.types.rets.get(&format!("{t}::{}", m.method)).cloned()
            }
            Expr::Paren(p) => self.recv_type(&p.expr),
            Expr::Reference(r) => self.recv_type(&r.expr),
            Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => self.recv_type(&u.expr),
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Uses<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.nested.push(i.clone());
    }

    fn visit_item_impl(&mut self, _: &'ast syn::ItemImpl) {}

    fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
        let t = self.recv_type(&m.receiver);
        self.uses.insert(Use::Method(t, m.method.to_string()));
        visit::visit_expr_method_call(self, m);
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
        run(&s, &BTreeSet::new()).expect("fixture parses")
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
                    if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.stop(); }
                    self.parse_b()
                }
                fn parse_b(&mut self) -> u32 { if self.more() { self.parse_a() } else { 0 } }
                fn more(&mut self) -> bool { self.checkpoint() > 0 }
                fn checkpoint(&mut self) -> u32 { self.lexer.checkpoint() }
                fn stop(&mut self) -> u32 { 0 }
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
                    if !self.config.may_recurse() { return 0; }
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
}
