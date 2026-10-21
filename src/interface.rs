use super::*;
use eframe::egui::{Align2, FontId, Pos2, RichText, Sense, Shape, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

const INK: Color32 = Color32::from_rgb(35, 38, 42);
const MUTED: Color32 = Color32::from_rgb(126, 130, 135);
const LINE: Color32 = Color32::from_rgb(228, 229, 228);
const CANVAS: Color32 = Color32::from_rgb(253, 253, 252);

impl NaranjaApp {
    fn wheel_options(&self, file: &std::path::Path) -> Vec<&'static str> {
        match self.tab {
            Tab::Convert => engine::formats(file),
            Tab::Tools => {
                let mut options = Vec::new();
                let kind = engine::kind(file);
                if kind == "Archive" {
                    options.push("Extract archive");
                } else {
                    options.push("Compress to ZIP");
                }
                match kind {
                    "Image" => options.extend(["Compress image", "Resize image", "Crop image"]),
                    "Audio" => options.extend(["Trim media", "Split media"]),
                    "Video" => options.extend([
                        "Compress video",
                        "Extract audio",
                        "Trim media",
                        "Split media",
                    ]),
                    _ => {}
                }
                if ["Audio", "Video"].contains(&kind) && self.files.len() > 1 {
                    options.push("Merge media");
                }
                options
            }
            Tab::Actions => vec![
                "Copy path",
                "Reveal in Finder",
                "Open file",
                "Open in Terminal",
                "Copy to folder",
                "Move to folder",
            ],
        }
    }

    fn activate_wheel(&mut self, choice: &'static str, ctx: &egui::Context) {
        match self.tab {
            Tab::Convert => self.convert_all(choice),
            Tab::Actions => self.action(choice, ctx),
            Tab::Tools => match choice {
                "Resize image" | "Crop image" | "Trim media" | "Split media" => {
                    self.active_tool = Some(choice.to_owned())
                }
                "Merge media" => {
                    let files = self.files.clone();
                    self.dispatch(move || merge_media(&files).map(|p| vec![p]));
                }
                other => self.run_tool(other),
            },
        }
    }

    fn tool_controls(&mut self, ui: &mut egui::Ui, file: &std::path::Path) {
        let Some(tool) = self.active_tool.clone() else {
            return;
        };
        egui::Frame::new()
            .fill(Color32::from_rgb(249, 249, 247))
            .stroke(Stroke::new(1.0_f32, LINE))
            .corner_radius(14.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&tool).size(16.0).strong().color(INK));
                    if ui.small_button("×").clicked() {
                        self.active_tool = None;
                    }
                });
                match tool.as_str() {
                    "Resize image" => {
                        ui.horizontal(|ui| {
                            ui.label("Width in pixels");
                            ui.add_sized(
                                [110.0, 30.0],
                                egui::TextEdit::singleline(&mut self.width).hint_text("1200"),
                            );
                            if ui
                                .add_enabled(!self.busy, egui::Button::new("Resize"))
                                .clicked()
                            {
                                let path = file.to_path_buf();
                                let width = self.width.clone();
                                self.dispatch(move || resize_image(&path, &width).map(|p| vec![p]));
                            }
                        });
                    }
                    "Crop image" => {
                        ui.horizontal(|ui| {
                            ui.label("Width:height:x:y");
                            ui.add_sized(
                                [145.0, 30.0],
                                egui::TextEdit::singleline(&mut self.crop).hint_text("800:600:0:0"),
                            );
                            if ui
                                .add_enabled(!self.busy, egui::Button::new("Crop"))
                                .clicked()
                            {
                                let path = file.to_path_buf();
                                let crop = self.crop.clone();
                                self.dispatch(move || crop_image(&path, &crop).map(|p| vec![p]));
                            }
                        });
                    }
                    "Trim media" => {
                        ui.horizontal(|ui| {
                            ui.label("Start");
                            ui.add_sized(
                                [92.0, 30.0],
                                egui::TextEdit::singleline(&mut self.trim_start)
                                    .hint_text("00:00:00"),
                            );
                            ui.label("Duration");
                            ui.add_sized(
                                [92.0, 30.0],
                                egui::TextEdit::singleline(&mut self.trim_duration)
                                    .hint_text("00:00:10"),
                            );
                            if ui
                                .add_enabled(!self.busy, egui::Button::new("Trim"))
                                .clicked()
                            {
                                let path = file.to_path_buf();
                                let start = self.trim_start.clone();
                                let duration = self.trim_duration.clone();
                                self.dispatch(move || {
                                    trim_media(&path, &start, &duration).map(|p| vec![p])
                                });
                            }
                        });
                    }
                    "Split media" => {
                        ui.horizontal(|ui| {
                            ui.label("Seconds per segment");
                            ui.add_sized(
                                [90.0, 30.0],
                                egui::TextEdit::singleline(&mut self.split_seconds).hint_text("30"),
                            );
                            if ui
                                .add_enabled(!self.busy, egui::Button::new("Split"))
                                .clicked()
                            {
                                let path = file.to_path_buf();
                                let seconds = self.split_seconds.clone();
                                self.dispatch(move || split_media(&path, &seconds));
                            }
                        });
                    }
                    _ => {}
                }
            });
    }
}

impl eframe::App for NaranjaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let hovered_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovered_files {
            let next = ctx.input(|i| {
                if i.modifiers.command {
                    Some(Tab::Tools)
                } else if i.modifiers.ctrl {
                    Some(Tab::Actions)
                } else if i.modifiers.shift {
                    Some(Tab::Convert)
                } else {
                    None
                }
            });
            if let Some(tab) = next {
                self.tab = tab;
                self.wheel_page = 0;
            }
        }
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        let dropped_now = !dropped.is_empty();
        if dropped_now {
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
        if self.busy || hovered_files {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }

        egui::TopBottomPanel::top("topbar")
            .exact_height(78.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    brand_mark(ui);
                    ui.add_space(6.0);
                    ui.label(RichText::new("naranja").size(25.0).strong().color(INK));
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("Your files, your Mac.")
                            .size(13.0)
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("FREE & OFFLINE")
                                .size(11.0)
                                .strong()
                                .color(ORANGE),
                        );
                    });
                });
            });

        egui::SidePanel::left("tray")
            .resizable(false)
            .exact_width(228.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(249, 249, 248))
                    .inner_margin(egui::Margin::symmetric(17, 20)),
            )
            .show(ctx, |ui| {
                ui.label(RichText::new("WORKSPACE").size(11.0).strong().color(MUTED));
                ui.add_space(12.0);
                if ui
                    .add_sized(
                        [194.0, 36.0],
                        egui::Button::new(
                            RichText::new("＋  Add files")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(ORANGE),
                    )
                    .clicked()
                    && let Some(files) = rfd::FileDialog::new().pick_files()
                {
                    self.add_files(files);
                }
                if ui
                    .add_sized([194.0, 34.0], egui::Button::new("Add folder"))
                    .clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.add_files([folder]);
                }
                ui.add_space(15.0);
                ui.separator();
                ui.label(
                    RichText::new(format!("FILES  ·  {}", self.files.len()))
                        .size(11.0)
                        .strong()
                        .color(MUTED),
                );
                ui.add_space(7.0);
                if self.files.is_empty() {
                    ui.label(
                        RichText::new("Drop something here to begin.")
                            .size(14.0)
                            .color(MUTED),
                    );
                } else {
                    let mut remove = None;
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 92.0)
                        .show(ui, |ui| {
                            for (index, file) in self.files.iter().enumerate() {
                                let selected = index == self.selected;
                                let fill = if selected {
                                    Color32::from_rgb(255, 235, 220)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                egui::Frame::new()
                                    .fill(fill)
                                    .corner_radius(10.0)
                                    .inner_margin(egui::Margin::symmetric(8, 8))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(format!(
                                                        "{}  {}",
                                                        icon(file),
                                                        truncate_name(file, 15)
                                                    ))
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false),
                                                )
                                                .clicked()
                                            {
                                                self.selected = index;
                                                self.wheel_page = 0;
                                                self.active_tool = None;
                                            }
                                            if ui.small_button("×").clicked() {
                                                remove = Some(index);
                                            }
                                        });
                                    });
                            }
                        });
                    if let Some(index) = remove {
                        self.files.remove(index);
                        self.selected = self.selected.min(self.files.len().saturating_sub(1));
                    }
                    if ui.small_button("Clear list").clicked() {
                        self.files.clear();
                        self.last_outputs.clear();
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("No uploads. No limits. No account.")
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            });

        egui::TopBottomPanel::bottom("footer")
            .exact_height(37.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if self.busy { "◌" } else { "●" }).color(
                        if self.busy {
                            ORANGE
                        } else {
                            Color32::from_rgb(82, 159, 104)
                        },
                    ));
                    ui.label(
                        RichText::new(if self.status.is_empty() {
                            "Ready to make something new."
                        } else {
                            &self.status
                        })
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("DRAG  ·  CHOOSE  ·  DONE")
                                .size(10.0)
                                .color(MUTED),
                        );
                    });
                });
            });

        let mut activate: Option<&'static str> = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(28, 16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let Some(file) = self.selected_file().cloned() else {
                            empty_state(ui, hovered_files);
                            return;
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("WORK WITH").size(11.0).strong().color(MUTED));
                            ui.label(
                                RichText::new(engine::kind(&file).to_uppercase())
                                    .size(11.0)
                                    .strong()
                                    .color(ORANGE),
                            );
                        });
                        ui.add_space(10.0);
                        file_card(ui, &file, self.files.len());
                        ui.add_space(17.0);
                        ui.horizontal(|ui| {
                            tab_button(
                                ui,
                                &mut self.tab,
                                Tab::Convert,
                                "⇧",
                                "Convert",
                                &mut self.wheel_page,
                            );
                            tab_button(
                                ui,
                                &mut self.tab,
                                Tab::Tools,
                                "⌘",
                                "Tools",
                                &mut self.wheel_page,
                            );
                            tab_button(
                                ui,
                                &mut self.tab,
                                Tab::Actions,
                                "⌃",
                                "Actions",
                                &mut self.wheel_page,
                            );
                        });
                        ui.add_space(2.0);

                        let options = self.wheel_options(&file);
                        let pages = options.len().div_ceil(6).max(1);
                        self.wheel_page = self.wheel_page.min(pages - 1);
                        let start = self.wheel_page * 6;
                        let page = &options[start..options.len().min(start + 6)];
                        let center_text = match self.tab {
                            Tab::Convert => "CONVERT",
                            Tab::Tools => "TOOLS",
                            Tab::Actions => "ACTIONS",
                        };
                        let wheel_width = ui.available_width().min(430.0);
                        ui.horizontal(|ui| {
                            ui.add_space((ui.available_width() - wheel_width).max(0.0) / 2.0);
                            let selection = paint_wheel(
                                ui,
                                page,
                                center_text,
                                wheel_width,
                                self.busy,
                                hovered_files,
                            );
                            if let Some(index) = selection.clicked.or(if dropped_now {
                                selection.hovered
                            } else {
                                None
                            }) {
                                activate = page.get(index).copied();
                            }
                        });
                        if pages > 1 {
                            ui.horizontal_centered(|ui| {
                                if ui.small_button("← Previous").clicked() {
                                    self.wheel_page = (self.wheel_page + pages - 1) % pages;
                                }
                                ui.label(
                                    RichText::new(format!("{} / {}", self.wheel_page + 1, pages))
                                        .color(MUTED),
                                );
                                if ui.small_button("Next →").clicked() {
                                    self.wheel_page = (self.wheel_page + 1) % pages;
                                }
                            });
                        }
                        ui.horizontal_centered(|ui| {
                            paint_keycap(ui, &self.tab);
                        });
                        if self.tab == Tab::Tools {
                            self.tool_controls(ui, &file);
                        }
                        if !self.last_outputs.is_empty() {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("CREATED").size(10.0).strong().color(MUTED));
                                let count = self.last_outputs.len();
                                if let Some(path) = self.last_outputs.last() {
                                    let label = if count == 1 {
                                        path.file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string()
                                    } else {
                                        format!("{count} files")
                                    };
                                    if ui.link(label).clicked() {
                                        let _ = Command::new("open").arg(path).spawn();
                                    }
                                }
                            });
                        }
                    });
            });

        if let Some(choice) = activate
            && !self.busy
        {
            self.activate_wheel(choice, ctx);
        }
    }
}

fn truncate_name(path: &std::path::Path, max: usize) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if name.chars().count() > max {
        format!("{}…", name.chars().take(max).collect::<String>())
    } else {
        name.to_string()
    }
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(33.0), Sense::hover());
    let center = rect.center();
    let p = ui.painter();
    p.circle_filled(center, 15.0, ORANGE);
    p.circle_stroke(
        center,
        13.0,
        Stroke::new(1.5_f32, Color32::from_rgb(255, 204, 151)),
    );
    for index in 0..6 {
        let angle = index as f32 * TAU / 6.0;
        p.line_segment(
            [
                center + Vec2::angled(angle) * 3.0,
                center + Vec2::angled(angle) * 12.0,
            ],
            Stroke::new(1.5_f32, Color32::WHITE),
        );
    }
    p.circle_filled(center, 2.6, Color32::WHITE);
}

fn paint_keycap(ui: &mut egui::Ui, tab: &Tab) {
    let (symbol, name) = match tab {
        Tab::Convert => ("⇧", "shift"),
        Tab::Tools => ("⌘", "command"),
        Tab::Actions => ("⌃", "control"),
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(144.0, 55.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        rect.translate(Vec2::new(0.0, 5.0)),
        10.0,
        Color32::from_rgb(155, 153, 148),
    );
    p.rect_filled(
        rect.shrink2(Vec2::new(3.0, 3.0)),
        10.0,
        Color32::from_rgb(230, 228, 223),
    );
    p.rect_filled(
        egui::Rect::from_min_max(
            rect.min + Vec2::new(7.0, 3.0),
            rect.max - Vec2::new(7.0, 13.0),
        ),
        8.0,
        Color32::from_rgb(248, 247, 243),
    );
    p.text(
        rect.center() + Vec2::new(0.0, -8.0),
        Align2::CENTER_CENTER,
        symbol,
        FontId::proportional(19.0),
        INK,
    );
    p.text(
        rect.center() + Vec2::new(0.0, 10.0),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        INK,
    );
}

fn empty_state(ui: &mut egui::Ui, dragging: bool) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.18).min(100.0));
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(132.0), Sense::hover());
        let p = ui.painter();
        p.circle_filled(rect.center(), 61.0, Color32::from_rgb(255, 240, 226));
        p.circle_stroke(rect.center(), 48.0, Stroke::new(2.0_f32, ORANGE));
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "↗",
            FontId::proportional(44.0),
            ORANGE,
        );
        ui.add_space(22.0);
        ui.label(
            RichText::new(if dragging {
                "Drop to begin"
            } else {
                "Every file has possibilities."
            })
            .size(28.0)
            .strong()
            .color(INK),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("Drag a file into Naranja, or add one from your Mac.")
                .size(14.0)
                .color(MUTED),
        );
        ui.add_space(20.0);
        ui.label(
            RichText::new("⇧  CONVERT     ⌘  TOOLS     ⌃  ACTIONS")
                .size(11.0)
                .color(ORANGE),
        );
    });
}

fn file_card(ui: &mut egui::Ui, path: &std::path::Path, count: usize) {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(15, 12))
        .show(ui, |ui| {
            ui.set_width(width - 32.0);
            ui.horizontal_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 10.0, Color32::from_rgb(255, 237, 222));
                p.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    icon(path),
                    FontId::proportional(27.0),
                    ORANGE,
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(truncate_name(path, 46))
                            .size(17.0)
                            .strong()
                            .color(INK),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {}",
                            engine::kind(path),
                            path.parent().unwrap_or(path).display()
                        ))
                        .size(11.0)
                        .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if count > 1 {
                        ui.label(
                            RichText::new(format!("+{} more", count - 1))
                                .size(11.0)
                                .color(MUTED),
                        );
                    }
                });
            });
        });
}

fn tab_button(
    ui: &mut egui::Ui,
    current: &mut Tab,
    tab: Tab,
    key: &str,
    label: &str,
    page: &mut usize,
) {
    let selected = *current == tab;
    let text = RichText::new(format!("{key}  {label}"))
        .size(13.0)
        .strong()
        .color(if selected { Color32::WHITE } else { INK });
    if ui
        .add(
            egui::Button::new(text)
                .fill(if selected {
                    ORANGE
                } else {
                    Color32::from_rgb(243, 243, 241)
                })
                .corner_radius(8.0),
        )
        .clicked()
    {
        *current = tab;
        *page = 0;
    }
}

struct WheelSelection {
    clicked: Option<usize>,
    hovered: Option<usize>,
}

fn paint_wheel(
    ui: &mut egui::Ui,
    options: &[&str],
    center_label: &str,
    width: f32,
    busy: bool,
    dragging: bool,
) -> WheelSelection {
    let side = width.clamp(310.0, 390.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let center = rect.center();
    let scale = side / 390.0;
    let outer = 178.0 * scale;
    let inner = 75.0 * scale;
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let hovered = pointer.and_then(|pos| wheel_index(pos, center, inner, outer, options.len()));
    let selected = hovered.unwrap_or(0);
    let p = ui.painter();
    p.circle_filled(
        center + Vec2::new(0.0, 8.0 * scale),
        outer + 7.0 * scale,
        Color32::from_rgba_unmultiplied(30, 35, 40, 21),
    );
    p.circle_filled(
        center,
        outer + 6.0 * scale,
        Color32::from_rgb(211, 213, 212),
    );
    p.circle_filled(
        center,
        outer + 3.0 * scale,
        Color32::from_rgb(238, 239, 238),
    );
    if options.is_empty() {
        p.circle_filled(center, outer, Color32::from_rgb(246, 246, 245));
        p.text(
            center,
            Align2::CENTER_CENTER,
            "NO OPTIONS",
            FontId::proportional(14.0),
            MUTED,
        );
        return WheelSelection {
            clicked: None,
            hovered: None,
        };
    }
    let step = TAU / options.len() as f32;
    for (index, label) in options.iter().enumerate() {
        let mid = -PI / 2.0 + index as f32 * step;
        let from = mid - step / 2.0 + 0.018;
        let to = mid + step / 2.0 - 0.018;
        let active = index == selected && !busy;
        let fill = if active {
            ORANGE
        } else {
            Color32::from_rgb(249, 249, 248)
        };
        let outline = if active {
            Color32::from_rgb(218, 84, 30)
        } else {
            Color32::from_rgb(212, 214, 214)
        };
        paint_ring_sector(
            p,
            center,
            (inner + 4.0 * scale, outer - 2.0 * scale),
            (from, to),
            fill,
            outline,
            scale,
        );
        if active {
            p.add(Shape::line(
                arc_points(center, outer - 7.0 * scale, from + 0.05, to - 0.05),
                Stroke::new(2.0 * scale, Color32::from_rgb(255, 181, 117)),
            ));
        } else {
            p.add(Shape::line(
                arc_points(center, inner + 11.0 * scale, from + 0.05, to - 0.05),
                Stroke::new(1.5 * scale, Color32::WHITE),
            ));
        }
        let label_at = center + Vec2::angled(mid) * ((inner + outer) * 0.55);
        let display = short_label(label);
        p.text(
            label_at,
            Align2::CENTER_CENTER,
            display,
            FontId::proportional(if label.len() > 13 {
                12.0 * scale
            } else {
                15.0 * scale
            }),
            if active { Color32::WHITE } else { INK },
        );
    }
    p.circle_filled(
        center + Vec2::new(0.0, 3.0 * scale),
        inner + 4.0 * scale,
        Color32::from_rgba_unmultiplied(48, 51, 55, 31),
    );
    p.circle_filled(
        center,
        inner + 2.0 * scale,
        Color32::from_rgb(218, 220, 220),
    );
    p.circle_filled(
        center,
        inner - 2.0 * scale,
        Color32::from_rgb(253, 253, 252),
    );
    p.circle_stroke(center, inner - 10.0 * scale, Stroke::new(1.0 * scale, LINE));
    p.text(
        center + Vec2::new(0.0, -8.0 * scale),
        Align2::CENTER_CENTER,
        center_label,
        FontId::proportional(16.0 * scale),
        INK,
    );
    p.text(
        center + Vec2::new(0.0, 14.0 * scale),
        Align2::CENTER_CENTER,
        if dragging {
            "DROP TO APPLY"
        } else {
            "CLICK A SLICE"
        },
        FontId::proportional(9.0 * scale),
        MUTED,
    );
    WheelSelection {
        clicked: if response.clicked() { hovered } else { None },
        hovered,
    }
}

fn short_label(label: &str) -> String {
    if label.len() <= 13 {
        label.to_uppercase()
    } else {
        label
            .split_once(' ')
            .map(|(a, b)| format!("{}\n{}", a.to_uppercase(), b.to_uppercase()))
            .unwrap_or_else(|| label.to_uppercase())
    }
}

fn wheel_index(pointer: Pos2, center: Pos2, inner: f32, outer: f32, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let offset = pointer - center;
    let distance = offset.length();
    if distance < inner || distance > outer {
        return None;
    }
    let angle = (offset.y.atan2(offset.x) + PI / 2.0).rem_euclid(TAU);
    let step = TAU / count as f32;
    Some((((angle + step / 2.0) / step).floor() as usize) % count)
}

fn arc_points(center: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    (0..=32)
        .map(|i| center + Vec2::angled(from + (to - from) * i as f32 / 32.0) * radius)
        .collect()
}

fn paint_ring_sector(
    p: &egui::Painter,
    center: Pos2,
    radii: (f32, f32),
    angles: (f32, f32),
    fill: Color32,
    outline: Color32,
    scale: f32,
) {
    let (inner, outer) = radii;
    let (from, to) = angles;
    let outer_points = arc_points(center, outer, from, to);
    let inner_points = arc_points(center, inner, from, to);
    // Each narrow quad is convex; a whole donut slice is not, so it cannot be filled as one polygon.
    for index in 0..32 {
        p.add(Shape::convex_polygon(
            vec![
                outer_points[index],
                outer_points[index + 1],
                inner_points[index + 1],
                inner_points[index],
            ],
            fill,
            Stroke::NONE,
        ));
    }
    let stroke = Stroke::new(1.2 * scale, outline);
    p.add(Shape::line(outer_points.clone(), stroke));
    p.add(Shape::line(inner_points.clone(), stroke));
    p.line_segment([outer_points[0], inner_points[0]], stroke);
    p.line_segment([outer_points[32], inner_points[32]], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_of_wheel_selects_first_slice() {
        let center = Pos2::new(200.0, 200.0);
        assert_eq!(
            wheel_index(Pos2::new(200.0, 90.0), center, 70.0, 180.0, 6),
            Some(0)
        );
        assert_eq!(wheel_index(center, center, 70.0, 180.0, 6), None);
    }
}
