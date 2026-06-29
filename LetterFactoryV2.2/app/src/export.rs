//! Single-document export and best-effort printing.
//! Port of the Java `ExportService` (DOCX path) and `PrintService`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;

use crate::docx;

fn timestamp() -> String {
    Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

fn strip_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) if idx > 0 => &name[..idx],
        _ => name,
    }
}

/// Export a single filled `.docx` into `output_dir`, returning the written path.
pub fn export_single(
    template: &Path,
    replacements: &HashMap<String, String>,
    output_dir: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("creating {}", output_dir.display()))?;

    let base = template
        .file_name()
        .and_then(|s| s.to_str())
        .map(strip_ext)
        .unwrap_or("Document");
    let file_name = format!("{base}_{}.docx", timestamp());
    let out_path = output_dir.join(file_name);

    docx::write_filled(template, replacements, &out_path)?;
    Ok(out_path)
}

/// Render the filled document to a temp file and hand it to the OS's default
/// handler so the user can print it. Best-effort and platform dependent
/// (needs an installed `.docx` handler such as Word).
pub fn print_single(template: &Path, replacements: &HashMap<String, String>) -> Result<()> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("letterfactory-print-{}.docx", timestamp()));
    docx::write_filled(template, replacements, &tmp)?;
    open_for_print(&tmp)
}

#[cfg(target_os = "windows")]
fn open_for_print(path: &Path) -> Result<()> {
    use std::process::Command;
    // Use the shell "Print" verb (handled by Word / the default .docx app).
    let arg = format!(
        "Start-Process -FilePath \"{}\" -Verb Print",
        path.display()
    );
    Command::new("powershell")
        .args(["-NoProfile", "-Command", &arg])
        .spawn()
        .context("failed to invoke the Windows print handler")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_for_print(path: &Path) -> Result<()> {
    use std::process::Command;
    // No headless .docx printing on macOS; open in the default app to print.
    Command::new("open")
        .arg(path)
        .spawn()
        .context("failed to open the document for printing")?;
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn open_for_print(path: &Path) -> Result<()> {
    use std::process::Command;
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context("failed to open the document for printing")?;
    Ok(())
}
