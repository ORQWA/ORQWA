# ADR 0016 — Power meters and cadence sources beside the trainer

- Status: accepted
- Date: 2026-10-14

## Context

R71 adds a power meter, such as Favero Assioma power pedals, beside the trainer, and R72 lets
the rider choose where the cadence comes from, including a dedicated cadence sensor. Trainer
and meter both report power, and often cadence, several times a second. `Telemetry::merge` keeps whichever
device spoke last, so the rider's power would flip between the two. Riders add a power meter
because they trust it more than the trainer, or to keep their numbers the same outdoors and
indoors.

Power meters talk the Bluetooth **Cycling Power Service** (`0x1818`). Its measurement has no
cadence field: a meter that knows its cadence sends a crank revolution counter and the time of
the last revolution (1/1024 s, wrapping every 64 s). Many trainers offer the Cycling Power
Service too, next to FTMS. Cadence sensors talk the **Cycling Speed and Cadence Service**
(`0x1816`), with the same crank counter.

## Decision

- **Standard drivers only**: `torqa_devices::cycling_power` for every meter that offers the
  Cycling Power Service, `torqa_devices::speed_cadence` for cadence sensors, with no vendor
  code. A device that offers FTMS stays a trainer, even if it also offers the Cycling Power
  Service.
- **Cadence from the crank counter**: revolutions over time between two measurements. If the
  counter stands still for 2.5 s, cadence is 0 (coasting). The first revolution after
  coasting gives no new value, as its time may be more than 64 s after the last one, so the
  time counter may have wrapped. Readings above 250 rpm are dropped. The calculation starts
  over after each reconnect.
- **Each device keeps its latest reading** in the ride, and the rider's values are chosen
  from them after every report. Power: the meter's, else the trainer's. Cadence: the chosen
  source's (`CadenceSource`: automatic, trainer or power meter), else the others', in the
  automatic order (sensor, meter, trainer). A device that drops loses its readings, so the
  next one stands in at once. When the trainer drops, the meter's power still moves the
  rider.
- **Choosing a cadence sensor sets the source to automatic**, where a connected sensor comes
  first. Choosing a source other than a sensor lets go of the sensor and stops reconnecting
  it. The choice is kept per installation, like the devices.
- **The trainer is still required**: it sets the resistance, and rides start when it
  connects. A meter on its own (riding a dumb trainer) is not supported yet.
- **ERG stays with the trainer**: in ERG the trainer holds the target by its own
  measurement. Matching the meter (nudging the target until the meter reads it) is a later,
  separate step.

## Consequences

- The rider's power, records and analysis come from the device they trust most, with no
  setting to choose between them; only the cadence has a choice, as pedals estimate it
  differently from trainers and sensors.
- A meter that reads a few percent off the trainer makes ERG targets look slightly missed
  until power matching follows.
- No `SensorDriver` trait yet: like the heart-rate strap, the meter runs as a `DeviceHandle`.
  The trait comes with a second transport (ANT+).
- The meter's left/right balance is read past and not recorded yet.
