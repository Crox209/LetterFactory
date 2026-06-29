//! Excel (.xlsx) reading for Bulk Mode. Port of the spreadsheet half of the
//! Java `BulkModeEngine`.
//!
//! Like the Java version (which used Apache POI's `DataFormatter`), we render
//! each cell exactly as Excel *displays* it, applying the cell's number-format
//! code. `calamine` only exposes raw stored values (so `5,000` came back as
//! `5000`), so instead we parse the `.xlsx` package (a ZIP of XML) directly and
//! apply Excel format codes with `ssfmt`. This means a comma only ever appears
//! when the cell's own format contains one — plain/General numbers stay plain.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::docx::{resolve_ref, text_to_string};

/// Header validation outcome against the template's placeholders.
pub struct Validation {
    pub documents_found: usize,
    pub warnings: Vec<String>,
    pub has_mismatch: bool,
}

/// Read the first sheet: returns (header cells as displayed, data rows as
/// displayed strings aligned to the header columns).
pub fn read_sheet(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let map = read_zip_map(path)?;

    let shared = match map.get("xl/sharedStrings.xml") {
        Some(bytes) => parse_shared_strings(bytes)?,
        None => Vec::new(),
    };
    let styles = match map.get("xl/styles.xml") {
        Some(bytes) => parse_styles(bytes)?,
        None => Styles::default(),
    };

    let sheet_path = first_sheet_path(&map)?;
    let sheet_xml = map
        .get(&sheet_path)
        .ok_or_else(|| anyhow!("worksheet part not found: {sheet_path}"))?;

    let mut rows = parse_worksheet(sheet_xml, &shared, &styles)?;
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let headers = rows.remove(0);
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

// ---------------------------------------------------------------------------
// ZIP + part lookup
// ---------------------------------------------------------------------------

/// Read all (non-directory) entries of the `.xlsx` zip into a name -> bytes map.
fn read_zip_map(path: &Path) -> Result<HashMap<String, Vec<u8>>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("not a valid .xlsx (zip) file")?;
    let mut map = HashMap::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        if zf.is_dir() {
            continue;
        }
        let name = zf.name().to_string();
        let mut buf = Vec::with_capacity(zf.size() as usize);
        zf.read_to_end(&mut buf)?;
        map.insert(name, buf);
    }
    Ok(map)
}

/// Resolve the package path of the workbook's first sheet (in tab order),
/// falling back to `sheet1.xml` / the first worksheet part if needed.
fn first_sheet_path(map: &HashMap<String, Vec<u8>>) -> Result<String> {
    if let Some(wb) = map.get("xl/workbook.xml") {
        if let Some(rid) = first_sheet_rid(wb)? {
            if let Some(rels) = map.get("xl/_rels/workbook.xml.rels") {
                if let Some(target) = rel_target(rels, &rid)? {
                    let full = normalize_target(&target);
                    if map.contains_key(&full) {
                        return Ok(full);
                    }
                }
            }
        }
    }

    if map.contains_key("xl/worksheets/sheet1.xml") {
        return Ok("xl/worksheets/sheet1.xml".to_string());
    }
    let mut sheets: Vec<&String> = map
        .keys()
        .filter(|k| k.starts_with("xl/worksheets/") && k.ends_with(".xml"))
        .collect();
    sheets.sort();
    sheets
        .first()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("The workbook has no sheets."))
}

/// First `<sheet>`'s relationship id (the `r:id` attribute) in `workbook.xml`.
fn first_sheet_rid(xml: &[u8]) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"sheet" => {
                for a in e.attributes().with_checks(false).flatten() {
                    let key = a.key.as_ref();
                    if key == b"r:id" || key.ends_with(b":id") || key == b"id" {
                        return Ok(Some(String::from_utf8_lossy(a.value.as_ref()).into_owned()));
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(None)
}

/// Target of the `<Relationship>` with the given id in a `.rels` part.
fn rel_target(xml: &[u8], rid: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                if attr_val(&e, b"Id").as_deref() == Some(rid) {
                    return Ok(attr_val(&e, b"Target"));
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(None)
}

/// Turn a workbook-relative relationship target into a full package path.
fn normalize_target(target: &str) -> String {
    let t = target.trim();
    if let Some(stripped) = t.strip_prefix('/') {
        stripped.to_string()
    } else if t.starts_with("xl/") {
        t.to_string()
    } else {
        format!("xl/{t}")
    }
}

// ---------------------------------------------------------------------------
// Shared strings
// ---------------------------------------------------------------------------

/// Parse `sharedStrings.xml` into the shared string table (rich-text runs within
/// a single `<si>` are concatenated, mirroring how Excel displays them).
fn parse_shared_strings(xml: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut in_si = false;
    let mut in_t = false;
    let mut current = String::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => match e.local_name().as_ref() {
                b"si" => {
                    in_si = true;
                    current.clear();
                }
                b"t" if in_si => in_t = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                b"si" => {
                    in_si = false;
                    out.push(std::mem::take(&mut current));
                }
                b"t" if in_si => in_t = false,
                _ => {}
            },
            Event::Text(t) if in_t => current.push_str(&text_to_string(&t)),
            Event::GeneralRef(r) if in_t => current.push_str(&resolve_ref(&r)),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Styles (number formats)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Styles {
    /// numFmtId -> format code (custom formats and any explicit overrides).
    num_fmts: HashMap<u32, String>,
    /// cell format records (`<cellXfs>`): index -> numFmtId.
    cell_xfs: Vec<u32>,
}

fn parse_styles(xml: &[u8]) -> Result<Styles> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut styles = Styles::default();
    let mut in_cell_xfs = false;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => match e.local_name().as_ref() {
                b"cellXfs" => in_cell_xfs = true,
                b"numFmt" => add_num_fmt(&e, &mut styles.num_fmts),
                b"xf" if in_cell_xfs => styles.cell_xfs.push(xf_num_fmt_id(&e)),
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"numFmt" => add_num_fmt(&e, &mut styles.num_fmts),
                b"xf" if in_cell_xfs => styles.cell_xfs.push(xf_num_fmt_id(&e)),
                _ => {}
            },
            Event::End(e) => {
                if e.local_name().as_ref() == b"cellXfs" {
                    in_cell_xfs = false;
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(styles)
}

fn add_num_fmt(e: &BytesStart, map: &mut HashMap<u32, String>) {
    let id = attr_val(e, b"numFmtId").and_then(|v| v.parse::<u32>().ok());
    let code = attr_val(e, b"formatCode").map(|c| {
        quick_xml::escape::unescape(&c)
            .map(|s| s.into_owned())
            .unwrap_or(c)
    });
    if let (Some(id), Some(code)) = (id, code) {
        map.insert(id, code);
    }
}

fn xf_num_fmt_id(e: &BytesStart) -> u32 {
    attr_val(e, b"numFmtId")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Worksheet cells
// ---------------------------------------------------------------------------

/// Parse a worksheet part into rectangular rows of display strings.
fn parse_worksheet(xml: &[u8], shared: &[String], styles: &Styles) -> Result<Vec<Vec<String>>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut rows: Vec<Vec<(usize, String)>> = Vec::new();
    let mut cur_row: Vec<(usize, String)> = Vec::new();
    let mut auto_col = 0usize;
    let mut in_sheet_data = false;

    // Current cell state.
    let mut in_cell = false;
    let mut cur_col = 0usize;
    let mut cur_type: Option<String> = None;
    let mut cur_style: Option<usize> = None;
    let mut in_v = false;
    let mut v_text = String::new();
    let mut in_is_t = false;
    let mut is_text = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => match e.local_name().as_ref() {
                b"sheetData" => in_sheet_data = true,
                b"row" if in_sheet_data => {
                    cur_row = Vec::new();
                    auto_col = 0;
                }
                b"c" if in_sheet_data => {
                    in_cell = true;
                    cur_type = attr_val(&e, b"t");
                    cur_style = attr_val(&e, b"s").and_then(|s| s.parse::<usize>().ok());
                    cur_col = cell_col(attr_val(&e, b"r").as_deref(), auto_col);
                    auto_col = cur_col + 1;
                    v_text.clear();
                    is_text.clear();
                }
                b"v" if in_cell => {
                    in_v = true;
                    v_text.clear();
                }
                b"t" if in_cell => in_is_t = true,
                _ => {}
            },
            Event::Empty(e) => {
                if in_sheet_data && e.local_name().as_ref() == b"c" {
                    let col = cell_col(attr_val(&e, b"r").as_deref(), auto_col);
                    auto_col = col + 1;
                }
            }
            Event::Text(t) => {
                if in_v {
                    v_text.push_str(&text_to_string(&t));
                } else if in_is_t {
                    is_text.push_str(&text_to_string(&t));
                }
            }
            Event::GeneralRef(r) => {
                if in_v {
                    v_text.push_str(&resolve_ref(&r));
                } else if in_is_t {
                    is_text.push_str(&resolve_ref(&r));
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"v" => in_v = false,
                b"t" if in_cell => in_is_t = false,
                b"c" => {
                    if in_cell {
                        let rendered = render_cell(
                            cur_type.as_deref(),
                            cur_style,
                            &v_text,
                            &is_text,
                            shared,
                            styles,
                        );
                        cur_row.push((cur_col, rendered));
                    }
                    in_cell = false;
                    cur_type = None;
                    cur_style = None;
                }
                b"row" => rows.push(std::mem::take(&mut cur_row)),
                b"sheetData" => in_sheet_data = false,
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }

    // Make rectangular: width = (max column index seen) + 1.
    let max_col = rows
        .iter()
        .flat_map(|r| r.iter().map(|(c, _)| *c))
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let mut v = vec![String::new(); max_col];
        for (c, val) in r {
            if c < v.len() {
                v[c] = val;
            }
        }
        out.push(v);
    }
    Ok(out)
}

/// Render one cell to the string Excel would display.
fn render_cell(
    t: Option<&str>,
    style: Option<usize>,
    v: &str,
    inline: &str,
    shared: &[String],
    styles: &Styles,
) -> String {
    match t {
        // Shared string: `<v>` is an index into the shared string table.
        Some("s") => v
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|i| shared.get(i))
            .cloned()
            .unwrap_or_default(),
        // Inline string.
        Some("inlineStr") => inline.to_string(),
        // Formula result that is a string.
        Some("str") => v.to_string(),
        // Boolean: mirror POI's DataFormatter (TRUE / FALSE).
        Some("b") => {
            if v.trim() == "1" {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        // Error literal (e.g. #DIV/0!).
        Some("e") => v.to_string(),
        // Numeric (explicit "n", or no type attribute).
        Some("n") | None => {
            let raw = v.trim();
            if raw.is_empty() {
                return String::new();
            }
            match raw.parse::<f64>() {
                Ok(num) => render_number(num, style, styles),
                Err(_) => v.to_string(),
            }
        }
        Some(_) => v.to_string(),
    }
}

/// Apply a numeric cell's number format to produce its displayed string.
fn render_number(value: f64, style: Option<usize>, styles: &Styles) -> String {
    let num_fmt_id = style
        .and_then(|s| styles.cell_xfs.get(s).copied())
        .unwrap_or(0);

    // General: emit a plain number so we never add a comma that Excel doesn't.
    if num_fmt_id == 0 {
        return plain_number(value);
    }

    // Custom format code (or an explicit override of a built-in id).
    if let Some(code) = styles.num_fmts.get(&num_fmt_id) {
        if code.trim().eq_ignore_ascii_case("general") || code.trim().is_empty() {
            return plain_number(value);
        }
        return ssfmt::format_default(value, code).unwrap_or_else(|_| plain_number(value));
    }

    // Built-in format id (thousands, currency, percent, dates, etc.).
    ssfmt::format_with_id_default(value, num_fmt_id).unwrap_or_else(|_| plain_number(value))
}

/// Format a number the way a spreadsheet's General format would: integers
/// without a decimal point, otherwise the shortest round-trippable form.
fn plain_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

// ---------------------------------------------------------------------------
// Small XML helpers
// ---------------------------------------------------------------------------

/// Value of an attribute by (raw) key name, if present.
fn attr_val(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(a.value.as_ref()).into_owned())
}

/// Column index (0-based) from a cell reference like `B3`. Falls back to
/// `auto_col` when the cell has no `r` attribute.
fn cell_col(r: Option<&str>, auto_col: usize) -> usize {
    if let Some(s) = r {
        let mut col = 0usize;
        let mut any = false;
        for ch in s.chars() {
            if ch.is_ascii_alphabetic() {
                any = true;
                col = col * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
            } else {
                break;
            }
        }
        if any {
            return col - 1;
        }
    }
    auto_col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_from_reference() {
        assert_eq!(cell_col(Some("A1"), 0), 0);
        assert_eq!(cell_col(Some("B3"), 0), 1);
        assert_eq!(cell_col(Some("Z9"), 0), 25);
        assert_eq!(cell_col(Some("AA1"), 0), 26);
        assert_eq!(cell_col(Some("AB12"), 0), 27);
        assert_eq!(cell_col(None, 5), 5);
    }

    #[test]
    fn general_numbers_stay_plain() {
        let styles = Styles::default();
        // No style -> General -> no comma is ever introduced.
        assert_eq!(render_number(5000.0, None, &styles), "5000");
        assert_eq!(render_number(5000.0, Some(0), &styles), "5000");
        assert_eq!(render_number(1234.5, None, &styles), "1234.5");
    }

    #[test]
    fn custom_thousands_format_keeps_comma() {
        let mut styles = Styles::default();
        styles.num_fmts.insert(164, "#,##0".to_string());
        styles.cell_xfs = vec![0, 164_u32.try_into().unwrap()];
        // Style index 1 -> numFmtId 164 -> "#,##0".
        assert_eq!(render_number(5000.0, Some(1), &styles), "5,000");
    }

    #[test]
    fn shared_string_lookup() {
        let shared = vec!["Hello".to_string(), "5,000".to_string()];
        let styles = Styles::default();
        // A text cell that literally contains "5,000" is preserved verbatim.
        assert_eq!(render_cell(Some("s"), None, "1", "", &shared, &styles), "5,000");
    }

    #[test]
    fn parse_shared_strings_concats_runs() {
        let xml = br#"<sst><si><t>Plain</t></si><si><r><t>Rich </t></r><r><t>Text</t></r></si></sst>"#;
        let out = parse_shared_strings(xml).unwrap();
        assert_eq!(out, vec!["Plain".to_string(), "Rich Text".to_string()]);
    }

    #[test]
    fn end_to_end_read_sheet_preserves_excel_display() {
        // A real .xlsx package: header [[Amount]]/[[Label]], then three rows that
        // exercise (a) a #,##0-formatted 5000, (b) a plain General 5000, and
        // (c) a literal text "5,000".
        let content_types = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"##.to_vec();
        let workbook = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"##.to_vec();
        let wb_rels = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"##.to_vec();
        let styles = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0"/></numFmts><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="164"/></cellXfs></styleSheet>"##.to_vec();
        let shared = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="6" uniqueCount="6"><si><t>[[Amount]]</t></si><si><t>[[Label]]</t></si><si><t>Formatted</t></si><si><t>Plain</t></si><si><t>5,000</t></si><si><t>Text</t></si></sst>"##.to_vec();
        let sheet = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" s="1"><v>5000</v></c><c r="B2" t="s"><v>2</v></c></row><row r="3"><c r="A3"><v>5000</v></c><c r="B3" t="s"><v>3</v></c></row><row r="4"><c r="A4" t="s"><v>4</v></c><c r="B4" t="s"><v>5</v></c></row></sheetData></worksheet>"##.to_vec();

        let entries = vec![
            ("[Content_Types].xml".to_string(), content_types),
            ("xl/workbook.xml".to_string(), workbook),
            ("xl/_rels/workbook.xml.rels".to_string(), wb_rels),
            ("xl/styles.xml".to_string(), styles),
            ("xl/sharedStrings.xml".to_string(), shared),
            ("xl/worksheets/sheet1.xml".to_string(), sheet),
        ];
        let bytes = crate::docx::rebuild_zip(&entries).unwrap();
        let tmp = std::env::temp_dir().join("lf_test_workbook.xlsx");
        std::fs::write(&tmp, &bytes).unwrap();

        let (headers, rows) = read_sheet(&tmp).unwrap();
        assert_eq!(headers, vec!["[[Amount]]".to_string(), "[[Label]]".to_string()]);
        assert_eq!(rows.len(), 3);
        // #,##0-formatted 5000 keeps its comma...
        assert_eq!(rows[0][0], "5,000");
        // ...a plain/General 5000 must NOT gain a comma...
        assert_eq!(rows[1][0], "5000");
        // ...and literal text "5,000" is passed through verbatim.
        assert_eq!(rows[2][0], "5,000");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn parse_styles_reads_cell_xfs_and_numfmts() {
        let xml = br##"<styleSheet>
            <numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0"/></numFmts>
            <cellStyleXfs count="1"><xf numFmtId="0"/></cellStyleXfs>
            <cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="164"/></cellXfs>
        </styleSheet>"##;
        let s = parse_styles(xml).unwrap();
        assert_eq!(s.cell_xfs, vec![0, 164]);
        assert_eq!(s.num_fmts.get(&164).map(String::as_str), Some("#,##0"));
    }
}
