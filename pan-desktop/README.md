# PAN · Play Anything Now

PAN is a desktop media player for **macOS, Windows and Linux**. Open a movie or song straight from your file manager, or keep a copy in PAN's local library. The interface is written in Rust and playback uses [LibVLC 3](https://www.videolan.org/vlc/libvlc.html).

[Download PAN](https://github.com/26-oliver/PAN/releases) · [Contributors](CONTRIBUTORS.md)

## Install

Install **VLC 3** for your platform first. The downloads contain PAN and its installer where applicable. They use the VLC engine already installed on your system.

| Platform | Package | Installation |
| --- | --- | --- |
| macOS 11 or later | `PAN-macOS.zip` | Unzip and move **PAN.app** to Applications. Install VLC 3 of the same architecture as PAN at `/Applications/VLC.app`. The universal build supports Apple Silicon and Intel. Its signature is ad hoc. The toolbar uses Liquid Glass on macOS 26 or later. |
| Windows x64 | `PAN-Windows-x64.zip` | Unzip, then run `install.ps1` in PowerShell to register PAN for your Windows account. You can also launch `pan.exe` from the extracted folder. Install the 64-bit edition of VLC 3. |
| Linux x86-64 | `PAN-Linux-x86_64.tar.gz` | Extract the archive and run `./install.sh`. Install VLC 3 with its playback plugins. PAN needs an X11 or Wayland session with OpenGL. The GitHub package is built on Ubuntu 22.04 (glibc 2.35). The file picker uses your desktop portal. |

Open a file with **Open With → PAN**. Set PAN as the default for a file type in your operating system if you want double-clicking that type to open it in PAN.

## Play and organize

- Open a local file or supported stream URL. Selecting several files builds a temporary queue. PAN remembers where you left off on local files along with volume, speed and subtitle settings.
- Open **Library** with **⌘L** on macOS or **Ctrl+L** on Windows and Linux. Choose **Add files** or drop files into the library to save independent copies. You can remove the originals from Downloads after import.
- Create a playlist and use a file tile's context menu to add it to the list. Search by title, creator, filename or description. Filter by media type, duration or import date and sort by last played.
- Edit the title, description and artwork from a tile's context menu. Choose a local image or search Wikipedia. PAN sends the search text when you run a search. The available artwork depends on the article.
- Use the track menus to select audio and subtitle tracks, load external subtitles or change synchronization. Subtitle language and text size preferences apply to the next file you open. The text size option depends on the subtitle format.

## Storage and shortcuts

Open **Settings** from the library or press **⌘,** or **Ctrl+,** to see library size, choose another disk, or customize playback shortcuts. Changing the location copies the media and keeps the previous folder. PAN skips byte-identical files during import and reports missing copies in the library.

| Platform | Default library |
| --- | --- |
| macOS | `~/Library/Application Support/PAN/Biblioteca` |
| Windows | `%LOCALAPPDATA%\PAN\Biblioteca` |
| Linux | `$XDG_DATA_HOME/PAN/Biblioteca`, or `~/.local/share/PAN/Biblioteca` |

Press **Space** to play or pause, **←/→** to skip five seconds, **↑/↓** to adjust volume, **M** to mute and **F** for fullscreen. **⌘O** or **Ctrl+O** opens the file picker.

## Build and test

Build the standalone Rust application from this directory:

```sh
cargo +stable build --locked --release
cargo +stable test --locked --release
```

On macOS, install Xcode or the Command Line Tools. The macOS 26 SDK enables the native toolbar style. After building, run `./packaging/macos/package.sh` to create `dist/PAN.app`. On Windows, use the MSVC Rust toolchain and the Windows SDK resource compiler. On Linux, install `libxkbcommon-dev` and `pkg-config` or the equivalents for your distribution.

With VLC 3 installed, run `cargo +stable test --locked --release -- --include-ignored` to check decoding, pausing and seeking with a synthetic video. [GitHub Actions](https://github.com/26-oliver/PAN/actions) runs the library and playback checks on macOS (Apple Silicon and Intel), Windows x64 and Linux x86-64.

Set `PAN_LIBVLC` to an absolute path to an alternative LibVLC 3 library with matching plugins. An absolute `PAN_LIBRARY_DIR` lets you start PAN with a separate local library. PAN is built independently of the VLC fork's Rust workspace.

## Credits

PAN Desktop is maintained by [26-oliver](https://github.com/26-oliver), with development and packaging contributions from [OpenAI Codex](https://openai.com/codex/). Playback is provided by VideoLAN's LibVLC. See [CONTRIBUTORS.md](CONTRIBUTORS.md) for details and links to the upstream credits.
