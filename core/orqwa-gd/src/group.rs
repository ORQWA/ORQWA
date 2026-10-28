//! Group view dictionaries; membership, readiness and lifecycle remain in the app core.
use godot::prelude::*;
use orqwa_app::App;
use std::time::Duration;

pub(super) fn status(app: &App) -> VarDictionary {
    let status = app.group();
    let mut roster = VarArray::new();
    let mut devices = VarArray::new();
    for (id, name, input, output) in &status.devices {
        devices.push(
            &vdict! {"id"=>id.as_str(),"name"=>name.as_str(),"input"=>*input,"output"=>*output}
                .to_variant(),
        );
    }
    let local = app.ride_state().map_or(0.0, |s| s.distance.0);
    for (id, rider) in status.roster.iter().enumerate() {
        let Some(rider) = rider else {
            continue;
        };
        let sample = status.states[id].filter(|(_, at)| at.elapsed() < Duration::from_secs(2));
        roster.push(&vdict!{"id"=>i64::try_from(id).unwrap_or(0),"name"=>rider.name.as_str(),"ready"=>status.ready[id],"me"=>id==usize::from(status.me),"muted"=>status.muted[id],"present"=>sample.is_some(),"gap_m"=>sample.map_or(0.0,|(s,_)|s.distance.0-local),"power_w"=>sample.map_or(0.0,|(s,_)|s.power.0)}.to_variant());
    }
    vdict! {"phase"=>status.phase.key(),"host"=>status.host,"invite"=>status.invite.as_str(),"reply"=>status.reply.as_str(),"prepared"=>status.prepared,"received"=>i64::try_from(status.received).unwrap_or(0),"total"=>i64::try_from(status.total).unwrap_or(0),"ready"=>status.ready[usize::from(status.me)],"can_start"=>status.can_start(),"name"=>status.selection.as_ref().map_or("",|s|s.name.as_str()),"error"=>status.error.as_str(),"roster"=>&roster,"devices"=>&devices,"voice"=>status.voice,"voice_pending"=>status.voice_pending,"microphone"=>status.microphone,"deafened"=>status.deafened,"countdown_s"=>status.start.map_or(0.0,|at|at.saturating_duration_since(std::time::Instant::now()).as_secs_f64()),"workout_id"=>status.workout_id.as_str()}
}
