package letterfactory;

import org.apache.poi.xwpf.usermodel.IBody;
import org.apache.poi.xwpf.usermodel.IBodyElement;
import org.apache.poi.xwpf.usermodel.PositionInParagraph;
import org.apache.poi.xwpf.usermodel.TextSegment;
import org.apache.poi.xwpf.usermodel.XWPFAbstractFootnoteEndnote;
import org.apache.poi.xwpf.usermodel.XWPFDocument;
import org.apache.poi.xwpf.usermodel.XWPFHeader;
import org.apache.poi.xwpf.usermodel.XWPFParagraph;
import org.apache.poi.xwpf.usermodel.XWPFFooter;
import org.apache.poi.xwpf.usermodel.XWPFRun;
import org.apache.poi.xwpf.usermodel.XWPFTable;
import org.apache.poi.xwpf.usermodel.XWPFTableCell;
import org.apache.poi.xwpf.usermodel.XWPFTableRow;
import org.apache.xmlbeans.XmlObject;

import org.openxmlformats.schemas.wordprocessingml.x2006.main.CTP;

import javax.xml.namespace.QName;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;

public final class DocxPlaceholderReplacer {

    private static final String W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    private static final String XPATH_ALL_PARAGRAPHS =
            "declare namespace w='" + W_NS + "' .//w:p";

    public Set<String> collectPlaceholders(XWPFDocument doc) {
        var out = new LinkedHashSet<String>();
        collectFromXmlRoot(doc.getDocument(), doc, out);
        for (XWPFHeader header : doc.getHeaderList()) {
            collectFromXmlRoot(header._getHdrFtr(), header, out);
        }
        for (XWPFFooter footer : doc.getFooterList()) {
            collectFromXmlRoot(footer._getHdrFtr(), footer, out);
        }
        for (XWPFAbstractFootnoteEndnote fn : doc.getFootnotes()) {
            collectFromXmlRoot(fn.getCTFtnEdn(), fn, out);
        }
        for (XWPFAbstractFootnoteEndnote en : doc.getEndnotes()) {
            collectFromXmlRoot(en.getCTFtnEdn(), en, out);
        }
        return out;
    }

    public Optional<String> collectFullText(XWPFDocument doc) {
        StringBuilder sb = new StringBuilder();
        appendTextFromXmlRoot(doc.getDocument(), doc, sb);
        for (XWPFHeader header : doc.getHeaderList()) {
            appendTextFromXmlRoot(header._getHdrFtr(), header, sb);
        }
        for (XWPFFooter footer : doc.getFooterList()) {
            appendTextFromXmlRoot(footer._getHdrFtr(), footer, sb);
        }
        for (XWPFAbstractFootnoteEndnote fn : doc.getFootnotes()) {
            appendTextFromXmlRoot(fn.getCTFtnEdn(), fn, sb);
        }
        for (XWPFAbstractFootnoteEndnote en : doc.getEndnotes()) {
            appendTextFromXmlRoot(en.getCTFtnEdn(), en, sb);
        }
        return Optional.of(sb.toString());
    }

    /**
     * Apply replacements to all reachable parts of the document:
     * body, tables, headers/footers, and textboxes.
     *
     * Keys in {@code replacements} are placeholder inner names (without brackets),
     * e.g. "Name" for token {@code [[Name]]}.
     */
    public void apply(XWPFDocument doc, Map<String, String> replacements) {
        Objects.requireNonNull(doc, "doc");
        Objects.requireNonNull(replacements, "replacements");

        applyToBody(doc, replacements);

        for (XWPFHeader header : doc.getHeaderList()) {
            applyToBody(header, replacements);
        }
        for (XWPFFooter footer : doc.getFooterList()) {
            applyToBody(footer, replacements);
        }

        // Footnotes/endnotes (best-effort; not explicitly required but commonly present)
        for (XWPFAbstractFootnoteEndnote fn : doc.getFootnotes()) {
            applyToBody(fn, replacements);
        }
        for (XWPFAbstractFootnoteEndnote en : doc.getEndnotes()) {
            applyToBody(en, replacements);
        }

        // Text boxes are typically embedded inside paragraphs but not accessible via normal paragraph traversal.
        applyToTextBoxes(doc, replacements);
        for (XWPFHeader header : doc.getHeaderList()) {
            applyToTextBoxes(header, replacements);
        }
        for (XWPFFooter footer : doc.getFooterList()) {
            applyToTextBoxes(footer, replacements);
        }

        // Universal pass: every w:p in the package (SDTs, nested tables, text boxes, etc.)
        applyViaXmlParagraphScan(doc.getDocument(), doc, replacements);
        for (XWPFHeader header : doc.getHeaderList()) {
            applyViaXmlParagraphScan(header._getHdrFtr(), header, replacements);
        }
        for (XWPFFooter footer : doc.getFooterList()) {
            applyViaXmlParagraphScan(footer._getHdrFtr(), footer, replacements);
        }
        for (XWPFAbstractFootnoteEndnote fn : doc.getFootnotes()) {
            applyViaXmlParagraphScan(fn.getCTFtnEdn(), fn, replacements);
        }
        for (XWPFAbstractFootnoteEndnote en : doc.getEndnotes()) {
            applyViaXmlParagraphScan(en.getCTFtnEdn(), en, replacements);
        }
    }

    private void collectFromXmlRoot(XmlObject root, IBody body, Set<String> sink) {
        if (root == null) return;
        XmlObject[] paragraphs = root.selectPath(XPATH_ALL_PARAGRAPHS);
        if (paragraphs == null) return;
        for (XmlObject pObj : paragraphs) {
            try {
                CTP ctp = CTP.Factory.parse(pObj.xmlText());
                var paragraph = new XWPFParagraph(ctp, body);
                sink.addAll(PlaceholderPatterns.scanText(paragraph.getText()));
            } catch (Exception ignored) {
            }
        }
    }

    private void appendTextFromXmlRoot(XmlObject root, IBody body, StringBuilder sink) {
        if (root == null) return;
        XmlObject[] paragraphs = root.selectPath(XPATH_ALL_PARAGRAPHS);
        if (paragraphs == null) return;
        for (XmlObject pObj : paragraphs) {
            try {
                CTP ctp = CTP.Factory.parse(pObj.xmlText());
                var paragraph = new XWPFParagraph(ctp, body);
                String text = paragraph.getText();
                if (text != null && !text.isEmpty()) {
                    sink.append(text).append('\n');
                }
            } catch (Exception ignored) {
            }
        }
    }

    private void applyViaXmlParagraphScan(XmlObject root, IBody body, Map<String, String> replacements) {
        if (root == null) return;
        XmlObject[] paragraphs = root.selectPath(XPATH_ALL_PARAGRAPHS);
        if (paragraphs == null) return;
        for (XmlObject pObj : paragraphs) {
            try {
                CTP ctp = CTP.Factory.parse(pObj.xmlText());
                var paragraph = new XWPFParagraph(ctp, body);
                applyToParagraph(paragraph, replacements);
                pObj.set(ctp);
            } catch (Exception ignored) {
            }
        }
    }

    private void applyToBody(IBody body, Map<String, String> replacements) {
        for (IBodyElement e : body.getBodyElements()) {
            if (e instanceof XWPFParagraph p) {
                applyToParagraph(p, replacements);
            } else if (e instanceof XWPFTable t) {
                applyToTable(t, replacements);
            }
        }
    }

    private void applyToTable(XWPFTable t, Map<String, String> replacements) {
        for (XWPFTableRow row : t.getRows()) {
            for (XWPFTableCell cell : row.getTableCells()) {
                applyToBody(cell, replacements);
            }
        }
    }

    private void applyToParagraph(XWPFParagraph paragraph, Map<String, String> replacements) {
        for (Map.Entry<String, String> entry : replacements.entrySet()) {
            String key = entry.getKey();
            String value = entry.getValue() == null ? "" : entry.getValue();
            replaceAllInParagraph(paragraph, "[[" + key + "]]", value);
        }
    }

    /**
     * Replace all occurrences of {@code token} in a paragraph using POI TextSegment,
     * so occurrences spanning multiple runs are handled without losing formatting.
     */
    private void replaceAllInParagraph(XWPFParagraph paragraph, String token, String replacement) {
        if (token == null || token.isEmpty()) return;
        if (paragraph.getRuns() == null || paragraph.getRuns().isEmpty()) return;

        PositionInParagraph pos = new PositionInParagraph(0, 0, 0);
        TextSegment seg;
        while ((seg = paragraph.searchText(token, pos)) != null) {
            replaceSegment(paragraph, seg, token, replacement);

            // Continue searching after the end run we just modified.
            // Using endRun as next "run" position avoids infinite loops.
            pos = new PositionInParagraph(seg.getEndRun(), 0, 0);
        }
    }

    private void replaceSegment(XWPFParagraph paragraph, TextSegment seg, String token, String replacement) {
        List<XWPFRun> runs = paragraph.getRuns();
        int beginRunIdx = seg.getBeginRun();
        int endRunIdx = seg.getEndRun();
        int beginChar = seg.getBeginChar();
        int endChar = seg.getEndChar();

        if (beginRunIdx < 0 || endRunIdx < 0 || beginRunIdx >= runs.size() || endRunIdx >= runs.size()) {
            return;
        }

        if (beginRunIdx == endRunIdx) {
            XWPFRun run = runs.get(beginRunIdx);
            String text = run.getText(0);
            if (text == null) return;

            String before = text.substring(0, Math.min(beginChar, text.length()));
            String after = text.substring(Math.min(endChar + 1, text.length()));
            run.setText(before + replacement + after, 0);
            return;
        }

        XWPFRun beginRun = runs.get(beginRunIdx);
        XWPFRun endRun = runs.get(endRunIdx);

        String beginText = beginRun.getText(0);
        String endText = endRun.getText(0);
        if (beginText == null) beginText = "";
        if (endText == null) endText = "";

        String beginPrefix = beginText.substring(0, Math.min(beginChar, beginText.length()));
        String endSuffix = endText.substring(Math.min(endChar + 1, endText.length()));

        // Put the replacement into the begin run to preserve formatting of the placeholder's first run.
        beginRun.setText(beginPrefix + replacement + endSuffix, 0);

        // Remove runs that were part of the matched segment (from end to begin+1),
        // leaving only the modified begin run.
        for (int i = endRunIdx; i > beginRunIdx; i--) {
            paragraph.removeRun(i);
        }
    }

    private void applyToTextBoxes(IBody body, Map<String, String> replacements) {
        for (IBodyElement e : body.getBodyElements()) {
            if (e instanceof XWPFParagraph p) {
                applyToTextBoxesInParagraph(p, replacements);
            } else if (e instanceof XWPFTable t) {
                for (XWPFTableRow row : t.getRows()) {
                    for (XWPFTableCell cell : row.getTableCells()) {
                        applyToTextBoxes(cell, replacements);
                    }
                }
            }
        }
    }

    private void applyToTextBoxesInParagraph(XWPFParagraph paragraph, Map<String, String> replacements) {
        // Locate embedded paragraphs in text boxes.
        XmlObject[] tb = paragraph.getCTP().selectPath(
                "declare namespace w='http://schemas.openxmlformats.org/wordprocessingml/2006/main' " +
                        "declare namespace wps='http://schemas.microsoft.com/office/word/2010/wordprocessingShape' " +
                        "declare namespace v='urn:schemas-microsoft-com:vml' " +
                        ".//*/wps:txbx/w:txbxContent | .//*/v:textbox/w:txbxContent"
        );
        if (tb == null || tb.length == 0) return;

        for (XmlObject o : tb) {
            XmlObject[] paras = o.selectChildren(new QName("http://schemas.openxmlformats.org/wordprocessingml/2006/main", "p"));
            if (paras == null) continue;

            for (XmlObject pObj : paras) {
                try {
                    var embedded = new XWPFParagraph(
                            org.openxmlformats.schemas.wordprocessingml.x2006.main.CTP.Factory.parse(pObj.xmlText()),
                            paragraph.getBody()
                    );
                    applyToParagraph(embedded, replacements);
                    // Push changes back into the original XML object
                    pObj.set(embedded.getCTP());
                } catch (Exception ignore) {
                    // ignore malformed embedded paragraph
                }
            }
        }
    }
}

