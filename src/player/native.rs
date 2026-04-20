use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc, Mutex, Once,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
};
use log::{debug, info, warn};

use crate::{
    config::paths::AppPaths,
    player::{backend::PlayerBackend, command::PlayerCommand, event::PlayerEvent},
    visualizer::VisualizerTap,
};

const OUTPUT_BUFFER_SECONDS: usize = 2;
const TARGET_BUFFER_MILLIS: usize = 400;
const WORKER_TICK: Duration = Duration::from_millis(15);
const MAX_REFILL_PACKETS_PER_TICK: usize = 8;
const MAX_REFILL_TIME_PER_TICK: Duration = Duration::from_millis(10);
const POSITION_EVENT_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_VOLUME_PERCENT: f32 = 50.0;

pub struct NativePlayerBackend {
    shared: Arc<Mutex<OutputState>>,
    event_tx: Sender<PlayerEvent>,
    event_rx: Receiver<PlayerEvent>,
    current_generation: Arc<AtomicU64>,
    active_decoder: Option<ActiveDecoder>,
    output_channels: usize,
    output_rate: u32,
    _stream: Stream,
}

struct ActiveDecoder {
    command_tx: Sender<DecoderCommand>,
}

enum DecoderCommand {
    SeekRelative { seconds: f64 },
    SeekAbsolute { seconds: f64 },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderLoopDirective {
    Continue,
    Stop,
}

impl NativePlayerBackend {
    pub fn spawn(_paths: &AppPaths, visualizer_tap: VisualizerTap) -> Result<Self> {
        init_ffmpeg()?;

        let output = OutputStreamContext::open(visualizer_tap)?;
        let (event_tx, event_rx) = mpsc::channel();

        Ok(Self {
            shared: output.shared,
            event_tx,
            event_rx,
            current_generation: Arc::new(AtomicU64::new(0)),
            active_decoder: None,
            output_channels: output.channels,
            output_rate: output.sample_rate,
            _stream: output.stream,
        })
    }

    fn next_generation(&self) -> u64 {
        self.current_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn replace_active_decoder(&mut self, next: Option<ActiveDecoder>) {
        if let Some(active) = self.active_decoder.take() {
            let _ = active.command_tx.send(DecoderCommand::Shutdown);
        }

        self.active_decoder = next;
    }

    fn spawn_decoder(&mut self, generation: u64, request: TrackLoadRequest) {
        let (command_tx, command_rx) = mpsc::channel();
        let event_tx = self.event_tx.clone();
        let shared = self.shared.clone();
        let current_generation = self.current_generation.clone();
        let output_channels = self.output_channels;
        let output_rate = self.output_rate;

        thread::spawn(move || {
            if let Err(error) = decoder_worker_loop(
                generation,
                request,
                command_rx,
                event_tx.clone(),
                shared,
                current_generation.clone(),
                output_channels,
                output_rate,
            ) {
                if generation_is_current(&current_generation, generation) {
                    let _ = event_tx.send(PlayerEvent::BackendError(error.to_string()));
                }
                warn!("native decoder worker exited with error: {error}");
            }
        });

        self.replace_active_decoder(Some(ActiveDecoder { command_tx }));
    }

    fn send_decoder_command(&mut self, command: DecoderCommand) {
        let send_failed = self
            .active_decoder
            .as_ref()
            .map(|active| active.command_tx.send(command).is_err())
            .unwrap_or(false);

        if send_failed {
            self.active_decoder = None;
        }
    }

    fn handle_command(&mut self, command: PlayerCommand) -> Result<()> {
        match command {
            PlayerCommand::LoadTrack {
                url,
                title,
                authorization,
                duration_seconds,
            } => {
                let generation = self.next_generation();
                clear_output(&self.shared, 0.0)?;
                self.spawn_decoder(
                    generation,
                    TrackLoadRequest {
                        url,
                        title,
                        authorization,
                        duration_seconds,
                    },
                );
            }
            PlayerCommand::Play => {
                let mut state = self
                    .shared
                    .lock()
                    .map_err(|_| anyhow!("output state lock poisoned"))?;
                if state.paused {
                    state.paused = false;
                    let _ = self.event_tx.send(PlayerEvent::PlaybackResumed);
                }
            }
            PlayerCommand::Pause => {
                let mut state = self
                    .shared
                    .lock()
                    .map_err(|_| anyhow!("output state lock poisoned"))?;
                if !state.paused {
                    state.paused = true;
                    let _ = self.event_tx.send(PlayerEvent::PlaybackPaused);
                }
            }
            PlayerCommand::TogglePause => {
                let mut state = self
                    .shared
                    .lock()
                    .map_err(|_| anyhow!("output state lock poisoned"))?;
                state.paused = !state.paused;
                let event = if state.paused {
                    PlayerEvent::PlaybackPaused
                } else {
                    PlayerEvent::PlaybackResumed
                };
                let _ = self.event_tx.send(event);
            }
            PlayerCommand::Stop => {
                self.next_generation();
                clear_output(&self.shared, 0.0)?;
                let had_active_decoder = self.active_decoder.is_some();
                self.replace_active_decoder(None);
                if had_active_decoder {
                    let _ = self.event_tx.send(PlayerEvent::PlaybackStopped);
                }
            }
            PlayerCommand::SeekRelative { seconds } => {
                self.send_decoder_command(DecoderCommand::SeekRelative { seconds });
            }
            PlayerCommand::SeekAbsolute { seconds } => {
                self.send_decoder_command(DecoderCommand::SeekAbsolute { seconds });
            }
            PlayerCommand::SetVolume { percent } => {
                let clamped = percent.clamp(0.0, 100.0) as f32;
                let mut state = self
                    .shared
                    .lock()
                    .map_err(|_| anyhow!("output state lock poisoned"))?;
                state.volume = clamped / 100.0;
                let _ = self.event_tx.send(PlayerEvent::VolumeChanged {
                    percent: clamped as f64,
                });
            }
            PlayerCommand::Shutdown => {
                self.next_generation();
                self.replace_active_decoder(None);
            }
        }

        Ok(())
    }
}

impl PlayerBackend for NativePlayerBackend {
    fn send(&mut self, command: PlayerCommand) -> Result<()> {
        debug!("sending native player command: {}", command_label(&command));
        self.handle_command(command)
    }

    fn poll_event(&mut self) -> Result<Option<PlayerEvent>> {
        match self.event_rx.try_recv() {
            Ok(event) => {
                if matches!(
                    event,
                    PlayerEvent::TrackEnded | PlayerEvent::BackendError(_) | PlayerEvent::PlaybackStopped
                ) {
                    self.active_decoder = None;
                }
                Ok(Some(event))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                bail!("native player worker disconnected unexpectedly")
            }
        }
    }
}

impl Drop for NativePlayerBackend {
    fn drop(&mut self) {
        let _ = self.handle_command(PlayerCommand::Shutdown);
    }
}

struct OutputStreamContext {
    stream: Stream,
    shared: Arc<Mutex<OutputState>>,
    channels: usize,
    sample_rate: u32,
}

impl OutputStreamContext {
    fn open(visualizer_tap: VisualizerTap) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default audio output device is available"))?;

        let supported = device
            .default_output_config()
            .or_else(|_| preferred_output_config(&device))?;
        let config = supported.config();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;
        let max_samples = sample_rate as usize * channels * OUTPUT_BUFFER_SECONDS;
        let shared = Arc::new(Mutex::new(OutputState::new(
            channels,
            sample_rate,
            max_samples,
        )));

        visualizer_tap.configure(sample_rate);
        let stream = build_output_stream(
            &device,
            &config,
            supported.sample_format(),
            shared.clone(),
            visualizer_tap,
        )?;
        stream
            .play()
            .context("failed to start native audio output stream")?;

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Unknown device".to_string());
        info!(
            "native audio output ready: device={}, channels={}, sample_rate={}Hz",
            device_name, channels, sample_rate
        );

        Ok(Self {
            stream,
            shared,
            channels,
            sample_rate,
        })
    }
}

struct OutputState {
    buffer: VecDeque<f32>,
    channels: usize,
    sample_rate: u32,
    max_samples: usize,
    paused: bool,
    volume: f32,
    played_frames: u64,
    error: Option<String>,
}

impl OutputState {
    fn new(channels: usize, sample_rate: u32, max_samples: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_samples.min(16_384)),
            channels,
            sample_rate,
            max_samples,
            paused: false,
            volume: DEFAULT_VOLUME_PERCENT / 100.0,
            played_frames: 0,
            error: None,
        }
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }

    fn clear_to_position(&mut self, seconds: f64) {
        self.clear();
        self.played_frames = seconds_to_frames(seconds, self.sample_rate);
    }

    fn position_seconds(&self) -> f64 {
        self.played_frames as f64 / self.sample_rate as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionAdvance {
    Active { produced_audio: bool },
    Stale,
}

impl SessionAdvance {
    fn active(produced_audio: bool) -> Self {
        Self::Active { produced_audio }
    }

    fn produced_audio(self) -> bool {
        match self {
            Self::Active { produced_audio } => produced_audio,
            Self::Stale => false,
        }
    }
}

fn preferred_output_config(device: &cpal::Device) -> Result<SupportedStreamConfig> {
    let mut stereo_f32 = None;
    let mut stereo_other = None;

    for range in device
        .supported_output_configs()
        .context("could not enumerate output configs")?
    {
        if range.channels() != 2 {
            continue;
        }

        let candidate = range.with_max_sample_rate();
        if candidate.sample_format() == SampleFormat::F32 {
            stereo_f32 = Some(candidate);
            break;
        }
        stereo_other = Some(candidate);
    }

    stereo_f32
        .or(stereo_other)
        .ok_or_else(|| anyhow!("no supported stereo output configuration was found"))
}

fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    shared: Arc<Mutex<OutputState>>,
    visualizer_tap: VisualizerTap,
) -> Result<Stream> {
    match sample_format {
        SampleFormat::F32 => build_output_stream_for::<f32>(device, config, shared, visualizer_tap),
        SampleFormat::I16 => build_output_stream_for::<i16>(device, config, shared, visualizer_tap),
        SampleFormat::U16 => build_output_stream_for::<u16>(device, config, shared, visualizer_tap),
        other => bail!("unsupported output sample format: {other:?}"),
    }
}

fn build_output_stream_for<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Mutex<OutputState>>,
    visualizer_tap: VisualizerTap,
) -> Result<Stream>
where
    T: cpal::Sample + SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let error_state = shared.clone();

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut rendered = Vec::with_capacity(data.len());

                let mut state = match shared.lock() {
                    Ok(state) => state,
                    Err(_) => return,
                };

                let mut popped_samples = 0usize;
                for sample in data.iter_mut() {
                    let value = if state.paused {
                        0.0
                    } else if let Some(next) = state.buffer.pop_front() {
                        popped_samples += 1;
                        next * state.volume
                    } else {
                        0.0
                    };

                    rendered.push(value);
                    *sample = T::from_sample(value);
                }

                state.played_frames = state
                    .played_frames
                    .saturating_add((popped_samples / channels) as u64);
                drop(state);

                if popped_samples > 0 {
                    visualizer_tap.push_interleaved(&rendered[..popped_samples], channels);
                }
            },
            move |error| {
                if let Ok(mut state) = error_state.lock() {
                    state.error = Some(format!("audio output failed: {error}"));
                }
            },
            None,
        )
        .context("failed to build native output stream")
}

struct PlaybackSession {
    title: String,
    input: ffmpeg::format::context::Input,
    stream_index: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    output_rate: u32,
    output_channels: usize,
    output_layout_mask: ffmpeg::ChannelLayoutMask,
    duration_seconds: Option<f64>,
    started_pending: bool,
    draining: bool,
    finished: bool,
}

impl PlaybackSession {
    fn open(request: TrackLoadRequest, output_rate: u32, output_channels: usize) -> Result<Self> {
        let mut options = ffmpeg::Dictionary::new();
        options.set("user_agent", "soundcloud-tui");
        options.set("reconnect", "1");
        options.set("reconnect_streamed", "1");
        options.set("reconnect_on_network_error", "1");
        if let Some(authorization) = request.authorization.as_deref() {
            options.set(
                "headers",
                &format!("Authorization: Bearer {authorization}\r\n"),
            );
        }

        let input = ffmpeg::format::input_with_dictionary(&request.url, options)
            .with_context(|| format!("could not open audio stream for {}", request.title))?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .ok_or_else(|| anyhow!("SoundCloud did not expose a playable audio stream"))?;
        let stream_index = stream.index();
        let time_base = stream.time_base();
        let stream_duration_seconds = if stream.duration() > 0 {
            Some(stream.duration() as f64 * f64::from(time_base))
        } else if input.duration() > 0 {
            Some(input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE))
        } else {
            request.duration_seconds
        };

        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .context("could not read audio stream parameters")?;
        let mut decoder = context
            .decoder()
            .audio()
            .context("could not create audio decoder")?;
        decoder.set_parameters(stream.parameters())?;

        let output_layout = output_channel_layout(output_channels);
        let output_layout_mask = output_layout
            .mask()
            .ok_or_else(|| anyhow!("could not determine output channel layout mask"))?;
        let resampler = create_resampler(&decoder, output_rate, output_channels)
            .context("could not create audio resampler")?;

        info!(
            "opened native stream: title={}, sample_rate={}Hz, channels={}, duration={:?}",
            request.title,
            decoder.rate(),
            decoder_channel_layout(&decoder).channels(),
            stream_duration_seconds
        );

        Ok(Self {
            title: request.title,
            input,
            stream_index,
            decoder,
            resampler,
            output_rate,
            output_channels,
            output_layout_mask,
            duration_seconds: stream_duration_seconds,
            started_pending: true,
            draining: false,
            finished: false,
        })
    }

    fn fill_buffer(
        &mut self,
        shared: &Arc<Mutex<OutputState>>,
        target_samples: usize,
        current_generation: &AtomicU64,
        generation: u64,
    ) -> Result<SessionAdvance> {
        let started = Instant::now();
        let mut processed_packets = 0usize;
        let mut pushed_any = false;

        while buffer_len(shared)? < target_samples
            && !self.finished
            && processed_packets < MAX_REFILL_PACKETS_PER_TICK
            && started.elapsed() < MAX_REFILL_TIME_PER_TICK
        {
            if !generation_is_current(current_generation, generation) {
                return Ok(SessionAdvance::Stale);
            }

            match self.input.packets().next() {
                Some(Ok((stream, packet))) => {
                    if !generation_is_current(current_generation, generation) {
                        return Ok(SessionAdvance::Stale);
                    }

                    if stream.index() != self.stream_index {
                        continue;
                    }

                    self.decoder
                        .send_packet(&packet)
                        .with_context(|| format!("could not decode {}", self.title))?;
                    match self.receive_frames(shared, current_generation, generation)? {
                        SessionAdvance::Active { produced_audio } => pushed_any |= produced_audio,
                        SessionAdvance::Stale => return Ok(SessionAdvance::Stale),
                    }
                    processed_packets += 1;
                }
                None => {
                    if !self.draining {
                        self.decoder
                            .send_eof()
                            .with_context(|| format!("could not drain decoder for {}", self.title))?;
                        self.draining = true;
                    }

                    let decoded = self.receive_frames(shared, current_generation, generation)?;
                    if matches!(decoded, SessionAdvance::Stale) {
                        return Ok(SessionAdvance::Stale);
                    }
                    let decoded_any = decoded.produced_audio();
                    pushed_any |= decoded_any;
                    let flushed_any = if decoded_any {
                        false
                    } else {
                        let flushed =
                            self.flush_resampler(shared, current_generation, generation)?;
                        if matches!(flushed, SessionAdvance::Stale) {
                            return Ok(SessionAdvance::Stale);
                        }
                        pushed_any |= flushed.produced_audio();
                        flushed.produced_audio()
                    };

                    if !decoded_any && !flushed_any {
                        self.finished = true;
                        break;
                    }
                }
                Some(Err(error)) => {
                    return Err(error)
                        .with_context(|| format!("could not read packets for {}", self.title));
                }
            }
        }

        Ok(SessionAdvance::active(pushed_any))
    }

    fn seek_to(
        &mut self,
        seconds: f64,
        shared: &Arc<Mutex<OutputState>>,
        current_generation: &AtomicU64,
        generation: u64,
    ) -> Result<SessionAdvance> {
        let timestamp = (seconds.max(0.0) * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64;
        self.input
            .seek(timestamp, ..timestamp)
            .with_context(|| format!("could not seek {}", self.title))?;
        self.decoder.flush();
        self.resampler = create_resampler(&self.decoder, self.output_rate, self.output_channels)
            .with_context(|| format!("could not reset audio resampler for {}", self.title))?;
        self.draining = false;
        self.finished = false;

        let mut state = shared
            .lock()
            .map_err(|_| anyhow!("output state lock poisoned"))?;
        if !generation_is_current(current_generation, generation) {
            return Ok(SessionAdvance::Stale);
        }
        state.clear_to_position(seconds);
        Ok(SessionAdvance::active(false))
    }

    fn receive_frames(
        &mut self,
        shared: &Arc<Mutex<OutputState>>,
        current_generation: &AtomicU64,
        generation: u64,
    ) -> Result<SessionAdvance> {
        let mut pushed_any = false;
        let mut decoded = ffmpeg::frame::Audio::empty();

        while self.decoder.receive_frame(&mut decoded).is_ok() {
            if !generation_is_current(current_generation, generation) {
                return Ok(SessionAdvance::Stale);
            }

            let output_samples = self.resampler_output_samples(decoded.samples())?;
            if output_samples == 0 {
                continue;
            }

            let mut converted = self.allocate_output_frame(output_samples);
            self.resampler
                .run(&decoded, &mut converted)
                .with_context(|| format!("could not resample {}", self.title))?;
            match self.push_converted_samples(&converted, shared, current_generation, generation)? {
                SessionAdvance::Active { produced_audio } => pushed_any |= produced_audio,
                SessionAdvance::Stale => return Ok(SessionAdvance::Stale),
            }
        }

        Ok(SessionAdvance::active(pushed_any))
    }

    fn flush_resampler(
        &mut self,
        shared: &Arc<Mutex<OutputState>>,
        current_generation: &AtomicU64,
        generation: u64,
    ) -> Result<SessionAdvance> {
        let mut pushed_any = false;

        loop {
            if !generation_is_current(current_generation, generation) {
                return Ok(SessionAdvance::Stale);
            }

            let output_samples = self.resampler_output_samples(0)?;
            if output_samples == 0 {
                break;
            }

            let mut converted = self.allocate_output_frame(output_samples);
            let delay = self
                .resampler
                .flush(&mut converted)
                .with_context(|| format!("could not flush audio resampler for {}", self.title))?;
            let produced_samples = converted.samples();
            match self.push_converted_samples(&converted, shared, current_generation, generation)? {
                SessionAdvance::Active { produced_audio } => {
                    pushed_any |= produced_audio || produced_samples > 0;
                }
                SessionAdvance::Stale => return Ok(SessionAdvance::Stale),
            }

            if delay.is_none() || produced_samples == 0 {
                break;
            }
        }

        Ok(SessionAdvance::active(pushed_any))
    }

    fn allocate_output_frame(&self, output_samples: usize) -> ffmpeg::frame::Audio {
        let mut frame = ffmpeg::frame::Audio::empty();
        unsafe {
            frame.alloc(
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                output_samples,
                self.output_layout_mask,
            );
        }
        frame
    }

    fn resampler_output_samples(&mut self, input_samples: usize) -> Result<usize> {
        let output_samples = unsafe {
            ffmpeg::ffi::swr_get_out_samples(self.resampler.as_mut_ptr(), input_samples as i32)
        };

        if output_samples < 0 {
            return Err(ffmpeg::Error::from(output_samples).into());
        }

        Ok(output_samples as usize)
    }

    fn push_converted_samples(
        &mut self,
        frame: &ffmpeg::frame::Audio,
        shared: &Arc<Mutex<OutputState>>,
        current_generation: &AtomicU64,
        generation: u64,
    ) -> Result<SessionAdvance> {
        let bytes = frame.data(0);
        let samples: &[f32] = unsafe {
            std::slice::from_raw_parts(
                bytes.as_ptr().cast::<f32>(),
                bytes.len() / std::mem::size_of::<f32>(),
            )
        };
        let sample_count = frame.samples() * self.output_channels;

        if sample_count == 0 || samples.is_empty() {
            return Ok(SessionAdvance::active(false));
        }

        let mut state = shared
            .lock()
            .map_err(|_| anyhow!("output state lock poisoned"))?;
        if !generation_is_current(current_generation, generation) {
            return Ok(SessionAdvance::Stale);
        }
        let available = samples.len().min(sample_count);
        let remaining = state.max_samples.saturating_sub(state.buffer.len());
        let to_copy = available.min(remaining);
        state.buffer.extend(samples.iter().take(to_copy).copied());
        Ok(SessionAdvance::active(to_copy > 0))
    }
}

#[derive(Debug)]
struct TrackLoadRequest {
    url: String,
    title: String,
    authorization: Option<String>,
    duration_seconds: Option<f64>,
}

fn decoder_worker_loop(
    generation: u64,
    request: TrackLoadRequest,
    command_rx: Receiver<DecoderCommand>,
    event_tx: Sender<PlayerEvent>,
    shared: Arc<Mutex<OutputState>>,
    current_generation: Arc<AtomicU64>,
    output_channels: usize,
    output_rate: u32,
) -> Result<()> {
    let target_samples = output_rate as usize * output_channels * TARGET_BUFFER_MILLIS / 1000;
    let mut session = PlaybackSession::open(request, output_rate, output_channels)?;
    if !generation_is_current(&current_generation, generation) {
        return Ok(());
    }

    if let Some(duration) = session.duration_seconds {
        let _ = event_tx.send(PlayerEvent::DurationChanged {
            seconds: Some(duration),
        });
    }

    let mut last_position_event = Instant::now();

    loop {
        match command_rx.recv_timeout(WORKER_TICK) {
            Ok(command) => {
                if handle_decoder_command(
                    command,
                    &mut session,
                    &shared,
                    &event_tx,
                    &current_generation,
                    generation,
                    &mut last_position_event,
                )? == DecoderLoopDirective::Stop
                {
                    break;
                }

                while let Ok(command) = command_rx.try_recv() {
                    if handle_decoder_command(
                        command,
                        &mut session,
                        &shared,
                        &event_tx,
                        &current_generation,
                        generation,
                        &mut last_position_event,
                    )? == DecoderLoopDirective::Stop
                    {
                        return Ok(());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if !generation_is_current(&current_generation, generation) {
            break;
        }

        if let Some(error) = take_output_error(&shared)? {
            if generation_is_current(&current_generation, generation) {
                clear_output(&shared, 0.0)?;
                let _ = event_tx.send(PlayerEvent::BackendError(error));
            }
            break;
        }

        let paused = is_paused(&shared)?;

        if !paused {
            match session.fill_buffer(&shared, target_samples, &current_generation, generation)? {
                SessionAdvance::Active { .. } => {}
                SessionAdvance::Stale => break,
            }

            if session.started_pending && buffer_len(&shared)? > 0 {
                session.started_pending = false;
                if generation_is_current(&current_generation, generation) {
                    let _ = event_tx.send(PlayerEvent::PlaybackStarted);
                }
            }

            if last_position_event.elapsed() >= POSITION_EVENT_INTERVAL
                && (!session.started_pending || buffer_len(&shared)? > 0)
                && generation_is_current(&current_generation, generation)
            {
                let _ = event_tx.send(PlayerEvent::PositionChanged {
                    seconds: current_position(&shared)?,
                });
                last_position_event = Instant::now();
            }
        }

        if session.finished && buffer_len(&shared)? == 0 {
            if generation_is_current(&current_generation, generation) {
                clear_output(&shared, 0.0)?;
                let _ = event_tx.send(PlayerEvent::TrackEnded);
            }
            break;
        }
    }

    Ok(())
}

fn handle_decoder_command(
    command: DecoderCommand,
    session: &mut PlaybackSession,
    shared: &Arc<Mutex<OutputState>>,
    event_tx: &Sender<PlayerEvent>,
    current_generation: &AtomicU64,
    generation: u64,
    last_position_event: &mut Instant,
) -> Result<DecoderLoopDirective> {
    match command {
        DecoderCommand::Shutdown => return Ok(DecoderLoopDirective::Stop),
        DecoderCommand::SeekRelative { seconds } => {
            let target = (current_position(shared)? + seconds).max(0.0);
            match session.seek_to(target, shared, current_generation, generation)? {
                SessionAdvance::Active { .. } => {
                    if generation_is_current(current_generation, generation) {
                        let _ = event_tx.send(PlayerEvent::PositionChanged { seconds: target });
                    }
                }
                SessionAdvance::Stale => return Ok(DecoderLoopDirective::Stop),
            }
        }
        DecoderCommand::SeekAbsolute { seconds } => {
            let target = seconds.max(0.0);
            match session.seek_to(target, shared, current_generation, generation)? {
                SessionAdvance::Active { .. } => {
                    if generation_is_current(current_generation, generation) {
                        let _ = event_tx.send(PlayerEvent::PositionChanged { seconds: target });
                    }
                }
                SessionAdvance::Stale => return Ok(DecoderLoopDirective::Stop),
            }
        }
    }

    *last_position_event = Instant::now();
    Ok(DecoderLoopDirective::Continue)
}

fn generation_is_current(current_generation: &AtomicU64, generation: u64) -> bool {
    current_generation.load(Ordering::SeqCst) == generation
}

fn init_ffmpeg() -> Result<()> {
    static INIT: Once = Once::new();
    static RESULT: Mutex<Option<String>> = Mutex::new(None);

    INIT.call_once(|| {
        ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Error);
        let error = ffmpeg::init().err().map(|error| error.to_string());
        if let Ok(mut slot) = RESULT.lock() {
            *slot = error;
        }
    });

    let slot = RESULT
        .lock()
        .map_err(|_| anyhow!("ffmpeg init state lock poisoned"))?;
    if let Some(error) = slot.as_ref() {
        bail!("could not initialize FFmpeg: {error}");
    }
    Ok(())
}

fn decoder_channel_layout(decoder: &ffmpeg::decoder::Audio) -> ffmpeg::ChannelLayout<'static> {
    let layout = decoder.ch_layout();

    if let Some(mask) = layout.mask() {
        ffmpeg::ChannelLayout::from_mask(mask)
            .unwrap_or_else(|| ffmpeg::ChannelLayout::default_for_channels(layout.channels()))
    } else {
        ffmpeg::ChannelLayout::default_for_channels(layout.channels())
    }
}

fn output_channel_layout(channels: usize) -> ffmpeg::ChannelLayout<'static> {
    match channels {
        1 => ffmpeg::ChannelLayout::MONO,
        2 => ffmpeg::ChannelLayout::STEREO,
        _ => ffmpeg::ChannelLayout::default_for_channels(channels as u32),
    }
}

fn create_resampler(
    decoder: &ffmpeg::decoder::Audio,
    output_rate: u32,
    output_channels: usize,
) -> Result<ffmpeg::software::resampling::Context> {
    ffmpeg::software::resampling::Context::get2(
        decoder.format(),
        decoder_channel_layout(decoder),
        decoder.rate(),
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
        output_channel_layout(output_channels),
        output_rate,
    )
    .map_err(Into::into)
}

fn clear_output(shared: &Arc<Mutex<OutputState>>, seconds: f64) -> Result<()> {
    let mut state = shared
        .lock()
        .map_err(|_| anyhow!("output state lock poisoned"))?;
    state.paused = false;
    state.clear_to_position(seconds);
    Ok(())
}

fn buffer_len(shared: &Arc<Mutex<OutputState>>) -> Result<usize> {
    let state = shared
        .lock()
        .map_err(|_| anyhow!("output state lock poisoned"))?;
    Ok(state.buffer.len())
}

fn current_position(shared: &Arc<Mutex<OutputState>>) -> Result<f64> {
    let state = shared
        .lock()
        .map_err(|_| anyhow!("output state lock poisoned"))?;
    Ok(state.position_seconds())
}

fn is_paused(shared: &Arc<Mutex<OutputState>>) -> Result<bool> {
    let state = shared
        .lock()
        .map_err(|_| anyhow!("output state lock poisoned"))?;
    Ok(state.paused)
}

fn take_output_error(shared: &Arc<Mutex<OutputState>>) -> Result<Option<String>> {
    let mut state = shared
        .lock()
        .map_err(|_| anyhow!("output state lock poisoned"))?;
    Ok(state.error.take())
}

fn seconds_to_frames(seconds: f64, sample_rate: u32) -> u64 {
    (seconds.max(0.0) * sample_rate as f64).round() as u64
}

fn command_label(command: &PlayerCommand) -> &'static str {
    match command {
        PlayerCommand::LoadTrack { .. } => "load_track",
        PlayerCommand::Play => "play",
        PlayerCommand::Pause => "pause",
        PlayerCommand::TogglePause => "toggle_pause",
        PlayerCommand::Stop => "stop",
        PlayerCommand::SeekRelative { .. } => "seek_relative",
        PlayerCommand::SeekAbsolute { .. } => "seek_absolute",
        PlayerCommand::SetVolume { .. } => "set_volume",
        PlayerCommand::Shutdown => "shutdown",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    static FFMPEG_INIT: Once = Once::new();

    fn init_ffmpeg_for_tests() {
        FFMPEG_INIT.call_once(|| {
            ffmpeg::init().expect("ffmpeg should initialize for tests");
        });
    }

    fn stereo_f32_frame(sample_rate: u32, samples: usize) -> ffmpeg::frame::Audio {
        let mut frame = ffmpeg::frame::Audio::empty();
        frame.set_rate(sample_rate);
        unsafe {
            frame.alloc(
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                samples,
                ffmpeg::ChannelLayout::STEREO
                    .mask()
                    .expect("stereo layout should expose a mask"),
            );
        }
        frame
    }

    #[test]
    fn resampler_preallocation_and_flush_preserve_duration_when_upsampling() {
        init_ffmpeg_for_tests();

        let input_rate = 44_100;
        let output_rate = 48_000;
        let mut resampler = ffmpeg::software::resampling::Context::get2(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::STEREO,
            input_rate,
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::STEREO,
            output_rate,
        )
        .expect("resampler should be created");

        let output_layout_mask = ffmpeg::ChannelLayout::STEREO
            .mask()
            .expect("stereo layout should expose a mask");
        let mut total_input_samples = 0usize;
        let mut total_output_samples = 0usize;

        for _ in 0..1_000 {
            let input = stereo_f32_frame(input_rate, 1024);
            let output_samples = unsafe {
                ffmpeg::ffi::swr_get_out_samples(resampler.as_mut_ptr(), input.samples() as i32)
            };
            assert!(output_samples > 0, "resampler should report output capacity");

            let mut output = ffmpeg::frame::Audio::empty();
            unsafe {
                output.alloc(
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    output_samples as usize,
                    output_layout_mask,
                );
            }
            resampler
                .run(&input, &mut output)
                .expect("resampling should succeed");
            total_input_samples += input.samples();
            total_output_samples += output.samples();
        }

        loop {
            let output_samples = unsafe { ffmpeg::ffi::swr_get_out_samples(resampler.as_mut_ptr(), 0) };
            if output_samples <= 0 {
                break;
            }

            let mut output = ffmpeg::frame::Audio::empty();
            unsafe {
                output.alloc(
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    output_samples as usize,
                    output_layout_mask,
                );
            }
            let delay = resampler
                .flush(&mut output)
                .expect("resampler flush should succeed");
            let produced_samples = output.samples();
            total_output_samples += produced_samples;

            if delay.is_none() || produced_samples == 0 {
                break;
            }
        }

        let input_seconds = total_input_samples as f64 / input_rate as f64;
        let output_seconds = total_output_samples as f64 / output_rate as f64;
        assert!(
            (output_seconds - input_seconds).abs() < 0.01,
            "expected resampled duration to match input duration, input={input_seconds}, output={output_seconds}",
        );
    }
}
