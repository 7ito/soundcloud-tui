use std::sync::{Arc, Mutex};

use crate::visualizer::{SpectrumFrame, VisualizerAnalyzer};

const DEFAULT_SAMPLE_RATE: u32 = 48_000;

#[derive(Clone)]
pub struct VisualizerTap {
    inner: Arc<Mutex<TapState>>,
}

struct TapState {
    analyzer: VisualizerAnalyzer,
}

impl Default for VisualizerTap {
    fn default() -> Self {
        Self::new(DEFAULT_SAMPLE_RATE)
    }
}

impl VisualizerTap {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TapState {
                analyzer: VisualizerAnalyzer::new(sample_rate),
            })),
        }
    }

    pub fn configure(&self, sample_rate: u32) {
        if let Ok(mut state) = self.inner.lock() {
            if state.analyzer.sample_rate() != sample_rate {
                state.analyzer = VisualizerAnalyzer::new(sample_rate);
            }
        }
    }

    pub fn push_interleaved(&self, samples: &[f32], channels: usize) {
        if channels == 0 || samples.is_empty() {
            return;
        }

        if let Ok(mut state) = self.inner.lock() {
            if channels == 1 {
                state.analyzer.push_samples(samples);
                return;
            }

            let mut mono = Vec::with_capacity(samples.len() / channels + 1);
            for frame in samples.chunks(channels) {
                let sum = frame.iter().copied().fold(0.0, |acc, sample| acc + sample);
                mono.push(sum / frame.len() as f32);
            }
            state.analyzer.push_samples(&mono);
        }
    }

    pub fn current_frame(&self) -> Result<SpectrumFrame, String> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "visualizer tap lock poisoned".to_string())?;
        Ok(state.analyzer.current_frame())
    }
}
