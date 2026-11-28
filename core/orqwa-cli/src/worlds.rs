//! The maintainers' tools for world files (ADR 0043): keys, certificates, withdrawals, sealing
//! and checking. Private keys are only ever written encrypted with `ORQWA_KEY_PASSWORD`.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use clap::Args;
use orqwa_licence::keys::{KeyPair, PublicKey};
use orqwa_licence::trust::{self, Purpose, Signed, Trust};
use orqwa_storage::world::{self, Sealer, WorldFile};

/// Seal a world folder into a signed world file.
#[derive(Args)]
pub struct SealArgs {
    /// The folder holding `world.json` and the parts it names.
    folder: PathBuf,
    /// The world file to write.
    out: PathBuf,
    /// The world key, as `key-new` wrote it.
    #[arg(long, required_unless_present = "dev")]
    key: Option<PathBuf>,
    /// A root's certificate for the world key, as `key-certify` wrote it.
    #[arg(long, required_unless_present = "dev")]
    certificate: Option<PathBuf>,
    /// Seal with the development world key, which only development builds trust.
    #[arg(long, conflicts_with_all = ["key", "certificate"])]
    dev: bool,
    /// Also trust this root (`*.pub.json`), for roots not yet built into ORQWA.
    #[arg(long)]
    trust: Option<PathBuf>,
}

fn password() -> Result<String> {
    std::env::var("ORQWA_KEY_PASSWORD")
        .ok()
        .filter(|p| p.len() >= 12)
        .context("set ORQWA_KEY_PASSWORD to the key's password (at least 12 characters)")
}

fn read_key(path: &Path) -> Result<KeyPair> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    KeyPair::unprotect(&text, &password()?)
        .with_context(|| format!("cannot open {}", path.display()))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not what was expected", path.display()))
}

fn write_new(path: &Path, text: &str) -> Result<()> {
    match orqwa_licence::files::write_new_private(path, text.as_bytes()) {
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => bail!(
            "{} exists already; keys and certificates are never overwritten",
            path.display()
        ),
        written => written.with_context(|| format!("cannot write {}", path.display())),
    }
}

fn public_path(key: &Path) -> PathBuf {
    let mut name = key.file_name().unwrap_or_default().to_os_string();
    name.push(".pub.json");
    key.with_file_name(name)
}

/// The trust ORQWA builds in, plus `extra` if given.
fn trust(extra: Option<&Path>) -> Result<Trust> {
    let builtin = Trust::builtin();
    let Some(extra) = extra else {
        return Ok(builtin);
    };
    Ok(builtin.with_root(read_json(extra)?))
}

/// Makes a new key, written encrypted to `out`, its public half to `out.pub.json`.
pub fn key_new(out: &Path) -> Result<()> {
    let password = password()?;
    let key = KeyPair::generate()?;
    write_new(out, &key.protect(&password)?)?;
    let public = serde_json::to_string_pretty(key.public())?;
    write_new(&public_path(out), &public)?;
    println!("key {} written to {}", key.public().id(), out.display());
    println!("public key in {}", public_path(out).display());
    Ok(())
}

/// A root certifies a working key.
pub fn key_certify(root: &Path, public: &Path, name: &str, out: &Path) -> Result<()> {
    let root = read_key(root)?;
    let public: PublicKey = read_json(public)?;
    let certificate = trust::certify(&root, &public, Purpose::World, name)?;
    write_new(out, &serde_json::to_string_pretty(&certificate)?)?;
    println!(
        "{} certified by root {} as {name}",
        public.id(),
        root.public().id()
    );
    Ok(())
}

/// Stores the images several of a world folder's glTF files embed once, under `textures/`.
pub fn share(folder: &Path) -> Result<()> {
    let shared = world::share_textures(folder)
        .with_context(|| format!("cannot share {}'s textures", folder.display()))?;
    println!(
        "{} textures shared by {} files, {} MB smaller",
        shared.textures,
        shared.files,
        shared.bytes_saved / 1_000_000
    );
    Ok(())
}

/// Seals a world folder, then checks the file as importing would.
pub fn seal(args: &SealArgs) -> Result<()> {
    let (key, certificate) = if args.dev {
        dev_key()?
    } else {
        let key = read_key(args.key.as_deref().context("--key")?)?;
        let certificate: Signed = read_json(args.certificate.as_deref().context("--certificate")?)?;
        (key, certificate)
    };
    let sealer = Sealer {
        key: &key,
        certificate,
    };
    let trust = trust(args.trust.as_deref())?;
    world::seal(&args.folder, &args.out, &sealer, &trust)
        .with_context(|| format!("cannot seal {}", args.folder.display()))?;
    check(&args.out, args.trust.as_deref())
}

#[cfg(feature = "dev-keys")]
#[allow(clippy::unnecessary_wraps)] // the same shape as without dev keys
fn dev_key() -> Result<(KeyPair, Signed)> {
    Ok((
        orqwa_licence::dev::world_key(),
        orqwa_licence::dev::world_certificate(),
    ))
}

#[cfg(not(feature = "dev-keys"))]
fn dev_key() -> Result<(KeyPair, Signed)> {
    bail!("--dev needs orqwa-cli built with --features dev-keys")
}

/// Checks a world file as importing it does, and shows what it holds and who signed it.
pub fn check(file: &Path, extra_root: Option<&Path>) -> Result<()> {
    let started = std::time::Instant::now();
    let trust = trust(extra_root)?;
    let world = world::validate(file, &trust)
        .with_context(|| format!("{} is not a valid world file", file.display()))?;
    let opened = WorldFile::open(file, &trust)?;
    println!(
        "{} {} — {}",
        world.name,
        world.version.as_deref().unwrap_or(""),
        world.licence
    );
    let (signer, signer_name) = opened.signer();
    println!("signed by {signer_name} ({signer})");
    println!(
        "{} sections, {} assets, horizon: {}",
        world.sections.len(),
        world.assets.len(),
        if world.horizon.is_some() { "yes" } else { "no" }
    );
    for route in &world.routes {
        println!("route {}: {}", route.id, route.name);
    }
    for credit in &world.attribution {
        println!("credit: {credit}");
    }
    println!("valid, checked in {:.1} s", started.elapsed().as_secs_f64());
    Ok(())
}

/// Run a world's script headless, as a ride would, with simulated riders (ADR 0044): for world
/// authors to see what it asks for.
#[derive(Args)]
pub struct ScriptRunArgs {
    /// The world file.
    file: PathBuf,
    /// The route to ride; the world's first if not given.
    #[arg(long)]
    route: Option<String>,
    /// The rider's steady power, watts.
    #[arg(long, default_value_t = 200.0)]
    power: f64,
    /// The rider's FTP, watts.
    #[arg(long, default_value_t = 250.0)]
    ftp: f64,
    /// How the rider looks: the rider on the bike, `female` or `male`.
    #[arg(long, default_value = "female", value_parser = ["female", "male"])]
    look: String,
    /// How long to ride, seconds.
    #[arg(long, default_value_t = 600)]
    seconds: u32,
    /// Press World action at these seconds.
    #[arg(long, value_delimiter = ',')]
    actions: Vec<u32>,
    /// Print what the script answers every tick, not only once a second.
    #[arg(long)]
    trace: bool,
    /// Also trust this root (`*.pub.json`), for roots not yet built into ORQWA.
    #[arg(long)]
    trust: Option<PathBuf>,
}

/// Rides `args.file`'s route with its script, printing each second what happened.
pub fn script_run(args: &ScriptRunArgs) -> Result<()> {
    use orqwa_domain::telemetry::Telemetry;
    use orqwa_domain::units::{Rpm, Watts};
    use orqwa_session::world::{WorldDriver, WorldRider};
    use orqwa_session::{Ride, RideConfig};

    let trust = trust(args.trust.as_deref())?;
    let file = WorldFile::open(&args.file, &trust)
        .with_context(|| format!("{} is not a valid world file", args.file.display()))?;
    let description = file.description();
    let script = description
        .script()?
        .context("this world has no script this ORQWA runs")?;
    let module = file.module()?.context("this world has no script")?;
    let script =
        orqwa_world_script::WorldScript::load(&module, script.api).map_err(anyhow::Error::msg)?;
    let id = args
        .route
        .clone()
        .or_else(|| description.routes.first().map(|r| r.id.clone()))
        .context("the world has no route")?;
    let points = file.route(&id)?;
    let origin = description.origin;
    let route = orqwa_routes::Route::from_world(
        Some(id.clone()),
        (origin.lat, origin.lon),
        &points
            .points
            .iter()
            .map(|p| orqwa_routes::WorldPoint {
                east: p.east,
                north: p.north,
                elevation: p.elevation,
                surface: match p.surface {
                    world::Surface::Ground => orqwa_routes::Surface::Ground,
                    world::Surface::Bridge => orqwa_routes::Surface::Bridge,
                    world::Surface::Tunnel => orqwa_routes::Surface::Tunnel,
                },
            })
            .collect::<Vec<_>>(),
    )?;
    let config = description
        .script()?
        .map(|s| s.config.clone())
        .unwrap_or_default();
    let rider = WorldRider {
        name: "Rider".into(),
        ftp: Watts(args.ftp),
        look: if args.look == "male" {
            orqwa_domain::profile::Avatar::Male
        } else {
            orqwa_domain::profile::Avatar::Female
        },
    };
    let mut ride = Ride::new(route, RideConfig::default())
        .with_world_script(WorldDriver::new(script, config, &id, rider));
    ride.on_telemetry(&Telemetry {
        power: Some(Watts(args.power)),
        cadence: Some(Rpm(90.0)),
        ..Telemetry::default()
    });
    let tick = orqwa_session::world::TICK;
    let per_second = 1_000 / u32::try_from(tick.as_millis()).unwrap_or(100);
    for step in 0..args.seconds * per_second {
        if step % per_second == 0 && args.actions.contains(&(step / per_second)) {
            ride.world_action();
        }
        let control = ride.tick(tick);
        if !(args.trace || step % per_second == 0 || control.is_some()) {
            continue;
        }
        let state = ride.state();
        let world = ride.world().context("the script was dropped")?;
        println!(
            "{:7.1} s {:8.1} m {:5.1} km/h {:?} trainer {:?}",
            state.elapsed.as_secs_f64(),
            state.distance.0,
            state.speed.as_kilometers_per_hour(),
            world.movement(),
            control,
        );
        if args.trace {
            println!("  {}", serde_json::to_string(world.output())?);
        }
        if let Some(stopped) = world.stopped() {
            println!("the script stopped: {stopped}");
            break;
        }
        if ride.is_finished() {
            break;
        }
    }
    let world = ride.world().context("the script was dropped")?;
    for (key, value) in world.values() {
        println!("result {key}: {value}");
    }
    for (at, mark) in world.marks() {
        println!("mark {} at {:.1} s", mark.name, at.as_secs_f64());
    }
    Ok(())
}
