use anyhow::{Context, Result};
use log::{debug, info};
use mpris_server::{
    LoopStatus as MprisLoopStatus, Metadata, PlaybackStatus as MprisPlaybackStatus, Player, Time,
    TrackId,
};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::{
    app::{AppEvent, AppState, PlaybackIntent, RepeatMode},
    soundcloud::models::TrackSummary,
};

const SEEKED_THRESHOLD_MICROS: i64 = 1_500_000;
const TRACK_ID_PREFIX: &str = "/io/github/tito/soundcloud_tui/track";

pub struct MprisIntegration {
    player: Player,
    last_projection: Option<MprisProjection>,
}

#[derive(Debug, Clone, PartialEq)]
struct MprisProjection {
    metadata: Metadata,
    track_id_path: String,
    playback_status: MprisPlaybackStatus,
    position: Time,
    volume: f64,
    shuffle: bool,
    loop_status: MprisLoopStatus,
    can_go_next: bool,
    can_go_previous: bool,
    can_play: bool,
    can_pause: bool,
    can_seek: bool,
}

impl MprisIntegration {
    pub async fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Result<Self> {
        let player = Player::builder("soundcloud-tui")
            .identity("soundcloud-tui")
            .desktop_entry("soundcloud-tui")
            .supported_uri_schemes(["http", "https"])
            .metadata(empty_metadata())
            .playback_status(MprisPlaybackStatus::Stopped)
            .position(Time::ZERO)
            .volume(0.5)
            .shuffle(false)
            .loop_status(MprisLoopStatus::None)
            .can_control(true)
            .can_play(false)
            .can_pause(false)
            .can_seek(false)
            .can_go_next(false)
            .can_go_previous(false)
            .build()
            .await
            .context("could not register org.mpris.MediaPlayer2.soundcloud-tui")?;

        connect_callbacks(&player, sender);

        let run_task = player.run();
        tokio::task::spawn_local(async move {
            run_task.await;
        });

        info!("registered soundcloud-tui on MPRIS");

        Ok(Self {
            player,
            last_projection: None,
        })
    }

    pub async fn sync_from_app(&mut self, app: &AppState) -> Result<()> {
        let projection = MprisProjection::from_app(app)?;
        let previous = self.last_projection.as_ref();

        if previous
            .map(|value| value.metadata != projection.metadata)
            .unwrap_or(true)
        {
            self.player
                .set_metadata(projection.metadata.clone())
                .await?;
        }

        if previous
            .map(|value| value.playback_status != projection.playback_status)
            .unwrap_or(true)
        {
            self.player
                .set_playback_status(projection.playback_status)
                .await?;
        }

        if previous
            .map(|value| (value.volume - projection.volume).abs() > f64::EPSILON)
            .unwrap_or(true)
        {
            self.player.set_volume(projection.volume).await?;
        }

        if previous
            .map(|value| value.shuffle != projection.shuffle)
            .unwrap_or(true)
        {
            self.player.set_shuffle(projection.shuffle).await?;
        }

        if previous
            .map(|value| value.loop_status != projection.loop_status)
            .unwrap_or(true)
        {
            self.player.set_loop_status(projection.loop_status).await?;
        }

        if previous
            .map(|value| value.can_go_next != projection.can_go_next)
            .unwrap_or(true)
        {
            self.player.set_can_go_next(projection.can_go_next).await?;
        }

        if previous
            .map(|value| value.can_go_previous != projection.can_go_previous)
            .unwrap_or(true)
        {
            self.player
                .set_can_go_previous(projection.can_go_previous)
                .await?;
        }

        if previous
            .map(|value| value.can_play != projection.can_play)
            .unwrap_or(true)
        {
            self.player.set_can_play(projection.can_play).await?;
        }

        if previous
            .map(|value| value.can_pause != projection.can_pause)
            .unwrap_or(true)
        {
            self.player.set_can_pause(projection.can_pause).await?;
        }

        if previous
            .map(|value| value.can_seek != projection.can_seek)
            .unwrap_or(true)
        {
            self.player.set_can_seek(projection.can_seek).await?;
        }

        let emit_seeked = previous
            .map(|value| {
                value.track_id_path == projection.track_id_path
                    && (value.position.as_micros() - projection.position.as_micros()).abs()
                        >= SEEKED_THRESHOLD_MICROS
            })
            .unwrap_or(false);

        self.player.set_position(projection.position);
        if emit_seeked {
            self.player.seeked(projection.position).await?;
        }

        self.last_projection = Some(projection);
        Ok(())
    }
}

impl MprisProjection {
    fn from_app(app: &AppState) -> Result<Self> {
        let current_track = app.now_playing.track.as_ref();
        let track_id_path = current_track
            .map(track_id_path)
            .unwrap_or_else(|| TrackId::NO_TRACK.as_str().to_string());

        Ok(Self {
            metadata: metadata_for_track(current_track)?,
            track_id_path,
            playback_status: playback_status_for_app(app),
            position: seconds_to_time(app.player.position_seconds),
            volume: (app.player.volume_percent / 100.0).clamp(0.0, 1.0),
            shuffle: app.player.shuffle_enabled,
            loop_status: loop_status_for_repeat_mode(app.player.repeat_mode),
            can_go_next: can_go_next(app),
            can_go_previous: can_go_previous(app),
            can_play: current_track.is_some(),
            can_pause: current_track.is_some(),
            can_seek: current_track.is_some(),
        })
    }
}

fn connect_callbacks(player: &Player, sender: mpsc::UnboundedSender<AppEvent>) {
    let play_tx = sender.clone();
    player.connect_play(move |_player| {
        debug!("received MPRIS Play");
        let _ = play_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::Play));
    });

    let pause_tx = sender.clone();
    player.connect_pause(move |_player| {
        debug!("received MPRIS Pause");
        let _ = pause_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::Pause));
    });

    let toggle_tx = sender.clone();
    player.connect_play_pause(move |_player| {
        debug!("received MPRIS PlayPause");
        let _ = toggle_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::TogglePause));
    });

    let next_tx = sender.clone();
    player.connect_next(move |_player| {
        debug!("received MPRIS Next");
        let _ = next_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::Next));
    });

    let previous_tx = sender.clone();
    player.connect_previous(move |_player| {
        debug!("received MPRIS Previous");
        let _ = previous_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::Previous));
    });

    let stop_tx = sender.clone();
    player.connect_stop(move |_player| {
        debug!("received MPRIS Stop");
        let _ = stop_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::Stop));
    });

    let seek_tx = sender.clone();
    player.connect_seek(move |_player, offset| {
        debug!("received MPRIS Seek: {} micros", offset.as_micros());
        let _ = seek_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::SeekRelative {
            seconds: time_to_seconds(offset),
        }));
    });

    let set_position_tx = sender.clone();
    player.connect_set_position(move |_player, _track_id, position| {
        debug!(
            "received MPRIS SetPosition: {} micros",
            position.as_micros()
        );
        let _ = set_position_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::SeekAbsolute {
            seconds: time_to_seconds(position),
        }));
    });

    let set_volume_tx = sender.clone();
    player.connect_set_volume(move |_player, volume| {
        debug!("received MPRIS SetVolume: {volume}");
        let _ = set_volume_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::SetVolume {
            percent: (volume * 100.0).clamp(0.0, 100.0),
        }));
    });

    let set_shuffle_tx = sender.clone();
    player.connect_set_shuffle(move |_player, shuffle| {
        debug!("received MPRIS SetShuffle: {shuffle}");
        let _ = set_shuffle_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::SetShuffle(
            shuffle,
        )));
    });

    let set_loop_tx = sender;
    player.connect_set_loop_status(move |_player, loop_status| {
        debug!("received MPRIS SetLoopStatus: {:?}", loop_status);
        let _ = set_loop_tx.send(AppEvent::PlaybackIntent(PlaybackIntent::SetRepeat(
            repeat_mode_for_loop_status(loop_status),
        )));
    });
}

fn metadata_for_track(track: Option<&TrackSummary>) -> Result<Metadata> {
    let Some(track) = track else {
        return Ok(empty_metadata());
    };

    let mut metadata = Metadata::new();
    metadata.set_trackid(Some(track_id(track)?));
    metadata.set_title(Some(track.title.clone()));
    metadata.set_artist(Some([track.artist.clone()]));
    metadata.set_length(track.duration_ms.map(duration_to_time));
    metadata.set_url(track.permalink_url.clone());
    metadata.set_art_url(track.artwork_url.clone());
    Ok(metadata)
}

fn empty_metadata() -> Metadata {
    let mut metadata = Metadata::new();
    metadata.set_trackid(Some(TrackId::NO_TRACK));
    metadata
}

fn playback_status_for_app(app: &AppState) -> MprisPlaybackStatus {
    match app.player.status {
        crate::app::state::PlaybackStatus::Playing
        | crate::app::state::PlaybackStatus::Buffering => MprisPlaybackStatus::Playing,
        crate::app::state::PlaybackStatus::Paused => MprisPlaybackStatus::Paused,
        crate::app::state::PlaybackStatus::Stopped => MprisPlaybackStatus::Stopped,
    }
}

fn can_go_next(app: &AppState) -> bool {
    app.can_play_next_track()
}

fn can_go_previous(app: &AppState) -> bool {
    app.can_play_previous_track()
}

fn repeat_mode_for_loop_status(loop_status: MprisLoopStatus) -> RepeatMode {
    match loop_status {
        MprisLoopStatus::None => RepeatMode::Off,
        MprisLoopStatus::Track => RepeatMode::Track,
        MprisLoopStatus::Playlist => RepeatMode::Queue,
    }
}

fn loop_status_for_repeat_mode(repeat_mode: RepeatMode) -> MprisLoopStatus {
    match repeat_mode {
        RepeatMode::Off => MprisLoopStatus::None,
        RepeatMode::Track => MprisLoopStatus::Track,
        RepeatMode::Queue => MprisLoopStatus::Playlist,
    }
}

fn duration_to_time(duration_ms: u64) -> Time {
    let micros = duration_ms.saturating_mul(1000).min(i64::MAX as u64) as i64;
    Time::from_micros(micros)
}

fn seconds_to_time(seconds: f64) -> Time {
    let micros = (seconds.max(0.0) * 1_000_000.0).round();
    let micros = micros.clamp(0.0, i64::MAX as f64) as i64;
    Time::from_micros(micros)
}

fn time_to_seconds(time: Time) -> f64 {
    time.as_micros() as f64 / 1_000_000.0
}

fn track_id(track: &TrackSummary) -> Result<TrackId> {
    TrackId::try_from(track_id_path(track)).context("could not build an MPRIS track id")
}

fn track_id_path(track: &TrackSummary) -> String {
    let digest = Sha256::digest(track.urn.as_bytes());
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{TRACK_ID_PREFIX}/{suffix}")
}
