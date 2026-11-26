//! String search and bulk copy on the string payload (H20 part 2; OXC-PERF wave 2
//! STR-SEARCH-SIMD).
//!
//! ```text
//! Cx::string_index_of       StringIndexOf (§6.1.4.1)            indexOf, includes
//! Cx::string_last_index_of  the lastIndexOf scan (§22.1.3.11)   lastIndexOf
//! Cx::push_code_units       one bulk copy of a range of units   the `units` helper
//! ```
//!
//! The two searches flatten both strings (the only GC point before the search), then read the
//! code units in place: no copy, no call per unit. The first-unit scan of a forward search
//! uses the SIMD body of the CPU (`string/search.rs`; SSE2 or AVX2 on x86-64, NEON on
//! aarch64). That body is a human fast path (FASTDIFF row `unit-search`):
//! `Cx::set_human_fast_paths(false)` makes the scan scalar. A search polls once per 256 code
//! units of work, as [`Cx::poll_interrupt`] does, and takes its views again after each poll.

use ep9_value::JsResult;

use super::{Args, Cx, FlatStr, JsString, Local};
use crate::string::{self, Dir, search};

impl<'s> Cx<'s> {
    /// StringIndexOf ( string, searchValue, fromIndex ) (§6.1.4.1) on UTF-16 code units: the
    /// first index `i >= from` at which `search` occurs in `s`, or `None` (the spec's
    /// not-found). An empty `search` gives `from` when `from <= len(s)`. Either string can be
    /// Latin-1 or UTF-16. Flattens both strings (a GC point; the only allocation), then
    /// searches their units in place. Polls once per 256 code units of work; `Err` only when
    /// the isolate terminates.
    ///
    /// ```
    /// # use ep9_vm::{Isolate, IsolateConfig};
    /// let mut iso = Isolate::new(IsolateConfig::default()).unwrap();
    /// let r = iso
    ///     .with_cx(|cx| {
    ///         let s = cx.new_string("abcabc")?;
    ///         let c = cx.new_string("ca")?;
    ///         Ok((cx.string_index_of(s, c, 0)?, cx.string_index_of(s, c, 3)?))
    ///     })
    ///     .unwrap();
    /// assert_eq!(r, (Some(2), None));
    /// ```
    pub fn string_index_of(
        &mut self,
        s: impl Into<Local<'s, JsString>>,
        search: impl Into<Local<'s, JsString>>,
        from: usize,
    ) -> JsResult<Option<usize>> {
        let (s, search) = (s.into().bits(), search.into().bits());
        let (len, n) = (string::str_len(s), string::str_len(search));
        if n == 0 {
            return Ok((from <= len).then_some(from));
        }
        if from >= len || n > len - from {
            return Ok(None);
        }
        self.search(Dir::Forward, s, search, from)
    }

    /// The scan of String.prototype.lastIndexOf (§22.1.3.11 steps 10-12) on UTF-16 code
    /// units: the last index `i <= from` at which `search` occurs in `s`, or `None`. An empty
    /// `search` gives `min(from, len(s))`. Flattens both strings (a GC point), then searches
    /// their units in place, backward. Polls once per 256 code units of work.
    ///
    /// ```
    /// # use ep9_vm::{Isolate, IsolateConfig};
    /// let mut iso = Isolate::new(IsolateConfig::default()).unwrap();
    /// let r = iso
    ///     .with_cx(|cx| {
    ///         let s = cx.new_string("abcabc")?;
    ///         let c = cx.new_string("bc")?;
    ///         Ok((cx.string_last_index_of(s, c, 9)?, cx.string_last_index_of(s, c, 3)?))
    ///     })
    ///     .unwrap();
    /// assert_eq!(r, (Some(4), Some(1)));
    /// ```
    pub fn string_last_index_of(
        &mut self,
        s: impl Into<Local<'s, JsString>>,
        search: impl Into<Local<'s, JsString>>,
        from: usize,
    ) -> JsResult<Option<usize>> {
        let (s, search) = (s.into().bits(), search.into().bits());
        let (len, n) = (string::str_len(s), string::str_len(search));
        let Some(last) = len.checked_sub(n) else {
            return Ok(None);
        };
        let start = from.min(last);
        if n == 0 {
            return Ok(Some(start));
        }
        self.search(Dir::Backward, s, search, start)
    }

    /// Appends the code units `from..to` of the flat string `s` to `out` in one bulk copy. Not
    /// a GC point. Panics on a range that is not inside `s` (`from <= to <= len(s)`).
    pub fn push_code_units(
        &self,
        s: Local<'s, FlatStr>,
        from: usize,
        to: usize,
        out: &mut Vec<u16>,
    ) {
        string::push_units(s.bits(), from, to, out);
    }

    /// The search loop of [`Cx::string_index_of`] and [`Cx::string_last_index_of`]: `search`
    /// is not empty, and `start` is a valid first candidate.
    fn search(&mut self, dir: Dir, s: u64, search: u64, start: usize) -> JsResult<Option<usize>> {
        // GC points: both strings are in the caller's Locals, and a flat form is held by its
        // CONS (V9-D13), so neither flat cell can die; cells never move (H19).
        let s = string::flatten(self.cx, s);
        let search = string::flatten(self.cx, search);
        let k = if crate::call::human_fast_paths(self.rt()) {
            search::Kernel::detect()
        } else {
            search::Kernel::scalar()
        };
        if dir == Dir::Forward && k != search::Kernel::scalar() {
            crate::call::fast_hit!(7);
        }
        let mut c = search::Cursor::new(start);
        loop {
            // Not a GC point: the step takes the views and drops them before it returns.
            match string::search_step(k, dir, s, search, &mut c) {
                search::Step::Found(i) => return Ok(Some(i)),
                search::Step::NotFound => return Ok(None),
                search::Step::Poll => self.poll_interrupt()?,
            }
        }
    }
}

/// String.prototype.charCodeAt ( pos ) (§22.1.3.3): the host function of `BuiltinId` 6.2
/// (`charCodeAt` of `%String.prototype%`). Moved from `ep9_builtins` (M15-P1 CHARCODE) with no
/// change of its steps, so that the VM can name it: the arm of `CallProperty1` in
/// `interp/slow/call.rs` (`call_property`) knows the builtin by this function pointer, not by a
/// name. The builtin table of `ep9_builtins` names it through
/// `ep9_vm::internals::STRING_CHAR_CODE_AT`. Not generic and never inlined, so the program has
/// one copy of it and one address.
#[inline(never)]
pub(crate) fn char_code_at<'s>(
    cx: &mut Cx<'s>,
    this: Local<'s>,
    args: Args<'s>,
    _new_target: Local<'s>,
) -> JsResult<Local<'s>> {
    // Fast path (FASTDIFF row `char-code-at`): a String value `this` and an int32 `pos`.
    // ToString(this) is `this` and ToIntegerOrInfinity(pos) is `pos`, so steps 1-3 run no
    // user code and the result is the same. Every other shape takes the spec steps below.
    if let (Some(s), Some(position)) = (this.as_string(), args.get(0).as_int32()) {
        let size = cx.string_len(s);
        if position < 0 || position as usize >= size {
            return Ok(Local::number(f64::NAN));
        }
        let s = cx.flatten(s)?;
        let unit = cx
            .code_unit_at(s, position as usize)
            .expect("position < size");
        return Ok(Local::int32(i32::from(unit)));
    }
    // 1. Let O be ? RequireObjectCoercible(this value).
    if this.is_undefined_or_null() {
        return Err(cx.throw_type_error("String.prototype.charCodeAt called on null or undefined"));
    }
    // 2. Let S be ? ToString(O).
    let s = cx.to_string(this)?;
    // 3. Let position be ? ToIntegerOrInfinity(pos).
    let position = cx.to_integer_or_infinity(args.get(0))?;
    // 4. Let size be the length of S.
    let size = cx.string_len(s);
    // 5. If position < 0 or position ≥ size, return NaN.
    if position < 0.0 || position >= size as f64 {
        return Ok(Local::number(f64::NAN));
    }
    // 6. Return the Number value for the numeric value of the code unit at index position
    //    within the String S.
    let s = cx.flatten(s)?;
    let unit = cx
        .code_unit_at(s, position as usize)
        .expect("position < size");
    Ok(Local::int32(i32::from(unit)))
}
