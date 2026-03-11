use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use cpal::{
    Device, SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::visualizer::VisualizerAnalyzer;

pub struct CpalCapture {
    _stream: Stream,
    analyzer: Arc<Mutex<VisualizerAnalyzer>>,
    device_name: String,
    active: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

impl CpalCapture {
    pub fn open() -> Result<Self, String> {
        let host = cpal::default_host();
        let target = CaptureTarget::detect(&host)?;
        let analyzer = Arc::new(Mutex::new(VisualizerAnalyzer::new(
            target.config.sample_rate.0,
        )));
        let active = Arc::new(AtomicBool::new(true));
        let error = Arc::new(Mutex::new(None));
        let stream = build_stream(
            &target.device,
            &target.config,
            target.sample_format,
            analyzer.clone(),
            active.clone(),
            error.clone(),
        )?;

        stream
            .play()
            .map_err(|err| format!("failed to start audio capture: {err}"))?;

        Ok(Self {
            _stream: stream,
            analyzer,
            device_name: target.device_name,
            active,
            error,
        })
    }

    pub fn frame(&self) -> Result<crate::visualizer::SpectrumFrame, String> {
        if let Some(error) = self.take_error() {
            return Err(error);
        }

        let mut analyzer = self
            .analyzer
            .lock()
            .map_err(|_| "visualizer analyzer lock poisoned".to_string())?;
        Ok(analyzer.current_frame())
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut error| error.take())
    }
}

struct CaptureTarget {
    device: Device,
    device_name: String,
    config: StreamConfig,
    sample_format: SampleFormat,
}

impl CaptureTarget {
    fn detect(host: &cpal::Host) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
            let device = host
                .default_output_device()
                .ok_or_else(|| "no default output device is available".to_string())?;
            let device_name = device_name(&device);
            let supported = device.default_output_config().map_err(|err| {
                format!("could not read the default output format for {device_name}: {err}")
            })?;

            return Ok(Self::from_supported(device, supported, device_name));
        }

        #[cfg(target_os = "linux")]
        {
            let default_output_name = host
                .default_output_device()
                .map(|device| device_name(&device));
            let devices = host
                .input_devices()
                .map_err(|err| format!("could not enumerate input devices: {err}"))?;

            let mut best: Option<(i32, Device, String)> = None;
            for device in devices {
                let name = device_name(&device);
                let score = linux_monitor_score(&name, default_output_name.as_deref());
                if score < 0 {
                    continue;
                }

                match &best {
                    Some((best_score, _, _)) if *best_score >= score => {}
                    _ => best = Some((score, device, name)),
                }
            }

            let Some((_, device, device_name)) = best else {
                return Err(
                    "no Linux monitor device was found. Use PipeWire/PulseAudio monitor capture and try again."
                        .to_string(),
                );
            };
            let supported = device.default_input_config().map_err(|err| {
                format!("could not read the default input format for {device_name}: {err}")
            })?;

            return Ok(Self::from_supported(device, supported, device_name));
        }

        #[cfg(target_os = "macos")]
        {
            let device = host.default_input_device().ok_or_else(|| {
                "no input device is available. Install BlackHole or another loopback device and try again."
                    .to_string()
            })?;
            let device_name = device_name(&device);
            let supported = device.default_input_config().map_err(|err| {
                format!("could not read the default input format for {device_name}: {err}")
            })?;

            return Ok(Self::from_supported(device, supported, device_name));
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let device = host
                .default_input_device()
                .ok_or_else(|| "no input device is available".to_string())?;
            let device_name = device_name(&device);
            let supported = device.default_input_config().map_err(|err| {
                format!("could not read the default input format for {device_name}: {err}")
            })?;

            Ok(Self::from_supported(device, supported, device_name))
        }
    }

    fn from_supported(
        device: Device,
        supported: SupportedStreamConfig,
        device_name: String,
    ) -> Self {
        Self {
            device,
            device_name,
            config: supported.config(),
            sample_format: supported.sample_format(),
        }
    }
}

fn build_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    analyzer: Arc<Mutex<VisualizerAnalyzer>>,
    active: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<Stream, String> {
    if config.channels == 0 {
        return Err("visualizer capture device reported zero channels".to_string());
    }

    match sample_format {
        SampleFormat::F32 => {
            build_stream_for_format::<f32, _>(device, config, analyzer, active, error, |v| v)
        }
        SampleFormat::I16 => {
            build_stream_for_format::<i16, _>(device, config, analyzer, active, error, |v| {
                v as f32 / i16::MAX as f32
            })
        }
        SampleFormat::U16 => {
            build_stream_for_format::<u16, _>(device, config, analyzer, active, error, |v| {
                (v as f32 - (u16::MAX as f32 / 2.0)) / (u16::MAX as f32 / 2.0)
            })
        }
        other => Err(format!("unsupported audio sample format: {other:?}")),
    }
}

fn build_stream_for_format<T, F>(
    device: &Device,
    config: &StreamConfig,
    analyzer: Arc<Mutex<VisualizerAnalyzer>>,
    active: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    convert: F,
) -> Result<Stream, String>
where
    T: cpal::SizedSample,
    F: Fn(T) -> f32 + Send + Copy + 'static,
{
    let channels = config.channels as usize;
    let error_for_callback = error.clone();
    let active_for_callback = active.clone();
    let mut mono = Vec::new();

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                mono.clear();
                mono.reserve(data.len() / channels + 1);

                for frame in data.chunks(channels) {
                    let sum = frame
                        .iter()
                        .copied()
                        .map(convert)
                        .fold(0.0, |acc, sample| acc + sample);
                    mono.push(sum / frame.len() as f32);
                }

                if let Ok(mut analyzer) = analyzer.lock() {
                    analyzer.push_samples(&mono);
                }
            },
            move |stream_error| {
                active_for_callback.store(false, Ordering::Relaxed);
                if let Ok(mut slot) = error_for_callback.lock() {
                    *slot = Some(format!("audio capture failed: {stream_error}"));
                }
            },
            None,
        )
        .map_err(|err| format!("failed to build audio capture stream: {err}"))
}

fn device_name(device: &Device) -> String {
    device
        .name()
        .unwrap_or_else(|_| "Unknown device".to_string())
}

#[cfg(target_os = "linux")]
fn linux_monitor_score(name: &str, default_output_name: Option<&str>) -> i32 {
    let name_lower = normalize_for_match(name);
    if !name_lower.contains("monitor") && !name_lower.contains("loopback") {
        return -1;
    }

    let mut score = 0;
    if name_lower.contains("monitor") {
        score += 40;
    }
    if name_lower.contains("loopback") {
        score += 40;
    }
    if name_lower.contains("pipewire") || name_lower.contains("pulse") {
        score += 10;
    }
    if let Some(output_name) = default_output_name {
        let output_lower = normalize_for_match(output_name);
        if name_lower.contains(&output_lower) || output_lower.contains(&name_lower) {
            score += 25;
        } else if shared_terms(&name_lower, &output_lower) >= 2 {
            score += 15;
        }
    }

    score
}

#[cfg(target_os = "linux")]
fn shared_terms(left: &str, right: &str) -> usize {
    left.split_whitespace()
        .filter(|term| term.len() > 2 && right.contains(term))
        .count()
}

#[cfg(target_os = "linux")]
fn normalize_for_match(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
