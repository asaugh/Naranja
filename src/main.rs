mod engine;
mod interface;

use eframe::egui::{self, Color32};
use std::{path::PathBuf, process::Command, sync::mpsc};

const ORANGE: Color32 = Color32::from_rgb(247, 109, 45);

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                println!(
                    "Naranja — local macOS file converter\n\nUsage:\n  naranja                  Open the app\n  naranja formats FILE     List available formats\n  naranja convert FILE EXT Convert a file\n  naranja extract FILE     Extract an archive"
                );
                return Ok(());
            }
            "formats" if args.len() == 3 => {
                println!("{}", engine::formats(&PathBuf::from(&args[2])).join(" "));
                return Ok(());
            }
            "convert" if args.len() == 4 => {
                let result =
                    engine::convert(&PathBuf::from(&args[2]), &args[3].to_ascii_lowercase());
                match result {
                    Ok(path) => println!("{}", path.display()),
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            "extract" if args.len() == 3 => {
                let result = engine::extract(&PathBuf::from(&args[2]));
                match result {
                    Ok(path) => println!("{}", path.display()),
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            _ => {
                eprintln!("Invalid command. Run naranja --help.");
                std::process::exit(2);
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Naranja")
            .with_inner_size([1080.0, 760.0])
            .with_min_inner_size([790.0, 610.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/naranja-icon.png"))
                    .expect("Valid app icon"),
            )
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Naranja",
        options,
        Box::new(|cc| {
            configure_style(&cc.egui_ctx);
            Ok(Box::<NaranjaApp>::default())
        }),
    )
}

fn configure_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::WHITE;
    visuals.window_fill = Color32::WHITE;
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(246, 246, 244);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(255, 235, 216);
    visuals.selection.bg_fill = ORANGE;
    visuals.selection.stroke.color = Color32::WHITE;
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(15.0, 9.0);
    ctx.set_style(style);
}

#[derive(Default, PartialEq)]
enum Tab {
    #[default]
    Convert,
    Tools,
    Actions,
}

#[derive(Default)]
struct NaranjaApp {
    files: Vec<PathBuf>,
    selected: usize,
    tab: Tab,
    receiver: Option<mpsc::Receiver<Result<Vec<PathBuf>, String>>>,
    busy: bool,
    status: String,
    last_outputs: Vec<PathBuf>,
    trim_start: String,
    trim_duration: String,
    width: String,
    crop: String,
    split_seconds: String,
    active_tool: Option<String>,
    wheel_page: usize,
}

impl NaranjaApp {
    fn add_files(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            if path.exists() && !self.files.contains(&path) {
                self.files.push(path);
            }
        }
        if !self.files.is_empty() {
            self.status = format!("{} item(s) ready", self.files.len());
        }
    }

    fn selected_file(&self) -> Option<&PathBuf> {
        self.files.get(self.selected)
    }

    fn dispatch(&mut self, work: impl FnOnce() -> Result<Vec<PathBuf>, String> + Send + 'static) {
        if self.busy {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.status = "Working locally on your Mac…".into();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
    }

    fn convert_all(&mut self, target: &'static str) {
        let files = self.files.clone();
        self.dispatch(move || {
            let mut outputs = Vec::new();
            for file in files {
                if engine::formats(&file).contains(&target) {
                    outputs.push(engine::convert(&file, target)?);
                }
            }
            if outputs.is_empty() {
                Err(format!("No selected items can convert to {target}"))
            } else {
                Ok(outputs)
            }
        });
    }

    fn run_tool(&mut self, tool: &str) {
        let Some(file) = self.selected_file().cloned() else {
            return;
        };
        match tool {
            "Extract archive" => self.dispatch(move || engine::extract(&file).map(|p| vec![p])),
            "Compress to ZIP" => {
                self.dispatch(move || engine::compress_zip(&file).map(|p| vec![p]))
            }
            "Compress video" => self.dispatch(move || {
                ffmpeg_tool(
                    &file,
                    "compressed.mp4",
                    &[
                        "-c:v", "libx264", "-crf", "28", "-preset", "medium", "-c:a", "aac",
                        "-b:a", "128k",
                    ],
                )
                .map(|p| vec![p])
            }),
            "Compress image" => self.dispatch(move || {
                ffmpeg_tool(&file, "compressed.jpg", &["-q:v", "5"]).map(|p| vec![p])
            }),
            "Extract audio" => {
                self.dispatch(move || engine::convert(&file, "mp3").map(|p| vec![p]))
            }
            _ => {}
        }
    }

    fn action(&mut self, action: &str, ctx: &egui::Context) {
        let Some(file) = self.selected_file() else {
            return;
        };
        let value = file.to_string_lossy().to_string();
        match action {
            "Copy path" => {
                ctx.copy_text(value);
                self.status = "Path copied".into();
            }
            "Reveal in Finder" => {
                let _ = Command::new("open").args(["-R", &value]).spawn();
                self.status = "Opened Finder".into();
            }
            "Open file" => {
                let _ = Command::new("open").arg(&value).spawn();
                self.status = "Opened file".into();
            }
            "Open in Terminal" => {
                let folder = file.parent().unwrap_or(file).to_string_lossy().to_string();
                let _ = Command::new("open")
                    .args(["-a", "Terminal", &folder])
                    .spawn();
                self.status = "Opened Terminal".into();
            }
            "Copy to folder" | "Move to folder" => {
                let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                    return;
                };
                let source = file.clone();
                let move_file = action == "Move to folder";
                self.dispatch(move || transfer_file(&source, &folder, move_file).map(|p| vec![p]));
            }
            _ => {}
        }
    }
}

fn icon(path: &std::path::Path) -> &'static str {
    match engine::kind(path) {
        "Image" => "▧",
        "Video" => "▶",
        "Audio" => "♫",
        "Document" => "▤",
        "Spreadsheet" => "▦",
        "Archive" => "▣",
        "Folder" => "▰",
        _ => "◈",
    }
}

fn unique_named(input: &std::path::Path, suffix: &str) -> PathBuf {
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let mut path = parent.join(format!("{stem}-{suffix}"));
    let mut number = 2;
    while path.exists() {
        path = parent.join(format!("{stem}-{number}-{suffix}"));
        number += 1;
    }
    path
}

fn ffmpeg_tool(input: &std::path::Path, suffix: &str, options: &[&str]) -> Result<PathBuf, String> {
    let output = unique_named(input, suffix);
    let status = Command::new(engine::binary("ffmpeg"))
        .args(["-nostdin", "-i"])
        .arg(input)
        .args(options)
        .arg(&output)
        .output()
        .map_err(|e| format!("FFmpeg is required for this tool: {e}"))?;
    if !status.status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(String::from_utf8_lossy(&status.stderr).trim().to_owned());
    }
    Ok(output)
}

fn trim_media(input: &std::path::Path, start: &str, duration: &str) -> Result<PathBuf, String> {
    if start.is_empty()
        || duration.is_empty()
        || !start
            .chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
        || !duration
            .chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
    {
        return Err("Enter a valid start and duration, such as 00:00:10".into());
    }
    let ext = engine::extension(input);
    ffmpeg_tool(
        input,
        &format!("trimmed.{ext}"),
        &["-ss", start, "-t", duration, "-c", "copy"],
    )
}

fn resize_image(input: &std::path::Path, width: &str) -> Result<PathBuf, String> {
    let pixels: u32 = width
        .parse()
        .map_err(|_| "Enter a width in pixels".to_owned())?;
    if !(1..=20_000).contains(&pixels) {
        return Err("Width must be between 1 and 20,000 pixels".into());
    }
    let ext = engine::extension(input);
    let output = unique_named(input, &format!("resized.{ext}"));
    let status = Command::new(engine::binary("sips"))
        .args(["--resampleWidth", width])
        .arg(input)
        .arg("--out")
        .arg(&output)
        .output()
        .map_err(|e| e.to_string())?;
    if !status.status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(String::from_utf8_lossy(&status.stderr).trim().to_owned());
    }
    Ok(output)
}

fn crop_image(input: &std::path::Path, crop: &str) -> Result<PathBuf, String> {
    let parts: Vec<u32> = crop
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "Enter crop as width:height:x:y".to_owned())?;
    if parts.len() != 4 || parts[0] == 0 || parts[1] == 0 || parts[0] > 20_000 || parts[1] > 20_000
    {
        return Err("Enter crop as width:height:x:y using positive dimensions".into());
    }
    let ext = engine::extension(input);
    ffmpeg_tool(
        input,
        &format!("cropped.{ext}"),
        &["-vf", &format!("crop={crop}")],
    )
}

fn split_media(input: &std::path::Path, seconds: &str) -> Result<Vec<PathBuf>, String> {
    let length: u32 = seconds
        .parse()
        .map_err(|_| "Enter a positive segment length in seconds".to_owned())?;
    if !(1..=86_400).contains(&length) {
        return Err("Segment length must be between 1 and 86,400 seconds".into());
    }
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let mut folder = parent.join(format!("{stem}-parts"));
    let mut n = 2;
    while folder.exists() {
        folder = parent.join(format!("{stem}-parts-{n}"));
        n += 1;
    }
    std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let pattern = folder.join(format!("part-%03d.{}", engine::extension(input)));
    let output = Command::new(engine::binary("ffmpeg"))
        .args(["-nostdin", "-i"])
        .arg(input)
        .args([
            "-c",
            "copy",
            "-f",
            "segment",
            "-segment_time",
            seconds,
            "-reset_timestamps",
            "1",
        ])
        .arg(&pattern)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let mut parts: Vec<_> = std::fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .filter_map(|item| item.ok().map(|entry| entry.path()))
        .collect();
    parts.sort();
    if parts.is_empty() {
        return Err("No segments were created".into());
    }
    Ok(parts)
}

fn merge_media(files: &[PathBuf]) -> Result<PathBuf, String> {
    if files.len() < 2 {
        return Err("Add at least two media files".into());
    }
    let ext = engine::extension(&files[0]);
    if !files.iter().all(|file| engine::extension(file) == ext) {
        return Err("Merge requires files with the same extension".into());
    }
    let output = unique_named(&files[0], &format!("merged.{ext}"));
    let temp = std::env::temp_dir().join(format!(
        "naranja-merge-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    std::fs::create_dir(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut list = String::new();
        for (index, file) in files.iter().enumerate() {
            let link = format!("part-{index:04}.{ext}");
            std::os::unix::fs::symlink(
                file.canonicalize().map_err(|e| e.to_string())?,
                temp.join(&link),
            )
            .map_err(|e| e.to_string())?;
            list.push_str(&format!("file '{link}'\n"));
        }
        std::fs::write(temp.join("list.txt"), list).map_err(|e| e.to_string())?;
        let result = Command::new(engine::binary("ffmpeg"))
            .current_dir(&temp)
            .args([
                "-nostdin", "-f", "concat", "-safe", "0", "-i", "list.txt", "-c", "copy",
            ])
            .arg(&output)
            .output()
            .map_err(|e| e.to_string())?;
        if result.status.success() {
            Ok(output.clone())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).trim().to_owned())
        }
    })();
    let _ = std::fs::remove_dir_all(&temp);
    if result.is_err() {
        let _ = std::fs::remove_file(&output);
    }
    result
}

fn transfer_file(
    source: &std::path::Path,
    folder: &std::path::Path,
    move_file: bool,
) -> Result<PathBuf, String> {
    let name = source.file_name().ok_or("Invalid file name")?;
    let mut destination = folder.join(name);
    let mut n = 2;
    while destination.exists() {
        let stem = source.file_stem().unwrap_or(name).to_string_lossy();
        let ext = source
            .extension()
            .map(|value| format!(".{}", value.to_string_lossy()))
            .unwrap_or_default();
        destination = folder.join(format!("{stem} {n}{ext}"));
        n += 1;
    }
    if move_file {
        // Rename is atomic on one volume; a cross-volume move reports an error instead of risking data loss.
        std::fs::rename(source, &destination).map_err(|e| format!("Could not move file: {e}"))?;
    } else if source.is_dir() {
        let result = Command::new(engine::binary("ditto"))
            .arg(source)
            .arg(&destination)
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(String::from_utf8_lossy(&result.stderr).trim().to_owned());
        }
    } else {
        std::fs::copy(source, &destination).map_err(|e| e.to_string())?;
    }
    Ok(destination)
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn split_and_merge_generated_audio() {
        if !engine::binary("ffmpeg").is_file() {
            return;
        }
        let folder = std::env::temp_dir().join(format!("naranja-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let first = folder.join("first.wav");
        let second = folder.join("second.wav");
        for file in [&first, &second] {
            let status = Command::new(engine::binary("ffmpeg"))
                .args([
                    "-nostdin",
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:duration=2",
                ])
                .arg(file)
                .status()
                .unwrap();
            assert!(status.success());
        }
        let parts = split_media(&first, "1").unwrap();
        assert!(!parts.is_empty());
        let merged = merge_media(&[first, second]).unwrap();
        assert!(merged.metadata().unwrap().len() > 0);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
