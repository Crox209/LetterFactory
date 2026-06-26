//! Merge several filled `.docx` files (all generated from the same template)
//! into a single document, one recipient per page.
//!
//! Pragmatic port of the Java `DocxPackageMerger`: the first document is the
//! host (it carries the shared parts - styles, images, headers/footers, and
//! their relationship ids). Each subsequent document's body content is copied
//! into the host body, separated by a next-page break, with trailing empty
//! filler paragraphs removed so recipients don't accumulate blank pages. The
//! last document keeps its content verbatim and its section properties become
//! the final body section.

use std::io::Cursor;

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use crate::docx;

/// Merge document byte blobs into one `.docx`. Returns the first blob unchanged
/// when only one document is supplied.
pub fn merge_docx(docs: &[Vec<u8>]) -> Result<Vec<u8>> {
    if docs.is_empty() {
        return Err(anyhow!("No documents to merge."));
    }
    if docs.len() == 1 {
        return Ok(docs[0].clone());
    }

    // Host parts (everything except document.xml is reused as-is).
    let mut host_entries = docx::load_entries_from_bytes(&docs[0])?;
    let host_doc_xml = host_entries
        .iter()
        .find(|(n, _)| n == "word/document.xml")
        .map(|(_, b)| b.clone())
        .ok_or_else(|| anyhow!("host document missing word/document.xml"))?;

    let host_events = parse_events(&host_doc_xml)?;
    let (body_start, body_end) = body_bounds(&host_events)
        .ok_or_else(|| anyhow!("host document has no <w:body>"))?;
    let head = host_events[..=body_start].to_vec();
    let tail = host_events[body_end..].to_vec();

    let mut merged_inner: Vec<Event<'static>> = Vec::new();
    let mut final_sectpr: Vec<Event<'static>> = Vec::new();
    let last = docs.len() - 1;

    for (i, doc) in docs.iter().enumerate() {
        let xml = if i == 0 {
            host_doc_xml.clone()
        } else {
            docx::load_entries_from_bytes(doc)?
                .into_iter()
                .find(|(n, _)| n == "word/document.xml")
                .map(|(_, b)| b)
                .ok_or_else(|| anyhow!("document {i} missing word/document.xml"))?
        };
        let events = parse_events(&xml)?;
        let (bs, be) = body_bounds(&events).ok_or_else(|| anyhow!("document {i} has no <w:body>"))?;
        let inner = events[bs + 1..be].to_vec();
        let (content, sectpr) = split_sectpr(inner);

        if i == last {
            merged_inner.extend(content);
            if !sectpr.is_empty() {
                final_sectpr = sectpr;
            }
        } else {
            let trimmed = strip_trailing_empty_paragraphs(content);
            merged_inner.extend(trimmed);
            merged_inner.extend(page_break_paragraph());
            if final_sectpr.is_empty() && !sectpr.is_empty() {
                final_sectpr = sectpr.clone();
            }
        }
    }

    // Assemble: head + all letter content + final section properties + tail.
    let mut out_events: Vec<Event<'static>> = Vec::new();
    out_events.extend(head);
    out_events.extend(merged_inner);
    out_events.extend(final_sectpr);
    out_events.extend(tail);

    let merged_xml = serialize(&out_events)?;

    for (name, bytes) in host_entries.iter_mut() {
        if name == "word/document.xml" {
            *bytes = merged_xml.clone();
        }
    }
    docx::rebuild_zip(&host_entries)
}

// ---------------------------------------------------------------------------
// Event helpers
// ---------------------------------------------------------------------------

fn parse_events(xml: &[u8]) -> Result<Vec<Event<'static>>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut events = Vec::new();
    loop {
        let ev = reader.read_event_into(&mut buf)?;
        match ev {
            Event::Eof => break,
            other => events.push(other.into_owned()),
        }
        buf.clear();
    }
    Ok(events)
}

fn serialize(events: &[Event<'static>]) -> Result<Vec<u8>> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    for ev in events {
        writer.write_event(ev.clone())?;
    }
    Ok(writer.into_inner().into_inner())
}

/// Index of `Start(w:body)` and of the matching `End(w:body)`.
fn body_bounds(events: &[Event<'static>]) -> Option<(usize, usize)> {
    let start = events.iter().position(|e| matches!(e, Event::Start(s) if s.name().as_ref() == b"w:body"))?;
    let end = events.iter().rposition(|e| matches!(e, Event::End(s) if s.name().as_ref() == b"w:body"))?;
    if end > start {
        Some((start, end))
    } else {
        None
    }
}

/// Split body inner events into (content, body-level sectPr block).
fn split_sectpr(inner: Vec<Event<'static>>) -> (Vec<Event<'static>>, Vec<Event<'static>>) {
    // Find the index of the top-level (depth 0) <w:sectPr>.
    let mut depth = 0i32;
    let mut sect_idx: Option<usize> = None;
    for (i, ev) in inner.iter().enumerate() {
        match ev {
            Event::Start(s) => {
                if depth == 0 && s.name().as_ref() == b"w:sectPr" {
                    sect_idx = Some(i);
                }
                depth += 1;
            }
            Event::Empty(s) => {
                if depth == 0 && s.name().as_ref() == b"w:sectPr" {
                    sect_idx = Some(i);
                }
            }
            Event::End(_) => depth -= 1,
            _ => {}
        }
    }
    match sect_idx {
        Some(idx) => {
            let sect = inner[idx..].to_vec();
            let content = inner[..idx].to_vec();
            (content, sect)
        }
        None => (inner, Vec::new()),
    }
}

/// `<w:p><w:r><w:br w:type="page"/></w:r></w:p>`
fn page_break_paragraph() -> Vec<Event<'static>> {
    let mut br = BytesStart::new("w:br");
    br.push_attribute(("w:type", "page"));
    vec![
        Event::Start(BytesStart::new("w:p")),
        Event::Start(BytesStart::new("w:r")),
        Event::Empty(br),
        Event::End(BytesEnd::new("w:r")),
        Event::End(BytesEnd::new("w:p")),
    ]
}

/// Drop trailing empty top-level paragraphs (template end-of-page filler).
fn strip_trailing_empty_paragraphs(mut content: Vec<Event<'static>>) -> Vec<Event<'static>> {
    loop {
        // Remove trailing whitespace-only text events.
        while matches!(content.last(), Some(Event::Text(t)) if docx::text_to_string(t).trim().is_empty()) {
            content.pop();
        }
        let ranges = top_level_ranges(&content);
        match ranges.last() {
            Some(&(s, e, ref name)) if name == b"w:p" && paragraph_is_empty(&content[s..=e]) => {
                content.truncate(s);
            }
            _ => break,
        }
    }
    content
}

/// (start, end, element-name) of each top-level element in the slice.
fn top_level_ranges(events: &[Event<'static>]) -> Vec<(usize, usize, Vec<u8>)> {
    let mut ranges = Vec::new();
    let mut depth = 0i32;
    let mut open: Option<(usize, Vec<u8>)> = None;
    for (i, ev) in events.iter().enumerate() {
        match ev {
            Event::Start(s) => {
                if depth == 0 {
                    open = Some((i, s.name().as_ref().to_vec()));
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    if let Some((start, name)) = open.take() {
                        ranges.push((start, i, name));
                    }
                }
            }
            Event::Empty(s) => {
                if depth == 0 {
                    ranges.push((i, i, s.name().as_ref().to_vec()));
                }
            }
            _ => {}
        }
    }
    ranges
}

/// True when a paragraph slice contains no non-empty `<w:t>` text.
fn paragraph_is_empty(events: &[Event<'static>]) -> bool {
    let mut in_text = false;
    for ev in events {
        match ev {
            Event::Start(s) if s.name().as_ref() == b"w:t" => in_text = true,
            Event::End(s) if s.name().as_ref() == b"w:t" => in_text = false,
            Event::Text(t) if in_text => {
                if !docx::text_to_string(t).is_empty() {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}
