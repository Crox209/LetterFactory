//! Bulk Mode: generate many documents from an `.xlsx`, with optional merge into
//! one `.docx`. Port of the Java `BulkModeEngine` (generation) and
//! `BulkModeWindow` (UI), adapted to egui.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use anyhow::{anyhow, Result};
use chrono::Local;

use crate::{docx, docx_merge, filenamer, theme, xlsx};

/// Messages sent from the generation worker to the UI.
pub enum BulkMsg {
    Preview { documents: usize, message: String, mismatch: bool },
    Progress { done: usize, total: usize },
    Finished { success: bool, message: String },
}

/// Status reported back to the main window's status line.
pub struct MainStatus {
    pub text: String,
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Generation engine
// ---------------------------------------------------------------------------

fn merged_timestamp() -> String {
    Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

fn safe_merged_name(name: &str) -> String {
    let base = if name.trim().is_empty() {
        format!("Merged_{}", merged_timestamp())
    } else {
        name.to_string()
    };
    let sanitized = filenamer::sanitize(&base);
    if sanitized.is_empty() {
        format!("Merged_{}", merged_timestamp())
    } else {
        sanitized
    }
}

/// Run a full bulk generation. Progress and completion are reported via `tx`.
pub fn run_generation(
    template: &Path,
    excel: &Path,
    output_dir: &Path,
    placeholders: &[String],
    filename_parts: &[String],
    merge_all: bool,
    merged_filename: &str,
    tx: &Sender<BulkMsg>,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    // Validate first (mirrors the Java preview step).
    let validation = xlsx::validate(excel, placeholders)?;
    let preview_msg = if validation.warnings.is_empty() {
        "All headers match template.".to_string()
    } else {
        validation.warnings.join("\n")
    };
    let _ = tx.send(BulkMsg::Preview {
        documents: validation.documents_found,
        message: preview_msg,
        mismatch: validation.has_mismatch,
    });

    let (headers, rows) = xlsx::read_sheet(excel)?;
    let data_rows: Vec<&Vec<String>> = rows.iter().filter(|r| xlsx::is_data_row(r)).collect();
    let total = data_rows.len();
    if total == 0 {
        return Err(anyhow!("No data rows found in the Excel sheet."));
    }

    if merge_all {
        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(total);
        for (done, row) in data_rows.iter().enumerate() {
            let values = xlsx::row_values(&headers, row);
            blocks.push(docx::fill_template_bytes(template, &values)?);
            let _ = tx.send(BulkMsg::Progress { done: done + 1, total });
        }
        let merged = docx_merge::merge_docx(&blocks)?;
        let out_name = format!("{}.docx", safe_merged_name(merged_filename));
        std::fs::write(output_dir.join(out_name), merged)?;
    } else {
        let mut used: HashSet<String> = HashSet::new();
        for (done, row) in data_rows.iter().enumerate() {
            let values = xlsx::row_values(&headers, row);
            let mut base = filenamer::build_name_from_parts(filename_parts, &values);
            base = filenamer::sanitize(&base);
            let base = if base.is_empty() { "Document".to_string() } else { base };
            let unique = filenamer::ensure_unique(&base, &mut used, 3);
            let out = output_dir.join(format!("{unique}.docx"));
            docx::write_filled(template, &values, &out)?;
            let _ = tx.send(BulkMsg::Progress { done: done + 1, total });
        }
    }

    let _ = tx.send(BulkMsg::Finished {
        success: true,
        message: format!("Bulk generation complete — {total} documents"),
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// UI window
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct BulkWindow {
    pub open: bool,
    template: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    placeholders: Vec<String>,

    excel_file: Option<PathBuf>,
    selected_file_label: String,
    preview_line1: String,
    preview_line2: String,
    preview_danger: bool,

    selected_parts: Vec<String>,
    merge_all: bool,
    merged_filename: String,

    generating: bool,
    progress: f32,
    progress_label: String,
    rx: Option<Receiver<BulkMsg>>,

    main_status: Option<MainStatus>,
}

impl BulkWindow {
    /// Open the window for a given template/output folder/placeholder set.
    pub fn open_with(&mut self, template: PathBuf, output_dir: PathBuf, placeholders: Vec<String>) {
        let mut sorted = placeholders;
        sorted.sort();
        *self = BulkWindow {
            open: true,
            template: Some(template),
            output_dir: Some(output_dir),
            placeholders: sorted,
            selected_file_label: "No file selected".to_string(),
            preview_line1: "Documents found: 0".to_string(),
            preview_line2: "Upload an Excel file to begin.".to_string(),
            merged_filename: format!("Merged_{}", merged_timestamp()),
            ..Default::default()
        };
    }

    /// Drain worker messages; returns a status update for the main window.
    pub fn take_main_status(&mut self) -> Option<MainStatus> {
        self.main_status.take()
    }

    fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    BulkMsg::Preview { documents, message, mismatch } => {
                        self.preview_line1 = format!("Documents found: {documents}");
                        self.preview_line2 = message;
                        self.preview_danger = mismatch;
                    }
                    BulkMsg::Progress { done, total } => {
                        self.progress = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        self.progress_label = format!("Progress: {done} / {total}");
                    }
                    BulkMsg::Finished { success, message } => {
                        self.preview_line2 = message.clone();
                        self.preview_danger = !success;
                        self.generating = false;
                        finished = true;
                        self.main_status = Some(MainStatus {
                            text: if success {
                                format!("Bulk Status: Ready — {}", self.preview_line1.trim_start_matches("Documents found: "))
                            } else {
                                "Bulk Status: Error".to_string()
                            },
                            success,
                        });
                    }
                }
            }
        }
        if finished {
            self.rx = None;
        }
    }

    fn choose_excel(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Excel Workbook", &["xlsx"])
            .set_title("Select an Excel file (.xlsx)")
            .pick_file()
        {
            self.load_excel(path);
        }
    }

    /// Load an Excel file dropped onto the window (used by the main app's
    /// drag-and-drop handler). No-op if the window isn't open.
    pub fn load_dropped_excel(&mut self, path: PathBuf) {
        if self.open {
            self.load_excel(path);
        }
    }

    fn load_excel(&mut self, path: PathBuf) {
        let is_xlsx = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("xlsx"))
            .unwrap_or(false);
        if !is_xlsx {
            self.preview_line2 = "Please choose an .xlsx Excel file.".to_string();
            self.preview_danger = true;
            return;
        }
        self.selected_file_label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.excel_file = Some(path);
        self.selected_parts.clear();
        self.preview_line1 = "Documents found: (validate on Generate)".to_string();
        self.preview_line2 = "Ready to generate.".to_string();
        self.preview_danger = false;
    }

    fn start_generation(&mut self) {
        let (Some(template), Some(output_dir), Some(excel)) =
            (self.template.clone(), self.output_dir.clone(), self.excel_file.clone())
        else {
            return;
        };
        let placeholders = self.placeholders.clone();
        let parts = self.selected_parts.clone();
        let merge_all = self.merge_all;
        let merged_filename = self.merged_filename.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.generating = true;
        self.progress = 0.0;
        self.progress_label = "Progress: 0 / 0".to_string();

        thread::spawn(move || {
            let result = run_generation(
                &template,
                &excel,
                &output_dir,
                &placeholders,
                &parts,
                merge_all,
                &merged_filename,
                &tx,
            );
            if let Err(e) = result {
                let _ = tx.send(BulkMsg::Finished {
                    success: false,
                    message: format!("Error: {e}"),
                });
            }
        });
    }

    fn preview_filename(&self) -> String {
        if self.selected_parts.is_empty() {
            "Preview: (default naming)".to_string()
        } else {
            format!("Preview: {}.docx", self.selected_parts.join(""))
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        self.poll();
        if self.generating {
            ctx.request_repaint();
        }

        let mut keep_open = self.open;
        egui::Window::new("LetterFactory — Bulk Mode")
            .open(&mut keep_open)
            .resizable(true)
            .default_width(640.0)
            .default_height(620.0)
            .frame(theme::window_frame())
            .show(ctx, |ui| {
                self.window_body(ui);
            });
        self.open = keep_open;
    }

    fn window_body(&mut self, ui: &mut egui::Ui) {
        // Drop zone / upload
        theme::glass_group(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.label(theme::title_text("Upload your Excel (.xlsx)"));
                ui.add_space(8.0);
                if theme::accent_button(ui, "Upload Excel").clicked() {
                    self.choose_excel();
                }
                ui.add_space(4.0);
                ui.label(theme::muted_text("or drag and drop a .xlsx file here"));
                ui.add_space(4.0);
                ui.label(theme::muted_text(&self.selected_file_label));
                ui.add_space(4.0);
            });
        });

        ui.add_space(8.0);

        // Preview
        theme::glass_group(ui, |ui| {
            ui.label(theme::body_text(&self.preview_line1));
            let color = if self.preview_danger { theme::DANGER } else { theme::SUCCESS };
            for line in self.preview_line2.lines() {
                ui.colored_label(color, line);
            }
        });

        ui.add_space(10.0);
        ui.label(theme::section_title("File Naming"));
        ui.label(theme::muted_text("Select placeholders to build each filename."));
        ui.add_space(4.0);

        // Placeholder checklist
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let placeholders = self.placeholders.clone();
                for p in &placeholders {
                    let mut checked = self.selected_parts.contains(p);
                    if ui.checkbox(&mut checked, p.as_str()).changed() {
                        if checked {
                            if !self.selected_parts.contains(p) {
                                self.selected_parts.push(p.clone());
                            }
                        } else {
                            self.selected_parts.retain(|x| x != p);
                        }
                    }
                }
            });

        ui.add_space(4.0);
        ui.colored_label(theme::ACCENT, self.preview_filename());

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        ui.checkbox(&mut self.merge_all, "Merge all into one file");
        ui.label(theme::muted_text("Combines all documents into a single .docx"));
        ui.add_space(4.0);
        ui.add_enabled_ui(self.merge_all, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::body_text("Merged Filename"));
                ui.add(egui::TextEdit::singleline(&mut self.merged_filename).desired_width(280.0));
            });
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let can_generate = self.excel_file.is_some() && !self.generating;
            ui.add_enabled_ui(can_generate, |ui| {
                if theme::accent_button(ui, "Generate All").clicked() {
                    self.start_generation();
                }
            });
            if ui.button("Close").clicked() {
                self.open = false;
            }
        });

        if self.generating || self.progress > 0.0 {
            ui.add_space(8.0);
            ui.label(theme::muted_text(&self.progress_label));
            ui.add(egui::ProgressBar::new(self.progress).fill(theme::ACCENT).animate(self.generating));
        }
    }
}
