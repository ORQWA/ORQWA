//! The extensions ORQWA knows (ADR 0044, R302–R305): a world's script, group permission and
//! workouts are read and checked with the world, and a world made for a later ORQWA still opens.

use serde_json::{Value, json};

use super::super::extensions::{GROUP, MAX_GROUP_RIDERS, SCRIPT, WORKOUTS};
use super::*;

const MODULE: &str = "ext/ORQWA_script/world.wasm";
const MODEL: &str = "ext/ORQWA_script/figure.glb";
const WORKOUT: &str = "ext/ORQWA_workouts/intervals.zwo";

/// The smallest WebAssembly module: its header alone.
fn module() -> Vec<u8> {
    b"\0asm\x01\0\0\0".to_vec()
}

/// A skinned, animated model: one joint, one channel turning it.
fn figure(patch: impl FnOnce(&mut Value)) -> Vec<u8> {
    glb(|g| {
        g["accessors"] = json!([
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 0, "componentType": 5126, "count": 1, "type": "SCALAR"},
            {"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC4"}
        ]);
        g["nodes"] = json!([{"mesh": 0, "skin": 0, "children": [1]}, {}]);
        g["skins"] = json!([{"joints": [1]}]);
        g["animations"] = json!([{
            "samplers": [{"input": 1, "output": 2, "interpolation": "LINEAR"}],
            "channels": [{"sampler": 0, "target": {"node": 1, "path": "rotation"}}]
        }]);
        patch(g);
    })
}

/// A world using all three extensions, with `patch` applied to its description.
fn featured(patch: impl FnOnce(&mut Value)) -> Vec<(String, Vec<u8>)> {
    let mut description = description();
    description["extensions_used"] = json!([SCRIPT, GROUP, WORKOUTS]);
    description["extensions_required"] = json!([SCRIPT, GROUP]);
    description["extensions"] = json!({
        SCRIPT: {"file": MODULE, "api": 1, "config": {"laps": 3},
                 "models": [{"name": "figure", "file": MODEL}]},
        GROUP: {"allowed": true, "max_riders": 4},
        WORKOUTS: {"files": [WORKOUT]}
    });
    patch(&mut description);
    let mut entries = entries();
    entries[0].1 = serde_json::to_vec(&description).unwrap();
    entries.extend([
        (MODULE.into(), module()),
        (MODEL.into(), figure(|_| {})),
        (WORKOUT.into(), b"<workout_file/>".to_vec()),
    ]);
    entries
}

fn replaced(
    mut entries: Vec<(String, Vec<u8>)>,
    name: &str,
    bytes: Vec<u8>,
) -> Vec<(String, Vec<u8>)> {
    entries.iter_mut().find(|(n, _)| n == name).unwrap().1 = bytes;
    entries
}

#[test]
fn a_world_with_a_script_group_permission_and_workouts_opens() {
    let path = archive(&featured(|_| {}));
    let description = validate(&path, &trust()).unwrap();
    let script = description.script().unwrap().unwrap();
    assert_eq!((script.file, script.api), (MODULE, 1));
    assert_eq!(script.config, &json!({"laps": 3}));
    assert_eq!(script.models[0].name, "figure");
    let group = description.group().unwrap().unwrap();
    assert!(group.allowed);
    assert_eq!(group.max_riders(), 4);
    assert_eq!(description.workouts().unwrap().unwrap().files, [WORKOUT]);

    let world = WorldFile::open(&path, &trust()).unwrap();
    assert_eq!(world.module().unwrap().unwrap(), module());
    assert!(world.glb(MODEL).is_ok(), "a script's model may move");
    assert_eq!(world.workout(WORKOUT).unwrap(), b"<workout_file/>");
    assert!(world.workout("routes/loop.json").is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_world_without_them_has_no_script_group_ride_or_workouts() {
    let path = archive(&entries());
    let description = validate(&path, &trust()).unwrap();
    assert_eq!(description.script().unwrap(), None);
    assert_eq!(description.group().unwrap(), None);
    assert_eq!(description.workouts().unwrap(), None);
    assert_eq!(
        WorldFile::open(&path, &trust()).unwrap().module().unwrap(),
        None
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn only_a_scripts_models_may_move() {
    // The same skinned, animated model as a section is refused.
    let entries = with("sections/0/lod0.glb", figure(|_| {}));
    refused(&entries, "not allowed");
    refused(
        &replaced(
            featured(|_| {}),
            MODEL,
            figure(|g| g["cameras"] = json!([{}])),
        ),
        "cameras are not allowed",
    );
    for (patch, because) in [
        (json!({"joints": [5]}), "a skin's joint refers to nothing"),
        (json!({"joints": []}), "a skin without joints"),
    ] {
        refused(
            &replaced(
                featured(|_| {}),
                MODEL,
                figure(|g| g["skins"] = json!([patch])),
            ),
            because,
        );
    }
    refused(
        &replaced(
            featured(|_| {}),
            MODEL,
            figure(|g| g["animations"][0]["channels"][0]["target"]["path"] = json!("pointer")),
        ),
        "unknown path",
    );
    refused(
        &replaced(
            featured(|_| {}),
            MODEL,
            figure(|g| g["animations"][0]["samplers"][0]["output"] = json!(9)),
        ),
        "an animation sampler refers to nothing",
    );
}

#[test]
fn a_script_is_a_webassembly_module_in_its_folder() {
    refused(
        &replaced(featured(|_| {}), MODULE, b"#!/bin/sh".to_vec()),
        "not a WebAssembly",
    );
    refused(
        &featured(|d| d["extensions"][SCRIPT]["file"] = json!("routes/world.wasm")),
        "is not a ext/ORQWA_script/",
    );
    refused(
        &featured(|d| d["extensions"][SCRIPT]["api"] = json!(0)),
        "api starts at 1",
    );
    refused(
        &featured(|d| {
            d["extensions"][SCRIPT]["models"] =
                json!([{"name": "figure", "file": MODEL}, {"name": "figure", "file": MODEL}]);
        }),
        "model name \"figure\" is not allowed",
    );
    let missing: Vec<_> = featured(|_| {})
        .into_iter()
        .filter(|(name, _)| name != MODULE)
        .collect();
    refused(&missing, "named by world.json but missing");
}

#[test]
fn a_script_for_a_newer_api_asks_for_an_update_only_if_required() {
    let path = archive(&featured(|d| d["extensions"][SCRIPT]["api"] = json!(2)));
    assert!(matches!(
        validate(&path, &trust()),
        Err(WorldError::NeedsExtension(name)) if name == "ORQWA_script API 2"
    ));
    std::fs::remove_file(path).unwrap();

    let path = archive(&featured(|d| {
        d["extensions"][SCRIPT]["api"] = json!(2);
        d["extensions_required"] = json!([GROUP]);
    }));
    let description = validate(&path, &trust()).expect("ridden without the script");
    assert_eq!(description.script().unwrap(), None);
    assert_eq!(
        WorldFile::open(&path, &trust()).unwrap().module().unwrap(),
        None
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn fields_a_later_orqwa_adds_are_ignored() {
    let path = archive(&featured(|d| {
        d["extensions"][SCRIPT]["sound"] = json!("ext/ORQWA_script/bell.ogg");
        d["extensions"][GROUP]["teams"] = json!(2);
        d["extensions"][WORKOUTS]["default"] = json!(0);
    }));
    assert!(validate(&path, &trust()).is_ok());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_world_allows_a_group_of_two_up_to_orqwas_limit() {
    for riders in [0, 1, u64::from(MAX_GROUP_RIDERS) + 1] {
        refused(
            &featured(|d| d["extensions"][GROUP]["max_riders"] = json!(riders)),
            "max_riders is 2 to 8",
        );
    }
    let path = archive(&featured(|d| {
        d["extensions"][GROUP] = json!({"allowed": false})
    }));
    let group = validate(&path, &trust()).unwrap().group().unwrap().unwrap();
    assert!(!group.allowed);
    assert_eq!(group.max_riders(), MAX_GROUP_RIDERS);
    std::fs::remove_file(path).unwrap();
    refused(
        &featured(|d| d["extensions"][GROUP] = json!({"max_riders": 4})),
        "missing field `allowed`",
    );
}

#[test]
fn workouts_are_files_orqwa_reads_in_their_folder() {
    refused(
        &featured(|d| d["extensions"][WORKOUTS]["files"] = json!(["ext/ORQWA_workouts/a.txt"])),
        "is not a .zwo, .erg, .mrc or .fit file",
    );
    refused(
        &featured(|d| d["extensions"][WORKOUTS]["files"] = json!(["workouts/a.zwo"])),
        "is not a ext/ORQWA_workouts/",
    );
    refused(
        &featured(|d| {
            d["extensions"].as_object_mut().unwrap().remove(WORKOUTS);
        }),
        "extension ORQWA_workouts: no data",
    );
}

#[test]
fn a_folder_with_a_script_seals_each_file_once() {
    let folder = std::env::temp_dir().join(format!("orqwa-featured-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    for (name, bytes) in featured(|_| {}) {
        let path = folder.join(&name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    // A file of the script's own, named by nothing in world.json, goes along too.
    std::fs::write(
        folder.join("ext/ORQWA_script/badge.png"),
        b"\x89PNG\r\n\x1a\n",
    )
    .unwrap();
    let out = folder.with_extension("oqx");
    seal(&folder, &out, &sealer(), &trust()).expect("sealed");
    let world = WorldFile::open(&out, &trust()).unwrap();
    assert!(world.module().unwrap().is_some());
    let mut zip = ZipArchive::new(File::open(&out).unwrap()).unwrap();
    assert!(zip.by_name("ext/ORQWA_script/badge.png").is_ok());
    drop(zip);
    std::fs::remove_file(out).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}
