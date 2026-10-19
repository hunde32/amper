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
}

impl Default for VisualizerData {
    fn default() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
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

            // Convert 16-bit little-endian PCM to float samples
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

            // Compute energy in each frequency band
            for (b, &(start_bin, end_bin)) in bin_ranges.iter().enumerate() {
                let mut sum_mag = 0.0f32;
                let count = (end_bin - start_bin).max(1);
                for k in start_bin..end_bin {
                    if k < complex_buffer.len() / 2 {
                        let c = complex_buffer[k];
                        let mag = (c.re * c.re + c.im * c.im).sqrt();
                        sum_mag += mag;
                    }
                }
                let avg_mag = sum_mag / (count as f32);

                // Boost higher frequencies for perceptual balance (pink noise tilt compensation)
                let tilt_factor = 1.0 + (b as f32 * 0.12);
                let raw_val = (avg_mag * tilt_factor * 12.0).clamp(0.0, 1.0);

                // Fluid attack & gravity decay
                if raw_val > smoothed_bands[b] {
                    // Fast attack on musical transients (beats, snares, kicks)
                    smoothed_bands[b] = smoothed_bands[b] * 0.25 + raw_val * 0.75;
                } else {
                    // Smooth exponential gravity decay
                    smoothed_bands[b] = (smoothed_bands[b] * 0.88).max(0.0);
                }

                // Peak hold & drop
                if smoothed_bands[b] >= peak_values[b] {
                    peak_values[b] = smoothed_bands[b];
                    peak_speeds[b] = 0.0;
                } else {
                    peak_speeds[b] += 0.008; // gravity acceleration
                    peak_values[b] = (peak_values[b] - peak_speeds[b]).max(0.0);
                }
            }

            // Update shared state
            if let Ok(mut lock) = data.lock() {
                lock.bands = smoothed_bands;
                lock.peaks = peak_values;
            }

            // Sleep ~20ms (~50 FPS visualizer sampling)
            thread::sleep(Duration::from_millis(20));
        }

        thread::sleep(Duration::from_millis(300));
    }
}

/// Compute logarithmic frequency bins from ~40 Hz up to ~10 kHz
fn compute_frequency_bins(num_bands: usize, fft_size: usize, sample_rate: f32) -> Vec<(usize, usize)> {
    let min_freq = 40.0f32;
    let max_freq = 9000.0f32;
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
