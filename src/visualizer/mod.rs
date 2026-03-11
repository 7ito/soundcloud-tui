pub mod analyzer;
pub mod cpal_capture;
#[cfg(target_os = "linux")]
pub mod pipewire_capture;
pub mod runtime;

pub use analyzer::{SpectrumFrame, VISUALIZER_BANDS, VisualizerAnalyzer};
pub use runtime::{VisualizerCommand, VisualizerHandle};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VisualizerStyle {
    Equalizer,
    BarGraph,
}

impl Default for VisualizerStyle {
    fn default() -> Self {
        Self::Equalizer
    }
}

impl VisualizerStyle {
    pub fn next(self) -> Self {
        match self {
            Self::Equalizer => Self::BarGraph,
            Self::BarGraph => Self::Equalizer,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Equalizer => "Equalizer",
            Self::BarGraph => "Bar Graph",
        }
    }
}
