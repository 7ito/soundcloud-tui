#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    Track,
    Queue,
}

impl RepeatMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Track => "Track",
            Self::Queue => "Queue",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackIntent {
    Play,
    Pause,
    TogglePause,
    Stop,
    Next,
    Previous,
    SeekRelative { seconds: f64 },
    SeekAbsolute { seconds: f64 },
    SetVolume { percent: f64 },
    SetShuffle(bool),
    SetRepeat(RepeatMode),
}
