//! DOCX reading and `[[placeholder]]` filling.
//!
//! Rust port of the Java `UniversalDocxProcessor` / `DocxPlaceholderReplacer` /
//! `Docx4jPlaceholderReplacer`. A `.docx` is a ZIP of XML parts; we operate at
//! the XML level over every `<w:p>` paragraph in the relevant parts (document
//! body, headers, footers, foot/endnotes). Because the scan is paragraph-based,
//! it inherently covers tables, content controls (SDTs) and text boxes (which
//! are just nested `<w:p>` elements), and it is split-run safe: tokens that are
//! broken across several `<w:r>`/`<w:t>` runs are reassembled before matching.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use zip::write::SimpleFileOptions;

use crate::placeholder;

/// Decode an XML text event to a plain `String` (charset-decode + entity unescape).
pub(crate) fn text_to_string(t: &BytesText) -> String {
    let decoded = t.decode().unwrap_or_default();
    match quick_xml::escape::unescape(decoded.as_ref()) {
        Ok(s) => s.into_owned(),
        Err(_) => decoded.into_owned(),
    }
}

/// Detect every `[[placeholder]]` inner name in a template, in first-seen order.
pub fn find_placeholders(template: &Path) -> Result<Vec<String>> {
    let entries = load_entries(template)?;
    let mut out: Vec<String> = Vec::new();
    for (name, bytes) in &entries {
        if !is_target_part(name) {
            continue;
        }
        for text in paragraph_texts(bytes)? {
            for ph in placeholder::scan_text(&text) {
                if !out.iter().any(|e| e == &ph) {
                    out.push(ph);
                }
            }
        }
    }
    Ok(out)
}

/// Fill the template with `replacements` (inner name -> value) and return the
/// finished `.docx` bytes.
pub fn fill_template_bytes(template: &Path, replacements: &HashMap<String, String>) -> Result<Vec<u8>> {
    let mut entries = load_entries(template)?;

    // Pre-build (token, value) pairs once: "[[Name]]" -> value.
    let repls: Vec<(String, String)> = replacements
        .iter()
        .map(|(k, v)| (placeholder::token(k), v.clone()))
        .collect();

    for (name, bytes) in entries.iter_mut() {
        if is_target_part(name) {
            *bytes = transform_fill(bytes, &repls)
                .with_context(|| format!("failed to process part {name}"))?;
        }
    }
    rebuild_zip(&entries)
}

/// Write a filled document to `out_path`.
pub fn write_filled(template: &Path, replacements: &HashMap<String, String>, out_path: &Path) -> Result<()> {
    let bytes = fill_template_bytes(template, replacements)?;
    std::fs::write(out_path, bytes).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ZIP helpers
// ---------------------------------------------------------------------------

/// XML parts that may carry visible `[[placeholders]]`.
fn is_target_part(name: &str) -> bool {
    name == "word/document.xml"
        || name == "word/footnotes.xml"
        || name == "word/endnotes.xml"
        || (name.starts_with("word/header") && name.ends_with(".xml"))
        || (name.starts_with("word/footer") && name.ends_with(".xml"))
}

/// Read all (non-directory) entries of a zip into memory as (name, bytes).
fn load_entries(path: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    read_archive(zip::ZipArchive::new(file).context("not a valid .docx (zip) file")?)
}

/// Read all (non-directory) entries of an in-memory zip.
pub(crate) fn load_entries_from_bytes(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).context("not a valid .docx (zip) file")?;
    read_archive(archive)
}

fn read_archive<R: Read + std::io::Seek>(mut archive: zip::ZipArchive<R>) -> Result<Vec<(String, Vec<u8>)>> {
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        if zf.is_dir() {
            continue;
        }
        let name = zf.name().to_string();
        let mut buf = Vec::with_capacity(zf.size() as usize);
        zf.read_to_end(&mut buf)?;
        entries.push((name, buf));
    }
    Ok(entries)
}

/// Rebuild a zip (deflate) from in-memory entries.
pub(crate) fn rebuild_zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(name.as_str(), options)?;
        writer.write_all(bytes)?;
    }
    let cursor = writer.finish()?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// XML scanning (read-only): combined text per paragraph
// ---------------------------------------------------------------------------

/// Return the combined visible text of every `<w:p>` paragraph in an XML part.
/// Each paragraph's text is assembled from its own (nearest-enclosing) `<w:t>`
/// and `<w:delText>` runs, so split-run tokens become whole.
fn paragraph_texts(xml: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut out = Vec::new();
    // Stack of per-paragraph text buffers (handles nested paragraphs in textboxes).
    let mut para_stack: Vec<String> = Vec::new();
    // Element-name stack to know the immediate parent of a text node.
    let mut elem_stack: Vec<Vec<u8>> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => {
                let name = e.name().as_ref().to_vec();
                if name == b"w:p" {
                    para_stack.push(String::new());
                }
                elem_stack.push(name);
            }
            Event::End(e) => {
                let name = e.name().as_ref().to_vec();
                if name == b"w:p" {
                    if let Some(text) = para_stack.pop() {
                        out.push(text);
                    }
                }
                elem_stack.pop();
            }
            Event::Text(t) => {
                if is_in_text_run(&elem_stack) {
                    if let Some(top) = para_stack.last_mut() {
                        top.push_str(&text_to_string(&t));
                    }
                }
            }
            Event::GeneralRef(r) => {
                if is_in_text_run(&elem_stack) {
                    if let Some(top) = para_stack.last_mut() {
                        top.push_str(&resolve_ref(&r));
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn is_in_text_run(elem_stack: &[Vec<u8>]) -> bool {
    matches!(elem_stack.last().map(|n| n.as_slice()), Some(b"w:t") | Some(b"w:delText"))
}

/// Resolve a general/character entity reference (the part between `&` and `;`).
pub(crate) fn resolve_ref(r: &quick_xml::events::BytesRef) -> String {
    let name = r.decode().unwrap_or_default();
    if let Some(rest) = name.strip_prefix('#') {
        let cp = if let Some(hex) = rest.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()
        } else {
            rest.parse::<u32>().ok()
        };
        return cp.and_then(char::from_u32).map(|c| c.to_string()).unwrap_or_default();
    }
    quick_xml::escape::resolve_predefined_entity(&name)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// XML transform (fill): rewrite text nodes with replacements applied
// ---------------------------------------------------------------------------

/// One `<w:t>`/`<w:delText>` text run, with its event range and combined value.
struct Run {
    start: usize, // index of the Start (or Empty) event
    end: usize,   // index of the matching End event (== start for Empty)
    is_empty: bool,
    value: String,
    para: usize,
}

fn transform_fill(xml: &[u8], repls: &[(String, String)]) -> Result<Vec<u8>> {
    // 1. Read every event into an owned buffer so we can index/modify it.
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut events: Vec<Event<'static>> = Vec::new();
    loop {
        let ev = reader.read_event_into(&mut buf)?;
        match ev {
            Event::Eof => break,
            other => events.push(other.into_owned()),
        }
        buf.clear();
    }

    // 2. Collect every text run (each `<w:t>` element) with its paragraph id.
    let mut runs: Vec<Run> = Vec::new();
    let mut para_stack: Vec<usize> = Vec::new();
    let mut next_para: usize = 0;
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(e) if e.name().as_ref() == b"w:p" => {
                next_para += 1;
                para_stack.push(next_para);
                i += 1;
            }
            Event::End(e) if e.name().as_ref() == b"w:p" => {
                para_stack.pop();
                i += 1;
            }
            Event::Start(e) if is_text_elem(e.name().as_ref()) => {
                let para = *para_stack.last().unwrap_or(&0);
                let mut value = String::new();
                let mut depth = 1;
                let mut j = i + 1;
                let mut end = i;
                while j < events.len() {
                    match &events[j] {
                        Event::Start(_) => depth += 1,
                        Event::End(_) => {
                            depth -= 1;
                            if depth == 0 {
                                end = j;
                                break;
                            }
                        }
                        Event::Text(t) => value.push_str(&text_to_string(t)),
                        Event::GeneralRef(r) => value.push_str(&resolve_ref(r)),
                        _ => {}
                    }
                    j += 1;
                }
                runs.push(Run { start: i, end, is_empty: false, value, para });
                i = end + 1;
            }
            Event::Empty(e) if is_text_elem(e.name().as_ref()) => {
                let para = *para_stack.last().unwrap_or(&0);
                runs.push(Run { start: i, end: i, is_empty: true, value: String::new(), para });
                i += 1;
            }
            _ => i += 1,
        }
    }

    // 3. Group runs by paragraph and apply split-run-safe replacement.
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for (ri, run) in runs.iter().enumerate() {
        match groups.iter_mut().find(|(g, _)| *g == run.para) {
            Some((_, v)) => v.push(ri),
            None => groups.push((run.para, vec![ri])),
        }
    }
    for (_, ris) in &groups {
        let mut vals: Vec<Vec<char>> = ris.iter().map(|&ri| runs[ri].value.chars().collect()).collect();
        if replace_in_paragraph(&mut vals, repls) {
            for (n, &ri) in ris.iter().enumerate() {
                runs[ri].value = vals[n].iter().collect();
            }
        }
    }

    // 4. Re-emit events, replacing each text run's inner content with a single
    //    escaped text node and forcing `xml:space="preserve"`.
    let mut run_at: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (ri, run) in runs.iter().enumerate() {
        run_at.insert(run.start, ri);
    }

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut k = 0;
    while k < events.len() {
        if let Some(&ri) = run_at.get(&k) {
            let run = &runs[ri];
            if run.is_empty {
                // Self-closing <w:t/>: keep as-is (never contains a token).
                writer.write_event(events[k].clone())?;
                k += 1;
            } else {
                if let Event::Start(e) = &events[run.start] {
                    writer.write_event(Event::Start(with_preserve(e)))?;
                }
                if !run.value.is_empty() {
                    let escaped = quick_xml::escape::escape(run.value.as_str()).into_owned();
                    writer.write_event(Event::Text(BytesText::from_escaped(escaped)))?;
                }
                if let Event::End(e) = &events[run.end] {
                    writer.write_event(Event::End(e.clone()))?;
                }
                k = run.end + 1;
            }
        } else {
            writer.write_event(events[k].clone())?;
            k += 1;
        }
    }
    Ok(writer.into_inner().into_inner())
}

fn is_text_elem(name: &[u8]) -> bool {
    name == b"w:t" || name == b"w:delText"
}

/// Copy a start tag, guaranteeing `xml:space="preserve"`.
fn with_preserve(e: &BytesStart) -> BytesStart<'static> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut ns = BytesStart::new(name);
    let mut wrote_space = false;
    for attr in e.attributes().with_checks(false).flatten() {
        if attr.key.as_ref() == b"xml:space" {
            ns.push_attribute(("xml:space", "preserve"));
            wrote_space = true;
        } else {
            let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
            let val = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
            ns.push_attribute((key.as_str(), val.as_str()));
        }
    }
    if !wrote_space {
        ns.push_attribute(("xml:space", "preserve"));
    }
    ns
}

// ---------------------------------------------------------------------------
// Split-run-safe replacement within one paragraph
// Port of Java `Docx4jPlaceholderReplacer.replaceInParagraph` / `replaceTokenRange`.
// ---------------------------------------------------------------------------

/// Apply all replacements to a paragraph's run text nodes. Returns true if any
/// node was changed.
fn replace_in_paragraph(nodes: &mut [Vec<char>], repls: &[(String, String)]) -> bool {
    let mut any = false;
    loop {
        let combined: Vec<char> = nodes.iter().flatten().copied().collect();
        if combined.is_empty() {
            break;
        }
        let mut changed = false;
        for (token, value) in repls {
            let tok: Vec<char> = token.chars().collect();
            if let Some(idx) = find_subslice(&combined, &tok) {
                replace_token_range(nodes, idx, idx + tok.len(), value);
                changed = true;
                any = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    any
}

fn find_subslice(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

/// Replace the half-open char range `[start, end)` of the combined text across
/// `nodes`, placing the replacement in the begin node and clearing the rest of
/// the matched span (preserving the begin run's formatting).
fn replace_token_range(nodes: &mut [Vec<char>], start: usize, end: usize, replacement: &str) {
    let mut char_pos = 0usize;
    let mut begin: Option<(usize, usize)> = None; // (node idx, char offset)
    let mut end_pos: Option<(usize, usize)> = None; // (node idx, inclusive char)

    for (i, node) in nodes.iter().enumerate() {
        let len = node.len();
        if begin.is_none() && start < char_pos + len {
            begin = Some((i, start - char_pos));
        }
        if begin.is_some() && end <= char_pos + len {
            end_pos = Some((i, end - char_pos - 1));
            break;
        }
        char_pos += len;
    }

    let (Some((bi, bc)), Some((ei, ec))) = (begin, end_pos) else {
        return;
    };
    let repl: Vec<char> = replacement.chars().collect();

    if bi == ei {
        let v = &nodes[bi];
        let head = &v[..bc.min(v.len())];
        let tail_from = (ec + 1).min(v.len());
        let tail = &v[tail_from..];
        let mut updated = Vec::with_capacity(head.len() + repl.len() + tail.len());
        updated.extend_from_slice(head);
        updated.extend_from_slice(&repl);
        updated.extend_from_slice(tail);
        nodes[bi] = updated;
        return;
    }

    let begin_prefix: Vec<char> = nodes[bi][..bc.min(nodes[bi].len())].to_vec();
    let end_suffix: Vec<char> = {
        let v = &nodes[ei];
        let from = (ec + 1).min(v.len());
        v[from..].to_vec()
    };
    let mut new_begin = begin_prefix;
    new_begin.extend_from_slice(&repl);
    new_begin.extend_from_slice(&end_suffix);
    nodes[bi] = new_begin;
    for node in nodes.iter_mut().take(ei + 1).skip(bi + 1) {
        node.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#;
    const TAIL: &str = r#"</w:body></w:document>"#;

    #[test]
    fn combines_split_runs() {
        let xml = format!(
            "{HEAD}<w:p><w:r><w:t>Hello [[Na</w:t></w:r><w:r><w:t>me]]!</w:t></w:r></w:p>{TAIL}"
        );
        let texts = paragraph_texts(xml.as_bytes()).unwrap();
        assert_eq!(texts[0], "Hello [[Name]]!");
        assert_eq!(placeholder::scan_text(&texts[0]), vec!["Name".to_string()]);
    }

    #[test]
    fn replaces_split_run_token() {
        let xml = format!(
            "{HEAD}<w:p><w:r><w:t>Hi [[Na</w:t></w:r><w:r><w:t>me]], welcome</w:t></w:r></w:p>{TAIL}"
        );
        let repls = vec![("[[Name]]".to_string(), "World".to_string())];
        let out = transform_fill(xml.as_bytes(), &repls).unwrap();
        let texts = paragraph_texts(&out).unwrap();
        assert_eq!(texts[0], "Hi World, welcome");
    }

    #[test]
    fn replaces_multiple_in_paragraph_and_preserves_other_runs() {
        let xml = format!(
            "{HEAD}<w:p><w:r><w:t>[[A]]</w:t></w:r><w:r><w:t> and [[B]]</w:t></w:r></w:p>{TAIL}"
        );
        let repls = vec![
            ("[[A]]".to_string(), "one".to_string()),
            ("[[B]]".to_string(), "two".to_string()),
        ];
        let out = transform_fill(xml.as_bytes(), &repls).unwrap();
        let texts = paragraph_texts(&out).unwrap();
        assert_eq!(texts[0], "one and two");
    }

    #[test]
    fn handles_entities() {
        let xml = format!(
            "{HEAD}<w:p><w:r><w:t>[[Name]] &amp; co</w:t></w:r></w:p>{TAIL}"
        );
        let repls = vec![("[[Name]]".to_string(), "A<B".to_string())];
        let out = transform_fill(xml.as_bytes(), &repls).unwrap();
        let texts = paragraph_texts(&out).unwrap();
        assert_eq!(texts[0], "A<B & co");
    }

    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

    fn make_docx(body: &str) -> Vec<u8> {
        let doc = format!("{HEAD}{body}{TAIL}");
        let entries = vec![
            ("[Content_Types].xml".to_string(), CONTENT_TYPES.as_bytes().to_vec()),
            ("word/document.xml".to_string(), doc.into_bytes()),
        ];
        rebuild_zip(&entries).unwrap()
    }

    fn document_texts(docx_bytes: &[u8]) -> Vec<String> {
        let entries = load_entries_from_bytes(docx_bytes).unwrap();
        let doc = entries.iter().find(|(n, _)| n == "word/document.xml").unwrap();
        paragraph_texts(&doc.1).unwrap()
    }

    #[test]
    fn full_zip_roundtrip_detect_and_fill() {
        // Includes a split-run placeholder and a placeholder inside a table cell.
        let body = r#"<w:p><w:r><w:t>Dear [[Na</w:t></w:r><w:r><w:t>me]],</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>City: [[City]]</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:sectPr/>"#;
        let bytes = make_docx(body);

        let tmp = std::env::temp_dir().join("lf_test_template.docx");
        std::fs::write(&tmp, &bytes).unwrap();

        let mut found = find_placeholders(&tmp).unwrap();
        found.sort();
        assert_eq!(found, vec!["City".to_string(), "Name".to_string()]);

        let mut repl = HashMap::new();
        repl.insert("Name".to_string(), "Ada".to_string());
        repl.insert("City".to_string(), "London".to_string());
        let filled = fill_template_bytes(&tmp, &repl).unwrap();

        let texts = document_texts(&filled);
        assert!(texts.iter().any(|t| t == "Dear Ada,"), "got {texts:?}");
        assert!(texts.iter().any(|t| t == "City: London"), "got {texts:?}");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn merge_combines_documents_with_page_break() {
        let a = make_docx(r#"<w:p><w:r><w:t>Letter A</w:t></w:r></w:p><w:sectPr/>"#);
        let b = make_docx(r#"<w:p><w:r><w:t>Letter B</w:t></w:r></w:p><w:sectPr/>"#);
        let merged = crate::docx_merge::merge_docx(&[a, b]).unwrap();
        let texts = document_texts(&merged);
        assert!(texts.iter().any(|t| t == "Letter A"), "got {texts:?}");
        assert!(texts.iter().any(|t| t == "Letter B"), "got {texts:?}");

        // A page break should have been inserted between the two letters.
        let entries = load_entries_from_bytes(&merged).unwrap();
        let doc = entries.iter().find(|(n, _)| n == "word/document.xml").unwrap();
        let xml = String::from_utf8_lossy(&doc.1);
        assert!(xml.contains("w:type=\"page\""), "expected page break in {xml}");
    }
}
