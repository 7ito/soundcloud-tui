#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    LoadTrack {
        url: String,
        title: String,
        authorization: Option<String>,
    },
    Play,
    Pause,
    TogglePause,
    Stop,
    SeekRelative {
        seconds: f64,
    },
    SeekAbsolute {
        seconds: f64,
    },
    SetVolume {
        percent: f64,
    },
    Shutdown,
}
