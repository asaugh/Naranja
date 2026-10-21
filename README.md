# Naranja 🍊

**A free, offline file converter for macOS, written in Rust.** Drop files onto its conversion wheel, choose a format, and find the result beside the original. No account, upload, conversion quota, or paid tier.

Naranja is inspired by the quick file workflow of [Tangerine for Mac](https://tangerineformac.com/). It is an independent project and is not affiliated with Tangerine. It uses its own name, interface, and code.

## What works today

| Area | Supported operations |
| --- | --- |
| Images | Convert among JPG, PNG, WebP, HEIC, TIFF, BMP, GIF, and AVIF where the installed codecs allow it; resize, crop, and compress |
| Audio | Convert MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, and WMA |
| Video | Convert MP4, MOV, MKV, WebM, AVI, WMV, and GIF; extract MP3; trim, split, merge, and compress |
| Documents | Convert TXT, RTF, HTML, DOCX, and ODT through macOS TextEdit's `textutil` |
| Tables | Convert CSV and TSV with quoted cells preserved |
| Subtitles | Convert SRT, VTT, and plain text |
| Archives | Create ZIP or TAR archives; extract ZIP, TAR, and GZIP archives |
| Actions | Copy path, reveal in Finder, open file, open its folder in Terminal, copy or move into another folder |

Conversions run locally. Outputs receive a new name when a file with the intended name already exists, so an original is not overwritten. Batch conversion applies the chosen format to all compatible files in the list.

The native interface uses a six-slice wheel. Switch between **Convert**, **Tools**, and **Actions** with the tabs or Shift, Command, and Control while dragging into the window. Hover a slice and drop to apply it, or click it after adding a file. The orange-slice icon is original artwork included as an editable SVG in `assets/`.

## Install and run

Requires an Apple silicon or Intel Mac with macOS 13 or later and a [Rust toolchain](https://rustup.rs/). FFmpeg is required for audio/video conversion and some image formats; install it with `brew install ffmpeg`.

```sh
git clone https://github.com/asaugh/Naranja.git
cd Naranja
cargo run --release
```

To create a normal `Naranja.app` bundle in `dist/`:

```sh
./scripts/build-app.sh
open dist/Naranja.app
```

The app bundle is unsigned. macOS may ask you to approve opening it in Privacy & Security. Building it yourself avoids downloading an untrusted binary.

There is also a command line interface:

```sh
cargo run --release -- formats photo.heic
cargo run --release -- convert photo.heic png
cargo run --release -- extract archive.zip
```

## Scope and roadmap

This is an early working release. Tangerine currently describes a much larger set of conversion paths and advanced tools. Naranja's wheel works within its app window; it does **not** yet appear globally during a Finder drag. AirDrop and floating-file actions, PDF and Office conversions, and some codec combinations also remain. The interface lists operations implemented in this repository; codecs supplied by FFmpeg and macOS can vary by machine. Contributions toward these areas are welcome.

Good next steps: a global drag trigger with macOS Accessibility permission, an explicit conversion capability registry, PDF/Office backends, more media tools, and signed release builds.

## Development

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The engine is in `src/engine.rs`; the native window and background jobs are in `src/main.rs`. Conversion processes are launched with argument arrays instead of a shell. Comments explain non-obvious behavior and safety choices.

## License

MIT. See [LICENSE](LICENSE).
