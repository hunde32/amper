# 🎵 Amper — Modern Terminal MPD Music Player

Amper is a modern, high-performance terminal music player built in Rust as a companion and TUI frontend for **MPD** (Music Player Daemon) and **mpc**.

It features an aesthetic, card-based interface inspired by modern mobile and vinyl players with a **smoothly spinning vinyl record**, album cover art rendering, live synchronized lyrics, interactive queue management, and audio library exploration.

---

## ✨ Features

- **💽 Spinning Vinyl Record**:
  - Displays circular vinyl grooves with realistic anisotropic light sheen reflections.
  - Crops and embeds the album cover art in the center of the spinning disk.
  - Rotates at ~30 FPS when music is playing, halts when paused.
  - Native 24-bit TrueColor half-block rendering (`▀`) with square-pixel aspect ratio matching terminal typography.
- **🎨 Warm Chocolate / Espresso Theme**:
  - Carefully tuned color palette (`#1C110C` card backgrounds, `#EB573D` coral red accents, muted beige typography).
- **📝 Synchronized Lyrics**:
  - Automatically fetches synchronized timestamped lyrics from **LRCLIB** (free, open-source lyrics API).
  - Also detects and parses local `.lrc` files in your music directories.
  - Auto-scrolls and highlights the currently sung lyric line with real-time audio playback synchronization.
- **📑 Queue & Playlist Manager**:
  - View full MPD queue with track numbers, durations, and currently playing indicators.
  - Real-time search/filter (`/`).
  - Jump to any track with `Enter`, delete with `d`, clear with `c`.
- **📁 Library Browser**:
  - Browse your entire music collection from MPD or `~/Music`.
  - Add songs to queue with `a` or `Enter`.
- **🚀 Effortless Startup**:
  - Automatically detects and connects to MPD (`127.0.0.1:6600` or `$MPD_HOST:$MPD_PORT`).
  - Auto-launches `mpd` daemon if not already running.
  - Automatic reconnect if MPD is restarted.

---

## ⌨️ Controls & Keybindings

| Key | Action |
| :--- | :--- |
| `Space` | Play / Pause toggle |
| `Left` / `h` | Seek backward 5 seconds |
| `Right` / `l` | Seek forward 5 seconds |
| `n` / `>` | Next track |
| `p` / `<` | Previous track |
| `Up` / `+` | Volume +5% (in Player view) |
| `Down` / `-` | Volume -5% (in Player view) |
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
cp target/release/amper ~/.local/bin/amper
```

To run Amper:
```bash
amper
```
