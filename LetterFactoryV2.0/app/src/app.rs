//! Main application window. Port of the Java `MainApp` / `MainWindow`.

use std::path::PathBuf;

use egui::RichText;

use crate::bulk::BulkWindow;
use crate::help::HelpWindow;
use crate::{docx, export, theme};

#[derive(Clone, Copy, PartialEq)]
enum StatusKind {
    Neutral,
    Success,
    Danger,
}

pub struct App {
    template: Option<PathBuf>,
    template_label: String,
    output_dir: Option<PathBuf>,
    output_label: String,

    placeholders: Vec<String>,
    /// (placeholder, value) preserving detection order.
    inputs: Vec<(String, String)>,

    status: String,
    status_kind: StatusKind,

    bulk: BulkWindow,
    help: HelpWindow,

    logo: Option<egui::TextureHandle>,
    dialog: Option<(String, String)>, // (title, message)
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        let logo = load_logo(ctx);
        Self {
            template: None,
            template_label: "Template".to_string(),
            output_dir: None,
            output_label: "Output Folder".to_string(),
            placeholders: Vec::new(),
            inputs: Vec::new(),
            status: "Bulk Status: N/A".to_string(),
            status_kind: StatusKind::Neutral,
            bulk: BulkWindow::default(),
            help: HelpWindow::default(),
            logo,
            dialog: None,
        }
    }

    // --- actions -----------------------------------------------------------

    fn choose_template(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Word Document", &["docx"])
            .set_title("Select a .docx template")
            .pick_file()
        {
            self.load_template(path);
        }
    }

    fn load_template(&mut self, path: PathBuf) {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let lower = name.to_lowercase();
        if !lower.ends_with(".docx") {
            self.dialog = Some(("Invalid template".into(), "Please choose a .docx file.".into()));
            return;
        }
        if name.starts_with("~$") {
            self.dialog = Some((
                "Invalid template".into(),
                "That file is a temporary Word lock file. Open the real .docx template instead.".into(),
            ));
            return;
        }

        match docx::find_placeholders(&path) {
            Ok(found) => {
                if found.is_empty() {
                    self.dialog = Some((
                        "No placeholders".into(),
                        "No placeholders found. Add [[brackets]] to your template to create input fields.".into(),
                    ));
                }
                self.placeholders = found.clone();
                self.inputs = found.into_iter().map(|p| (p, String::new())).collect();
                self.template = Some(path);
                self.template_label = truncate_middle(&name, 16);
            }
            Err(e) => {
                self.placeholders.clear();
                self.inputs.clear();
                self.template = None;
                self.template_label = "Template".to_string();
                self.dialog = Some(("Template error".into(), format!("Could not read template: {e}")));
            }
        }
    }

    fn choose_output(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Select output folder").pick_folder() {
            let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            self.output_label = truncate_middle(&name, 16);
            self.output_dir = Some(dir);
        }
    }

    fn require_output(&mut self) -> bool {
        if self.output_dir.as_ref().map(|d| d.is_dir()).unwrap_or(false) {
            return true;
        }
        self.dialog = Some((
            "Output folder required".into(),
            "You need to select an output folder before exporting.".into(),
        ));
        false
    }

    fn all_fields_filled(&self) -> bool {
        self.template.is_some()
            && !self.inputs.is_empty()
            && self.inputs.iter().all(|(_, v)| !v.trim().is_empty())
    }

    fn replacements(&self) -> std::collections::HashMap<String, String> {
        self.inputs.iter().cloned().collect()
    }

    fn export_single(&mut self) {
        let Some(template) = self.template.clone() else { return };
        if !self.require_output() {
            return;
        }
        let output = self.output_dir.clone().unwrap();
        match export::export_single(&template, &self.replacements(), &output) {
            Ok(path) => {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                self.status = format!("Exported: {name}");
                self.status_kind = StatusKind::Success;
            }
            Err(e) => self.dialog = Some(("Export failed".into(), e.to_string())),
        }
    }

    fn print_single(&mut self) {
        let Some(template) = self.template.clone() else { return };
        match export::print_single(&template, &self.replacements()) {
            Ok(()) => {
                self.status = "Sent to printer / default handler".to_string();
                self.status_kind = StatusKind::Success;
            }
            Err(e) => self.dialog = Some(("Print failed".into(), e.to_string())),
        }
    }

    fn open_bulk(&mut self) {
        if self.template.is_none() {
            self.dialog = Some(("Template required".into(), "Load a .docx template before opening Bulk Mode.".into()));
            return;
        }
        if !self.require_output() {
            return;
        }
        let template = self.template.clone().unwrap();
        let output = self.output_dir.clone().unwrap();
        self.bulk.open_with(template, output, self.placeholders.clone());
    }

    // --- drag and drop -----------------------------------------------------

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            if let Some(path) = f.path {
                if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false) {
                    self.load_template(path);
                    break;
                }
            }
        }
    }

    // --- panels ------------------------------------------------------------

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(RichText::new("Letter").size(26.0).strong().color(theme::TEXT));
                ui.label(RichText::new("Factory").size(26.0).strong().color(theme::ACCENT));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let help = egui::Button::new(RichText::new("?").strong().color(theme::TEXT))
                    .fill(theme::PANEL_RAISED)
                    .corner_radius(egui::CornerRadius::same(16))
                    .min_size(egui::vec2(32.0, 32.0));
                if ui.add(help).clicked() {
                    self.help.open = true;
                }
            });
        });
        ui.add_space(6.0);
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(theme::label_text("Template"));
                if theme::surface_button(ui, &self.template_label).clicked() {
                    self.choose_template();
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(theme::label_text("Output Folder"));
                if theme::surface_button(ui, &self.output_label).clicked() {
                    self.choose_output();
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(theme::label_text("Bulk Mode"));
                if theme::surface_button(ui, "Bulk Mode").clicked() {
                    self.open_bulk();
                }
            });
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let can = self.all_fields_filled() && self.output_dir.is_some();
            ui.add_enabled_ui(can, |ui| {
                if theme::accent_button(ui, "Export").clicked() {
                    self.export_single();
                }
            });
            ui.add_space(6.0);
            ui.add_enabled_ui(can, |ui| {
                if theme::surface_button(ui, "Print").clicked() {
                    self.print_single();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let color = match self.status_kind {
                    StatusKind::Success => theme::SUCCESS,
                    StatusKind::Danger => theme::DANGER,
                    StatusKind::Neutral => theme::MUTED,
                };
                ui.colored_label(color, self.status.as_str());
            });
        });
        ui.add_space(6.0);
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        if self.inputs.is_empty() {
            self.empty_state(ui);
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (placeholder, value) in self.inputs.iter_mut() {
                ui.add_space(6.0);
                ui.label(RichText::new(placeholder.as_str()).size(13.0).strong().color(theme::TEXT));
                ui.add(
                    egui::TextEdit::singleline(value)
                        .hint_text(format!("Enter {placeholder}..."))
                        .desired_width(f32::INFINITY),
                );
            }
            ui.add_space(8.0);
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        // Keep the looping animation running.
        let t = ui.input(|i| i.time) as f32;
        ui.ctx().request_repaint();

        // Vertically center the animated scene + text within the available area.
        let avail_h = ui.available_height();
        let text_block = 70.0;
        let scene_h = (avail_h - text_block - 24.0).clamp(240.0, 440.0);
        let top = ((avail_h - scene_h - text_block) / 2.0).max(8.0);
        ui.add_space(top);

        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, scene_h), egui::Sense::hover());

        // Animated gears + conveyor belt behind/around the icon.
        theme::paint_factory_scene(ui.painter(), rect, t);

        // Centered icon on top of the scene.
        if let Some(logo) = &self.logo {
            let s = (scene_h * 0.42).min(190.0);
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.center().y - scene_h * 0.06),
                egui::vec2(s, s),
            );
            egui::Image::new(logo).paint_at(ui, icon_rect);
        }

        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Load a template to begin").size(20.0).color(theme::MUTED));
            ui.add_space(6.0);
            ui.label(RichText::new("Click Template or drag and drop a .docx file here").size(14.0).color(theme::MUTED));
        });
    }

    fn dialog(&mut self, ctx: &egui::Context) {
        let Some((title, message)) = self.dialog.clone() else { return };
        let mut close = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(theme::window_frame())
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(theme::body_text(&message));
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::accent_button(ui, "OK").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.dialog = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);

        // Pull any status updates from a finished bulk run.
        if let Some(status) = self.bulk.take_main_status() {
            self.status = status.text;
            self.status_kind = if status.success { StatusKind::Success } else { StatusKind::Danger };
        }

        egui::Panel::top("header")
            .resizable(false)
            .frame(egui::Frame::default().fill(theme::BG).inner_margin(egui::Margin::symmetric(18, 6)))
            .show(ui, |ui| self.header(ui));

        egui::Panel::bottom("controls")
            .resizable(false)
            .frame(
                egui::Frame::default()
                    .fill(theme::PANEL_SOLID)
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .show(ui, |ui| self.controls(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::BG).inner_margin(egui::Margin::same(24)))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                theme::paint_background(ui, rect);
                self.central(ui);
            });

        self.bulk.ui(&ctx);
        self.help.ui(&ctx);
        self.dialog(&ctx);
    }
}

// --- helpers ---------------------------------------------------------------

fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../assets/icon_256.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture("logo", color, egui::TextureOptions::LINEAR))
}

fn truncate_middle(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_len {
        return s.to_string();
    }
    let keep = ((max_len.saturating_sub(3)) / 2).max(2);
    let head: String = chars[..keep].iter().collect();
    let tail: String = chars[chars.len() - keep..].iter().collect();
    format!("{head}...{tail}")
}
