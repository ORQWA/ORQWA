# Open questions and findings

Things to review or decide together. Newest first; remove entries once settled. Work that is
decided but not done goes into a GitHub issue instead.

## The Zwift Click v2 driver is unchecked (R77, 2026-10-16)

`torqa_devices::zwift_click_v2` was written from the community's drivers (BikeControl, QZ,
MaximumTrainer; ADR 0003), with no pod at hand. Three things need a Click v2 owner to confirm,
best with `torqa-cli gear-check --controller "Zwift Click v2"` and `RUST_LOG=torqa_devices=debug`:
the pods' device types 10 and 11 in the advertising data (BikeControl marks them unconfirmed
and still ships them), the buttons' bits in the `0x23` map (two tables circulate; the one in
BikeControl's code is used), and whether the handshake every 3 s keeps a pod awake without
upsetting it (QZ settled on it after drops; MaximumTrainer went back and forth). Also whether
the left pod needs the `ff 04 00` acknowledgement to stay unlocked, or stays so anyway.
