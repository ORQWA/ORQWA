//! Runs a world's script (ADR 0044, `docs/world-files.md`, "`ORQWA_script`"): a WebAssembly
//! module ORQWA hands the ride's state each tick and asks what it wants, in a sandbox.
//!
//! The module reaches nothing but what this crate gives it: one function, `orqwa_v1.log`
//! (R303). Each tick has a fuel budget, memory and documents are capped, and a module that
//! traps, runs out of fuel or answers with something unreadable is stopped for the rest of the
//! ride; the ride then goes on as if the world had no script. Every API version up to
//! [`orqwa_world_api::API`] stays supported for good (R304).

use orqwa_world_api::{Input, Output};
use thiserror::Error;
use wasmi::{
    Caller, Config, Engine, ExternType, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TrapCode, TypedFunc, ValType,
};

/// The only import module a script may use, and every function it has. Later API versions add
/// further modules (`orqwa_v2`, …) beside it; this one never changes.
pub const IMPORT_MODULE: &str = "orqwa_v1";
/// The most a module may grow its memory to.
pub const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;
/// The largest output a tick may return.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// The fuel a tick may use: roughly one per instruction.
pub const FUEL_PER_TICK: u64 = 20_000_000;
/// The first tick also reads the route, and may set itself up.
pub const FIRST_TICK_FUEL: u64 = 5 * FUEL_PER_TICK;
/// How many log lines a tick may write, and how long each may be.
const MAX_LOG_LINES: usize = 16;
const MAX_LOG_BYTES: usize = 1024;

/// What can go wrong with a world's script.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// The module breaks the rules: it is not a module, uses a WebAssembly feature outside the
    /// accepted set, imports something it may not, or lacks an export.
    #[error("the world's script is not allowed: {0}")]
    Refused(String),
    /// The module trapped.
    #[error("the world's script stopped: {0}")]
    Trapped(String),
    /// The module used its tick's fuel.
    #[error("the world's script took too long")]
    OutOfFuel,
    /// The module's output is not as the format describes.
    #[error("the world's script answered with something ORQWA cannot read: {0}")]
    BadOutput(String),
    /// The module stopped earlier in the ride.
    #[error("the world's script has stopped")]
    Stopped,
}

fn refused(message: impl Into<String>) -> ScriptError {
    ScriptError::Refused(message.into())
}

/// The engine every script runs on: an interpreter with fuel, accepting exactly the pinned
/// WebAssembly features (`docs/world-files.md`). The set only ever grows (R304).
fn engine() -> Engine {
    let mut config = Config::default();
    config
        .consume_fuel(true)
        .allow_start_fn(false)
        .wasm_mutable_global(true)
        .wasm_sign_extension(true)
        .wasm_saturating_float_to_int(true)
        .wasm_bulk_memory(true)
        .wasm_multi_value(true)
        .wasm_reference_types(true)
        .wasm_multi_memory(false)
        .wasm_tail_call(false)
        .wasm_extended_const(false)
        .wasm_custom_page_sizes(false)
        .wasm_wide_arithmetic(false);
    Engine::new(&config)
}

/// Checks `wasm` as a world script: valid under the pinned features, importing only what it
/// may, and exporting what ORQWA calls. For checking a world before it is ridden.
///
/// # Errors
/// [`ScriptError::Refused`] naming what is wrong.
pub fn check(wasm: &[u8]) -> Result<(), ScriptError> {
    compile(&engine(), wasm).map(|_| ())
}

fn compile(engine: &Engine, wasm: &[u8]) -> Result<Module, ScriptError> {
    let module = Module::new(engine, wasm).map_err(|e| refused(e.to_string()))?;
    for import in module.imports() {
        let log = import.module() == IMPORT_MODULE
            && import.name() == "log"
            && matches!(import.ty(), ExternType::Func(ty)
                if ty.params() == [ValType::I32, ValType::I32] && ty.results().is_empty());
        if !log {
            return Err(refused(format!(
                "it imports {}.{}, which ORQWA does not offer",
                import.module(),
                import.name()
            )));
        }
    }
    let export = |name: &str| module.exports().find(|e| e.name() == name).map(|e| e.ty().clone());
    let function = |name: &str, params: &[ValType], results: &[ValType]| {
        matches!(export(name), Some(ExternType::Func(ty))
            if ty.params() == params && ty.results() == results)
    };
    if !matches!(export("memory"), Some(ExternType::Memory(_)))
        || !function("alloc", &[ValType::I32], &[ValType::I32])
        || !function("tick", &[ValType::I32, ValType::I32], &[ValType::I64])
    {
        return Err(refused(
            "it does not export memory, alloc(i32) -> i32 and tick(i32, i32) -> i64",
        ));
    }
    Ok(module)
}

/// What the host keeps for a module.
struct Host {
    limits: StoreLimits,
    log: Vec<String>,
}

/// A world's script, running.
pub struct WorldScript {
    store: Store<Host>,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    tick: TypedFunc<(i32, i32), i64>,
    state: serde_json::Value,
    ticks: u64,
    stopped: bool,
}

impl std::fmt::Debug for WorldScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldScript")
            .field("ticks", &self.ticks)
            .field("stopped", &self.stopped)
            .finish_non_exhaustive()
    }
}

impl WorldScript {
    /// Loads `wasm`, a script for `api`.
    ///
    /// # Errors
    /// [`ScriptError::Refused`] if the module breaks the rules or needs a newer API.
    pub fn load(wasm: &[u8], api: u32) -> Result<Self, ScriptError> {
        if api == 0 || api > orqwa_world_api::API {
            return Err(refused(format!("it needs script API {api}")));
        }
        let engine = engine();
        let module = compile(&engine, wasm)?;
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY_BYTES)
            .memories(1)
            .tables(4)
            .table_elements(100_000)
            .instances(1)
            .build();
        let mut store = Store::new(
            &engine,
            Host {
                limits,
                log: Vec::new(),
            },
        );
        store.limiter(|host| &mut host.limits);
        let mut linker = Linker::<Host>::new(&engine);
        linker
            .func_wrap(IMPORT_MODULE, "log", log)
            .map_err(|e| refused(e.to_string()))?;
        // Setting up memory and globals costs fuel too.
        store
            .set_fuel(FUEL_PER_TICK)
            .map_err(|e| refused(e.to_string()))?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| refused(e.to_string()))?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| refused("no memory"))?;
        let alloc = instance
            .get_typed_func(&store, "alloc")
            .map_err(|e| refused(e.to_string()))?;
        let tick = instance
            .get_typed_func(&store, "tick")
            .map_err(|e| refused(e.to_string()))?;
        Ok(Self {
            store,
            memory,
            alloc,
            tick,
            state: serde_json::Value::Null,
            ticks: 0,
            stopped: false,
        })
    }

    /// Whether the script has stopped for the rest of the ride.
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// One tick: hands the module `input`, with the state it returned last, and returns its
    /// output as written. Any error stops the script for good; later ticks return
    /// [`ScriptError::Stopped`].
    ///
    /// # Errors
    /// [`ScriptError`] if the module traps, runs out of fuel or answers unreadably.
    pub fn tick(&mut self, input: &Input) -> Result<Output, ScriptError> {
        if self.stopped {
            return Err(ScriptError::Stopped);
        }
        let result = self.run(input);
        for line in self.store.data_mut().log.drain(..) {
            tracing::info!(target: "orqwa_world_script", "{line}");
        }
        if result.is_err() {
            self.stopped = true;
        }
        result
    }

    fn run(&mut self, input: &Input) -> Result<Output, ScriptError> {
        let input = Input {
            state: self.state.clone(),
            ..input.clone()
        };
        let bytes = serde_json::to_vec(&input).map_err(|e| ScriptError::Trapped(e.to_string()))?;
        let fuel = if self.ticks == 0 {
            FIRST_TICK_FUEL
        } else {
            FUEL_PER_TICK
        };
        self.ticks += 1;
        self.store.set_fuel(fuel).map_err(|e| trapped(&e))?;
        let length = i32::try_from(bytes.len()).map_err(|_| ScriptError::Trapped("input too large".into()))?;
        let at = self.alloc.call(&mut self.store, length).map_err(|e| trapped(&e))?;
        let start = usize::try_from(at).map_err(|_| bad("alloc returned a negative address"))?;
        self.memory
            .write(&mut self.store, start, &bytes)
            .map_err(|_| bad("alloc returned room outside its memory"))?;
        let answer = self
            .tick
            .call(&mut self.store, (at, length))
            .map_err(|e| trapped(&e))?;
        // The low 32 bits are the address, the high 32 bits the length.
        #[allow(clippy::cast_sign_loss)] // a bit pattern, not a number
        let answer = answer as u64;
        let address = usize::try_from(answer & 0xFFFF_FFFF).map_err(|_| bad("address"))?;
        let length = usize::try_from(answer >> 32).map_err(|_| bad("length"))?;
        if length > MAX_OUTPUT_BYTES {
            return Err(bad("output over 64 KiB"));
        }
        let mut output = vec![0; length];
        self.memory
            .read(&self.store, address, &mut output)
            .map_err(|_| bad("output outside its memory"))?;
        let output: Output =
            serde_json::from_slice(&output).map_err(|e| bad(&format!("output: {e}")))?;
        // The state is part of the output, so it is within the output's limit too.
        self.state = output.state.clone();
        Ok(output)
    }
}

fn trapped(error: &wasmi::Error) -> ScriptError {
    if error.as_trap_code() == Some(TrapCode::OutOfFuel) {
        ScriptError::OutOfFuel
    } else {
        ScriptError::Trapped(error.to_string())
    }
}

fn bad(message: &str) -> ScriptError {
    ScriptError::BadOutput(message.to_owned())
}

/// `orqwa_v1.log(ptr, len)`: a UTF-8 line for ORQWA's log, a few per tick at most.
fn log(mut caller: Caller<'_, Host>, ptr: i32, len: i32) {
    let (Ok(start), Ok(length)) = (usize::try_from(ptr), usize::try_from(len)) else {
        return;
    };
    if caller.data().log.len() >= MAX_LOG_LINES {
        return;
    }
    let Some(wasmi::Extern::Memory(memory)) = caller.get_export("memory") else {
        return;
    };
    let mut line = vec![0; length.min(MAX_LOG_BYTES)];
    if memory.read(&caller, start, &mut line).is_err() {
        return;
    }
    let line = String::from_utf8_lossy(&line).into_owned();
    caller.data_mut().log.push(line);
}

#[cfg(test)]
mod tests;
