//! A world's script in a ride (ADR 0044, R302, R303): the ride tells it what happens and carries
//! out what it asks, within ORQWA's limits. The physics, the trainer and the recording stay the
//! ride's; the script only sets parameters and requests.
//!
//! The limits here are applied on each rider's own device, whoever's script asked: a host's
//! script cannot ask more of another rider's trainer than that rider's own ORQWA allows.

use std::collections::BTreeMap;
use std::time::Duration;

use orqwa_domain::telemetry::TrainerControl;
use orqwa_domain::units::{GradePercent, Kilograms, MetersPerSecond, Percent, Watts};
use orqwa_physics::RiderSetup;
use orqwa_routes::Route;
use orqwa_world_api::{
    Action, BodyKind, BodyState, Input, Mark, Output, Rider, TrainerKind, TrainerRequest,
};
use orqwa_world_script::{ScriptError, WorldScript};
use serde_json::Value;

/// How often a script is asked; between its ticks the ride carries out its last answer.
pub const TICK: Duration = Duration::from_millis(100);
/// The steepest grades a script may ask a trainer for, before the rider's difficulty scales
/// them: those of real roads.
pub const MIN_GRADE: GradePercent = GradePercent(-20.0);
/// See [`MIN_GRADE`].
pub const MAX_GRADE: GradePercent = GradePercent(25.0);
/// A script may ask a trainer for at most this times the rider's FTP.
pub const MAX_POWER_FTP: f64 = 2.0;
/// What a script may add to or scale on a body it drives.
pub const MAX_ADDED_MASS: Kilograms = Kilograms(500.0);
/// See [`MAX_ADDED_MASS`]: the range of the factors on rolling and air resistance.
pub const RESISTANCE_FACTORS: (f64, f64) = (0.5, 3.0);
/// The fastest a scripted body may move.
pub const MAX_SCRIPTED_SPEED: MetersPerSecond = MetersPerSecond(30.0);
/// The most samples of the route a script is given; longer routes are sampled more coarsely.
const MAX_ROUTE_SAMPLES: f64 = 10_000.0;
/// The id of a rider's own body, as the script sees it.
const OWN_BODY: &str = "own";

/// What the ride knows of the rider for the script: what the API may tell it (R303).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldRider {
    /// Their name as the ride shows it.
    pub name: String,
    /// Their FTP.
    pub ftp: Watts,
}

/// How the rider moves, as the script set it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Movement {
    /// By ORQWA's physics, with the body's mass and resistances.
    Driven {
        /// Mass carried beyond the rider's.
        added: Kilograms,
        /// Factor on rolling resistance.
        rolling: f64,
        /// Factor on air resistance.
        air: f64,
    },
    /// At a speed the script sets.
    Scripted(MetersPerSecond),
}

impl Default for Movement {
    fn default() -> Self {
        Self::Driven {
            added: Kilograms(0.0),
            rolling: 1.0,
            air: 1.0,
        }
    }
}

impl Movement {
    /// `setup` carrying this body's mass, with its resistances.
    #[must_use]
    pub fn setup(self, setup: RiderSetup) -> RiderSetup {
        match self {
            Self::Driven {
                added,
                rolling,
                air,
            } => RiderSetup {
                mass: Kilograms(setup.mass.0 + added.0),
                crr: setup.crr * rolling,
                cda: orqwa_domain::units::SquareMeters(setup.cda.0 * air),
                ..setup
            },
            Self::Scripted(_) => setup,
        }
    }
}

/// What the script asks of the trainer, within the limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    /// The road, as without a script.
    Road,
    /// A grade, before the rider's difficulty scales it.
    Grade(GradePercent),
    /// A power.
    Power(Watts),
    /// A resistance.
    Resistance(Percent),
}

impl Request {
    /// `request` for a rider with `ftp`, within ORQWA's limits: a power the rider cannot be
    /// asked for, or anything not a number, is the road.
    #[must_use]
    pub fn limited(request: &TrainerRequest, ftp: Watts) -> Self {
        let finite = |v: f64| v.is_finite().then_some(v);
        match request.kind {
            TrainerKind::Grade => finite(request.grade).map_or(Self::Road, |grade| {
                Self::Grade(GradePercent((grade * 100.0).clamp(MIN_GRADE.0, MAX_GRADE.0)))
            }),
            TrainerKind::Power => match finite(request.power_w) {
                Some(power) if ftp.0 > 0.0 => {
                    Self::Power(Watts(power.clamp(0.0, MAX_POWER_FTP * ftp.0)))
                }
                _ => Self::Road,
            },
            TrainerKind::Resistance => finite(request.percent)
                .map_or(Self::Road, |p| Self::Resistance(Percent(p.clamp(0.0, 100.0)))),
            TrainerKind::Road | TrainerKind::Unknown => Self::Road,
        }
    }
}

fn factor(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(RESISTANCE_FACTORS.0, RESISTANCE_FACTORS.1)
    } else {
        1.0
    }
}

/// What the script asked for one body, within the limits.
#[must_use]
pub fn movement(body: &orqwa_world_api::Body) -> Movement {
    match body.kind {
        BodyKind::Scripted => Movement::Scripted(MetersPerSecond(if body.speed_mps.is_finite() {
            body.speed_mps.clamp(0.0, MAX_SCRIPTED_SPEED.0)
        } else {
            0.0
        })),
        BodyKind::Driven | BodyKind::Unknown => Movement::Driven {
            added: Kilograms(if body.mass_kg.is_finite() {
                body.mass_kg.clamp(0.0, MAX_ADDED_MASS.0)
            } else {
                0.0
            }),
            rolling: factor(body.rolling),
            air: factor(body.air),
        },
    }
}

/// The route as a script is given it: grade and elevation every `step` metres.
fn route_shape(route: &Route, id: &str) -> orqwa_world_api::Route {
    let length = route.length().0;
    let step = (length / MAX_ROUTE_SAMPLES).max(10.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // at most 10,001
    let count = (length / step).floor() as usize + 1;
    let (grade, elevation_m) = (0..count)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)] // small counts
            let position = route.position(orqwa_domain::units::Meters(i as f64 * step));
            (position.grade.0 / 100.0, position.elevation.0)
        })
        .unzip();
    orqwa_world_api::Route {
        id: id.to_owned(),
        length_m: length,
        step_m: step,
        grade,
        elevation_m,
    }
}

/// What the ride tells the script about the local rider each tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    /// Time since the ride began.
    pub elapsed: Duration,
    /// Where the rider is on the route.
    pub distance: f64,
    /// How fast they move.
    pub speed: MetersPerSecond,
    /// Their power, if their trainer or meter reports it.
    pub power: Option<Watts>,
    /// Their cadence, if reported.
    pub cadence: Option<f64>,
    /// Rider and bike.
    pub mass: Kilograms,
}

/// A world's script running in a ride.
#[derive(Debug)]
pub struct WorldDriver {
    script: WorldScript,
    config: Value,
    route_id: String,
    rider: WorldRider,
    route_sent: bool,
    since: Duration,
    actions: u32,
    output: Output,
    movement: Movement,
    request: Request,
    values: BTreeMap<String, f64>,
    marks: Vec<(Duration, Mark)>,
    stopped: Option<ScriptError>,
}

impl WorldDriver {
    /// A driver for `script`, with the world's `config` for it, on the route `route_id`.
    #[must_use]
    pub fn new(script: WorldScript, config: Value, route_id: &str, rider: WorldRider) -> Self {
        Self {
            script,
            config,
            route_id: route_id.to_owned(),
            rider,
            route_sent: false,
            since: TICK,
            actions: 0,
            output: Output::default(),
            movement: Movement::default(),
            request: Request::Road,
            values: BTreeMap::new(),
            marks: Vec::new(),
            stopped: None,
        }
    }

    /// The rider pressed World action.
    pub fn action(&mut self) {
        self.actions = self.actions.saturating_add(1);
    }

    /// The rider changed their profile during the ride.
    pub fn change_rider(&mut self, rider: WorldRider) {
        self.rider = rider;
    }

    /// Asks the script once `TICK` has passed since it was asked last; returns whether it was.
    pub(crate) fn update(&mut self, dt: Duration, now: Moment, route: Option<&Route>) -> bool {
        self.since += dt;
        if self.stopped.is_some() || self.since < TICK {
            return false;
        }
        let first = !self.route_sent;
        let input = Input {
            api: orqwa_world_api::API,
            features: orqwa_world_api::FEATURES
                .iter()
                .map(|&f| f.to_owned())
                .collect(),
            dt_s: if first { 0.0 } else { self.since.as_secs_f64() },
            time_s: now.elapsed.as_secs_f64(),
            config: self.config.clone(),
            route: route
                .filter(|_| first)
                .map(|route| route_shape(route, &self.route_id)),
            riders: vec![Rider {
                slot: 0,
                name: self.rider.name.clone(),
                power_w: now.power.map_or(0.0, |p| p.0),
                cadence_rpm: now.cadence.unwrap_or(0.0),
                mass_kg: now.mass.0,
                ftp_w: self.rider.ftp.0,
                connected: now.power.is_some(),
                body: self.body_id().to_owned(),
            }],
            bodies: vec![BodyState {
                id: self.body_id().to_owned(),
                distance_m: now.distance,
                speed_mps: now.speed.0,
            }],
            actions: (self.actions > 0)
                .then_some(Action {
                    slot: 0,
                    count: self.actions,
                })
                .into_iter()
                .collect(),
            state: Value::Null,
        };
        self.since = Duration::ZERO;
        self.actions = 0;
        self.route_sent = true;
        match self.script.tick(&input) {
            Ok(output) => self.apply(&output.resolved(), now.elapsed),
            Err(error) => self.stop(error),
        }
        true
    }

    fn body_id(&self) -> &str {
        self.output
            .bodies
            .iter()
            .find(|body| body.riders.contains(&0))
            .map_or(OWN_BODY, |body| body.id.as_str())
    }

    fn apply(&mut self, output: &Output, at: Duration) {
        self.movement = output
            .bodies
            .iter()
            .find(|body| body.riders.contains(&0))
            .map_or_else(Movement::default, movement);
        self.request = output
            .trainer
            .iter()
            .find(|request| request.slot == 0)
            .map_or(Request::Road, |request| Request::limited(request, self.rider.ftp));
        for (key, value) in &output.record.values {
            if value.is_finite() {
                self.values.insert(key.clone(), *value);
            }
        }
        self.marks
            .extend(output.record.marks.iter().map(|mark| (at, mark.clone())));
        self.output = output.clone();
    }

    fn stop(&mut self, error: ScriptError) {
        tracing::warn!("{error}; the world is ridden without its script from here on");
        self.stopped = Some(error);
        self.output = Output::default();
        self.movement = Movement::default();
        self.request = Request::Road;
    }

    /// How the rider moves now.
    #[must_use]
    pub fn movement(&self) -> Movement {
        self.movement
    }

    /// What the script asks of the trainer now.
    #[must_use]
    pub fn request(&self) -> Request {
        self.request
    }

    /// The script's last answer, as this ORQWA carries it out: for the scene and the screen.
    #[must_use]
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Why the script stopped, if it did.
    #[must_use]
    pub fn stopped(&self) -> Option<&ScriptError> {
        self.stopped.as_ref()
    }

    /// The results the script recorded, as last given.
    #[must_use]
    pub fn values(&self) -> &BTreeMap<String, f64> {
        &self.values
    }

    /// The moments the script marked, at the ride's time.
    #[must_use]
    pub fn marks(&self) -> &[(Duration, Mark)] {
        &self.marks
    }
}

/// The trainer control for `request`, if it is not the road: the rider's difficulty scales a
/// grade, as on a road.
pub(crate) fn control(
    request: Request,
    setup: RiderSetup,
    difficulty: Percent,
) -> Option<TrainerControl> {
    match request {
        Request::Road => None,
        Request::Grade(grade) => Some(TrainerControl::Simulation(
            setup.simulation_parameters(orqwa_physics::trainer_grade(grade, difficulty)),
        )),
        Request::Power(power) => Some(TrainerControl::TargetPower(power)),
        Request::Resistance(percent) => Some(TrainerControl::Resistance(percent)),
    }
}

#[cfg(test)]
mod tests;
