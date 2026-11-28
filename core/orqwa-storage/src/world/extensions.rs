//! The world extensions this ORQWA knows (`docs/world-files.md`, "Extensions"; ADR 0044):
//! `ORQWA_script`, `ORQWA_group` and `ORQWA_workouts`.
//!
//! Their data is read leniently: fields a later ORQWA adds are ignored here, so a world made for
//! it still opens in this one (R304). What this ORQWA does read must hold to the rules.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use super::{Description, Part, WorldError, identifier, invalid, name};

/// A WebAssembly module the world runs during the ride, with the models it may place.
pub const SCRIPT: &str = "ORQWA_script";
/// Whether the world may be ridden as a group, and by how many.
pub const GROUP: &str = "ORQWA_group";
/// Workout files that come with the world.
pub const WORKOUTS: &str = "ORQWA_workouts";
/// The highest script API version this ORQWA runs; it runs every version up to it (R304).
pub const SCRIPT_API: u32 = 1;
/// ORQWA's group limit: a world may allow fewer riders, never more.
pub const MAX_GROUP_RIDERS: u8 = 8;

/// Where a script's files lie.
const SCRIPT_FOLDER: &str = "ext/ORQWA_script/";
const WORKOUT_FOLDER: &str = "ext/ORQWA_workouts/";
const WORKOUT_TYPES: [&str; 4] = [".zwo", ".erg", ".mrc", ".fit"];
pub(super) const MAX_MODULE_BYTES: u64 = 4 * 1024 * 1024;
pub(super) const MAX_WORKOUT_BYTES: u64 = 1024 * 1024;
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_MODELS: usize = 256;
const MAX_WORKOUTS: usize = 256;
/// The start of every WebAssembly 1.0 binary: its magic and version.
const WASM_HEADER: [u8; 8] = *b"\0asm\x01\0\0\0";

/// A world's `ORQWA_script`.
#[derive(Debug, Clone, PartialEq)]
pub struct Script<'a> {
    /// The module, `ext/ORQWA_script/….wasm`.
    pub file: &'a str,
    /// The lowest script API version it needs, at most [`SCRIPT_API`].
    pub api: u32,
    /// Handed to the module unchanged; `null` when the world gives none.
    pub config: &'a Value,
    /// The models the module may place, by name.
    pub models: Vec<ScriptModel<'a>>,
}

/// A model a script may place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ScriptModel<'a> {
    /// How the script names it: 1–64 of `a–z 0–9 - _`, unique.
    #[serde(borrow)]
    pub name: &'a str,
    /// The `.glb`, `ext/ORQWA_script/….glb`. It may have skins and animations.
    #[serde(borrow)]
    pub file: &'a str,
}

#[derive(Deserialize)]
struct ScriptData<'a> {
    #[serde(borrow)]
    file: &'a str,
    api: u32,
    #[serde(borrow, default)]
    models: Vec<ScriptModel<'a>>,
}

/// A world's `ORQWA_group`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Group {
    /// Whether the world's routes may be chosen for a group ride.
    pub allowed: bool,
    #[serde(default)]
    max_riders: Option<u8>,
}

impl Group {
    /// The most riders the world may be ridden by together.
    #[must_use]
    pub fn max_riders(&self) -> u8 {
        self.max_riders.unwrap_or(MAX_GROUP_RIDERS)
    }
}

/// A world's `ORQWA_workouts`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Workouts<'a> {
    /// The workout files, `ext/ORQWA_workouts/…` in a format ORQWA reads.
    #[serde(borrow)]
    pub files: Vec<&'a str>,
}

static NULL: Value = Value::Null;

impl Description {
    /// The data of `extension`, if the world uses it.
    fn extension_data(&self, extension: &str) -> Result<Option<&Value>, WorldError> {
        if !self.extensions_used.iter().any(|used| used == extension) {
            return Ok(None);
        }
        self.extensions
            .get(extension)
            .map(Some)
            .ok_or_else(|| invalid(format!("extension {extension}: no data")))
    }

    fn required(&self, extension: &str) -> bool {
        self.extensions_required.iter().any(|r| r == extension)
    }

    /// The world's script, if it has one this ORQWA can run. A script that needs a newer API
    /// asks for an update if the world requires it, and is left out otherwise: the world is
    /// ridden without it.
    ///
    /// # Errors
    /// [`WorldError::Invalid`] if its data breaks the rules, [`WorldError::NeedsExtension`] if
    /// the world requires a newer API.
    pub fn script(&self) -> Result<Option<Script<'_>>, WorldError> {
        let Some(data) = self.extension_data(SCRIPT)? else {
            return Ok(None);
        };
        let ScriptData { file, api, models } = ScriptData::deserialize(data)
            .map_err(|e| invalid(format!("extension {SCRIPT}: {e}")))?;
        if api == 0 {
            return Err(invalid(format!("extension {SCRIPT}: api starts at 1")));
        }
        if api > SCRIPT_API {
            return if self.required(SCRIPT) {
                Err(WorldError::NeedsExtension(format!("{SCRIPT} API {api}")))
            } else {
                Ok(None)
            };
        }
        Ok(Some(Script {
            file,
            api,
            config: data.get("config").unwrap_or(&NULL),
            models,
        }))
    }

    /// The world's group-ride permission; `None` if it gives none, and it may not be ridden as
    /// a group.
    ///
    /// # Errors
    /// [`WorldError::Invalid`] if its data breaks the rules.
    pub fn group(&self) -> Result<Option<Group>, WorldError> {
        let Some(data) = self.extension_data(GROUP)? else {
            return Ok(None);
        };
        let group =
            Group::deserialize(data).map_err(|e| invalid(format!("extension {GROUP}: {e}")))?;
        if !(2..=MAX_GROUP_RIDERS).contains(&group.max_riders()) {
            return Err(invalid(format!(
                "extension {GROUP}: max_riders is 2 to {MAX_GROUP_RIDERS}"
            )));
        }
        Ok(Some(group))
    }

    /// The workouts that come with the world, if any.
    ///
    /// # Errors
    /// [`WorldError::Invalid`] if its data breaks the rules.
    pub fn workouts(&self) -> Result<Option<Workouts<'_>>, WorldError> {
        let Some(data) = self.extension_data(WORKOUTS)? else {
            return Ok(None);
        };
        Workouts::deserialize(data)
            .map(Some)
            .map_err(|e| invalid(format!("extension {WORKOUTS}: {e}")))
    }

    /// Checks the known extensions' data and names the files they bring as the parts they are.
    pub(super) fn check_known_extensions<'a>(
        &'a self,
        parts: &mut BTreeMap<&'a str, Part>,
    ) -> Result<(), WorldError> {
        if let Some(script) = self.script()? {
            let at = |e| super::prefixed(&format!("extension {SCRIPT}"), e);
            name(script.file, SCRIPT_FOLDER, ".wasm", Part::Module, parts).map_err(at)?;
            if serde_json::to_vec(script.config)
                .ok()
                .is_none_or(|c| c.len() > MAX_CONFIG_BYTES)
            {
                return Err(invalid(format!("extension {SCRIPT}: config is over 1 MiB")));
            }
            if script.models.len() > MAX_MODELS {
                return Err(invalid(format!("extension {SCRIPT}: too many models")));
            }
            let mut names = BTreeSet::new();
            for model in &script.models {
                if !identifier(model.name, true) || !names.insert(model.name) {
                    return Err(invalid(format!(
                        "extension {SCRIPT}: model name {:?} is not allowed",
                        model.name
                    )));
                }
                name(model.file, SCRIPT_FOLDER, ".glb", Part::ScriptModel, parts).map_err(at)?;
            }
        }
        self.group()?;
        if let Some(workouts) = self.workouts()? {
            if workouts.files.len() > MAX_WORKOUTS {
                return Err(invalid(format!("extension {WORKOUTS}: too many files")));
            }
            for file in workouts.files {
                let kind = WORKOUT_TYPES
                    .into_iter()
                    .find(|kind| file.ends_with(kind))
                    .ok_or_else(|| {
                        invalid(format!(
                            "extension {WORKOUTS}: {file:?} is not a .zwo, .erg, .mrc or .fit file"
                        ))
                    })?;
                name(file, WORKOUT_FOLDER, kind, Part::Workout, parts)
                    .map_err(|e| super::prefixed(&format!("extension {WORKOUTS}"), e))?;
            }
        }
        Ok(())
    }
}

/// Checks a module's header, the rest of it left to the runtime; returns the bytes read.
pub(super) fn module_header(input: &mut impl std::io::Read) -> Result<u64, WorldError> {
    let mut header = [0u8; 8];
    input
        .read_exact(&mut header)
        .map_err(|_| invalid("too short for WebAssembly"))?;
    if header != WASM_HEADER {
        return Err(invalid("not a WebAssembly 1.0 module"));
    }
    Ok(8)
}
