use std::{
    mem,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use pipewire as pw;
use pw::spa::{
    param::audio::{AudioFormat, AudioInfoRaw},
    pod::{Pod, Value, serialize::PodSerializer},
    utils::{Direction, SpaTypes},
};

use crate::visualizer::{SpectrumFrame, VisualizerAnalyzer};

pub struct PipeWireCapture {
    analyzer: Arc<Mutex<VisualizerAnalyzer>>,
    active: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    _thread: thread::JoinHandle<()>,
}

impl PipeWireCapture {
    pub fn open() -> Result<Self, String> {
        let analyzer = Arc::new(Mutex::new(VisualizerAnalyzer::new(48_000)));
        let active = Arc::new(AtomicBool::new(true));
        let error = Arc::new(Mutex::new(None));

        let analyzer_for_thread = analyzer.clone();
        let active_for_thread = active.clone();
        let error_for_thread = error.clone();

        let thread = thread::Builder::new()
            .name("visualizer-pipewire".to_string())
            .spawn(move || {
                if let Err(message) = run_pipewire_capture(
                    analyzer_for_thread,
                    active_for_thread.clone(),
                    error_for_thread.clone(),
                ) {
                    active_for_thread.store(false, Ordering::Relaxed);
                    if let Ok(mut slot) = error_for_thread.lock() {
                        *slot = Some(message);
                    }
                }
            })
            .map_err(|err| format!("failed to spawn PipeWire capture thread: {err}"))?;

        std::thread::sleep(std::time::Duration::from_millis(150));

        if let Some(message) = error.lock().ok().and_then(|slot| slot.clone()) {
            return Err(message);
        }

        Ok(Self {
            analyzer,
            active,
            error,
            _thread: thread,
        })
    }

    pub fn frame(&self) -> Result<SpectrumFrame, String> {
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
        "PipeWire default sink monitor"
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut error| error.take())
    }
}

#[derive(Default)]
struct StreamData {
    channels: std::sync::atomic::AtomicU32,
    sample_rate: std::sync::atomic::AtomicU32,
    format: std::sync::Mutex<AudioInfoRaw>,
}

fn run_pipewire_capture(
    analyzer: Arc<Mutex<VisualizerAnalyzer>>,
    active: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<(), String> {
    let mainloop = pw::main_loop::MainLoopBox::new(None)
        .map_err(|err| format!("could not create PipeWire main loop: {err}"))?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)
        .map_err(|err| format!("could not create PipeWire context: {err}"))?;
    let core = context
        .connect(None)
        .map_err(|err| format!("could not connect to PipeWire: {err}"))?;

    let props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::NODE_NAME => "soundcloud-tui-visualizer",
        *pw::keys::NODE_DESCRIPTION => "soundcloud-tui visualizer",
        *pw::keys::STREAM_CAPTURE_SINK => "true",
    };

    let stream = pw::stream::StreamBox::new(&core, "soundcloud-tui-visualizer", props)
        .map_err(|err| format!("could not create PipeWire stream: {err}"))?;

    let active_for_process = active.clone();
    let error_for_stop = error.clone();
    let analyzer_for_process = analyzer.clone();

    let _listener = stream
        .add_local_listener_with_user_data(StreamData::default())
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }

            if let Ok(mut format) = user_data.format.lock() {
                if format.parse(param).is_ok() {
                    user_data
                        .channels
                        .store(format.channels().max(1), Ordering::Relaxed);
                    user_data
                        .sample_rate
                        .store(format.rate().max(1), Ordering::Relaxed);
                }
            }
        })
        .process(move |stream, user_data| {
            if !active_for_process.load(Ordering::Relaxed) {
                return;
            }

            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            let Some(data) = datas.get_mut(0) else {
                return;
            };

            let chunk = data.chunk();
            let n_bytes = chunk.size() as usize;
            if n_bytes == 0 {
                return;
            }

            let channels = user_data.channels.load(Ordering::Relaxed).max(1) as usize;
            let sample_rate = user_data.sample_rate.load(Ordering::Relaxed).max(1);

            if let Some(bytes) = data.data() {
                let valid_bytes = &bytes[..n_bytes.min(bytes.len())];
                let mono_samples = bytes_to_mono(valid_bytes, channels);
                if mono_samples.is_empty() {
                    return;
                }

                if let Ok(mut analyzer) = analyzer_for_process.lock() {
                    if analyzer.sample_rate() != sample_rate {
                        *analyzer = VisualizerAnalyzer::new(sample_rate);
                    }
                    analyzer.push_samples(&mono_samples);
                }
            }
        })
        .register()
        .map_err(|err| format!("could not register PipeWire stream listener: {err}"))?;

    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    audio_info.set_rate(48_000);
    audio_info.set_channels(2);

    let obj = pw::spa::pod::Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: pw::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };

    let values = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
        .map_err(|_| "could not serialize PipeWire format request".to_string())?
        .0
        .into_inner();
    let pod = Pod::from_bytes(&values)
        .ok_or_else(|| "could not construct PipeWire format request".to_string())?;
    let mut params = [pod];

    stream
        .connect(
            Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut params,
        )
        .map_err(|err| format!("could not connect to the PipeWire monitor stream: {err}"))?;

    mainloop.run();

    if active.load(Ordering::Relaxed) {
        if let Ok(mut slot) = error_for_stop.lock() {
            if slot.is_none() {
                *slot = Some("PipeWire capture loop stopped unexpectedly".to_string());
            }
        }
        active.store(false, Ordering::Relaxed);
    }

    Ok(())
}

fn bytes_to_mono(bytes: &[u8], channels: usize) -> Vec<f32> {
    let sample_width = mem::size_of::<f32>();
    let frame_width = channels * sample_width;
    if frame_width == 0 {
        return Vec::new();
    }

    let frame_count = bytes.len() / frame_width;
    let mut mono = Vec::with_capacity(frame_count);

    for frame in bytes.chunks_exact(frame_width) {
        let mut sum = 0.0;
        for sample in frame.chunks_exact(sample_width).take(channels) {
            let mut raw = [0_u8; mem::size_of::<f32>()];
            raw.copy_from_slice(sample);
            sum += f32::from_le_bytes(raw);
        }
        mono.push(sum / channels as f32);
    }

    mono
}

#[cfg(test)]
mod tests {
    use super::bytes_to_mono;

    #[test]
    fn bytes_to_mono_averages_channels() {
        let samples = [0.5_f32, -0.5_f32, 1.0_f32, 0.0_f32];
        let mut bytes = Vec::new();
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        let mono = bytes_to_mono(&bytes, 2);

        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.0).abs() < 0.0001);
        assert!((mono[1] - 0.5).abs() < 0.0001);
    }
}
