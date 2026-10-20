use rustfft::{num_complex::Complex32, FftPlanner};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FFT_SIZE: usize = 512;
const NUM_BANDS: usize = 24;

pub struct VisualizerData {
    pub bands: [f32; NUM_BANDS],
    pub peaks: [f32; NUM_BANDS],
    pub overall_energy: f32,
    pub bass_energy: f32,
    pub groove_energy: f32,
}

impl Default for VisualizerData {
    fn default() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            overall_energy: 0.0,
            bass_energy: 0.0,
            groove_energy: 0.0,
        }
    }
}

pub struct AudioVisualizer {
    data: Arc<Mutex<VisualizerData>>,
    shutdown: Arc<AtomicBool>,
    _worker: Option<JoinHandle<()>>,
}

impl AudioVisualizer {
    pub fn new() -> Self {
        let data = Arc::new(Mutex::new(VisualizerData::default()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let data_clone = Arc::clone(&data);
        let shutdown_clone = Arc::clone(&shutdown);

        let worker = thread::spawn(move || {
            run_audio_capture_loop(data_clone, shutdown_clone);
        });

        Self {
            data,
            shutdown,
            _worker: Some(worker),
        }
    }

    /// Retrieve the current real-time frequency spectrum values.
    /// Returns (bands, peaks), each normalized to [0.0, 1.0].
    pub fn get_spectrum(&self, requested_bands: usize) -> (Vec<f32>, Vec<f32>) {
        let lock = match self.data.lock() {
            Ok(guard) => guard,
            Err(_) => return (vec![0.0; requested_bands], vec![0.0; requested_bands]),
        };

        if requested_bands == NUM_BANDS {
            (lock.bands.to_vec(), lock.peaks.to_vec())
        } else {
            // Resample to requested number of bands
            let mut out_bands = Vec::with_capacity(requested_bands);
            let mut out_peaks = Vec::with_capacity(requested_bands);
            for i in 0..requested_bands {
                let src_idx = (i * NUM_BANDS) / requested_bands;
                let src_idx = src_idx.min(NUM_BANDS - 1);
                out_bands.push(lock.bands[src_idx]);
                out_peaks.push(lock.peaks[src_idx]);
            }
            (out_bands, out_peaks)
        }
    }

    /// Retrieve smoothed overall audio energy [0.0, 1.0] for pulsing animations.
    pub fn get_overall_energy(&self) -> f32 {
        self.data.lock().map(|d| d.overall_energy).unwrap_or(0.0)
    }

    /// Retrieve fast-reacting bass/kick energy [0.0, 1.0].
    #[allow(dead_code)]
    pub fn get_bass_energy(&self) -> f32 {
        self.data.lock().map(|d| d.bass_energy).unwrap_or(0.0)
    }

    /// Musical groove energy [0.0, 1.0] representing sustained rhythm tempo,
    /// holding steady across micro-gaps between beats and transitioning smoothly.
    pub fn get_reactive_energy(&self) -> f32 {
        if let Ok(d) = self.data.lock() {
            d.groove_energy.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl Drop for AudioVisualizer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

enum AudioSource {
    Fifo(File),
    Parec(Child),
}

impl Read for AudioSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            AudioSource::Fifo(f) => f.read(buf),
            AudioSource::Parec(child) => {
                if let Some(ref mut stdout) = child.stdout {
                    stdout.read(buf)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No stdout",
                    ))
                }
            }
        }
    }
}

impl Drop for AudioSource {
    fn drop(&mut self) {
        if let AudioSource::Parec(child) = self {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn open_audio_source() -> Option<AudioSource> {
    // 1. Try MPD FIFO first (/tmp/mpd.fifo)
    let fifo_path = Path::new("/tmp/mpd.fifo");
    if fifo_path.exists() {
        if let Ok(file) = File::open(fifo_path) {
            return Some(AudioSource::Fifo(file));
        }
    }

    // 2. Fall back to live PulseAudio/PipeWire monitor via parec
    let child = Command::new("parec")
        .args([
            "-d",
            "@DEFAULT_SINK@.monitor",
            "--rate=22050",
            "--channels=1",
            "--format=s16le",
            "--latency-msec=20",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    if let Ok(c) = child {
        return Some(AudioSource::Parec(c));
    }

    None
}

fn run_audio_capture_loop(data: Arc<Mutex<VisualizerData>>, shutdown: Arc<AtomicBool>) {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    // Precompute Hann window
    let mut hann = [0.0f32; FFT_SIZE];
    for n in 0..FFT_SIZE {
        hann[n] = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos());
    }

    // Logarithmic frequency bins for 24 bands across 22.05 kHz (Nyquist 11.025 kHz)
    let bin_ranges = compute_frequency_bins(NUM_BANDS, FFT_SIZE, 22050.0);

    let mut smoothed_bands = [0.0f32; NUM_BANDS];
    let mut peak_values = [0.0f32; NUM_BANDS];
    let mut peak_speeds = [0.0f32; NUM_BANDS];
    let mut dynamic_ceiling_db = -6.0f32;
    let mut smoothed_energy = 0.0f32;
    let mut smoothed_bass = 0.0f32;
    let mut smoothed_groove = 0.0f32;

    let mut pcm_buffer = [0i16; FFT_SIZE];
    let mut raw_bytes = [0u8; FFT_SIZE * 2];

    while !shutdown.load(Ordering::Relaxed) {
        let mut source = match open_audio_source() {
            Some(s) => s,
            None => {
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        while !shutdown.load(Ordering::Relaxed) {
            if source.read_exact(&mut raw_bytes).is_err() {
                // Stream dropped or restarted
                break;
            }

            // Convert 16-bit little-endian PCM to float samples [-1.0, 1.0]
            for i in 0..FFT_SIZE {
                let lo = raw_bytes[i * 2] as u16;
                let hi = raw_bytes[i * 2 + 1] as i16;
                let s16 = (hi << 8) | (lo as i16);
                pcm_buffer[i] = s16;
            }

            // Prepare FFT input with Hann window
            let mut complex_buffer: Vec<Complex32> = pcm_buffer
                .iter()
                .enumerate()
                .map(|(i, &sample)| Complex32 {
                    re: (sample as f32 / 32768.0) * hann[i],
                    im: 0.0,
                })
                .collect();

            fft.process(&mut complex_buffer);

            // Compute normalized energy in each frequency band
            let mut band_dbs = [0.0f32; NUM_BANDS];
            for (b, &(start_bin, end_bin)) in bin_ranges.iter().enumerate() {
                let mut sum_mag = 0.0f32;
                let mut max_mag = 0.0f32;
                let count = (end_bin - start_bin).max(1);

                for k in start_bin..end_bin {
                    if k < complex_buffer.len() / 2 {
                        let c = complex_buffer[k];
                        // True normalized magnitude (Hann coherent gain on 512 pt FFT)
                        let mag = (c.re * c.re + c.im * c.im).sqrt() / 128.0;
                        sum_mag += mag;
                        if mag > max_mag {
                            max_mag = mag;
                        }
                    }
                }

                let avg_mag = sum_mag / (count as f32);
                let band_mag = max_mag * 0.65 + avg_mag * 0.35;

                // Gentle natural acoustic curve (avoids artificial high-beat saturation)
                let eq_weight = 1.0 + (b as f32 / NUM_BANDS as f32) * 0.75;
                let weighted_mag = band_mag * eq_weight;

                // Decibels: 0 dB is full-scale sine
                let db = 20.0 * weighted_mag.max(1e-5).log10();
                band_dbs[b] = db;
            }

            // Adaptive Automatic Gain Control (AGC):
            // Prevents clipping at the top while maintaining responsiveness
            let current_max_db = band_dbs.iter().copied().fold(-100.0f32, f32::max);
            if current_max_db > dynamic_ceiling_db {
                dynamic_ceiling_db = dynamic_ceiling_db * 0.85 + current_max_db * 0.15;
            } else {
                dynamic_ceiling_db = (dynamic_ceiling_db - 0.03).max(-24.0);
            }

            let floor_db = dynamic_ceiling_db - 36.0; // 36 dB visible dynamic range

            // Apply smoothing and peak physics
            for b in 0..NUM_BANDS {
                let db = band_dbs[b];
                let norm = (db - floor_db) / (dynamic_ceiling_db - floor_db);
                // Allow peaks to breathe naturally up to 0.90 – NEVER stuck at 1.0
                let target_val = norm.clamp(0.0, 0.90);

                // Fluid attack & gravity decay
                if target_val > smoothed_bands[b] {
                    // Quick, crisp rise on beats
                    smoothed_bands[b] = smoothed_bands[b] * 0.35 + target_val * 0.65;
                } else {
                    // Smooth exponential gravity decay
                    smoothed_bands[b] = (smoothed_bands[b] * 0.84).max(0.0);
                }

                // Peak hold & drop
                if smoothed_bands[b] >= peak_values[b] {
                    peak_values[b] = smoothed_bands[b];
                    peak_speeds[b] = 0.0;
                } else {
                    peak_speeds[b] += 0.010; // gravity acceleration
                    peak_values[b] = (peak_values[b] - peak_speeds[b]).max(0.0);
                }
            }

            // Calculate bass energy (sub-bass & kick drum bands 0..4, ~45-250 Hz)
            let bass_avg: f32 = smoothed_bands[..4].iter().sum::<f32>() / 4.0;
            if bass_avg > smoothed_bass {
                smoothed_bass = smoothed_bass * 0.15 + bass_avg * 0.85; // snappy attack on beats
            } else {
                smoothed_bass = (smoothed_bass * 0.80).max(0.0); // fast decay
            }

            // Calculate overall energy (primarily low-to-mid frequencies)
            let bass_mid_avg: f32 = smoothed_bands[..12].iter().sum::<f32>() / 12.0;
            if bass_mid_avg > smoothed_energy {
                smoothed_energy = smoothed_energy * 0.4 + bass_mid_avg * 0.6;
            } else {
                smoothed_energy = (smoothed_energy * 0.88).max(0.0);
            }

            // Macro musical groove energy:
            // Holds steady across rhythmic gaps between beats (doesn't twitch on micro-variations).
            let instant_energy = (bass_avg * 0.55 + bass_mid_avg * 0.45).clamp(0.0, 1.0);
            if instant_energy > smoothed_groove {
                // Smooth attack over ~200ms
                smoothed_groove = smoothed_groove * 0.70 + instant_energy * 0.30;
            } else {
                // Musical tempo hold & gentle release (bridges gaps between beats so rain doesn't stutter)
                smoothed_groove = (smoothed_groove * 0.965).max(0.0);
            }

            // Update shared state
            if let Ok(mut lock) = data.lock() {
                lock.bands = smoothed_bands;
                lock.peaks = peak_values;
                lock.overall_energy = smoothed_energy;
                lock.bass_energy = smoothed_bass;
                lock.groove_energy = smoothed_groove;
            }

            // Sleep ~20ms (~50 FPS visualizer sampling)
            thread::sleep(Duration::from_millis(20));
        }

        thread::sleep(Duration::from_millis(300));
    }
}

/// Compute logarithmic frequency bins from ~45 Hz up to ~9 kHz
fn compute_frequency_bins(num_bands: usize, fft_size: usize, sample_rate: f32) -> Vec<(usize, usize)> {
    let min_freq = 45.0f32;
    let max_freq = 8500.0f32;
    let nyquist = sample_rate / 2.0;

    let mut ranges = Vec::with_capacity(num_bands);
    let bin_res = nyquist / (fft_size as f32 / 2.0);

    for b in 0..num_bands {
        let t0 = b as f32 / num_bands as f32;
        let t1 = (b + 1) as f32 / num_bands as f32;

        let f_low = min_freq * (max_freq / min_freq).powf(t0);
        let f_high = min_freq * (max_freq / min_freq).powf(t1);

        let bin_low = (f_low / bin_res).floor() as usize;
        let bin_high = ((f_high / bin_res).ceil() as usize).max(bin_low + 1);

        ranges.push((bin_low.max(1), bin_high.min(fft_size / 2)));
    }

    ranges
}
