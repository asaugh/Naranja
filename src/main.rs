mod engine;

use eframe::egui::{self, Color32, RichText, Stroke};
use std::{path::PathBuf, process::Command, sync::mpsc};

const ORANGE: Color32 = Color32::from_rgb(255, 143, 70);
const CREAM: Color32 = Color32::from_rgb(252, 240, 221);
const DARK: Color32 = Color32::from_rgb(32, 35, 40);

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
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([600.0, 480.0])
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
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = DARK;
    visuals.window_fill = DARK;
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(55, 58, 63);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(82, 68, 59);
    visuals.selection.bg_fill = ORANGE;
    visuals.selection.stroke.color = DARK;
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
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

impl eframe::App for NaranjaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped: Vec<_> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_files(dropped);
        }
        if let Some(receiver) = &self.receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.busy = false;
            match result {
                Ok(outputs) => {
                    self.status = format!("Created {} item(s)", outputs.len());
                    self.last_outputs = outputs;
                }
                Err(error) => self.status = error,
            }
            self.receiver = None;
        }
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").size(28.0).color(ORANGE));
                ui.label(RichText::new("Naranja").size(29.0).strong().color(CREAM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("FREE  •  OFFLINE  •  OPEN SOURCE")
                            .size(11.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            });
            ui.label(
                RichText::new("The little file workshop for your Mac.")
                    .size(15.0)
                    .color(Color32::LIGHT_GRAY),
            );
            ui.add_space(16.0);
        });

        egui::SidePanel::left("files")
            .resizable(true)
            .default_width(265.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("YOUR FILES")
                        .size(11.0)
                        .strong()
                        .color(ORANGE),
                );
                ui.add_space(8.0);
                if ui.button("＋  Add files").clicked()
                    && let Some(paths) = rfd::FileDialog::new().pick_files()
                {
                    self.add_files(paths);
                }
                if ui.button("＋  Add folder").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.add_files([path]);
                }
                ui.separator();
                if self.files.is_empty() {
                    ui.add_space(28.0);
                    ui.label(RichText::new("Drop files here").size(18.0).color(CREAM));
                    ui.label("or use the buttons above");
                } else {
                    let mut remove = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (index, file) in self.files.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let label = file.file_name().unwrap_or_default().to_string_lossy();
                                if ui
                                    .selectable_label(
                                        self.selected == index,
                                        format!("{}  {label}", icon(file)),
                                    )
                                    .clicked()
                                {
                                    self.selected = index;
                                }
                                if ui.small_button("×").clicked() {
                                    remove = Some(index);
                                }
                            });
                        }
                    });
                    if let Some(index) = remove {
                        self.files.remove(index);
                        self.selected = self.selected.min(self.files.len().saturating_sub(1));
                    }
                    ui.add_space(8.0);
                    if ui.small_button("Clear all").clicked() {
                        self.files.clear();
                        self.last_outputs.clear();
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Convert, "Convert");
                ui.selectable_value(&mut self.tab, Tab::Tools, "Tools");
                ui.selectable_value(&mut self.tab, Tab::Actions, "Actions");
            });
            ui.separator();
            ui.add_space(16.0);
            let Some(selected) = self.selected_file().cloned() else {
                ui.label(
                    RichText::new("A fresh start for every file.")
                        .size(24.0)
                        .strong()
                        .color(CREAM),
                );
                ui.add_space(8.0);
                ui.label("Add a file or drag one into this window to see what you can do.");
                return;
            };
            ui.label(
                RichText::new(format!(
                    "{}  {}",
                    icon(&selected),
                    selected.file_name().unwrap_or_default().to_string_lossy()
                ))
                .size(23.0)
                .strong()
                .color(CREAM),
            );
            ui.label(format!(
                "{} • {}",
                engine::kind(&selected),
                selected.parent().unwrap_or(&selected).display()
            ));
            ui.add_space(18.0);

            match self.tab {
                Tab::Convert => {
                    ui.label(
                        RichText::new("CONVERT TO")
                            .size(11.0)
                            .strong()
                            .color(ORANGE),
                    );
                    ui.label("The same format is applied to every compatible file in your list.");
                    ui.add_space(8.0);
                    let formats = engine::formats(&selected);
                    if formats.is_empty() {
                        ui.label("No conversions are available for this file yet.");
                    }
                    for chunk in formats.chunks(4) {
                        ui.horizontal(|ui| {
                            for format in chunk {
                                if ui
                                    .add_enabled(
                                        !self.busy,
                                        egui::Button::new(format!("  {}  ", format.to_uppercase()))
                                            .stroke(Stroke::new(1.0_f32, ORANGE)),
                                    )
                                    .clicked()
                                {
                                    self.convert_all(format);
                                }
                            }
                        });
                    }
                }
                Tab::Tools => {
                    ui.label(
                        RichText::new("FILE TOOLS")
                            .size(11.0)
                            .strong()
                            .color(ORANGE),
                    );
                    let kind = engine::kind(&selected);
                    let mut tools = Vec::new();
                    if selected.is_dir() || kind != "Archive" {
                        tools.push("Compress to ZIP");
                    }
                    if kind == "Archive" {
                        tools.push("Extract archive");
                    }
                    if kind == "Image" {
                        tools.push("Compress image");
                    }
                    if kind == "Video" {
                        tools.extend(["Compress video", "Extract audio"]);
                    }
                    for tool in tools {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new(tool))
                            .clicked()
                        {
                            self.run_tool(tool);
                        }
                    }
                    if kind == "Video" || kind == "Audio" {
                        ui.separator();
                        ui.label("Trim media");
                        ui.horizontal(|ui| {
                            ui.label("Start");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.trim_start)
                                    .hint_text("00:00:00"),
                            );
                            ui.label("Duration");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.trim_duration)
                                    .hint_text("00:00:10"),
                            );
                        });
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Trim"))
                            .clicked()
                        {
                            let start = self.trim_start.clone();
                            let duration = self.trim_duration.clone();
                            let file = selected.clone();
                            self.dispatch(move || {
                                trim_media(&file, &start, &duration).map(|p| vec![p])
                            });
                        }
                    }
                    if kind == "Image" {
                        ui.separator();
                        ui.label("Resize image");
                        ui.horizontal(|ui| {
                            ui.label("Width (px)");
                            ui.add(egui::TextEdit::singleline(&mut self.width).hint_text("1200"));
                        });
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Resize"))
                            .clicked()
                        {
                            let width = self.width.clone();
                            let file = selected.clone();
                            self.dispatch(move || resize_image(&file, &width).map(|p| vec![p]));
                        }
                        ui.label("Crop image: width:height:x:y");
                        ui.add(egui::TextEdit::singleline(&mut self.crop).hint_text("800:600:0:0"));
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Crop"))
                            .clicked()
                        {
                            let crop = self.crop.clone();
                            let file = selected.clone();
                            self.dispatch(move || crop_image(&file, &crop).map(|p| vec![p]));
                        }
                    }
                    if kind == "Video" || kind == "Audio" {
                        ui.separator();
                        ui.label("Split into segments (seconds)");
                        ui.add(egui::TextEdit::singleline(&mut self.split_seconds).hint_text("30"));
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Split"))
                            .clicked()
                        {
                            let seconds = self.split_seconds.clone();
                            let file = selected.clone();
                            self.dispatch(move || split_media(&file, &seconds));
                        }
                        if self.files.len() > 1
                            && ui
                                .add_enabled(
                                    !self.busy,
                                    egui::Button::new("Merge matching media in list order"),
                                )
                                .clicked()
                        {
                            let files = self.files.clone();
                            self.dispatch(move || merge_media(&files).map(|p| vec![p]));
                        }
                    }
                }
                Tab::Actions => {
                    ui.label(
                        RichText::new("QUICK ACTIONS")
                            .size(11.0)
                            .strong()
                            .color(ORANGE),
                    );
                    for action in [
                        "Copy path",
                        "Reveal in Finder",
                        "Open file",
                        "Open in Terminal",
                        "Copy to folder",
                        "Move to folder",
                    ] {
                        if ui.button(action).clicked() {
                            self.action(action, ctx);
                        }
                    }
                }
            }
            ui.add_space(24.0);
            if !self.last_outputs.is_empty() {
                ui.separator();
                ui.label(
                    RichText::new("RECENT OUTPUT")
                        .size(11.0)
                        .strong()
                        .color(ORANGE),
                );
                for path in &self.last_outputs {
                    if ui
                        .link(path.file_name().unwrap_or_default().to_string_lossy())
                        .clicked()
                    {
                        let _ = Command::new("open").arg(path).spawn();
                    }
                }
            }
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(if self.busy { "◌" } else { "●" }).color(ORANGE));
                ui.label(if self.status.is_empty() {
                    "Ready. Your files stay on this Mac."
                } else {
                    &self.status
                });
            });
            ui.add_space(8.0);
        });
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
