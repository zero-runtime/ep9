//! The hint join (H37 A1h item 3; `docs/plan/01-front-end.md` section 10): the records of one
//! file of a `.ep9h` side table meet the nodes of the oxc AST before any lowering, by the key
//! (start, end, kind) (S1). No codegen reads the result before A4 (S9): BytecodeGen does not
//! call this module, and a compile with no `.ep9h` file runs none of it.
//!
//! ```text
//!  source --strip one U+FEFF (S2)--> text --SHA-256 of its UTF-8 bytes (H37-Q3)--> hash
//!  Hints::file_hints(path, hash): not in the table, type errors, another hash (S3)
//!    | Err ------------------------------------------------> JoinError::Hints      (counted)
//!    v Ok(records of the file)
//!  the path is not a plain .js, .mjs or .cjs path -------> JoinError::TsSource   (counted)
//!    v   (plain: no ':', '?' or '#', so no scheme, drive, query or fragment)
//!  the goal of `opts` is not the goal of the path
//!  (.mjs: Module, .cjs: Script; .js: none) --------------> JoinError::GoalMismatch (counted)
//!    v
//!  the front end on its worker (lib.rs with_front_end): layer 0, the parse with layer 1,
//!  layer 2 and the budget check of layer 1, oxc_semantic --> JoinError::Compile    (counted)
//!    v
//!  the join walk (oxc Visit; l1::backstop at its depth step) --> node keys (start, end, kind)
//!    v                                      and one (start, end, ANY_NODE) key per AST node
//!    v                                              a stop --> Compile(Range(StackLimit))
//!  each record: its key is a node key -------------> joined
//!               its span is a node of another kind -> JoinError::KindMismatch (S3, counted)
//!               (any AST node: also one that has no NodeKind, as a BooleanLiteral)
//!               no AST node has its span ----------> the record is dropped     (no-node, counted)
//! ```
//!
//! Rules:
//! - JS only: the front end parses the JS goals (lib.rs `parse`: `SourceType::script()` or
//!   `mjs()`). A TS text can parse there as other nodes on the same spans: in TS
//!   `f<number>(1) + 2` is an addition of a call, in JS the comparison
//!   `(f < number) > ((1) + 2)`, with the same root span and kind. So a path that is not a
//!   `.js`, `.mjs` or `.cjs` path (a `.ts`, `.tsx`, `.mts`, `.cts` or `.jsx` path) gives
//!   [`JoinError::TsSource`], before the parse. The TS parse needs a language in `SourceOpts`
//!   (H3). A path with ':', '?' or '#' (a scheme, a drive, a query or a fragment) is not a JS
//!   path either: `src/a.ts?x.js` can name a TS resource.
//! - Goal: the two JS goals can also parse as other nodes on the same spans: in a module
//!   `await(1)+2` is `(await 1) + 2`, in a script the call `await(1)` plus 2, with the same
//!   root span and kind. So a `.mjs` path needs the module goal and a `.cjs` path the script
//!   goal, else [`JoinError::GoalMismatch`], before the parse. Limit: a `.js` path fixes no
//!   goal (package.json `type`), so the join takes the goal of `opts`, and a wrong goal there
//!   can join. A check needs the goal of the producer in the `.ep9h` file entry (a schema
//!   change; H37 item 3).
//! - Offsets: oxc spans of the text after the BOM strip are UTF-8 byte offsets of the decoded
//!   text, as the records are (S2, H37-Q4). Both sides hash the UTF-8 bytes of that text, never
//!   an engine string form (MC-w49-6).
//! - S4: a JSDoc cast keys on its ParenthesizedExpression. The parse keeps parentheses (the oxc
//!   default), so `(e)` is a node of its own, with the span of the parentheses.
//! - A node with an empty span is not source text (the strict directive of `force_strict`):
//!   it has no key.
//! - The caller reads the `.ep9h` bytes with `ep9_hints::read` and its build's `Expect` (S5),
//!   and counts a dropped input with [`JoinCounts::add_read_drop`]. This module reads no file.
//! - No oxc type leaves this module (H1): its API has `ep9_hints` types and crate types.
//! - The one match over an oxc enum is `visit_property_key`, exhaustive with no `_` arm: a
//!   change of the oxc key forms (the 0.160.0 PropertyKey change, DEPS-BUMP-3) fails to compile
//!   there, and the key-form test shows what the join sees. Kinds come from the typed `visit_`
//!   methods, not from a match over `AstKind`.
//! - Stack: the walk recurses with the AST. It runs only after layer 2 and the budget check of
//!   layer 1 (lib.rs `check_depth`: at most `AST_CAP` levels, and the levels at
//!   `stack::PASS_PER_LEVEL` with the margin fit the stack). Its frame per level is the join
//!   column of T-A (`probe::measure_forms`), checked against `PASS_PER_LEVEL`. It reads the
//!   stack at every `l1::BACKSTOP_STEP`-th level (`l1::backstop`); after a stop the 11 gated
//!   points of the depth pass (depth.rs) end the descent, so the walk unwinds.

use oxc_ast::ast::{
    ArrayExpression, ArrowFunctionExpression, AssignmentExpression, AssignmentTarget,
    AwaitExpression, BinaryExpression, BindingIdentifier, BindingPattern, CallExpression, Class,
    ComputedMemberExpression, ConditionalExpression, Expression, FormalParameter, Function,
    IdentifierReference, JSXAttributeValue, JSXChild, JSXMemberExpressionObject, LogicalExpression,
    MethodDefinition, NewExpression, NumericLiteral, ObjectExpression, ParenthesizedExpression,
    PrivateFieldExpression, Program, PropertyDefinition, PropertyKey, Statement,
    StaticMemberExpression, StringLiteral, TSAsExpression, TSImportTypeQualifier,
    TSNamespaceDeclarationBody, TSNonNullExpression, TSType, TSTypeName, TemplateLiteral,
    ThisExpression, UnaryExpression, UpdateExpression, VariableDeclarator,
};
use oxc_ast::{AstKind, match_expression};
use oxc_ast_visit::{Visit, walk};
use oxc_semantic::ScopeFlags;
use oxc_span::{GetSpan, Span};

use ep9_hints::{DropCounts, DropReason, Hints, NodeKind, Record, TextHash};

use crate::{CompileError, Goal, RangeError, SourceOpts};

/// One slot per `NodeKind` code (codes 1 to `NodeKind::ALL.len()`). Slot 0 counts the
/// [`ANY_NODE`] keys; no accessor reads it.
const KIND_SLOTS: usize = NodeKind::ALL.len() + 1;

/// The code of the node key that every AST node with a non-empty span has, of a `NodeKind` or
/// not. A record on its span with no key of the record's kind is a kind mismatch (S3). No
/// record has code 0 (H37-Q2: code 0 is no kind), so no record key is an `ANY_NODE` key.
const ANY_NODE: u16 = 0;

// The codes are 1..=ALL.len() in order (H37-Q2), so a code is a slot index.
const _: () = {
    let mut i = 0;
    while i < NodeKind::ALL.len() {
        assert!(NodeKind::ALL[i].code() as usize == i + 1);
        i += 1;
    }
};

/// The key of a record or a node: (start, end, `NodeKind` code), as `Record::key`.
type Key = (u32, u32, u16);

/// The text that both sides key and hash: `source` without one leading U+FEFF (S2; tsrs strips
/// it the same way).
#[must_use]
pub fn decoded_text(source: &str) -> &str {
    source.strip_prefix('\u{FEFF}').unwrap_or(source)
}

/// The decoded-text hash of a file entry (H37-Q3): the SHA-256 of the UTF-8 bytes of
/// [`decoded_text`].
#[must_use]
pub fn text_hash(source: &str) -> TextHash {
    ep9_bytecode::sha256::sha256(decoded_text(source).as_bytes())
}

/// Why the join of one file gave no hints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinError {
    /// `ep9_hints` dropped the file's hints: its path is not in the table, it has type errors,
    /// or its text hash is not the hash of this text (S3).
    Hints(DropReason),
    /// The path is not a plain `.js`, `.mjs` or `.cjs` path (it has another suffix, or a ':',
    /// '?' or '#'), so its text may be TS (or JSX): the front end parses the JS goals only,
    /// and the JS parse of a TS text can put other nodes on the record spans (module doc).
    /// Every hint of the file is dropped; no parse runs.
    TsSource,
    /// The goal of the `SourceOpts` is not the goal that the path fixes (`.mjs`: Module,
    /// `.cjs`: Script; a `.js` path fixes none): the other goal can put other nodes on the
    /// record spans (module doc). Every hint of the file is dropped; no parse runs.
    GoalMismatch,
    /// S3: the span of this record is a node of another kind (any AST node, also one with no
    /// `NodeKind`), so the two parses do not agree: every hint of the file is dropped.
    KindMismatch {
        /// The record key start.
        start: u32,
        /// The record key end.
        end: u32,
        /// The record key kind.
        kind: NodeKind,
    },
    /// The source does not pass the front end: the error that a compile gives. No hints.
    Compile(CompileError),
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinError::Hints(reason) => write!(f, "hints dropped: {reason}"),
            JoinError::TsSource => write!(f, "hints dropped: ts-source (the join parses JS only)"),
            JoinError::GoalMismatch => write!(
                f,
                "hints dropped: goal-mismatch (the goal is not the goal of the path)"
            ),
            JoinError::KindMismatch { start, end, kind } => write!(
                f,
                "hints dropped: kind-mismatch ({} at {start}..{end})",
                kind.name()
            ),
            JoinError::Compile(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for JoinError {}

/// The join of one file: the records whose key is a node of the AST, and the counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HintJoin {
    records: Vec<Record>,
    no_node: u32,
    nodes: [u32; KIND_SLOTS],
    joined: [u32; KIND_SLOTS],
    /// The deepest level of the walk (the depth unit of layer 2: Program is 1).
    pub(crate) depth: u32,
}

impl HintJoin {
    /// The joined records, in the order of the file.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// The records that no AST node has the span of (each dropped alone).
    #[must_use]
    pub fn no_node(&self) -> u32 {
        self.no_node
    }

    /// The nodes of `kind` in the AST (each key once).
    #[must_use]
    pub fn nodes(&self, kind: NodeKind) -> u32 {
        self.nodes[usize::from(kind.code())]
    }

    /// The nodes of `kind` that a record joined (the A1h join gate reads it per core kind).
    #[must_use]
    pub fn joined(&self, kind: NodeKind) -> u32 {
        self.joined[usize::from(kind.code())]
    }
}

/// Join the hints of `path` in `hints` with the oxc AST of `source` (module doc).
///
/// `source` is the decoded text as tsrs read it (UTF-8; one leading BOM is stripped here, once).
/// The positions of a [`JoinError::Compile`] error are in the text after that strip.
///
/// # Errors
///
/// [`JoinError::Hints`] when `ep9_hints` drops the file (S3), [`JoinError::TsSource`] when
/// `path` is not a plain `.js`, `.mjs` or `.cjs` path, [`JoinError::GoalMismatch`] when the
/// goal of `opts` is not the goal of a `.mjs` or `.cjs` path, [`JoinError::Compile`] when the
/// front end rejects the source, and [`JoinError::KindMismatch`] when a record's span is a
/// node of another kind (S3).
pub fn join_hints(
    source: &str,
    opts: SourceOpts,
    path: &str,
    hints: &Hints,
) -> Result<HintJoin, JoinError> {
    let text = decoded_text(source);
    let hash = ep9_bytecode::sha256::sha256(text.as_bytes());
    let records = hints.file_hints(path, &hash).map_err(JoinError::Hints)?;
    if !is_js_path(path) {
        return Err(JoinError::TsSource);
    }
    if path_goal(path).is_some_and(|goal| goal != opts.goal()) {
        return Err(JoinError::GoalMismatch);
    }
    let walked = crate::with_front_end(text, opts, |fe| {
        walk(fe.program)
            .map_err(|at| CompileError::Range(RangeError::stack_limit(fe.source, Some(at))))
    })
    .map_err(JoinError::Compile)?;
    join_records(records, walked)
}

/// True for a path whose text the JS parse of the front end reads as its author wrote it: a
/// plain `.js`, `.mjs` or `.cjs` path (case as written; the file table keeps the path of the
/// file). Plain: no ':' (a scheme or a drive), no '?' (a query) and no '#' (a fragment), as
/// such a key can name another resource (`src/a.ts?x.js`).
fn is_js_path(path: &str) -> bool {
    !path.contains([':', '?', '#'])
        && [".js", ".mjs", ".cjs"]
            .iter()
            .any(|ext| path.ends_with(ext))
}

/// The goal that the suffix of a JS path fixes: `.mjs` Module, `.cjs` Script. A `.js` path
/// fixes none (package.json `type` decides; the join does not read it).
fn path_goal(path: &str) -> Option<Goal> {
    if path.ends_with(".mjs") {
        Some(Goal::Module)
    } else if path.ends_with(".cjs") {
        Some(Goal::Script)
    } else {
        None
    }
}

/// The node keys of a walk (a `NodeKind` key and an [`ANY_NODE`] key per node) and its
/// deepest level.
pub(crate) struct Walked {
    pub(crate) nodes: Vec<Key>,
    pub(crate) depth: u32,
}

/// The join walk over `program`. `Err` with the byte offset where the layer-1 backstop stopped
/// it (only on a thread smaller than the budget).
pub(crate) fn walk(program: &Program<'_>) -> Result<Walked, u32> {
    let mut walk = JoinWalk {
        depth: 0,
        max: 0,
        stopped: None,
        nodes: Vec::new(),
    };
    walk.visit_program(program);
    match walk.stopped {
        None => Ok(Walked {
            nodes: walk.nodes,
            depth: walk.max,
        }),
        Some(at) => Err(at),
    }
}

fn join_records(records: &[Record], walked: Walked) -> Result<HintJoin, JoinError> {
    let Walked { mut nodes, depth } = walked;
    nodes.sort_unstable();
    nodes.dedup();
    let mut out = HintJoin {
        records: Vec::new(),
        no_node: 0,
        nodes: [0; KIND_SLOTS],
        joined: [0; KIND_SLOTS],
        depth,
    };
    for &(_, _, kind) in &nodes {
        out.nodes[usize::from(kind)] += 1;
    }
    for record in records {
        let key = record.key();
        if nodes.binary_search(&key).is_ok() {
            out.joined[usize::from(key.2)] += 1;
            out.records.push(*record);
            continue;
        }
        // The first key on or after the record span: on its span it is a key of another kind,
        // a `NodeKind` key or the `ANY_NODE` key of a node with no `NodeKind`.
        let at = nodes.partition_point(|n| (n.0, n.1) < (key.0, key.1));
        if nodes.get(at).is_some_and(|n| (n.0, n.1) == (key.0, key.1)) {
            return Err(JoinError::KindMismatch {
                start: record.start,
                end: record.end,
                kind: record.kind,
            });
        }
        out.no_node += 1;
    }
    Ok(out)
}

/// The drops and joins of a build, per reason (H37 S3, S5; "ep9c counts and reports each drop").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JoinCounts {
    hints: DropCounts,
    ts_source: u64,
    goal_mismatch: u64,
    kind_mismatch: u64,
    not_compiled: u64,
    files: u64,
    records: u64,
    no_node: u64,
}

impl JoinCounts {
    /// Counts the join of one file.
    pub fn add(&mut self, result: &Result<HintJoin, JoinError>) {
        match result {
            Ok(join) => {
                self.files += 1;
                self.records += join.records.len() as u64;
                self.no_node += u64::from(join.no_node);
            }
            Err(JoinError::Hints(reason)) => self.hints.add(*reason),
            Err(JoinError::TsSource) => self.ts_source += 1,
            Err(JoinError::GoalMismatch) => self.goal_mismatch += 1,
            Err(JoinError::KindMismatch { .. }) => self.kind_mismatch += 1,
            Err(JoinError::Compile(_)) => self.not_compiled += 1,
        }
    }

    /// Counts a whole input that `ep9_hints::read` dropped (a header mismatch, S5, or a bad file).
    pub fn add_read_drop(&mut self, reason: DropReason) {
        self.hints.add(reason);
    }

    /// The files that joined.
    #[must_use]
    pub fn files(&self) -> u64 {
        self.files
    }

    /// The records that joined.
    #[must_use]
    pub fn records(&self) -> u64 {
        self.records
    }

    /// Every drop reason with its count, by stable name: the 16 reasons of `ep9_hints` (an
    /// input or a file), then `ts-source`, `goal-mismatch`, `kind-mismatch` and
    /// `not-compiled` (a file), then `no-node` (a record).
    #[must_use]
    pub fn per_reason(&self) -> Vec<(&'static str, u64)> {
        let mut rows: Vec<(&'static str, u64)> = DropReason::ALL
            .iter()
            .map(|&r| (r.name(), self.hints.get(r)))
            .collect();
        rows.push(("ts-source", self.ts_source));
        rows.push(("goal-mismatch", self.goal_mismatch));
        rows.push(("kind-mismatch", self.kind_mismatch));
        rows.push(("not-compiled", self.not_compiled));
        rows.push(("no-node", self.no_node));
        rows
    }
}

/// The join walk: the key of every node of a `NodeKind`, the [`ANY_NODE`] key of every AST
/// node, the depth, and the layer-1 backstop.
struct JoinWalk {
    depth: u32,
    max: u32,
    /// The offset of the node where the backstop stopped the walk.
    stopped: Option<u32>,
    nodes: Vec<Key>,
}

impl JoinWalk {
    /// False once the backstop stopped the walk: the gated points then skip their subtree.
    #[inline]
    fn may_descend(&self) -> bool {
        self.stopped.is_none()
    }

    #[inline]
    fn node(&mut self, span: Span, kind: NodeKind) {
        if !span.is_empty() {
            self.nodes.push((span.start, span.end, kind.code()));
        }
    }
}

/// The typed `visit_` method of each `NodeKind` (codes 1-30): the key, then the oxc walk.
macro_rules! node_kinds {
    ($($method:ident($ty:ident) $walk:ident => $kind:ident,)*) => {
        $(
            fn $method(&mut self, it: &$ty<'a>) {
                self.node(it.span, NodeKind::$kind);
                walk::$walk(self, it);
            }
        )*
    };
}

impl<'a> Visit<'a> for JoinWalk {
    #[inline]
    fn enter_node(&mut self, kind: AstKind<'a>) {
        self.depth += 1;
        if self.depth > self.max {
            self.max = self.depth;
        }
        let span = kind.span();
        // S3: a record on the span of a node with no `NodeKind` is a kind mismatch too.
        if !span.is_empty() {
            self.nodes.push((span.start, span.end, ANY_NODE));
        }
        // Layer 1 backstop (l1.rs): on a thread smaller than the budget.
        if self.stopped.is_none() && crate::l1::backstop(self.depth) {
            self.stopped = Some(span.start);
        }
    }

    #[inline]
    fn leave_node(&mut self, _kind: AstKind<'a>) {
        self.depth -= 1;
    }

    node_kinds! {
        visit_identifier_reference(IdentifierReference) walk_identifier_reference => IdentifierReference,
        visit_binding_identifier(BindingIdentifier) walk_binding_identifier => BindingIdentifier,
        visit_static_member_expression(StaticMemberExpression) walk_static_member_expression => StaticMemberExpression,
        visit_computed_member_expression(ComputedMemberExpression) walk_computed_member_expression => ComputedMemberExpression,
        visit_private_field_expression(PrivateFieldExpression) walk_private_field_expression => PrivateFieldExpression,
        visit_call_expression(CallExpression) walk_call_expression => CallExpression,
        visit_new_expression(NewExpression) walk_new_expression => NewExpression,
        visit_parenthesized_expression(ParenthesizedExpression) walk_parenthesized_expression => ParenthesizedExpression,
        visit_arrow_function_expression(ArrowFunctionExpression) walk_arrow_function_expression => ArrowFunctionExpression,
        visit_class(Class) walk_class => Class,
        visit_property_definition(PropertyDefinition) walk_property_definition => PropertyDefinition,
        visit_method_definition(MethodDefinition) walk_method_definition => MethodDefinition,
        visit_formal_parameter(FormalParameter) walk_formal_parameter => FormalParameter,
        visit_object_expression(ObjectExpression) walk_object_expression => ObjectExpression,
        visit_array_expression(ArrayExpression) walk_array_expression => ArrayExpression,
        visit_variable_declarator(VariableDeclarator) walk_variable_declarator => VariableDeclarator,
        visit_binary_expression(BinaryExpression) walk_binary_expression => BinaryExpression,
        visit_logical_expression(LogicalExpression) walk_logical_expression => LogicalExpression,
        visit_assignment_expression(AssignmentExpression) walk_assignment_expression => AssignmentExpression,
        visit_unary_expression(UnaryExpression) walk_unary_expression => UnaryExpression,
        visit_update_expression(UpdateExpression) walk_update_expression => UpdateExpression,
        visit_conditional_expression(ConditionalExpression) walk_conditional_expression => ConditionalExpression,
        visit_numeric_literal(NumericLiteral) walk_numeric_literal => NumericLiteral,
        visit_string_literal(StringLiteral) walk_string_literal => StringLiteral,
        visit_template_literal(TemplateLiteral) walk_template_literal => TemplateLiteral,
        visit_await_expression(AwaitExpression) walk_await_expression => AwaitExpression,
        visit_ts_as_expression(TSAsExpression) walk_ts_as_expression => TSAsExpression,
        visit_ts_non_null_expression(TSNonNullExpression) walk_ts_non_null_expression => TSNonNullExpression,
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.node(it.span, NodeKind::Function);
        walk::walk_function(self, it, flags);
    }

    fn visit_this_expression(&mut self, it: &ThisExpression) {
        self.node(it.span, NodeKind::ThisExpression);
        walk::walk_this_expression(self, it);
    }

    // The key of an object property, a class member or a binding property: the walk of
    // `walk_property_key`, written out with no `_` arm. A literal or computed key is an
    // expression (its nodes have keys); a name (`a`, `#a`) is not a `NodeKind`.
    fn visit_property_key(&mut self, it: &PropertyKey<'a>) {
        match it {
            PropertyKey::StaticIdentifier(name) => self.visit_identifier_name(name),
            PropertyKey::PrivateIdentifier(name) => self.visit_private_identifier(name),
            match_expression!(PropertyKey) => self.visit_expression(it.to_expression()),
        }
    }

    // The 11 gated points of the depth pass (depth.rs; the test
    // `the_walk_gates_the_points_of_the_depth_pass`): every cycle of the oxc visitor graph
    // passes one of them.

    fn visit_expression(&mut self, it: &Expression<'a>) {
        if self.may_descend() {
            walk::walk_expression(self, it);
        }
    }

    fn visit_statement(&mut self, it: &Statement<'a>) {
        if self.may_descend() {
            walk::walk_statement(self, it);
        }
    }

    fn visit_binding_pattern(&mut self, it: &BindingPattern<'a>) {
        if self.may_descend() {
            walk::walk_binding_pattern(self, it);
        }
    }

    fn visit_assignment_target(&mut self, it: &AssignmentTarget<'a>) {
        if self.may_descend() {
            walk::walk_assignment_target(self, it);
        }
    }

    fn visit_ts_type(&mut self, it: &TSType<'a>) {
        if self.may_descend() {
            walk::walk_ts_type(self, it);
        }
    }

    fn visit_ts_type_name(&mut self, it: &TSTypeName<'a>) {
        if self.may_descend() {
            walk::walk_ts_type_name(self, it);
        }
    }

    fn visit_ts_import_type_qualifier(&mut self, it: &TSImportTypeQualifier<'a>) {
        if self.may_descend() {
            walk::walk_ts_import_type_qualifier(self, it);
        }
    }

    fn visit_ts_namespace_declaration_body(&mut self, it: &TSNamespaceDeclarationBody<'a>) {
        if self.may_descend() {
            walk::walk_ts_namespace_declaration_body(self, it);
        }
    }

    fn visit_jsx_member_expression_object(&mut self, it: &JSXMemberExpressionObject<'a>) {
        if self.may_descend() {
            walk::walk_jsx_member_expression_object(self, it);
        }
    }

    fn visit_jsx_child(&mut self, it: &JSXChild<'a>) {
        if self.may_descend() {
            walk::walk_jsx_child(self, it);
        }
    }

    fn visit_jsx_attribute_value(&mut self, it: &JSXAttributeValue<'a>) {
        if self.may_descend() {
            walk::walk_jsx_attribute_value(self, it);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Read as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::{Duration, Instant};

    use ep9_hints::{Expect, FileEntry, Header, Repr};
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    use super::*;
    use crate::forms::{FORMS, Form};
    use crate::{AST_CAP, DepthLayer, PRESCAN_CAP, RangeErrorKind, Strictness};

    /// A JS path: the join parses the JS goals only (`JoinError::TsSource`).
    const PATH: &str = "src/a.js";

    fn rec(start: u32, end: u32, kind: NodeKind) -> Record {
        Record {
            start,
            end,
            kind,
            repr: Repr::None,
            element: Repr::None,
            non_null: false,
            dts_lib: false,
            enum_const: None,
            confidence: 0,
            class: None,
            slot: None,
            call_target: None,
        }
    }

    /// A one-file table for the decoded `text` (its hash is of exactly these bytes, as tsrs
    /// writes it), written and read back by `ep9_hints` (so it is canonical).
    fn table(text: &str, records: &[Record]) -> Hints {
        table_at(PATH, text, records)
    }

    /// [`table`] with the file at `path`.
    fn table_at(path: &str, text: &str, records: &[Record]) -> Hints {
        let mut records = records.to_vec();
        records.sort_by_key(Record::key);
        let hints = Hints {
            header: Header::default(),
            files: vec![FileEntry {
                path: path.to_owned(),
                text_hash: ep9_bytecode::sha256::sha256(text.as_bytes()),
                has_type_errors: false,
                first_record: 0,
                record_count: u32::try_from(records.len()).unwrap(),
            }],
            classes: Vec::new(),
            fields: Vec::new(),
            records,
        };
        let bytes = ep9_hints::write(&hints).expect("a valid table");
        ep9_hints::read(&bytes, &Expect::default()).expect("read back")
    }

    fn script() -> SourceOpts {
        SourceOpts::script(Strictness::Sloppy)
    }

    /// The node keys of a TS parse on the test thread (no layer-1 limit: the default parser).
    fn ts_nodes(src: &str) -> Vec<Key> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, src, SourceType::ts()).parse();
        assert!(!parsed.diagnostics.has_errors(), "{src}");
        let mut nodes = walk(&parsed.program).expect("no limit here").nodes;
        nodes.sort_unstable();
        nodes
    }

    /// S1: the key is the oxc span plus the node kind.
    #[test]
    fn joins_on_start_end_and_kind() {
        let src = "let a = b + 1;";
        let records = [
            rec(4, 5, NodeKind::BindingIdentifier),
            rec(4, 13, NodeKind::VariableDeclarator),
            rec(8, 9, NodeKind::IdentifierReference),
            rec(8, 13, NodeKind::BinaryExpression),
            rec(12, 13, NodeKind::NumericLiteral),
        ];
        let join = join_hints(src, script(), PATH, &table(src, &records)).unwrap();
        assert_eq!(join.records().len(), 5);
        assert_eq!(join.no_node(), 0);
        for kind in [
            NodeKind::BindingIdentifier,
            NodeKind::VariableDeclarator,
            NodeKind::IdentifierReference,
            NodeKind::BinaryExpression,
            NodeKind::NumericLiteral,
        ] {
            assert_eq!((join.nodes(kind), join.joined(kind)), (1, 1), "{kind:?}");
        }
        assert_eq!(join.nodes(NodeKind::CallExpression), 0);
        // Program, VariableDeclaration, VariableDeclarator, BinaryExpression, NumericLiteral.
        assert_eq!(join.depth, 5);
    }

    /// A record whose span is no node is dropped alone, and counted.
    #[test]
    fn a_record_with_no_node_is_dropped_alone() {
        let src = "let a = b + 1;";
        let records = [
            rec(0, 3, NodeKind::IdentifierReference),
            rec(8, 9, NodeKind::IdentifierReference),
        ];
        let join = join_hints(src, script(), PATH, &table(src, &records)).unwrap();
        assert_eq!(join.records(), &[records[1]]);
        assert_eq!(join.no_node(), 1);
    }

    /// S3: a record on the span of a node of another kind drops every hint of the file.
    #[test]
    fn a_kind_mismatch_drops_the_file() {
        let src = "let a = b + 1;";
        let records = [
            rec(4, 5, NodeKind::BindingIdentifier),
            rec(8, 9, NodeKind::NumericLiteral),
        ];
        let got = join_hints(src, script(), PATH, &table(src, &records));
        assert_eq!(
            got,
            Err(JoinError::KindMismatch {
                start: 8,
                end: 9,
                kind: NodeKind::NumericLiteral
            })
        );
    }

    /// The rows of `counts` that are not 0, by name.
    fn nonzero(counts: &JoinCounts) -> Vec<(&'static str, u64)> {
        let mut rows: Vec<(&'static str, u64)> = counts
            .per_reason()
            .into_iter()
            .filter(|r| r.1 > 0)
            .collect();
        rows.sort_unstable();
        rows
    }

    /// Codex A1H-JOIN r1 F1: the front end parses the JS goals only. In TS,
    /// `f<number>(1)+2` is the addition of the call `f<number>(1)`; in JS it is the comparison
    /// `(f<number) > ((1)+2)` on the same root span, with the same kind. So a TS path gives
    /// `TsSource` and no parse, and is counted once as `ts-source`.
    #[test]
    fn a_ts_path_is_not_joined_with_the_js_parse() {
        let src = "import{f}from\"./g\";f<number>(1)+2";
        // The TS reading, keys by hand: the call is 19..31, the addition 19..33.
        assert_eq!(&src[19..31], "f<number>(1)");
        assert_eq!(&src[19..33], "f<number>(1)+2");
        let records = [
            rec(19, 31, NodeKind::CallExpression),
            rec(19, 33, NodeKind::BinaryExpression),
        ];
        let module = SourceOpts::module();
        let got = join_hints(
            src,
            module,
            "src/a.ts",
            &table_at("src/a.ts", src, &records),
        );
        assert_eq!(got, Err(JoinError::TsSource));
        let mut counts = JoinCounts::default();
        counts.add(&got);
        assert_eq!(nonzero(&counts), [("ts-source", 1)]);
        assert_eq!((counts.files(), counts.records()), (0, 0));
        // The hazard, at a .js path (where the JS parse is the right one): the addition record
        // joins the comparison `>` on 19..33, and no node has the call span 19..31.
        let js = join_hints(src, module, PATH, &table(src, &records)).unwrap();
        assert_eq!(js.records(), &[records[1]]);
        assert_eq!(js.no_node(), 1);
        // Each path that is not .js, .mjs or .cjs; the file drops of ep9_hints come first.
        for path in [
            "src/a.tsx",
            "src/a.mts",
            "src/a.cts",
            "src/a.d.ts",
            "src/a.jsx",
        ] {
            let got = join_hints(src, module, path, &table_at(path, src, &records));
            assert_eq!(got, Err(JoinError::TsSource), "{path}");
        }
        // The text has an import, so it needs the module goal: a .mjs path joins as at the .js
        // path; a .cjs path needs the script goal, so the module goal there is a goal mismatch.
        let path = "src/a.mjs";
        let mjs = join_hints(src, module, path, &table_at(path, src, &records)).unwrap();
        assert_eq!((mjs.records(), mjs.no_node()), (&records[1..], 1));
        let path = "src/a.cjs";
        let got = join_hints(src, module, path, &table_at(path, src, &records));
        assert_eq!(got, Err(JoinError::GoalMismatch));
        let got = join_hints(
            src,
            module,
            "src/b.ts",
            &table_at("src/a.ts", src, &records),
        );
        assert_eq!(got, Err(JoinError::Hints(DropReason::NotInTable)));
        // A TS text that the JS goal does not parse: TsSource too, not `not-compiled`.
        let typed = "let a: number = 1;";
        let got = join_hints(typed, module, "src/a.ts", &table_at("src/a.ts", typed, &[]));
        assert_eq!(got, Err(JoinError::TsSource));
    }

    /// Codex A1H-JOIN r2: the two JS goals can parse as other nodes on the same spans. In a
    /// module `await(1)+2` is `(await 1) + 2`; in a script it is the call `await(1)` plus 2.
    /// The root BinaryExpression is 0..10 in both. So a `.mjs` path with the script goal and a
    /// `.cjs` path with the module goal give `GoalMismatch` and no parse, each counted as
    /// `goal-mismatch`; with the goal of the path, each reading joins by its exact keys.
    #[test]
    fn a_goal_that_is_not_the_goal_of_the_path_drops_the_file() {
        let src = "await(1)+2;";
        // Keys by hand: `await` 0..5, `await(1)` 0..8, the sum 0..10.
        assert_eq!(
            (&src[0..5], &src[0..8], &src[0..10]),
            ("await", "await(1)", "await(1)+2")
        );
        let root = [rec(0, 10, NodeKind::BinaryExpression)];
        let module_keys = [
            rec(0, 8, NodeKind::AwaitExpression),
            rec(0, 10, NodeKind::BinaryExpression),
        ];
        let script_keys = [
            rec(0, 5, NodeKind::IdentifierReference),
            rec(0, 8, NodeKind::CallExpression),
            rec(0, 10, NodeKind::BinaryExpression),
        ];
        let module = SourceOpts::module();
        let strict = SourceOpts::script(Strictness::Strict);
        let mut counts = JoinCounts::default();
        for (path, opts) in [
            ("src/a.mjs", script()),
            ("src/a.mjs", strict),
            ("src/a.cjs", module),
        ] {
            let got = join_hints(src, opts, path, &table_at(path, src, &root));
            assert_eq!(got, Err(JoinError::GoalMismatch), "{path} {opts:?}");
            counts.add(&got);
        }
        assert_eq!(nonzero(&counts), [("goal-mismatch", 3)]);
        assert_eq!((counts.files(), counts.records()), (0, 0));
        // The goal of the path: each reading joins by its exact keys.
        let path = "src/a.mjs";
        let join = join_hints(src, module, path, &table_at(path, src, &module_keys)).unwrap();
        assert_eq!((join.records(), join.no_node()), (&module_keys[..], 0));
        let path = "src/a.cjs";
        for opts in [script(), strict] {
            let join = join_hints(src, opts, path, &table_at(path, src, &script_keys)).unwrap();
            assert_eq!(
                (join.records(), join.no_node()),
                (&script_keys[..], 0),
                "{opts:?}"
            );
        }
        // The two readings differ: under the other goal (at a .js path) the 0..8 key of each
        // reading is a node of another kind (S3).
        let got = join_hints(src, script(), PATH, &table(src, &module_keys));
        let kind = NodeKind::AwaitExpression;
        assert_eq!(
            got,
            Err(JoinError::KindMismatch {
                start: 0,
                end: 8,
                kind
            })
        );
        let got = join_hints(src, module, PATH, &table(src, &script_keys));
        let kind = NodeKind::CallExpression;
        assert_eq!(
            got,
            Err(JoinError::KindMismatch {
                start: 0,
                end: 8,
                kind
            })
        );
        // Limit (H37 item 3): a .js path fixes no goal, so the root record alone joins under
        // both goals. A check needs the goal of the producer in the `.ep9h` file entry.
        for opts in [script(), module] {
            let join = join_hints(src, opts, PATH, &table(src, &root)).unwrap();
            assert_eq!(join.records(), &root, "{opts:?}");
        }
    }

    /// Codex A1H-JOIN r2: a key with ':', '?' or '#' (a scheme, a drive, a query or a
    /// fragment) can name a TS resource, so it is not a JS path, also with a `.js`, `.mjs` or
    /// `.cjs` suffix: `TsSource` and no parse, each counted as `ts-source`.
    #[test]
    fn a_url_like_key_is_not_a_js_path() {
        // The TS hazard of r1 F1: the record of the TS addition joins the JS comparison.
        let src = "import{f}from\"./g\";f<number>(1)+2";
        let records = [rec(19, 33, NodeKind::BinaryExpression)];
        let module = SourceOpts::module();
        let mut counts = JoinCounts::default();
        for path in [
            "file:///src/a.ts?cache=x.js",
            "src/a.ts?x.js",
            "src/a.ts#x.mjs",
            "file:src/a.cjs",
        ] {
            let got = join_hints(src, module, path, &table_at(path, src, &records));
            assert_eq!(got, Err(JoinError::TsSource), "{path}");
            counts.add(&got);
        }
        assert_eq!(nonzero(&counts), [("ts-source", 4)]);
        assert_eq!((counts.files(), counts.records()), (0, 0));
        // A plain .js path with the same table joins (the r1 F1 test).
        let join = join_hints(src, module, PATH, &table(src, &records)).unwrap();
        assert_eq!(join.records(), &records);
    }

    /// Codex A1H-JOIN r1 F2 (S3): a kind mismatch is a record on the span of any AST node of
    /// another kind, also one with no `NodeKind`. `true` is a BooleanLiteral (no `NodeKind`):
    /// a NumericLiteral record on its span drops every hint of the file, also its valid
    /// sibling record. A span that no AST node has stays a `no-node` drop of the record alone.
    #[test]
    fn a_record_on_a_node_with_no_node_kind_drops_the_file() {
        let src = "let a = true;";
        // Keys by hand: `a` is 4..5, `true` 8..12.
        assert_eq!((&src[4..5], &src[8..12]), ("a", "true"));
        let records = [
            rec(4, 5, NodeKind::BindingIdentifier),
            rec(8, 12, NodeKind::NumericLiteral),
        ];
        let got = join_hints(src, script(), PATH, &table(src, &records));
        assert_eq!(
            got,
            Err(JoinError::KindMismatch {
                start: 8,
                end: 12,
                kind: NodeKind::NumericLiteral
            })
        );
        let mut counts = JoinCounts::default();
        counts.add(&got);
        assert_eq!(nonzero(&counts), [("kind-mismatch", 1)]);
        // The valid record alone joins.
        let join = join_hints(src, script(), PATH, &table(src, &records[..1])).unwrap();
        assert_eq!((join.records(), join.no_node()), (&records[..1], 0));
        // 9..12 (`rue`) is no AST node: that record is dropped alone.
        let part = [records[0], rec(9, 12, NodeKind::NumericLiteral)];
        let join = join_hints(src, script(), PATH, &table(src, &part)).unwrap();
        assert_eq!((join.records(), join.no_node()), (&part[..1], 1));
        // The statement `let a = true;` (0..13) and the Program have no `NodeKind` either.
        let whole = [rec(0, 13, NodeKind::VariableDeclarator)];
        let got = join_hints(src, script(), PATH, &table(src, &whole));
        assert!(
            matches!(got, Err(JoinError::KindMismatch { .. })),
            "{got:?}"
        );
    }

    /// S3 in `ep9_hints`: another path, type errors, another text: no parse, one reason each.
    #[test]
    fn the_file_drops_of_ep9_hints_are_kept() {
        let src = "f(1);";
        let hints = table(src, &[rec(0, 4, NodeKind::CallExpression)]);
        let got = join_hints(src, script(), "src/b.ts", &hints);
        assert_eq!(got, Err(JoinError::Hints(DropReason::NotInTable)));
        let got = join_hints("f(2);", script(), PATH, &hints);
        assert_eq!(got, Err(JoinError::Hints(DropReason::HashMismatch)));
        let mut typed = hints.clone();
        typed.files[0].has_type_errors = true;
        let got = join_hints(src, script(), PATH, &typed);
        assert_eq!(got, Err(JoinError::Hints(DropReason::TypeErrors)));
        // A dropped file is not parsed: a text that does not compile gives the drop.
        let got = join_hints("f(", script(), PATH, &hints);
        assert_eq!(got, Err(JoinError::Hints(DropReason::HashMismatch)));
        assert_eq!(
            join_hints(src, script(), PATH, &hints)
                .unwrap()
                .records()
                .len(),
            1
        );
    }

    /// The front end decides first: a source that does not compile gives its error, no hints.
    #[test]
    fn a_source_that_does_not_compile_gives_its_error() {
        let src = "let = ;";
        let got = join_hints(src, script(), PATH, &table(src, &[]));
        assert!(
            matches!(got, Err(JoinError::Compile(CompileError::Syntax(_)))),
            "{got:?}"
        );
        let deep = format!(
            "{}0{}",
            "[".repeat(AST_CAP as usize),
            "]".repeat(AST_CAP as usize)
        );
        let got = join_hints(&deep, script(), PATH, &table(&deep, &[]));
        let Err(JoinError::Compile(CompileError::Range(e))) = got else {
            panic!("{got:?}")
        };
        assert_eq!(e.kind, RangeErrorKind::TooDeep(DepthLayer::Ast));
    }

    /// S5 and the counts: a header mismatch drops the whole input at the read; every reason has
    /// a row with a stable name.
    #[test]
    fn every_drop_reason_is_counted() {
        let src = "let a = b;";
        let hints = table(src, &[rec(4, 5, NodeKind::BindingIdentifier)]);
        let mut counts = JoinCounts::default();
        let bytes = ep9_hints::write(&hints).unwrap();
        let other = Expect {
            fact_schema_version: 2,
            ..Expect::default()
        };
        let Err(read) = ep9_hints::read(&bytes, &other) else {
            panic!("another fact-schema version reads")
        };
        assert_eq!(read, DropReason::FactSchemaVersion);
        counts.add_read_drop(read);
        for (text, path) in [(src, PATH), (src, "x.ts"), ("let a = c;", PATH)] {
            counts.add(&join_hints(text, script(), path, &hints));
        }
        let bad = table(src, &[rec(4, 5, NodeKind::IdentifierReference)]);
        counts.add(&join_hints(src, script(), PATH, &bad));
        let none = table("let = ;", &[]);
        counts.add(&join_hints("let = ;", script(), PATH, &none));
        let lost = table(src, &[rec(0, 1, NodeKind::StringLiteral)]);
        counts.add(&join_hints(src, script(), PATH, &lost));
        let mjs = table_at("x.mjs", src, &[rec(4, 5, NodeKind::BindingIdentifier)]);
        counts.add(&join_hints(src, script(), "x.mjs", &mjs));
        let rows: BTreeMap<&str, u64> = counts.per_reason().into_iter().collect();
        assert_eq!(rows.len(), DropReason::ALL.len() + 5);
        assert_eq!(
            nonzero(&counts),
            [
                ("fact-schema-version", 1),
                ("goal-mismatch", 1),
                ("hash-mismatch", 1),
                ("kind-mismatch", 1),
                ("no-node", 1),
                ("not-compiled", 1),
                ("not-in-table", 1),
            ]
        );
        assert_eq!((counts.files(), counts.records()), (2, 1));
    }

    /// S2 and MC-w49-6: one BOM is stripped before the hash and the offsets; both sides hash the
    /// decoded UTF-8 bytes, and the offsets are UTF-8 bytes, not UTF-16 units (V8 821506fed4 has
    /// the BOM rule).
    #[test]
    fn a_bom_and_non_ascii_text_join_on_utf8_offsets() {
        let source = "\u{FEFF}let \u{e9} = '\u{20ac}\u{1d7d8}';";
        let decoded = "let \u{e9} = '\u{20ac}\u{1d7d8}';";
        assert_eq!(decoded_text(source), decoded);
        assert_eq!(
            text_hash(source),
            ep9_bytecode::sha256::sha256(decoded.as_bytes())
        );
        assert_ne!(
            text_hash(source),
            ep9_bytecode::sha256::sha256(source.as_bytes())
        );
        // UTF-8: e-acute is 2 bytes, the euro sign 3, the math digit 4.
        let records = [
            rec(4, 6, NodeKind::BindingIdentifier),
            rec(4, 18, NodeKind::VariableDeclarator),
            rec(9, 18, NodeKind::StringLiteral),
        ];
        let hints = table(decoded, &records);
        assert_eq!(hints.files[0].text_hash, text_hash(source));
        let join = join_hints(source, script(), PATH, &hints).unwrap();
        assert_eq!((join.records().len(), join.no_node()), (3, 0));
        // The same nodes in UTF-16 units: no node has those spans.
        let utf16 = [
            rec(4, 5, NodeKind::BindingIdentifier),
            rec(8, 13, NodeKind::StringLiteral),
        ];
        let join = join_hints(source, script(), PATH, &table(decoded, &utf16)).unwrap();
        assert_eq!((join.records().len(), join.no_node()), (0, 2));
        // An engine string form (UTF-16 LE bytes) is another hash.
        let mut wide = hints.clone();
        wide.files[0].text_hash = ep9_bytecode::sha256::sha256(
            &decoded
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<u8>>(),
        );
        let got = join_hints(source, script(), PATH, &wide);
        assert_eq!(got, Err(JoinError::Hints(DropReason::HashMismatch)));
        // Only one BOM is stripped: a second U+FEFF is text (whitespace to oxc), at offset 0.
        let twice = "\u{FEFF}\u{FEFF}x;";
        assert_eq!(decoded_text(twice), "\u{FEFF}x;");
        let hints = table(
            decoded_text(twice),
            &[rec(3, 4, NodeKind::IdentifierReference)],
        );
        let join = join_hints(twice, script(), PATH, &hints).unwrap();
        assert_eq!(join.records().len(), 1);
    }

    /// S4: a JSDoc cast keys on its ParenthesizedExpression; the inner node has its own key.
    #[test]
    fn a_jsdoc_cast_keys_on_the_parentheses() {
        let src = "/** @type {X} */ (a.b);";
        let records = [
            rec(17, 22, NodeKind::ParenthesizedExpression),
            rec(18, 19, NodeKind::IdentifierReference),
            rec(18, 21, NodeKind::StaticMemberExpression),
        ];
        let join = join_hints(src, script(), PATH, &table(src, &records)).unwrap();
        assert_eq!(join.records().len(), 3);
    }

    /// The strict directive that `force_strict` adds has an empty span: it is no node.
    #[test]
    fn the_strict_directive_has_no_key() {
        let src = "0;";
        let strict = SourceOpts::script(Strictness::Strict);
        let join = join_hints(src, strict, PATH, &table(src, &[])).unwrap();
        assert_eq!(join.nodes(NodeKind::StringLiteral), 0);
        assert_eq!(join.nodes(NodeKind::NumericLiteral), 1);
        let module = join_hints(src, SourceOpts::module(), PATH, &table(src, &[])).unwrap();
        assert_eq!(module.nodes(NodeKind::NumericLiteral), 1);
    }

    /// Every `NodeKind` code (1-30) has a `visit_` point: one TS text holds each kind.
    #[test]
    fn every_node_kind_has_a_visit_point() {
        let src = "class C extends B { #p = 1; f = this.#p; m(x) { return x; } }\n\
                   function g(a) { return a; }\n\
                   const o = { k: [1, \"s\", `t${o}`] };\n\
                   let v = (o.k[0] + 1) && !g(2) ? new C() : (v = 3);\n\
                   v++;\n\
                   async function h() { await h(); }\n\
                   const r = (o as any)!;\n\
                   const q = () => 0;\n";
        let seen: BTreeSet<u16> = ts_nodes(src).into_iter().map(|k| k.2).collect();
        for kind in NodeKind::ALL {
            assert!(seen.contains(&kind.code()), "no node of {kind:?}");
        }
    }

    /// The key forms (the DEPS-BUMP-3 PropertyKey list: "a", 1, 1.5, 1e21, 0x10, 1_000, .5,
    /// 1n, 0x1n) in an object literal, a getter, a class field, a class method and an object
    /// pattern, at oxc 0.158.0: a string or number key is a literal node; a BigInt key and a
    /// name key give no `NodeKind` key, only an `ANY_NODE` key (a record there is a kind
    /// mismatch, S3); a computed key gives its expression's nodes. An oxc change of the key
    /// forms changes this list (or fails to compile in `visit_property_key`).
    #[test]
    fn the_key_forms_at_the_pinned_oxc() {
        let src = "({\"a\": v, 1: v, 1.5: v, 1e21: v, 0x10: v, 1_000: v, .5: v, 1n: v, 0x1n: v, \
                   b: v, [c]: v, get 2() { return v; }});\n\
                   class K { \"a\" = v; 1 = v; 1n = v; d = v; #e = v; [c] = v; 3() {} get 4() { return v; } }\n\
                   ({\"a\": u, 5: w, 1n: y, z} = o);\n";
        let nodes = ts_nodes(src);
        let text = |k: &Key| &src[k.0 as usize..k.1 as usize];
        let literals: Vec<&str> = nodes
            .iter()
            .filter(|k| {
                k.2 == NodeKind::NumericLiteral.code() || k.2 == NodeKind::StringLiteral.code()
            })
            .map(text)
            .collect();
        assert_eq!(
            literals,
            [
                "\"a\"", "1", "1.5", "1e21", "0x10", "1_000", ".5", "2", "\"a\"", "1", "3", "4",
                "\"a\"", "5"
            ]
        );
        let refs: Vec<&str> = nodes
            .iter()
            .filter(|k| k.2 == NodeKind::IdentifierReference.code())
            .map(text)
            .filter(|t| *t != "v")
            .collect();
        assert_eq!(refs, ["c", "c", "u", "w", "y", "z", "o"]);
        // No `NodeKind` key covers a BigInt or name key; its node has the `ANY_NODE` key.
        for key in ["1n", "0x1n", "b", "d", "#e"] {
            let at = u32::try_from(
                src.find(&format!("{key}:"))
                    .or(src.find(&format!("{key} =")))
                    .unwrap(),
            )
            .unwrap();
            assert!(
                !nodes
                    .iter()
                    .any(|k| k.0 == at && text(k) == key && k.2 != ANY_NODE),
                "a NodeKind key on the key {key}"
            );
            assert!(
                nodes.contains(&(at, at + u32::try_from(key.len()).unwrap(), ANY_NODE)),
                "no ANY_NODE key on the key {key}"
            );
        }
    }

    /// The gated `visit_` methods of a source text: a body that starts with the descent check.
    fn gated(text: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = text;
        while let Some(at) = rest.find("fn visit_") {
            let tail = &rest[at + 3..];
            let name = &tail[..tail.find('(').unwrap()];
            if let Some(open) = tail.find(") {")
                && tail[open + 3..]
                    .trim_start()
                    .starts_with("if self.may_descend()")
            {
                out.insert(name.to_owned());
            }
            rest = &tail[name.len()..];
        }
        out
    }

    /// After a stop the walk unwinds: it gates the 11 points of the depth pass, whose set
    /// `depth_tests::the_depth_walk_gates_every_visitor_cycle` proves on the pinned oxc source.
    #[test]
    fn the_walk_gates_the_points_of_the_depth_pass() {
        let depth = gated(include_str!("depth.rs"));
        assert_eq!(depth.len(), 11, "{depth:?}");
        assert_eq!(gated(include_str!("hints.rs")), depth);
    }

    /// Layer 1: with a limit 32 KiB below the walk (as on a thread smaller than the budget), a
    /// deep nest stops the walk at its backstop; with the limit of the real worker it does not.
    #[test]
    fn the_walk_stops_at_the_stack_limit() {
        let src = format!("{}0{}", "[".repeat(3_000), "]".repeat(3_000));
        let (stopped, full) = crate::worker::run(crate::stack::WORKER_STACK_BYTES, move || {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &src, SourceType::script()).parse();
            assert!(!parsed.diagnostics.has_errors());
            let stopped = {
                let _tight = crate::l1::LimitScope::room(32 << 10);
                walk(&parsed.program).err()
            };
            (stopped, walk(&parsed.program).map(|w| w.depth))
        })
        .unwrap();
        assert!(stopped.is_some_and(|at| at > 0), "{stopped:?}");
        assert_eq!(full, Ok(3_003));
    }

    // T-J (R-COVER; MAP14 M14-P2/A1H-JOIN): the join at AST_CAP for every FORMS row (the 26
    // STACK-FORMS rows are FORMS rows) and the join of the T-HOLES rows, each in a child process
    // of this test binary: never a signal. A FORMS row at its cap gives Ok with the walk at the
    // AST depth, one level more the RangeError of its layer. A T-HOLES row gives a word of its
    // `expect` column, read for a join (`tj_join_words`); a StackLimit only where the front end
    // alone gives it, never from the join walk (Claim D). The T-HOLES sizes, except where the
    // front end rejects the text before the walk (T-HOLES runs that front end): a size whose
    // words have no Ok, and the 2,000,000 run of an abort row that has an `n_abort`.

    /// The child env var: `<job spec>`.
    const TJ_CHILD: &str = "EP9_TJ_CHILD";
    /// The T-HOLES table (`tests/stack_holes.tsv`; its header has the column rules).
    const TJ_TABLE: &str = include_str!("../tests/stack_holes.tsv");
    /// The T-HOLES full size.
    const TJ_BIG: usize = 2_000_000;
    /// The slow rows run only their hunt sizes up to this, and only in release (the T-HOLES
    /// rule: their parse time and memory grow as the square of n).
    const TJ_SLOW_MAX_N: usize = 5_000;
    /// A child may not take longer than this (the T-HOLES time-out).
    const TJ_TIME_OUT: Duration = Duration::from_secs(600);
    /// A child takes one slot per this many bytes of text (the T-HOLES rule).
    const TJ_BYTES_PER_SLOT: usize = 8 << 20;
    /// The words of a join (the T-HOLES words that the front end can give).
    const TJ_WORDS: [&str; 5] = [
        "Ok",
        "Syntax",
        "Range(PreScan)",
        "Range(Ast)",
        "Range(StackLimit)",
    ];
    const TJ_STOP: &str = "Range(StackLimit)";

    enum TjJob {
        /// A FORMS row at n levels: the word it must give and its AST depth at n.
        Form(&'static Form, usize, &'static str, u32),
        /// A T-HOLES row at n levels.
        Row {
            name: &'static str,
            goal: &'static str,
            parts: [String; 5],
            n: usize,
            allowed: BTreeSet<&'static str>,
        },
    }

    impl TjJob {
        fn spec(&self) -> String {
            match self {
                TjJob::Form(form, n, ..) => format!("form:{}:{n}", form.name),
                TjJob::Row { name, n, .. } => format!("row:{name}:{n}"),
            }
        }

        /// The text length, near enough for the slot weight (a `{i}` counts as 3 bytes, the
        /// T-HOLES rule).
        fn text_len(&self) -> usize {
            match self {
                TjJob::Form(form, n, ..) => (form.build)(16).len() * n / 16,
                TjJob::Row { parts, n, .. } => {
                    let [p, o, m, c, s] = parts;
                    p.len() + m.len() + s.len() + (o.len() + c.len()) * n
                }
            }
        }

        fn text(&self) -> String {
            match self {
                TjJob::Form(form, n, ..) => (form.build)(*n),
                TjJob::Row { parts, n, .. } => {
                    let [prefix, open, mid, close, suffix] = parts;
                    let mut s = String::new();
                    s.push_str(prefix);
                    tj_repeat(&mut s, open, *n);
                    s.push_str(mid);
                    tj_repeat(&mut s, close, *n);
                    s.push_str(suffix);
                    s
                }
            }
        }

        fn opts(&self) -> SourceOpts {
            match self {
                TjJob::Form(form, ..) if form.module => SourceOpts::module(),
                TjJob::Form(..) => script(),
                TjJob::Row { goal, .. } => match *goal {
                    "module" => SourceOpts::module(),
                    "strict" => SourceOpts::script(Strictness::Strict),
                    "script" => script(),
                    other => panic!("a goal {other}"),
                },
            }
        }
    }

    /// `part` x n; `{i}` is the level index (the T-HOLES rule).
    fn tj_repeat(s: &mut String, part: &str, n: usize) {
        match part.split_once("{i}") {
            Some((head, tail)) => {
                for i in 0..n {
                    s.push_str(head);
                    s.push_str(&i.to_string());
                    s.push_str(tail);
                }
            }
            None => s.push_str(&part.repeat(n)),
        }
    }

    /// The escapes of the table: `\n \r \t \\ \u{HEX}`.
    fn tj_unescape(field: &str) -> String {
        let mut out = String::new();
        let mut chars = field.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some('u') => {
                    assert_eq!(chars.next(), Some('{'), "{field}: a \\u escape");
                    let hex: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    let code = u32::from_str_radix(&hex, 16).expect("hex digits");
                    out.push(char::from_u32(code).expect("a scalar value"));
                }
                other => panic!("{field}: the escape \\{other:?}"),
            }
        }
        out
    }

    /// `(a, b)` with `f(n) = a * n + b`, checked on three points (the depth_tests form).
    fn tj_linear(f: impl Fn(usize) -> u32) -> (u32, u32) {
        let (d10, d11, d17) = (f(10), f(11), f(17));
        let a = d11 - d10;
        let b = d10 - 10 * a;
        assert_eq!(d17, 17 * a + b, "not linear: {d10} {d11} {d17}");
        (a, b)
    }

    /// The FORMS jobs: each form at its cap (the largest n that both layer 0 and layer 2 take)
    /// and one level more.
    fn tj_form_jobs() -> Vec<TjJob> {
        let mut jobs = Vec::new();
        for form in FORMS {
            let opts = if form.module {
                SourceOpts::module()
            } else {
                script()
            };
            let (a, b) = tj_linear(|n| {
                let src = (form.build)(n);
                let allocator = Allocator::default();
                let program =
                    crate::l1::with_limit(2 << 20, || crate::parse(&allocator, &src, opts))
                        .expect("small instances parse");
                crate::depth::check_ast_depth(&program, u32::MAX).expect("no cap")
            });
            let (ea, eb) = tj_linear(|n| {
                crate::prescan::estimate((form.build)(n).as_bytes(), form.module).depth
            });
            // No growth: that layer never binds.
            let n_ast = (AST_CAP - b).checked_div(a).unwrap_or(u32::MAX);
            let n_pre = (PRESCAN_CAP - eb).checked_div(ea).unwrap_or(u32::MAX);
            let n_cap = n_ast.min(n_pre);
            assert!(n_cap < u32::MAX, "{}: no growth", form.name);
            let above = if n_pre <= n_ast {
                "Range(PreScan)"
            } else {
                "Range(Ast)"
            };
            let n = n_cap as usize;
            jobs.push(TjJob::Form(form, n, "Ok", a * n_cap + b));
            jobs.push(TjJob::Form(form, n + 1, above, 0));
        }
        jobs
    }

    /// A compile word of the `expect` column read for a join: the join runs no scope pass and no
    /// BytecodeGen, so their results are Ok; a StackLimit may also come from them (then Ok).
    fn tj_join_words(kind: &'static str) -> &'static [&'static str] {
        match kind {
            "Ok" | "Unsupported" | "Limit" | "Range(Pass)" => &["Ok"],
            "Range(StackLimit)" => &["Range(StackLimit)", "Ok"],
            "Syntax" => &["Syntax"],
            "Range(Ast)" => &["Range(Ast)"],
            "Range(PreScan)" => &["Range(PreScan)"],
            other => panic!("an expect kind {other}"),
        }
    }

    /// The T-HOLES jobs, at the T-HOLES sizes (tests/stack_holes.rs `Row::jobs`).
    fn tj_row_jobs() -> Vec<TjJob> {
        let mut jobs: BTreeMap<(&'static str, usize), TjJob> = BTreeMap::new();
        for line in TJ_TABLE.lines().filter(|l| !l.starts_with('#')) {
            let cols: Vec<&'static str> = line.split('\t').collect();
            assert_eq!(cols.len(), 13, "a T-HOLES row has 13 columns: {line}");
            let (name, goal, path, hunt) = (cols[0], cols[2], cols[3], cols[10]);
            let parts: [String; 5] = std::array::from_fn(|i| tj_unescape(cols[4 + i]));
            let n_abort = cols[9].parse::<usize>().ok().filter(|&n| n != TJ_BIG);
            for case in cols[12].split(';') {
                let (kinds, at) = match case.split_once('@') {
                    Some((k, n)) => (k, Some(n.parse::<usize>().expect("a size"))),
                    None => (case, None),
                };
                let sizes: Vec<usize> = match (hunt, at) {
                    // Its 2,000,000 run stops in the front end (layer 1 or 2; T-HOLES runs it).
                    ("ABORT", _) => vec![n_abort.unwrap_or(TJ_BIG)],
                    ("timing", Some(n)) if n <= TJ_SLOW_MAX_N && !cfg!(debug_assertions) => {
                        vec![n]
                    }
                    ("timing", _) => Vec::new(),
                    (_, Some(n)) => vec![n],
                    (_, None) => vec![TJ_BIG],
                };
                // A row of another path joins its text as a script: any front-end word.
                let allowed: BTreeSet<&'static str> = if path == "compile" {
                    kinds.split('|').flat_map(tj_join_words).copied().collect()
                } else {
                    TJ_WORDS.into_iter().collect()
                };
                for n in sizes {
                    let job = jobs.entry((name, n)).or_insert_with(|| TjJob::Row {
                        name,
                        goal,
                        parts: parts.clone(),
                        n,
                        allowed: BTreeSet::new(),
                    });
                    if let TjJob::Row { allowed: a, .. } = job {
                        a.extend(allowed.iter().copied());
                    }
                }
            }
        }
        // A size where the walk cannot run (no Ok among its words) is T-HOLES work only.
        jobs.into_values()
            .filter(|j| matches!(j, TjJob::Row { allowed, .. } if allowed.contains("Ok")))
            .collect()
    }

    fn tj_word(result: &Result<(), CompileError>) -> String {
        match result {
            Ok(()) => "Ok".to_owned(),
            Err(CompileError::Syntax(_)) => "Syntax".to_owned(),
            Err(CompileError::Range(e)) => match e.kind {
                RangeErrorKind::TooDeep(DepthLayer::PreScan) => "Range(PreScan)".to_owned(),
                RangeErrorKind::TooDeep(DepthLayer::Ast) => "Range(Ast)".to_owned(),
                RangeErrorKind::TooDeep(DepthLayer::Pass) => "Range(Pass)".to_owned(),
                RangeErrorKind::StackLimit => "Range(StackLimit)".to_owned(),
                RangeErrorKind::StackUnavailable => "Range(StackUnavailable)".to_owned(),
            },
        }
    }

    /// The child: join the job's text with a one-record table of its hash (the record names no
    /// node, so every join is a full walk); print the word, the walk depth, and the word of the
    /// front end alone when the join gives a StackLimit.
    fn tj_run(job: &TjJob) -> String {
        let text = job.text();
        let far = rec(u32::MAX - 1, u32::MAX, NodeKind::NumericLiteral);
        let hints = table(decoded_text(&text), &[far]);
        let (word, depth) = match join_hints(&text, job.opts(), PATH, &hints) {
            Ok(join) if join.records().is_empty() && join.no_node() == 1 => {
                ("Ok".to_owned(), join.depth)
            }
            Ok(_) => ("BadJoin".to_owned(), 0),
            Err(JoinError::Compile(e)) => (tj_word(&Err(e)), 0),
            Err(e) => (format!("{e}"), 0),
        };
        let front = if word == TJ_STOP {
            tj_word(&crate::check_early_errors(&text, job.opts()))
        } else {
            "-".to_owned()
        };
        format!("{word} depth {depth} front {front}")
    }

    fn tj_allowed(job: &TjJob, line: &str) -> bool {
        let mut it = line.split(' ');
        let (Some(word), Some("depth"), Some(depth), Some("front"), Some(front)) =
            (it.next(), it.next(), it.next(), it.next(), it.next())
        else {
            return false;
        };
        match job {
            TjJob::Form(_, _, want, at) => {
                word == *want && (word != "Ok" || depth.parse::<u32>() == Ok(*at))
            }
            TjJob::Row { allowed, .. } => {
                allowed.contains(word) && (word != TJ_STOP || front == TJ_STOP)
            }
        }
    }

    #[test]
    fn t_j_the_join_never_aborts() {
        if let Ok(spec) = std::env::var(TJ_CHILD) {
            // The child builds only its own job: a form needs no fit here (the parent checks).
            let job = if let ["form", name, n] = spec.split(':').collect::<Vec<_>>()[..] {
                TjJob::Form(
                    FORMS.iter().find(|f| f.name == name).expect("a form"),
                    n.parse().expect("a size"),
                    "",
                    0,
                )
            } else {
                tj_row_jobs()
                    .into_iter()
                    .find(|j| j.spec() == spec)
                    .unwrap_or_else(|| panic!("no T-J job {spec}"))
            };
            println!("TJ {spec} -> {}", tj_run(&job));
            return;
        }
        let jobs: Vec<TjJob> = tj_form_jobs().into_iter().chain(tj_row_jobs()).collect();
        let width = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
        let used = Mutex::new(0usize);
        let free = Condvar::new();
        let next = AtomicUsize::new(0);
        let results = Mutex::new(Vec::new());
        let start = Instant::now();
        std::thread::scope(|s| {
            for _ in 0..width {
                s.spawn(|| {
                    while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let weight = (job.text_len() / TJ_BYTES_PER_SLOT + 1).min(width);
                        {
                            let mut u = used.lock().unwrap();
                            while *u + weight > width {
                                u = free.wait(u).unwrap();
                            }
                            *u += weight;
                        }
                        let spec = job.spec();
                        let begin = Instant::now();
                        let mut child =
                            std::process::Command::new(std::env::current_exe().unwrap())
                                .args([
                                    "--exact",
                                    "hints::tests::t_j_the_join_never_aborts",
                                    "--nocapture",
                                    "--test-threads=1",
                                ])
                                .env(TJ_CHILD, &spec)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::piped())
                                .stderr(std::process::Stdio::piped())
                                .spawn()
                                .unwrap();
                        let mut out = child.stdout.take().unwrap();
                        let mut err = child.stderr.take().unwrap();
                        let out = std::thread::spawn(move || {
                            let mut v = Vec::new();
                            out.read_to_end(&mut v).ok();
                            v
                        });
                        let err = std::thread::spawn(move || {
                            let mut v = Vec::new();
                            err.read_to_end(&mut v).ok();
                            v
                        });
                        let status = loop {
                            if let Some(status) = child.try_wait().unwrap() {
                                break Some(status);
                            }
                            if begin.elapsed() > TJ_TIME_OUT {
                                child.kill().ok();
                                child.wait().ok();
                                break None;
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        };
                        *used.lock().unwrap() -= weight;
                        free.notify_all();
                        let stdout = String::from_utf8_lossy(&out.join().unwrap()).into_owned();
                        let stderr = String::from_utf8_lossy(&err.join().unwrap()).into_owned();
                        // The harness prints "test ... " on the same line before the child's line.
                        let mark = format!("TJ {spec} -> ");
                        let line = stdout
                            .lines()
                            .find_map(|l| l.find(&mark).map(|at| l[at + mark.len()..].to_owned()));
                        let ok = status.is_some_and(|s| s.success())
                            && line.as_deref().is_some_and(|l| tj_allowed(job, l));
                        results.lock().unwrap().push((
                            spec,
                            status,
                            line,
                            stderr,
                            begin.elapsed(),
                            ok,
                        ));
                    }
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        results.sort_by(|a, b| a.0.cmp(&b.0));
        let mut failures = Vec::new();
        let mut words: BTreeMap<String, usize> = BTreeMap::new();
        for (spec, status, line, stderr, _, ok) in &results {
            if let Some(word) = line.as_deref().and_then(|l| l.split(' ').next()) {
                *words.entry(word.to_owned()).or_default() += 1;
            }
            if !ok {
                let tail: Vec<&str> = stderr.lines().rev().take(12).collect();
                let tail: Vec<&str> = tail.into_iter().rev().collect();
                failures.push(format!(
                    "{spec}: status {status:?}, line {line:?}\nstderr tail:\n{}",
                    tail.join("\n")
                ));
            }
        }
        let forms = results.iter().filter(|r| r.0.starts_with("form:")).count();
        let mut by_time: Vec<_> = results.iter().map(|r| (r.4, r.0.as_str())).collect();
        by_time.sort_unstable_by(|a, b| b.cmp(a));
        let slowest: Vec<String> = by_time
            .iter()
            .take(6)
            .map(|(t, spec)| format!("{spec} {:.1} s", t.as_secs_f64()))
            .collect();
        println!(
            "T-J: {} children ({forms} FORMS jobs, {} T-HOLES jobs) in {:.1} s, {width} at a time, \
             slowest {}; words: {words:?}; {} failed",
            results.len(),
            results.len() - forms,
            start.elapsed().as_secs_f64(),
            slowest.join(", "),
            failures.len()
        );
        assert!(
            failures.is_empty(),
            "{} of {} T-J children failed:\n{}",
            failures.len(),
            results.len(),
            failures.join("\n\n")
        );
    }
}
