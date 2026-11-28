//! The documents a world script and ORQWA exchange each tick (ADR 0044,
//! `docs/world-files.md`, "`ORQWA_script`"): what ORQWA tells the script ([`Input`]) and what
//! the script asks for ([`Output`]).
//!
//! These documents are a promise kept for good (R304). Fields are only ever added, optional,
//! with defaults that keep earlier behaviour; none is removed or renamed or changes type, unit
//! or meaning. Both sides ignore fields they do not know. A kind ORQWA does not know falls back
//! to the element's `fallback`, or to a safe default ([`Output::resolved`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The highest API version these documents describe.
pub const API: u32 = 1;

/// What this ORQWA offers, as `<primitive>.<kind>`. Scripts check these, not [`API`], before
/// using something; the list only grows.
pub const FEATURES: &[&str] = &[
    "body.driven",
    "body.scripted",
    "trainer.road",
    "trainer.grade",
    "trainer.power",
    "trainer.resistance",
    "scene.node",
    "scene.line",
    "scene.clip",
    "scene.rider",
    "scene.camera",
    "ui.text",
    "ui.value",
    "ui.bar",
    "ui.grid",
    "ui.image",
    "record.values",
    "record.marks",
    "input.actions",
];

/// A rider's stable place in the ride, from 0.
pub type Slot = u8;

/// Text in several languages, by language code (`en`, `de_CH`, …); the empty key is the
/// default.
pub type Text = BTreeMap<String, String>;

// ---------------------------------------------------------------------------------------------
// Input

/// What ORQWA tells the script each tick.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    /// The host's API version.
    pub api: u32,
    /// What the host offers, as in [`FEATURES`].
    pub features: Vec<String>,
    /// Seconds since the last tick; 0 on the first.
    pub dt_s: f64,
    /// Seconds since the ride began, pauses excluded.
    pub time_s: f64,
    /// The world's `config` for the script, unchanged.
    pub config: Value,
    /// The route's shape: given on the first tick, and again only if it changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// The riders now in the ride.
    pub riders: Vec<Rider>,
    /// The bodies now moving.
    pub bodies: Vec<BodyState>,
    /// World action presses since the last tick.
    pub actions: Vec<Action>,
    /// What the script returned as `state` last tick; `null` at first.
    pub state: Value,
}

/// A route's shape, as ORQWA rides it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Route {
    /// The route's id in the world.
    pub id: String,
    /// Its length, metres.
    pub length_m: f64,
    /// The spacing of the samples, metres, from 0.
    pub step_m: f64,
    /// The grade at each sample, as a fraction (0.05 for 5 %).
    pub grade: Vec<f64>,
    /// The elevation at each sample, metres.
    pub elevation_m: Vec<f64>,
}

/// A rider in the ride.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rider {
    /// Their slot.
    pub slot: Slot,
    /// Their name as the ride shows it.
    pub name: String,
    /// Power, watts.
    pub power_w: f64,
    /// Cadence, revolutions per minute.
    pub cadence_rpm: f64,
    /// Rider and bike, kilograms.
    pub mass_kg: f64,
    /// Their FTP, watts.
    pub ftp_w: f64,
    /// Whether their data is current.
    pub connected: bool,
    /// The body they move with.
    pub body: String,
}

/// Where a body is.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BodyState {
    /// Its id.
    pub id: String,
    /// Distance along the route, metres.
    pub distance_m: f64,
    /// Speed, metres per second.
    pub speed_mps: f64,
}

/// A rider's presses of World action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Action {
    /// Whose.
    pub slot: Slot,
    /// How many since the last tick.
    pub count: u32,
}

// ---------------------------------------------------------------------------------------------
// Output

/// What the script asks for this tick. Every part is optional; what is not given stays as it
/// would be without a script.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Output {
    /// The bodies riders move with.
    pub bodies: Vec<Body>,
    /// Requests for riders' trainers.
    pub trainer: Vec<TrainerRequest>,
    /// What the scene shows.
    pub scene: Scene,
    /// Panels on screen.
    pub ui: Vec<Panel>,
    /// What is saved with the activity.
    pub record: Record,
    /// Handed back as [`Input::state`] next tick.
    pub state: Value,
}

/// A kind this ORQWA does not know reads as `Unknown`, and its element falls back.
macro_rules! kinds {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $text)] $variant,)+
            /// A kind a later API adds: the element falls back.
            #[default]
            #[serde(other)]
            Unknown,
        }
    };
}

kinds!(
    /// How a body moves.
    BodyKind {
        /// By ORQWA's physics, from the summed power of the riders it carries.
        Driven = "driven",
        /// At the speed the script sets.
        Scripted = "scripted",
    }
);

/// Something that moves along the route, carrying riders.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Body {
    /// Its id: 1–64 of `a–z 0–9 - _`.
    pub id: String,
    /// How it moves.
    pub kind: BodyKind,
    /// The riders it carries; a rider is on at most one body.
    pub riders: Vec<Slot>,
    /// Mass beyond its riders', kilograms.
    pub mass_kg: f64,
    /// Factor on rolling resistance.
    #[serde(default = "one")]
    pub rolling: f64,
    /// Factor on air resistance.
    #[serde(default = "one")]
    pub air: f64,
    /// The speed of a scripted body, metres per second.
    pub speed_mps: f64,
    /// Used if this ORQWA does not know `kind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<Body>>,
}

fn one() -> f64 {
    1.0
}

kinds!(
    /// What a trainer is asked for.
    TrainerKind {
        /// The road, as without a script.
        Road = "road",
        /// A grade.
        Grade = "grade",
        /// A power.
        Power = "power",
        /// A resistance.
        Resistance = "resistance",
    }
);

/// A request for one rider's trainer. ORQWA limits it on that rider's own device.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TrainerRequest {
    /// Whose trainer.
    pub slot: Slot,
    /// What it is asked for.
    pub kind: TrainerKind,
    /// For `grade`: as a fraction (0.05 for 5 %).
    pub grade: f64,
    /// For `power`: watts.
    pub power_w: f64,
    /// For `resistance`: percent of the trainer's range.
    pub percent: f64,
    /// Used if this ORQWA does not know `kind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<TrainerRequest>>,
}

/// What the scene shows.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene {
    /// Models placed in the world.
    pub nodes: Vec<Node>,
    /// Lines between nodes.
    pub lines: Vec<Line>,
    /// Whether ORQWA's own rider is shown, per slot; shown when not given.
    pub riders: Vec<RiderShown>,
    /// Where the camera would like to be; the rider's own choice comes first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
}

/// What a node or camera is placed relative to: exactly one of the three.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Anchor {
    /// A body, by id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// A rider, by slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
    /// A place on the route, metres from its start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_m: Option<f64>,
}

impl Anchor {
    /// Whether it names exactly one thing to be placed by.
    #[must_use]
    pub fn is_known(&self) -> bool {
        usize::from(self.body.is_some())
            + usize::from(self.slot.is_some())
            + usize::from(self.route_m.is_some())
            == 1
    }
}

/// A model placed in the world.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    /// Its id, for lines to name.
    pub id: String,
    /// A model the world's script lists, or `rider` for ORQWA's rider and bike.
    pub model: String,
    /// What it is placed by.
    pub anchor: Anchor,
    /// Right, up and forward from the anchor, metres.
    pub offset_m: [f64; 3],
    /// Pitch, yaw and roll, degrees.
    pub rotation_deg: [f64; 3],
    /// Uniform scale.
    #[serde(default = "one")]
    pub scale: f64,
    /// Whether it is drawn.
    #[serde(default = "yes")]
    pub visible: bool,
    /// The model's own animation to play.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<Clip>,
    /// Used if this ORQWA cannot place it (an anchor it does not know).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<Node>>,
}

fn yes() -> bool {
    true
}

/// An animation clip of a node's model.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Clip {
    /// The clip's name in the model.
    pub name: String,
    /// How fast it plays, 1 as authored; ignored when `phase` is given.
    #[serde(default = "one")]
    pub rate: f64,
    /// Held at this point of the clip, 0–1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<f64>,
}

/// A point on a node, in the node's own metres.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NodePoint {
    /// The node, by id.
    pub node: String,
    /// Right, up and forward of the node's origin.
    pub point: [f64; 3],
}

/// A line drawn between two node points.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Line {
    /// Where it starts.
    pub from: NodePoint,
    /// Where it ends.
    pub to: NodePoint,
    /// Used if this ORQWA cannot draw it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<Line>>,
}

/// Whether a rider's ORQWA rider is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RiderShown {
    /// Whose.
    pub slot: Slot,
    /// Shown or not.
    pub shown: bool,
}

/// Where the camera would like to be.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Camera {
    /// What it follows.
    pub anchor: Anchor,
    /// How far behind, metres.
    pub back_m: f64,
    /// How far up, metres.
    pub up_m: f64,
    /// How far above the anchor it looks, metres: the height of what it follows.
    #[serde(default = "one")]
    pub aim_up_m: f64,
}

kinds!(
    /// Where on screen a panel goes.
    PanelSlot {
        /// Along the top.
        Top = "top",
        /// At the side.
        Side = "side",
        /// In the middle.
        Centre = "centre",
        /// Along the bottom.
        Bottom = "bottom",
    }
);

/// Who sees a panel: `"all"` or one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Audience {
    /// Everyone in the ride.
    #[default]
    All,
    /// One rider.
    Slot(Slot),
}

impl Serialize for Audience {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.serialize_str("all"),
            Self::Slot(slot) => serializer.serialize_u8(*slot),
        }
    }
}

impl<'de> Deserialize<'de> for Audience {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(all) if all == "all" => Ok(Self::All),
            value => value
                .as_u64()
                .and_then(|slot| Slot::try_from(slot).ok())
                .map(Self::Slot)
                .ok_or_else(|| serde::de::Error::custom("expected \"all\" or a slot")),
        }
    }
}

/// A panel of screen items.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Panel {
    /// Its id: a panel is replaced by the next one with its id.
    pub id: String,
    /// Where it goes.
    pub slot: PanelSlot,
    /// Who sees it.
    pub to: Audience,
    /// How long it shows; until the next output names it if not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    /// What it shows, in order.
    pub items: Vec<Item>,
    /// Used if this ORQWA does not know `slot`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<Panel>>,
}

kinds!(
    /// What a screen item is.
    ItemKind {
        /// Text.
        Text = "text",
        /// A number with its unit.
        Value = "value",
        /// A bar filled 0–1.
        Bar = "bar",
        /// Rows of cells.
        Grid = "grid",
        /// An image the world brings.
        Image = "image",
    }
);

/// One thing on a panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    /// What it is.
    pub kind: ItemKind,
    /// For `text`, and a label for `value` and `bar`.
    pub text: Text,
    /// Drawn stronger.
    pub emphasis: bool,
    /// For `value` (in `unit`) and `bar` (0–1).
    pub value: f64,
    /// For `value`: an SI unit (`m`, `m/s`, `W`, `s`, `rpm`, `kg`, `%`), shown in the rider's.
    pub unit: String,
    /// For `value`: decimals shown.
    pub decimals: u8,
    /// For `grid`: rows of cell texts.
    pub rows: Vec<Vec<Text>>,
    /// For `grid`: the row drawn highlighted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<usize>,
    /// For `image`: a PNG or WebP under `ext/ORQWA_script/`.
    pub image: String,
    /// Used if this ORQWA does not know `kind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<Item>>,
}

/// What is saved with the activity.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Results, as last given.
    pub values: BTreeMap<String, f64>,
    /// Moments, saved at the ride's time when given.
    pub marks: Vec<Mark>,
}

/// A moment of the ride worth keeping.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mark {
    /// What it is.
    pub name: String,
    /// Whose, if one rider's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
}

// ---------------------------------------------------------------------------------------------
// Falling back

/// What ORQWA makes of an element: itself, its fallback, or nothing.
fn resolve<'a, T>(
    element: &'a T,
    known: impl Fn(&T) -> bool,
    fallback: impl Fn(&'a T) -> Option<&'a T>,
) -> Option<&'a T> {
    let mut at = element;
    // A fallback chain is as long as the document allows; each step is one level shorter.
    loop {
        if known(at) {
            return Some(at);
        }
        at = fallback(at)?;
    }
}

impl Output {
    /// The output as this ORQWA carries it out: each element replaced by the first of itself
    /// and its fallbacks that this ORQWA knows. Bodies, nodes, lines, panels and items with no
    /// known form are left out, so their riders stay on their own bodies; trainer requests with
    /// none become the road.
    #[must_use]
    pub fn resolved(&self) -> Output {
        let item_known = |i: &Item| i.kind != ItemKind::Unknown;
        Output {
            bodies: self
                .bodies
                .iter()
                .filter_map(|b| {
                    resolve(
                        b,
                        |b| b.kind != BodyKind::Unknown,
                        |b| b.fallback.as_deref(),
                    )
                })
                .map(|b| Body {
                    fallback: None,
                    ..b.clone()
                })
                .collect(),
            trainer: self
                .trainer
                .iter()
                .map(|t| {
                    resolve(
                        t,
                        |t| t.kind != TrainerKind::Unknown,
                        |t| t.fallback.as_deref(),
                    )
                    .map_or_else(
                        || TrainerRequest {
                            slot: t.slot,
                            kind: TrainerKind::Road,
                            ..TrainerRequest::default()
                        },
                        |t| TrainerRequest {
                            fallback: None,
                            ..t.clone()
                        },
                    )
                })
                .collect(),
            scene: Scene {
                nodes: self
                    .scene
                    .nodes
                    .iter()
                    .filter_map(|n| resolve(n, |n| n.anchor.is_known(), |n| n.fallback.as_deref()))
                    .map(|n| Node {
                        fallback: None,
                        ..n.clone()
                    })
                    .collect(),
                lines: self
                    .scene
                    .lines
                    .iter()
                    .map(|l| Line {
                        fallback: None,
                        ..l.clone()
                    })
                    .collect(),
                riders: self.scene.riders.clone(),
                camera: self.scene.camera.clone().filter(|c| c.anchor.is_known()),
            },
            ui: self
                .ui
                .iter()
                .filter_map(|p| {
                    resolve(
                        p,
                        |p| p.slot != PanelSlot::Unknown,
                        |p| p.fallback.as_deref(),
                    )
                })
                .map(|p| Panel {
                    fallback: None,
                    items: p
                        .items
                        .iter()
                        .filter_map(|i| resolve(i, item_known, |i| i.fallback.as_deref()))
                        .map(|i| Item {
                            fallback: None,
                            ..i.clone()
                        })
                        .collect(),
                    ..p.clone()
                })
                .collect(),
            record: self.record.clone(),
            state: self.state.clone(),
        }
    }
}
