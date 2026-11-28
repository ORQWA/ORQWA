//! A world's script runs in a sandbox and stays readable for good (R302–R304). Modules are
//! written as WebAssembly text here; the runtime reads binaries, and tests build with the text
//! parser.

use std::path::Path;

use orqwa_world_api::{Input, Output};
use serde_json::{Value, json};

use super::*;

/// A module whose `tick` runs `body` and answers `{}` from address 0.
fn module(body: &str) -> String {
    module_with("", body)
}

fn module_with(extra: &str, body: &str) -> String {
    format!(
        r#"(module
          {extra}
          (memory (export "memory") 2)
          (data (i32.const 0) "{{}}")
          (func (export "alloc") (param i32) (result i32) (i32.const 1024))
          (func (export "tick") (param i32 i32) (result i64)
            {body}
            (i64.shl (i64.const 2) (i64.const 32))))"#
    )
}

fn input() -> Input {
    Input {
        api: orqwa_world_api::API,
        ..Input::default()
    }
}

fn run(wat: &str) -> Result<Output, ScriptError> {
    WorldScript::load(wat.as_bytes(), 1)?.tick(&input())
}

#[test]
fn a_module_that_keeps_to_the_rules_runs() {
    assert_eq!(run(&module("")).unwrap(), Output::default());
    assert!(check(module("").as_bytes()).is_ok());
}

#[test]
fn a_module_reaches_nothing_but_the_log() {
    for import in [
        r#"(import "env" "now" (func (result i64)))"#,
        r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
        r#"(import "orqwa_v1" "log" (func (param i64)))"#,
        r#"(import "orqwa_v1" "fetch" (func (param i32 i32)))"#,
        r#"(import "orqwa_v1" "memory" (memory 1))"#,
    ] {
        let error = run(&module_with(import, "")).unwrap_err();
        assert!(
            matches!(error, ScriptError::Refused(_)),
            "{import}: {error}"
        );
    }
    let logging = module_with(
        r#"(import "orqwa_v1" "log" (func $log (param i32 i32)))"#,
        "(call $log (i32.const 0) (i32.const 2))",
    );
    assert!(run(&logging).is_ok());
}

#[test]
fn only_the_pinned_webassembly_features_are_accepted() {
    // SIMD, memory64, tail calls and several memories are outside the set.
    for wat in [
        module("(drop (v128.const i64x2 0 0))"),
        r#"(module (memory (export "memory") i64 1)
             (func (export "alloc") (param i32) (result i32) (i32.const 0))
             (func (export "tick") (param i32 i32) (result i64) (i64.const 0)))"#
            .to_owned(),
        module_with("(func $f (result i64) (return_call $f))", ""),
        module_with("(memory $second 1)", ""),
    ] {
        assert!(matches!(run(&wat), Err(ScriptError::Refused(_))), "{wat}");
    }
    // Bulk memory, sign extension and saturating conversions are in it.
    let accepted = module(
        "(memory.fill (i32.const 8) (i32.const 0) (i32.const 4))
         (drop (i32.extend8_s (i32.const 255)))
         (drop (i32.trunc_sat_f32_s (f32.const 1e10)))",
    );
    assert!(run(&accepted).is_ok());
}

#[test]
fn a_module_without_its_exports_or_with_a_start_function_is_refused() {
    let no_tick = r#"(module (memory (export "memory") 1)
        (func (export "alloc") (param i32) (result i32) (i32.const 0)))"#;
    assert!(matches!(run(no_tick), Err(ScriptError::Refused(_))));
    let wrong_tick = r#"(module (memory (export "memory") 1)
        (func (export "alloc") (param i32) (result i32) (i32.const 0))
        (func (export "tick") (param i32 i32) (result i32) (i32.const 0)))"#;
    assert!(matches!(run(wrong_tick), Err(ScriptError::Refused(_))));
    let start = module_with("(func $go) (start $go)", "");
    assert!(matches!(run(&start), Err(ScriptError::Refused(_))));
    assert!(matches!(
        WorldScript::load(b"not a module", 1),
        Err(ScriptError::Refused(_))
    ));
}

#[test]
fn a_script_for_an_api_this_orqwa_does_not_have_is_refused() {
    assert!(WorldScript::load(module("").as_bytes(), 0).is_err());
    assert!(WorldScript::load(module("").as_bytes(), orqwa_world_api::API + 1).is_err());
}

#[test]
fn a_module_that_traps_or_runs_too_long_is_stopped_for_the_ride() {
    let mut script = WorldScript::load(module("(unreachable)").as_bytes(), 1).unwrap();
    assert!(matches!(
        script.tick(&input()),
        Err(ScriptError::Trapped(_))
    ));
    assert!(script.stopped());
    assert_eq!(script.tick(&input()), Err(ScriptError::Stopped));

    let forever = module("(loop $again (br $again))");
    assert_eq!(run(&forever), Err(ScriptError::OutOfFuel));
}

#[test]
fn a_module_cannot_grow_beyond_its_memory() {
    // 300 pages are more than 16 MiB: growing fails, and this module then traps.
    let greedy =
        module("(if (i32.eq (memory.grow (i32.const 300)) (i32.const -1)) (then (unreachable)))");
    assert!(matches!(run(&greedy), Err(ScriptError::Trapped(_))));
    let modest = module("(drop (memory.grow (i32.const 100)))");
    assert!(run(&modest).is_ok());
}

#[test]
fn output_that_is_not_as_described_stops_the_script() {
    let answer = |data: &str, address: u32, length: u64| {
        format!(
            r#"(module (memory (export "memory") 2)
                 (data (i32.const 0) "{data}")
                 (func (export "alloc") (param i32) (result i32) (i32.const 1024))
                 (func (export "tick") (param i32 i32) (result i64)
                   (i64.or (i64.const {address}) (i64.shl (i64.const {length}) (i64.const 32)))))"#
        )
    };
    for (wat, why) in [
        (answer("not json", 0, 8), "not JSON"),
        (answer(r#"{\"bodies\": 3}"#, 0, 13), "of the wrong shape"),
        (answer("{}", 200_000, 2), "outside memory"),
        (answer("{}", 0, 70_000), "too long"),
    ] {
        assert!(
            matches!(run(&wat), Err(ScriptError::BadOutput(_))),
            "output {why}"
        );
    }
}

/// A frozen module and the outputs it gave for its inputs, which every later ORQWA must
/// reproduce (R304).
fn golden(name: &str) -> (WorldScript, Vec<Value>) {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let wat = std::fs::read(folder.join(format!("{name}.wat"))).unwrap();
    let trace: Value =
        serde_json::from_slice(&std::fs::read(folder.join(format!("{name}.json"))).unwrap())
            .unwrap();
    let api = u32::try_from(trace["api"].as_u64().unwrap()).unwrap();
    let script = WorldScript::load(&wat, api).unwrap();
    (script, trace["ticks"].as_array().unwrap().clone())
}

#[test]
fn frozen_modules_give_the_outputs_they_always_gave() {
    for name in ["fixed", "counter", "fallbacks"] {
        let (mut script, ticks) = golden(name);
        for (at, tick) in ticks.iter().enumerate() {
            let input: Input = serde_json::from_value(tick["input"].clone()).unwrap();
            let expected: Output = serde_json::from_value(tick["output"].clone()).unwrap();
            let output = script.tick(&input).unwrap();
            assert_eq!(output, expected, "{name}, tick {at}");
            if let Some(resolved) = tick.get("resolved") {
                let resolved: Output = serde_json::from_value(resolved.clone()).unwrap();
                assert_eq!(output.resolved(), resolved, "{name}, tick {at}, resolved");
            }
        }
    }
}

#[test]
fn a_module_is_handed_its_state_and_its_input_whole() {
    // Answering with its own input: the state is the one it gave, unknown fields are ignored.
    let echo = r#"(module (memory (export "memory") 4)
        (func (export "alloc") (param i32) (result i32) (i32.const 0))
        (func (export "tick") (param $at i32) (param $len i32) (result i64)
          (i64.or (i64.extend_i32_u (local.get $at))
                  (i64.shl (i64.extend_i32_u (local.get $len)) (i64.const 32)))))"#;
    let mut script = WorldScript::load(echo.as_bytes(), 1).unwrap();
    let ride = Input {
        state: json!("ignored: ORQWA hands back the script's own"),
        config: json!({"laps": 3}),
        ..input()
    };
    assert_eq!(script.tick(&ride).unwrap().state, Value::Null);
}
