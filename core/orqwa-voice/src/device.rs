//! Native audio streams with bounded callback queues and no network work in callbacks.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use orqwa_domain::voice::{
    CapturedVoice, FRAME_SAMPLES, PcmFrame, SAMPLE_RATE, VoiceAudio, VoiceDeviceError,
};

/// Device IDs from [`devices`]. Unspecified IDs use the operating system's defaults.
#[derive(Clone, Debug, Default)]
pub struct DeviceOptions {
    /// Microphone ID; no input stream opens until explicit unmute.
    pub input: Option<String>,
    /// Speaker/headphone output ID.
    pub output: Option<String>,
}
/// One available local audio device; enumeration does not capture microphone samples.
#[derive(Debug)]
pub struct AudioDevice {
    /// Stable CPAL device ID used by voice input/output options.
    pub id: String,
    /// Human-readable local device name.
    pub name: String,
    /// Whether the device has input configurations.
    pub input: bool,
    /// Whether the device has output configurations.
    pub output: bool,
}
/// Lists local audio devices without opening capture or playback.
///
/// # Errors
/// Returns [`VoiceDeviceError`] if the OS cannot enumerate devices.
pub fn devices() -> Result<Vec<AudioDevice>, VoiceDeviceError> {
    let host = cpal::default_host();
    host.devices()
        .map_err(|_| VoiceDeviceError)?
        .map(|device| {
            Ok(AudioDevice {
                id: device.id().map_err(|_| VoiceDeviceError)?.to_string(),
                name: device.to_string(),
                input: device.supports_input(),
                output: device.supports_output(),
            })
        })
        .collect()
}

struct Gates {
    stop: Arc<AtomicBool>,
    activation: AtomicU32,
    deafened: AtomicBool,
    revision: AtomicU32,
    failed: AtomicBool,
}
struct Output {
    samples: PcmFrame,
    at: Instant,
    revision: u32,
}

/// CPAL audio backend. Its streams remain on the owning audio thread.
pub struct NativeAudio {
    options: DeviceOptions,
    gates: Arc<Gates>,
    _output: Stream,
    input: Option<Stream>,
    capture_send: SyncSender<CapturedVoice>,
    capture_receive: Receiver<CapturedVoice>,
    play_send: SyncSender<Output>,
}
impl NativeAudio {
    /// Opens output only. Capture is constructed lazily by [`VoiceAudio::microphone`].
    ///
    /// # Errors
    /// Returns [`VoiceDeviceError`] for unavailable devices or unsupported configurations.
    pub fn open(options: DeviceOptions, stop: Arc<AtomicBool>) -> Result<Self, VoiceDeviceError> {
        let host = cpal::default_host();
        let device = choose(&host, options.output.as_deref(), false)?;
        let supported = device
            .default_output_config()
            .map_err(|_| VoiceDeviceError)?;
        let format = supported.sample_format();
        let config = configuration(&supported);
        validate(&config)?;
        let gates = Arc::new(Gates {
            stop,
            activation: AtomicU32::new(0),
            deafened: AtomicBool::new(false),
            revision: AtomicU32::new(0),
            failed: AtomicBool::new(false),
        });
        let (play_send, receive) = sync_channel(4);
        let output = match format {
            SampleFormat::F32 => output::<f32>(&device, config, receive, gates.clone()),
            SampleFormat::F64 => output::<f64>(&device, config, receive, gates.clone()),
            SampleFormat::I8 => output::<i8>(&device, config, receive, gates.clone()),
            SampleFormat::I16 => output::<i16>(&device, config, receive, gates.clone()),
            SampleFormat::I32 => output::<i32>(&device, config, receive, gates.clone()),
            SampleFormat::I64 => output::<i64>(&device, config, receive, gates.clone()),
            SampleFormat::U8 => output::<u8>(&device, config, receive, gates.clone()),
            SampleFormat::U16 => output::<u16>(&device, config, receive, gates.clone()),
            SampleFormat::U32 => output::<u32>(&device, config, receive, gates.clone()),
            SampleFormat::U64 => output::<u64>(&device, config, receive, gates.clone()),
            _ => return Err(VoiceDeviceError),
        }?;
        output.play().map_err(|_| VoiceDeviceError)?;
        let (capture_send, capture_receive) = sync_channel(4);
        Ok(Self {
            options,
            gates,
            _output: output,
            input: None,
            capture_send,
            capture_receive,
            play_send,
        })
    }
}

fn choose(
    host: &cpal::Host,
    id: Option<&str>,
    input: bool,
) -> Result<cpal::Device, VoiceDeviceError> {
    match id {
        Some(id) => host.device_by_id(&id.parse().map_err(|_| VoiceDeviceError)?),
        None if input => host.default_input_device(),
        None => host.default_output_device(),
    }
    .ok_or(VoiceDeviceError)
}
fn validate(config: &StreamConfig) -> Result<(), VoiceDeviceError> {
    if !(1..=8).contains(&config.channels) || !(8000..=192_000).contains(&config.sample_rate) {
        return Err(VoiceDeviceError);
    }
    Ok(())
}
fn configuration(supported: &cpal::SupportedStreamConfig) -> StreamConfig {
    let mut config = supported.config();
    if let cpal::SupportedBufferSize::Range { min, max } = supported.buffer_size()
        && min <= max
    {
        config.buffer_size = cpal::BufferSize::Fixed((config.sample_rate / 100).clamp(*min, *max));
    }
    config
}
impl VoiceAudio for NativeAudio {
    fn microphone(&mut self, activation: Option<u32>) -> Result<(), VoiceDeviceError> {
        self.gates.activation.store(0, Ordering::Release);
        self.input = None;
        while self.capture_receive.try_recv().is_ok() {}
        let Some(activation) = activation else {
            return Ok(());
        };
        if activation == 0 || self.gates.stop.load(Ordering::Acquire) {
            return Err(VoiceDeviceError);
        }
        let host = cpal::default_host();
        let device = choose(&host, self.options.input.as_deref(), true)?;
        let supported = device
            .default_input_config()
            .map_err(|_| VoiceDeviceError)?;
        let format = supported.sample_format();
        let config = configuration(&supported);
        validate(&config)?;
        let send = self.capture_send.clone();
        let stream = match format {
            SampleFormat::F32 => {
                input::<f32>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::F64 => {
                input::<f64>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::I8 => input::<i8>(&device, config, send, self.gates.clone(), activation),
            SampleFormat::I16 => {
                input::<i16>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::I32 => {
                input::<i32>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::I64 => {
                input::<i64>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::U8 => input::<u8>(&device, config, send, self.gates.clone(), activation),
            SampleFormat::U16 => {
                input::<u16>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::U32 => {
                input::<u32>(&device, config, send, self.gates.clone(), activation)
            }
            SampleFormat::U64 => {
                input::<u64>(&device, config, send, self.gates.clone(), activation)
            }
            _ => return Err(VoiceDeviceError),
        }?;
        if self.gates.stop.load(Ordering::Acquire) {
            return Err(VoiceDeviceError);
        }
        self.gates.activation.store(activation, Ordering::Release);
        stream.play().map_err(|_| {
            self.gates.activation.store(0, Ordering::Release);
            VoiceDeviceError
        })?;
        self.input = Some(stream);
        Ok(())
    }
    fn capture(&mut self) -> Option<CapturedVoice> {
        self.capture_receive.try_recv().ok()
    }
    fn play(&mut self, frame: PcmFrame) {
        if !self.gates.deafened.load(Ordering::Relaxed) {
            let _ = self.play_send.try_send(Output {
                samples: frame,
                at: Instant::now(),
                revision: self.gates.revision.load(Ordering::Relaxed),
            });
        }
    }
    fn deafen(&mut self, deafened: bool) {
        self.gates.deafened.store(deafened, Ordering::Release);
        self.gates.revision.fetch_add(1, Ordering::AcqRel);
    }
    fn healthy(&self) -> bool {
        !self.gates.failed.load(Ordering::Relaxed)
    }
}
impl Drop for NativeAudio {
    fn drop(&mut self) {
        self.gates.activation.store(0, Ordering::Release);
        self.gates.deafened.store(true, Ordering::Release);
    }
}

fn input<T>(
    device: &cpal::Device,
    config: StreamConfig,
    send: SyncSender<CapturedVoice>,
    gates: Arc<Gates>,
    activation: u32,
) -> Result<Stream, VoiceDeviceError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let mut resampler = Resampler::new(config.sample_rate);
    let sample_rate = config.sample_rate;
    let mut samples = [0.0; FRAME_SAMPLES];
    let mut count = 0;
    let failures = gates.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if gates.stop.load(Ordering::Acquire)
                    || gates.activation.load(Ordering::Acquire) != activation
                {
                    count = 0;
                    return;
                }
                let frames = data.len() / channels;
                let callback_at = Instant::now();
                for (index, frame) in data.chunks_exact(channels).enumerate() {
                    let mut mono = 0.0_f32;
                    for sample in frame {
                        mono += sample.to_sample::<f32>();
                    }
                    #[allow(clippy::cast_precision_loss)] // channel count is bounded to eight
                    {
                        mono /= channels as f32;
                    }
                    if !mono.is_finite() {
                        mono = 0.0;
                    }
                    mono = mono.clamp(-1.0, 1.0);
                    resampler.push(mono, |sample| {
                        samples[count] = sample;
                        count += 1;
                        if count == FRAME_SAMPLES {
                            #[allow(clippy::cast_precision_loss)] // callback sizes are below 2^53
                            let remaining = (frames - index - 1) as f64 / f64::from(sample_rate);
                            let at = callback_at
                                .checked_sub(std::time::Duration::from_secs_f64(remaining))
                                .unwrap_or(callback_at);
                            if at.elapsed() <= std::time::Duration::from_millis(60) {
                                let _ = send.try_send(CapturedVoice {
                                    samples,
                                    activation,
                                    at,
                                });
                            }
                            count = 0;
                        }
                    });
                }
            },
            move |_| {
                failures.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|_| VoiceDeviceError)
}

struct Resampler {
    previous: f32,
    position: f64,
    step: f64,
}
impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            previous: 0.0,
            position: 1.0,
            step: f64::from(rate) / f64::from(SAMPLE_RATE),
        }
    }
    fn push(&mut self, mono: f32, mut emit: impl FnMut(f32)) {
        while self.position <= 1.0 {
            #[allow(clippy::cast_possible_truncation)] // interpolation fraction is in [0,1]
            emit(self.previous + (mono - self.previous) * self.position as f32);
            self.position += self.step;
        }
        self.position -= 1.0;
        self.previous = mono;
    }
}

#[allow(clippy::cast_precision_loss)] // frame length is exactly 960, representable as f64
fn output<T>(
    device: &cpal::Device,
    config: StreamConfig,
    receive: Receiver<Output>,
    gates: Arc<Gates>,
) -> Result<Stream, VoiceDeviceError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let step = f64::from(SAMPLE_RATE) / f64::from(config.sample_rate);
    let mut samples = [0.0; FRAME_SAMPLES];
    let mut position = FRAME_SAMPLES as f64;
    let mut revision = 0;
    let failures = gates.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let current = gates.revision.load(Ordering::Acquire);
                if gates.stop.load(Ordering::Acquire)
                    || gates.deafened.load(Ordering::Acquire)
                    || revision != current
                {
                    while receive.try_recv().is_ok() {}
                    samples.fill(0.0);
                    position = FRAME_SAMPLES as f64;
                    revision = current;
                    data.fill(T::from_sample(0.0));
                    return;
                }
                for frame in data.chunks_exact_mut(channels) {
                    if position >= FRAME_SAMPLES as f64 {
                        position -= FRAME_SAMPLES as f64;
                        samples.fill(0.0);
                        while let Ok(next) = receive.try_recv() {
                            if next.revision == revision
                                && next.at.elapsed() <= std::time::Duration::from_millis(100)
                            {
                                samples = next.samples;
                                break;
                            }
                        }
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    // phase is maintained in [0,960)
                    let index = position as usize;
                    let value = samples[index];
                    for sample in frame {
                        *sample = T::from_sample(value);
                    }
                    position += step;
                }
            },
            move |_| {
                failures.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|_| VoiceDeviceError)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_rates_produce_forty_eight_thousand_finite_mono_samples_per_second() {
        for rate in [8000, 16_000, 44_100, 48_000, 96_000, 192_000] {
            let mut resampler = Resampler::new(rate);
            let mut count = 0_u32;
            for _ in 0..rate {
                resampler.push(0.25, |sample| {
                    assert!(sample.is_finite() && (sample - 0.25).abs() < 0.001);
                    count += 1;
                });
            }
            assert!(
                count.abs_diff(SAMPLE_RATE) <= 6,
                "native rate {rate}, count {count}"
            );
        }
    }
}
