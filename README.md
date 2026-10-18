# 🎵 Amper — Modern Terminal MPD Music Player

Amper is a modern, cyberpunk/synthwave terminal music player built in Rust as a companion and TUI frontend for **MPD** (Music Player Daemon) and **mpc**.

It features a 100% transparent, responsive interface that dynamically adapts to your terminal wallpaper colors (via Kitty & Matugen/Pywal), high-resolution static album art or spinning vinyl records, real-time waveform visualizers, live synchronized lyrics, and a modular customizable bottom panel.

---

## ✨ Features

- **🖼️ Customizable Cover Art (Static or Vinyl)**:
  - **Static Album Cover** (Default): Crisp, high-resolution square album artwork with rounded corners. Zero dizziness or headaches.
  - **Spinning Vinyl Record**: Classic circular vinyl with grooves, realistic anisotropic light sheen, and smooth rotation.
  - **Press `v`** to toggle instantly between Static Cover Art and Spinning Vinyl!
- **⚡ Kitty Graphics Protocol (GPU-Accelerated Clarity)**:
  - Renders cover art in true display pixel resolution via Kitty graphics protocol with bilinear anti-aliasing.
  - Automatic half-block truecolor fallback for non-Kitty terminals.
- **🌌 100% Transparent & Dynamic Wallpaper Colors**:
  - Zero solid brown backgrounds. Transparent (`Color::Reset`) so your terminal transparency and wallpaper shine through.
  - Automatically loads colors from `~/.config/kitty/colors.conf` (Matugen/Pywal) and updates live when wallpaper changes.
- **🎛️ Customizable Bottom Panel**:
  - **Press `m` or `b`** to cycle between 4 modular panels:
    1. **Audio Monitor // Telemetry**: Audio format, bitrate, bit depth, size, PipeWire output, volume meter, and dynamic activity dots.
    2. **Spectrum // Visualizer**: Multi-band graphic equalizer bars bouncing to the music.
    3. **Live Lyrics // Mini**: Live 3-line synchronized scrolling lyrics directly beneath the player.
    4. **Track Details // Metadata**: Detailed ID3 tags, album, artist, duration, and file URI.
- **🌊 Symmetrical Waveform Banner**:
  - Top audio signal visualizer `● SIGNAL // LIVE [SYMMETRIC_BARS]` pulsating with audio playback.
- **🔘 Interactive Boxed Cyberpunk Controls**:
  - `[ ⇄ ] [ |<< ] [  || / ▶  ] [ >>| ] [ ↻ ]`
  - Active and clicked buttons light up with a bold filled background block.
- **📝 Synchronized Lyrics**:
  - Automatically fetches synchronized timestamped lyrics from **LRCLIB** (and local `.lrc` files).
  - Highlights currently sung lines with real-time audio playback synchronization.
- **📑 Queue & Playlist Manager**:
  - Interactive playlist browser with live search filter (`/`), track numbers, and durations.
  - `Enter` to play, `d` to delete, `c` to clear.
- **📁 Library Browser**:
  - Browse your entire music collection from MPD or `~/Music`.
  - Add songs to queue with `a` or `Enter`.
- **🚀 Effortless Startup**:
  - Auto-detects local MPD on port 6600 (or `$MPD_HOST:$MPD_PORT`). If MPD is not yet running, it starts it in the background and reconnects.

---

## ⌨️ Controls & Keybindings

| Key | Action |
| :--- | :--- |
| `Space` | Play / Pause toggle (highlights button) |
| `Left` / `h` | Seek backward 5 seconds |
| `Right` / `l` | Seek forward 5 seconds |
| `n` / `>` | Next track |
| `p` / `<` | Previous track |
| `Up` / `+` | Volume +5% (in Player view) |
| `Down` / `-` | Volume -5% (in Player view) |
| `v` | **Toggle Cover Display** (Static Card ⮂ Spinning Vinyl) |
| `m` / `b` | **Cycle Bottom Panel** (Telemetry ➔ Spectrum ➔ Lyrics ➔ Details) |
| `r` | Toggle Repeat mode |
| `s` | Toggle Shuffle / Random mode |
| `f` | Toggle Favorite / Like ♥ |
| `Tab` | Switch between views (`Songs` ➔ `Lyrics` ➔ `Queue` ➔ `Library`) |
| `1` - `4` | Directly jump to view (1: Player, 2: Lyrics, 3: Queue, 4: Library) |
| `/` | Search filter (in Queue and Library views) |
| `j` / `k` / `↑` / `↓` | Navigate list items or scroll lyrics |
| `Enter` | Play selected song in Queue / Add in Library |
| `d` | Delete track from Queue |
| `c` | Clear Queue (or re-sync lyrics scroll to playback) |
| `L` | Force refresh / re-fetch lyrics from LRCLIB |
| `?` | Open Help modal cheat-sheet |
| `q` / `Esc` | Quit Amper |

---

## 🛠️ Installation & Building

```bash
# Build release binary
cargo build --release

# Install to ~/.local/bin
install -m 755 target/release/amper ~/.local/bin/amper
```

To run Amper:
```bash
amper
```
