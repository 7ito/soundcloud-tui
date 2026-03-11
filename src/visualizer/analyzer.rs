use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex, num_complex::Complex32};

pub const VISUALIZER_BANDS: usize = 12;

const FFT_SIZE: usize = 2048;
const MIN_FREQUENCY_HZ: f32 = 40.0;
const MAX_FREQUENCY_HZ: f32 = 16_000.0;
const RISE_SMOOTHING: f32 = 0.3;
const DECAY_STEP: f32 = 0.035;
const PEAK_DECAY_STEP: f32 = 0.04;
const NOISE_GATE: f32 = 0.004;
const BAND_GAINS: [f32; VISUALIZER_BANDS] = [
    1.25, 1.18, 1.08, 1.0, 0.96, 0.92, 0.96, 1.02, 1.08, 1.16, 1.24, 1.32,
];

#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumFrame {
    pub bands: [f32; VISUALIZER_BANDS],
    pub peak: f32,
}

impl Default for SpectrumFrame {
    fn default() -> Self {
        Self {
            bands: [0.0; VISUALIZER_BANDS],
            peak: 0.0,
        }
    }
}

pub struct VisualizerAnalyzer {
    fft: Arc<dyn RealToComplex<f32>>,
    sample_rate: u32,
    window: Vec<f32>,
    ring: Vec<f32>,
    fft_input: Vec<f32>,
    fft_output: Vec<Complex32>,
    write_pos: usize,
    sample_count: usize,
    band_ranges: [(usize, usize); VISUALIZER_BANDS],
    frame: SpectrumFrame,
}

impl VisualizerAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let band_ranges = build_band_ranges(sample_rate);
        let window = (0..FFT_SIZE)
            .map(|index| {
                0.5 - 0.5 * ((2.0 * std::f32::consts::PI * index as f32) / FFT_SIZE as f32).cos()
            })
            .collect();

        Self {
            fft_input: fft.make_input_vec(),
            fft_output: fft.make_output_vec(),
            fft,
            sample_rate,
            window,
            ring: vec![0.0; FFT_SIZE],
            write_pos: 0,
            sample_count: 0,
            band_ranges,
            frame: SpectrumFrame::default(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.ring[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % FFT_SIZE;
            self.sample_count = (self.sample_count + 1).min(FFT_SIZE);
        }
    }

    pub fn current_frame(&mut self) -> SpectrumFrame {
        if self.sample_count < FFT_SIZE / 4 {
            self.decay_existing_frame();
            return self.frame.clone();
        }

        for (index, input) in self.fft_input.iter_mut().enumerate() {
            let ring_index = (self.write_pos + index) % FFT_SIZE;
            *input = self.ring[ring_index] * self.window[index];
        }

        if self
            .fft
            .process(&mut self.fft_input, &mut self.fft_output)
            .is_err()
        {
            self.decay_existing_frame();
            return self.frame.clone();
        }

        let mut next = [0.0; VISUALIZER_BANDS];

        for (band_index, (start, end)) in self.band_ranges.iter().copied().enumerate() {
            let mut energy = 0.0;
            let mut bins = 0usize;
            for bin in start..end {
                let magnitude = self.fft_output[bin].norm() / (FFT_SIZE as f32 * 0.5);
                energy += magnitude * magnitude;
                bins += 1;
            }

            if bins == 0 {
                continue;
            }

            let amplitude = (energy / bins as f32).sqrt() * BAND_GAINS[band_index] * 8.0;
            let compressed = amplitude.powf(0.65).clamp(0.0, 1.0);
            next[band_index] = compressed;
        }

        let peak = next.iter().copied().fold(0.0, f32::max);

        for (index, value) in next.iter().copied().enumerate() {
            self.frame.bands[index] = smooth_band(self.frame.bands[index], value);
        }
        self.frame.peak = smooth_peak(self.frame.peak, peak);

        self.frame.clone()
    }

    fn decay_existing_frame(&mut self) {
        for band in &mut self.frame.bands {
            *band = (*band - DECAY_STEP).max(0.0);
        }
        self.frame.peak = (self.frame.peak - PEAK_DECAY_STEP).max(0.0);
    }
}

fn smooth_band(current: f32, next: f32) -> f32 {
    if next >= current {
        let value = current * RISE_SMOOTHING + next * (1.0 - RISE_SMOOTHING);
        if value < NOISE_GATE { 0.0 } else { value }
    } else {
        let value = (current - DECAY_STEP).max(next);
        if value < NOISE_GATE { 0.0 } else { value }
    }
}

fn smooth_peak(current: f32, next: f32) -> f32 {
    if next >= current {
        current * 0.45 + next * 0.55
    } else {
        (current - PEAK_DECAY_STEP).max(next).max(0.0)
    }
}

fn build_band_ranges(sample_rate: u32) -> [(usize, usize); VISUALIZER_BANDS] {
    let nyquist = sample_rate as f32 / 2.0;
    let max_frequency = MAX_FREQUENCY_HZ.min(nyquist.max(MIN_FREQUENCY_HZ + 1.0));
    let log_min = MIN_FREQUENCY_HZ.ln();
    let log_max = max_frequency.ln();
    let mut ranges = [(1, 2); VISUALIZER_BANDS];
    let mut previous_end = 1usize;

    for (index, range) in ranges.iter_mut().enumerate() {
        let start_ratio = index as f32 / VISUALIZER_BANDS as f32;
        let end_ratio = (index + 1) as f32 / VISUALIZER_BANDS as f32;
        let start_hz = (log_min + (log_max - log_min) * start_ratio).exp();
        let end_hz = (log_min + (log_max - log_min) * end_ratio).exp();
        let mut start = frequency_to_bin(start_hz, sample_rate).max(previous_end);
        let mut end = frequency_to_bin(end_hz, sample_rate).max(start + 1);
        let max_bin = FFT_SIZE / 2;
        start = start.clamp(1, max_bin.saturating_sub(1));
        end = end.clamp(start + 1, max_bin + 1);
        *range = (start, end);
        previous_end = end;
    }

    ranges[VISUALIZER_BANDS - 1].1 = FFT_SIZE / 2 + 1;
    ranges
}

fn frequency_to_bin(frequency_hz: f32, sample_rate: u32) -> usize {
    ((frequency_hz / sample_rate as f32) * FFT_SIZE as f32).round() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 48_000;

    #[test]
    fn analyzer_emits_non_zero_frame_for_sine_wave() {
        let mut analyzer = VisualizerAnalyzer::new(SAMPLE_RATE);
        let samples = sine_wave(440.0, FFT_SIZE, SAMPLE_RATE);

        analyzer.push_samples(&samples);
        let frame = analyzer.current_frame();

        assert!(frame.peak > 0.01);
        assert!(frame.bands.iter().any(|band| *band > 0.01));
    }

    #[test]
    fn analyzer_decays_after_signal_disappears() {
        let mut analyzer = VisualizerAnalyzer::new(SAMPLE_RATE);
        let samples = sine_wave(220.0, FFT_SIZE, SAMPLE_RATE);

        analyzer.push_samples(&samples);
        let active = analyzer.current_frame();
        analyzer.push_samples(&vec![0.0; FFT_SIZE]);
        let faded = analyzer.current_frame();

        assert!(active.peak > 0.01);
        assert!(faded.peak <= active.peak);
    }

    #[test]
    fn analyzer_exposes_sample_rate() {
        let analyzer = VisualizerAnalyzer::new(SAMPLE_RATE);

        assert_eq!(analyzer.sample_rate(), SAMPLE_RATE);
    }

    fn sine_wave(frequency_hz: f32, len: usize, sample_rate: u32) -> Vec<f32> {
        (0..len)
            .map(|index| {
                let phase =
                    index as f32 * frequency_hz * 2.0 * std::f32::consts::PI / sample_rate as f32;
                phase.sin() * 0.8
            })
            .collect()
    }
}
