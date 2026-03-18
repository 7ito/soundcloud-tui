pub mod analyzer;
pub mod runtime;
pub mod tap;

pub use analyzer::{SpectrumFrame, VISUALIZER_BANDS, VisualizerAnalyzer};
pub use runtime::{VisualizerCommand, VisualizerHandle};
pub use tap::VisualizerTap;

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
