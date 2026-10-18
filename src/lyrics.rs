use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

#[derive(Debug, Clone, Default)]
pub struct LyricLine {
    pub timestamp: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct LyricsData {
    pub artist: String,
    pub title: String,
    pub is_synced: bool,
    pub lines: Vec<LyricLine>,
}

impl Default for LyricsData {
    fn default() -> Self {
        Self {
            artist: String::new(),
            title: String::new(),
            is_synced: false,
            lines: Vec::new(),
        }
    }
}

pub struct LyricsManager {
    current_key: String,
    current_lyrics: Option<LyricsData>,
    is_loading: bool,
    rx: Receiver<(String, Option<LyricsData>)>,
    tx: Sender<(String, Option<LyricsData>)>,
}

impl LyricsManager {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            current_key: String::new(),
            current_lyrics: None,
            is_loading: false,
            rx,
            tx,
        }
    }

    pub fn update_track(
        &mut self,
        artist: &str,
        title: &str,
        file_path: Option<&PathBuf>,
        duration: f64,
    ) {
        let key = format!("{} - {}", artist, title);
        if key == self.current_key {
            // Check if background worker sent results
            while let Ok((res_key, data)) = self.rx.try_recv() {
                if res_key == self.current_key {
                    self.current_lyrics = data;
                    self.is_loading = false;
                }
            }
            return;
        }

        self.current_key = key.clone();
        self.current_lyrics = None;
        self.is_loading = true;

        let tx = self.tx.clone();
        let artist = artist.to_string();
        let title = title.to_string();
        let file_path = file_path.cloned();

        thread::spawn(move || {
            let data = Self::load_lyrics(&artist, &title, file_path.as_ref(), duration);
            let _ = tx.send((key, data));
        });
    }

    pub fn poll_updates(&mut self) {
        while let Ok((res_key, data)) = self.rx.try_recv() {
            if res_key == self.current_key {
                self.current_lyrics = data;
                self.is_loading = false;
            }
        }
    }

    pub fn current_lyrics(&self) -> Option<&LyricsData> {
        self.current_lyrics.as_ref()
    }

    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    pub fn get_active_line_index(&self, elapsed_secs: f64) -> Option<usize> {
        let lyrics = self.current_lyrics.as_ref()?;
        if !lyrics.is_synced || lyrics.lines.is_empty() {
            return None;
        }

        let mut active = None;
        for (i, line) in lyrics.lines.iter().enumerate() {
            if line.timestamp <= elapsed_secs + 0.15 {
                active = Some(i);
            } else {
                break;
            }
        }
        active
    }

    fn load_lyrics(
        artist: &str,
        title: &str,
        file_path: Option<&PathBuf>,
        duration: f64,
    ) -> Option<LyricsData> {
        // 1. Check local .lrc file next to audio file
        if let Some(path) = file_path {
            let lrc_path = path.with_extension("lrc");
            if lrc_path.exists() {
                if let Ok(content) = fs::read_to_string(&lrc_path) {
                    if let Some(data) = Self::parse_lrc(artist, title, &content) {
                        return Some(data);
                    }
                }
            }
        }

        // 2. Fetch from LRCLIB
        if !artist.is_empty() && !title.is_empty() {
            if let Some(data) = Self::fetch_from_lrclib(artist, title, duration) {
                return Some(data);
            }
        }

        None
    }

    pub fn parse_lrc(artist: &str, title: &str, content: &str) -> Option<LyricsData> {
        let mut lines = Vec::new();
        let mut has_timestamps = false;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') {
                if let Some(closing) = line.find(']') {
                    let time_part = &line[1..closing];
                    let text_part = line[closing + 1..].trim();

                    if let Some(secs) = Self::parse_timestamp(time_part) {
                        has_timestamps = true;
                        lines.push(LyricLine {
                            timestamp: secs,
                            text: text_part.to_string(),
                        });
                        continue;
                    }
                }
            }

            if !line.is_empty() && !line.starts_with("[ti:") && !line.starts_with("[ar:") && !line.starts_with("[al:") {
                lines.push(LyricLine {
                    timestamp: 0.0,
                    text: line.to_string(),
                });
            }
        }

        if lines.is_empty() {
            return None;
        }

        lines.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));

        Some(LyricsData {
            artist: artist.to_string(),
            title: title.to_string(),
            is_synced: has_timestamps,
            lines,
        })
    }

    fn parse_timestamp(s: &str) -> Option<f64> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            Some(mins * 60.0 + secs)
        } else {
            None
        }
    }

    fn fetch_from_lrclib(artist: &str, title: &str, duration: f64) -> Option<LyricsData> {
        let encoded_artist = urlencoding(artist);
        let encoded_title = urlencoding(title);
        let url = if duration > 0.0 {
            format!(
                "https://lrclib.net/api/get?artist_name={}&track_name={}&duration={}",
                encoded_artist, encoded_title, duration as u32
            )
        } else {
            format!(
                "https://lrclib.net/api/get?artist_name={}&track_name={}",
                encoded_artist, encoded_title
            )
        };

        let resp_result = ureq::get(&url)
            .header("User-Agent", "AmperMusicPlayer/0.1")
            .call();

        if let Ok(mut resp) = resp_result {
            if let Ok(json_val) = resp.body_mut().read_json::<serde_json::Value>() {
                if let Some(synced) = json_val.get("syncedLyrics").and_then(|v| v.as_str()) {
                    if !synced.trim().is_empty() {
                        return Self::parse_lrc(artist, title, synced);
                    }
                }
                if let Some(plain) = json_val.get("plainLyrics").and_then(|v| v.as_str()) {
                    if !plain.trim().is_empty() {
                        let lines = plain
                            .lines()
                            .map(|l| LyricLine {
                                timestamp: 0.0,
                                text: l.to_string(),
                            })
                            .collect();
                        return Some(LyricsData {
                            artist: artist.to_string(),
                            title: title.to_string(),
                            is_synced: false,
                            lines,
                        });
                    }
                }
            }
        }
        None
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
