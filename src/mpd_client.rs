use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Play,
    Pause,
    Stop,
}

#[derive(Debug, Clone)]
pub struct MpdStatus {
    pub state: PlaybackState,
    pub elapsed: f64,
    pub duration: f64,
    pub volume: i32,
    pub repeat: bool,
    pub random: bool,
    pub single: bool,
    pub consume: bool,
    pub song_id: Option<u32>,
    pub song_pos: Option<u32>,
    pub playlist_length: usize,
}

impl Default for MpdStatus {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stop,
            elapsed: 0.0,
            duration: 0.0,
            volume: 100,
            repeat: false,
            random: false,
            single: false,
            consume: false,
            song_id: None,
            song_pos: None,
            playlist_length: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SongInfo {
    pub id: u32,
    pub pos: u32,
    pub file: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub date: String,
}

impl SongInfo {
    pub fn display_title(&self) -> String {
        if !self.title.is_empty() {
            self.title.clone()
        } else if !self.file.is_empty() {
            let path = std::path::Path::new(&self.file);
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| self.file.clone())
        } else {
            "Unknown Title".to_string()
        }
    }

    pub fn display_artist(&self) -> String {
        if !self.artist.is_empty() {
            self.artist.clone()
        } else {
            "Unknown Artist".to_string()
        }
    }

    pub fn display_album(&self) -> String {
        if !self.album.is_empty() {
            self.album.clone()
        } else {
            "Unknown Album".to_string()
        }
    }
}

pub struct MpdClient {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    music_dir: Option<PathBuf>,
}

impl MpdClient {
    pub fn new() -> Self {
        let host = std::env::var("MPD_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("MPD_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(6600);

        let music_dir = Self::detect_music_dir();

        Self {
            host,
            port,
            stream: None,
            music_dir,
        }
    }

    pub fn music_dir(&self) -> Option<&PathBuf> {
        self.music_dir.as_ref()
    }

    fn detect_music_dir() -> Option<PathBuf> {
        // 1. Check ~/.config/mpd/mpd.conf
        if let Some(home) = dirs::home_dir() {
            let config_path = home.join(".config/mpd/mpd.conf");
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("music_directory") {
                        if let Some(val) = trimmed.split_whitespace().nth(1) {
                            let path_str = val.trim_matches('"');
                            let expanded = if path_str.starts_with("~/") {
                                home.join(&path_str[2..])
                            } else {
                                PathBuf::from(path_str)
                            };
                            if expanded.exists() {
                                return Some(expanded);
                            }
                        }
                    }
                }
            }
            // Fallback default: ~/Music
            let default_music = home.join("Music");
            if default_music.exists() {
                return Some(default_music);
            }
        }
        None
    }

    pub fn connect(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let timeout = Duration::from_millis(1500);

        let stream_res = addr
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .and_then(|a| TcpStream::connect_timeout(&a, timeout).ok());

        let stream = match stream_res {
            Some(s) => s,
            None => {
                // If MPD is not running, try starting it!
                let _ = Command::new("mpd").output();
                std::thread::sleep(Duration::from_millis(300));
                let addrs = addr.to_socket_addrs()?.next().ok_or_else(|| anyhow!("Invalid address"))?;
                TcpStream::connect_timeout(&addrs, timeout)
                    .map_err(|e| anyhow!("Failed to connect to MPD at {}: {}", addr, e))?
            }
        };

        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;

        // Read greeting
        let mut reader = BufReader::new(&stream);
        let mut greeting = String::new();
        reader.read_line(&mut greeting)?;
        if !greeting.starts_with("OK MPD") {
            return Err(anyhow!("Unexpected MPD greeting: {}", greeting));
        }

        self.stream = Some(stream);
        Ok(())
    }

    pub fn ensure_connected(&mut self) -> Result<()> {
        if self.stream.is_none() {
            self.connect()?;
        }
        Ok(())
    }

    fn send_command(&mut self, cmd: &str) -> Result<String> {
        let mut retried = false;
        loop {
            self.ensure_connected()?;
            let stream = self.stream.as_mut().unwrap();

            let full_cmd = if cmd.ends_with('\n') {
                cmd.to_string()
            } else {
                format!("{}\n", cmd)
            };

            if let Err(e) = stream.write_all(full_cmd.as_bytes()) {
                self.stream = None;
                if !retried {
                    retried = true;
                    continue;
                }
                return Err(anyhow!("Failed to write to MPD: {}", e));
            }

            let mut reader = BufReader::new(stream);
            let mut response = String::new();
            let mut line = String::new();

            loop {
                line.clear();
                let bytes_read = match reader.read_line(&mut line) {
                    Ok(b) => b,
                    Err(e) => {
                        self.stream = None;
                        if !retried {
                            retried = true;
                            break;
                        }
                        return Err(anyhow!("Failed to read from MPD: {}", e));
                    }
                };

                if bytes_read == 0 {
                    self.stream = None;
                    if !retried {
                        retried = true;
                        break;
                    }
                    return Err(anyhow!("MPD closed connection unexpectedly"));
                }

                if line.starts_with("OK\n") || line == "OK\r\n" {
                    return Ok(response);
                }

                if line.starts_with("ACK ") {
                    return Err(anyhow!("MPD error: {}", line.trim()));
                }

                response.push_str(&line);
            }
        }
    }

    pub fn get_status(&mut self) -> Result<MpdStatus> {
        let resp = self.send_command("status")?;
        let mut status = MpdStatus::default();

        for line in resp.lines() {
            if let Some((key, val)) = line.split_once(": ") {
                match key {
                    "state" => {
                        status.state = match val {
                            "play" => PlaybackState::Play,
                            "pause" => PlaybackState::Pause,
                            _ => PlaybackState::Stop,
                        };
                    }
                    "elapsed" => {
                        status.elapsed = val.parse().unwrap_or(0.0);
                    }
                    "duration" => {
                        status.duration = val.parse().unwrap_or(0.0);
                    }
                    "volume" => {
                        status.volume = val.parse().unwrap_or(-1);
                    }
                    "repeat" => {
                        status.repeat = val == "1";
                    }
                    "random" => {
                        status.random = val == "1";
                    }
                    "single" => {
                        status.single = val == "1";
                    }
                    "consume" => {
                        status.consume = val == "1";
                    }
                    "songid" => {
                        status.song_id = val.parse().ok();
                    }
                    "song" => {
                        status.song_pos = val.parse().ok();
                    }
                    "playlistlength" => {
                        status.playlist_length = val.parse().unwrap_or(0);
                    }
                    _ => {}
                }
            }
        }

        Ok(status)
    }

    pub fn get_current_song(&mut self) -> Result<Option<SongInfo>> {
        let resp = self.send_command("currentsong")?;
        if resp.trim().is_empty() {
            return Ok(None);
        }

        let mut song = SongInfo::default();
        for line in resp.lines() {
            if let Some((key, val)) = line.split_once(": ") {
                match key {
                    "Id" => song.id = val.parse().unwrap_or(0),
                    "Pos" => song.pos = val.parse().unwrap_or(0),
                    "file" => song.file = val.to_string(),
                    "Title" => song.title = val.to_string(),
                    "Artist" => song.artist = val.to_string(),
                    "Album" => song.album = val.to_string(),
                    "duration" | "Time" => {
                        if song.duration == 0.0 {
                            song.duration = val.parse().unwrap_or(0.0);
                        }
                    }
                    "Date" => song.date = val.to_string(),
                    _ => {}
                }
            }
        }

        Ok(Some(song))
    }

    pub fn get_playlist(&mut self) -> Result<Vec<SongInfo>> {
        let resp = self.send_command("playlistinfo")?;
        let mut list = Vec::new();
        let mut cur = SongInfo::default();
        let mut in_song = false;

        for line in resp.lines() {
            if let Some((key, val)) = line.split_once(": ") {
                if key == "file" {
                    if in_song {
                        list.push(cur);
                    }
                    cur = SongInfo::default();
                    cur.file = val.to_string();
                    in_song = true;
                } else if in_song {
                    match key {
                        "Id" => cur.id = val.parse().unwrap_or(0),
                        "Pos" => cur.pos = val.parse().unwrap_or(0),
                        "Title" => cur.title = val.to_string(),
                        "Artist" => cur.artist = val.to_string(),
                        "Album" => cur.album = val.to_string(),
                        "duration" | "Time" => {
                            if cur.duration == 0.0 {
                                cur.duration = val.parse().unwrap_or(0.0);
                            }
                        }
                        "Date" => cur.date = val.to_string(),
                        _ => {}
                    }
                }
            }
        }
        if in_song {
            list.push(cur);
        }

        Ok(list)
    }

    pub fn toggle_pause(&mut self) -> Result<()> {
        let status = self.get_status()?;
        match status.state {
            PlaybackState::Play => {
                let _ = self.send_command("pause 1");
            }
            PlaybackState::Pause => {
                let _ = self.send_command("pause 0");
            }
            PlaybackState::Stop => {
                let _ = self.send_command("play");
            }
        }
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        self.send_command("play")?;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.send_command("pause 1")?;
        Ok(())
    }

    pub fn next(&mut self) -> Result<()> {
        self.send_command("next")?;
        Ok(())
    }

    pub fn previous(&mut self) -> Result<()> {
        self.send_command("previous")?;
        Ok(())
    }

    pub fn seek_relative(&mut self, delta: f64) -> Result<()> {
        let sign = if delta >= 0.0 { "+" } else { "" };
        self.send_command(&format!("seekcur {}{}", sign, delta))?;
        Ok(())
    }

    pub fn seek_absolute(&mut self, seconds: f64) -> Result<()> {
        self.send_command(&format!("seekcur {:.1}", seconds.max(0.0)))?;
        Ok(())
    }

    pub fn set_volume(&mut self, vol: i32) -> Result<()> {
        let clamped = vol.clamp(0, 100);
        self.send_command(&format!("setvol {}", clamped))?;
        Ok(())
    }

    pub fn change_volume(&mut self, delta: i32) -> Result<()> {
        let status = self.get_status()?;
        if status.volume >= 0 {
            self.set_volume(status.volume + delta)?;
        }
        Ok(())
    }

    pub fn toggle_repeat(&mut self) -> Result<()> {
        let status = self.get_status()?;
        let next_val = if status.repeat { "0" } else { "1" };
        self.send_command(&format!("repeat {}", next_val))?;
        Ok(())
    }

    pub fn toggle_random(&mut self) -> Result<()> {
        let status = self.get_status()?;
        let next_val = if status.random { "0" } else { "1" };
        self.send_command(&format!("random {}", next_val))?;
        Ok(())
    }

    pub fn play_id(&mut self, id: u32) -> Result<()> {
        self.send_command(&format!("playid {}", id))?;
        Ok(())
    }

    pub fn delete_id(&mut self, id: u32) -> Result<()> {
        self.send_command(&format!("deleteid {}", id))?;
        Ok(())
    }

    pub fn clear_queue(&mut self) -> Result<()> {
        self.send_command("clear")?;
        Ok(())
    }

    pub fn add_to_queue(&mut self, uri: &str) -> Result<()> {
        self.send_command(&format!("add \"{}\"", uri))?;
        Ok(())
    }

    pub fn list_all_files(&mut self) -> Result<Vec<String>> {
        let resp = self.send_command("listall")?;
        let mut files = Vec::new();
        for line in resp.lines() {
            if let Some(file) = line.strip_prefix("file: ") {
                files.push(file.to_string());
            }
        }
        Ok(files)
    }

    /// Read album art directly from MPD protocol
    pub fn read_album_art_from_mpd(&mut self, file_uri: &str) -> Option<Vec<u8>> {
        self.ensure_connected().ok()?;
        let stream = self.stream.as_mut()?;

        let mut offset = 0usize;
        let mut image_bytes = Vec::new();

        loop {
            let cmd = format!("readpicture \"{}\" {}\n", file_uri, offset);
            if stream.write_all(cmd.as_bytes()).is_err() {
                return None;
            }

            let mut reader = BufReader::new(&mut *stream);
            let mut line = String::new();
            let mut chunk_size = 0usize;
            let mut total_size = 0usize;

            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return None;
                }
                let trimmed = line.trim();
                if trimmed.starts_with("size: ") {
                    total_size = trimmed[6..].parse().unwrap_or(0);
                } else if trimmed.starts_with("binary: ") {
                    chunk_size = trimmed[8..].parse().unwrap_or(0);
                    break;
                } else if trimmed == "OK" {
                    return if image_bytes.is_empty() { None } else { Some(image_bytes) };
                } else if trimmed.starts_with("ACK ") {
                    return None;
                }
            }

            if chunk_size == 0 {
                break;
            }

            let mut chunk = vec![0u8; chunk_size];
            if reader.read_exact(&mut chunk).is_err() {
                return None;
            }
            image_bytes.extend_from_slice(&chunk);
            offset += chunk_size;

            // Read the trailing newline + OK
            line.clear();
            let _ = reader.read_line(&mut line);
            line.clear();
            let _ = reader.read_line(&mut line);

            if offset >= total_size || chunk_size == 0 {
                break;
            }
        }

        if image_bytes.is_empty() {
            None
        } else {
            Some(image_bytes)
        }
    }
}
