#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    PlaybackStarted,
    PlaybackPaused,
    PlaybackResumed,
    PlaybackStopped,
    TrackEnded,
    PositionChanged { seconds: f64 },
    DurationChanged { seconds: Option<f64> },
    VolumeChanged { percent: f64 },
    BackendError(String),
}
