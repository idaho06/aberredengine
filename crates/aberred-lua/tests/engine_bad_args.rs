//! Calls every `engine.*` function from `engine.__meta.functions` with missing
//! and wrong-typed arguments. Each such call must be a Lua error: never a Rust
//! panic, never a silent success.
//!
//! Not swept, because mlua converts them leniently (LuaJIT build) or they need
//! their own probes:
//! - a fractional number in an `integer` slot truncates;
//! - a missing or any-valued `boolean` slot reads as Lua truthiness (`nil` is
//!   false);
//! - a numeric string is accepted in a `number`/`integer` slot, and a number in
//!   a `string` slot;
//! - optional parameters, table-like parameters and out-of-range values;
//! - the `EntityBuilder`/`CollisionEntityBuilder` methods in
//!   `engine.__meta.classes`.

use aberred_lua::resources::lua_runtime::LuaRuntime;
use mlua::prelude::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// A Lua error: the wanted outcome.
    Rejected,
    SilentSuccess,
    /// A Rust panic unwound out of the call.
    Panic,
}

/// Calls `func` with `args` and classifies the outcome.
fn probe(func: &LuaFunction, args: Vec<LuaValue>) -> Verdict {
    match catch_unwind(AssertUnwindSafe(|| {
        func.call::<LuaMultiValue>(LuaVariadic::from_iter(args))
    })) {
        Err(_) => Verdict::Panic,
        Ok(Err(_)) => Verdict::Rejected,
        Ok(Ok(_)) => Verdict::SilentSuccess,
    }
}

/// The classifier tells the three outcomes apart; the sweep depends on it.
#[test]
fn probe_tells_a_panic_a_silent_success_and_a_lua_error_apart() {
    let lua = Lua::new();
    let panics = lua
        .create_function(|_, _: LuaMultiValue| -> LuaResult<()> { panic!("boom") })
        .unwrap();
    let accepts = lua.create_function(|_, _: LuaMultiValue| Ok(())).unwrap();
    let rejects = lua
        .create_function(|_, _: LuaMultiValue| -> LuaResult<()> {
            Err(LuaError::runtime("bad argument"))
        })
        .unwrap();

    assert_eq!(probe(&panics, vec![]), Verdict::Panic);
    assert_eq!(probe(&accepts, vec![LuaValue::Nil]), Verdict::SilentSuccess);
    assert_eq!(probe(&rejects, vec![]), Verdict::Rejected);
}

/// A function `engine.__meta.functions` declares, with its required parameter
/// types (`number`, `integer`, `string`, `boolean`, or a table-like type).
struct Function {
    name: String,
    required: Vec<String>,
}

fn engine_table(lua: &Lua) -> LuaTable {
    lua.globals().get("engine").expect("engine table")
}

/// The functions `engine.__meta.functions` declares, sorted by name.
fn declared_functions(lua: &Lua) -> Vec<Function> {
    let meta: LuaTable = engine_table(lua).get("__meta").expect("engine.__meta");
    let functions: LuaTable = meta.get("functions").expect("meta.functions");
    let mut result = Vec::new();
    for pair in functions.pairs::<String, LuaTable>() {
        let (name, entry) = pair.expect("function entry");
        let params: LuaTable = entry.get("params").expect("params");
        let required = params
            .sequence_values::<LuaTable>()
            .map(|p| p.expect("param").get::<String>("type").expect("param type"))
            .filter(|ty| !ty.ends_with('?'))
            .collect();
        result.push(Function { name, required });
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// A call shape the sweep tries; every one must be rejected.
#[derive(Debug, Clone, Copy)]
enum Case {
    /// Only the arguments before this required slot.
    Missing(usize),
    /// A table in this `number`/`integer`/`string` slot (never coerced).
    WrongType(usize),
}

fn cases(function: &Function) -> Vec<Case> {
    let mut cases = Vec::new();
    for (slot, ty) in function.required.iter().enumerate() {
        if ty != "boolean" {
            cases.push(Case::Missing(slot));
        }
        if matches!(ty.as_str(), "number" | "integer" | "string") {
            cases.push(Case::WrongType(slot));
        }
    }
    cases
}

/// A valid-looking argument for a slot of type `ty`.
fn baseline(lua: &Lua, ty: &str) -> LuaValue {
    match ty {
        "number" => LuaValue::Number(1.5),
        "integer" => LuaValue::Integer(1),
        "string" => LuaValue::String(lua.create_string("x").expect("string")),
        "boolean" => LuaValue::Boolean(true),
        _ => LuaValue::Table(lua.create_table().expect("table")),
    }
}

/// The arguments `case` calls `function` with.
fn arguments(lua: &Lua, function: &Function, case: Case) -> Vec<LuaValue> {
    let mut args: Vec<LuaValue> = function
        .required
        .iter()
        .map(|ty| baseline(lua, ty))
        .collect();
    match case {
        Case::Missing(slot) => args.truncate(slot),
        Case::WrongType(slot) => args[slot] = baseline(lua, "table"),
    }
    args
}

/// One line per call that is not a Lua error. One runtime serves a function's
/// calls (they only queue commands); a panic gets the next call a fresh one.
fn sweep() -> Vec<String> {
    let mut findings = Vec::new();
    for function in declared_functions(LuaRuntime::new().expect("runtime").lua()) {
        let mut runtime = LuaRuntime::new().expect("runtime");
        for case in cases(&function) {
            let lua = runtime.lua();
            let func: LuaFunction = engine_table(lua)
                .get(function.name.as_str())
                .expect("engine function");
            let verdict = probe(&func, arguments(lua, &function, case));
            if verdict == Verdict::Panic {
                runtime = LuaRuntime::new().expect("runtime");
            }
            if verdict != Verdict::Rejected {
                findings.push(format!("engine.{} {case:?}: {verdict:?}", function.name));
            }
        }
    }
    findings
}

#[test]
fn every_engine_function_rejects_missing_and_wrong_typed_arguments() {
    let findings = sweep();
    assert!(
        findings.is_empty(),
        "{} calls were not a Lua error:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

/// The sweep only covers what the metadata declares: a function registered
/// without metadata would escape it (and the generated stubs).
#[test]
fn every_engine_function_is_declared_in_the_metadata() {
    let runtime = LuaRuntime::new().expect("runtime");
    let declared: Vec<String> = declared_functions(runtime.lua())
        .into_iter()
        .map(|f| f.name)
        .collect();
    let undeclared: Vec<String> = engine_table(runtime.lua())
        .pairs::<String, LuaValue>()
        .map(|pair| pair.expect("engine entry"))
        .filter(|(name, value)| value.is_function() && !declared.contains(name))
        .map(|(name, _)| format!("engine.{name}"))
        .collect();

    assert!(undeclared.is_empty(), "no metadata for: {undeclared:?}");
}

/// A guard against the sweep silently covering (almost) nothing.
#[test]
fn the_sweep_covers_the_engine_api() {
    let functions = declared_functions(LuaRuntime::new().expect("runtime").lua());
    assert!(functions.len() > 200, "{} functions", functions.len());
    let calls: usize = functions.iter().map(|f| cases(f).len()).sum();
    assert!(calls > 500, "{calls} calls");
}
