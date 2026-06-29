//! Excel (.xlsx) reading for Bulk Mode. Port of the spreadsheet half of the
//! Java `BulkModeEngine`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook_auto, Data, Reader};

/// Header validation outcome against the template's placeholders.
pub struct Validation {
    pub documents_found: usize,
    pub warnings: Vec<String>,
    pub has_mismatch: bool,
}

/// Read the first sheet: returns (header cells as displayed, data rows as
/// displayed strings aligned to the header columns).
pub fn read_sheet(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut wb = open_workbook_auto(path).with_context(|| format!("opening {}", path.display()))?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| anyhow!("The workbook has no sheets."))?
        .context("could not read the first sheet")?;

    let mut rows_iter = range.rows();
    let header_row = match rows_iter.next() {
        Some(r) => r,
        None => return Ok((Vec::new(), Vec::new())),
    };
    let headers: Vec<String> = header_row.iter().map(cell_to_string).collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    for r in rows_iter {
        rows.push(r.iter().map(cell_to_string).collect());
    }
    Ok((headers, rows))
}

/// Validate the Excel headers against the template placeholders.
pub fn validate(path: &Path, template_placeholders: &[String]) -> Result<Validation> {
    let (headers, rows) = read_sheet(path)?;
    let mut warnings = Vec::new();
    let mut mismatch = false;

    if headers.is_empty() {
        warnings.push("Missing header row (Row 1).".to_string());
        return Ok(Validation { documents_found: 0, warnings, has_mismatch: true });
    }

    // Headers present in the sheet (non-empty), de-duplicated, order-preserving.
    let mut excel_headers: Vec<String> = Vec::new();
    for h in &headers {
        let v = h.trim().to_string();
        if !v.is_empty() && !excel_headers.contains(&v) {
            excel_headers.push(v);
        }
    }

    for h in &excel_headers {
        if let Some(inner) = unwrap(h) {
            if !template_placeholders.iter().any(|p| p == &inner) {
                warnings.push(format!("Warning: {h} not found in template."));
                mismatch = true;
            }
        }
    }
    for p in template_placeholders {
        let wrapped = format!("[[{p}]]");
        if !excel_headers.iter().any(|h| h == &wrapped) {
            warnings.push(format!("Warning: template placeholder missing in Excel: {wrapped}"));
            mismatch = true;
        }
    }

    let docs = rows.iter().filter(|r| is_data_row(r)).count();
    Ok(Validation { documents_found: docs, warnings, has_mismatch: mismatch })
}

/// Map a data row to placeholder inner-name -> value using the header brackets.
pub fn row_values(headers: &[String], row: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        if let Some(inner) = unwrap(h) {
            let v = row.get(i).cloned().unwrap_or_default();
            out.insert(inner, v);
        }
    }
    out
}

/// True if any cell in the row has content.
pub fn is_data_row(row: &[String]) -> bool {
    row.iter().any(|c| !c.trim().is_empty())
}

/// `[[Name]]` -> `Some("Name")`; anything else -> `None`.
fn unwrap(header_cell: &str) -> Option<String> {
    let s = header_cell.trim();
    if s.starts_with("[[") && s.ends_with("]]") && s.len() >= 4 {
        let inner = s[2..s.len() - 2].trim();
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    } else {
        None
    }
}

fn cell_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => format_number(*f),
        Data::DateTime(dt) => match dt.as_datetime() {
            Some(ndt) => {
                if ndt.time() == chrono::NaiveTime::MIN {
                    ndt.format("%Y-%m-%d").to_string()
                } else {
                    ndt.format("%Y-%m-%d %H:%M:%S").to_string()
                }
            }
            None => format_number(dt.as_f64()),
        },
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
    }
}

/// Format a float like a spreadsheet would: integers without a decimal point,
/// otherwise trimmed of trailing zeros.
fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        let s = format!("{f}");
        s
    }
}
