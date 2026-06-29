package letterfactory;

import org.apache.poi.xwpf.usermodel.XWPFDocument;
import org.docx4j.openpackaging.packages.WordprocessingMLPackage;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

/**
 * Fills and scans arbitrary DOCX templates: all parts, split runs, text boxes, SDTs, headers/footers, etc.
 */
public final class UniversalDocxProcessor {
    private static final int MAX_FILL_PASSES = 4;

    private final Docx4jPlaceholderReplacer docx4jReplacer = new Docx4jPlaceholderReplacer();
    private final DocxPlaceholderReplacer poiReplacer = new DocxPlaceholderReplacer();

    public Set<String> findPlaceholders(File templateDocx) throws IOException {
        var found = new LinkedHashSet<String>();
        try {
            WordprocessingMLPackage pkg = WordprocessingMLPackage.load(templateDocx);
            found.addAll(docx4jReplacer.collectPlaceholders(pkg));
        } catch (Exception ignored) {
            // fall back to POI-only scan below
        }

        try (var in = new FileInputStream(templateDocx);
             var doc = new XWPFDocument(in)) {
            found.addAll(poiReplacer.collectPlaceholders(doc));
        }
        return found;
    }

    public byte[] fillTemplate(File templateDocx, Map<String, String> replacements) throws Exception {
        WordprocessingMLPackage pkg = WordprocessingMLPackage.load(templateDocx);
        byte[] current = null;

        for (int pass = 0; pass < MAX_FILL_PASSES; pass++) {
            if (pass == 0) {
                docx4jReplacer.apply(pkg, replacements);
            } else if (current != null) {
                pkg = WordprocessingMLPackage.load(new ByteArrayInputStream(current));
                docx4jReplacer.apply(pkg, replacements);
            }

            ByteArrayOutputStream buf = new ByteArrayOutputStream();
            pkg.save(buf);
            current = buf.toByteArray();

            if (!hasUnresolvedPlaceholders(current)) {
                return current;
            }

            try (var in = new ByteArrayInputStream(current);
                 var doc = new XWPFDocument(in)) {
                poiReplacer.apply(doc, replacements);
                ByteArrayOutputStream poiOut = new ByteArrayOutputStream();
                doc.write(poiOut);
                current = poiOut.toByteArray();
            }

            if (!hasUnresolvedPlaceholders(current)) {
                return current;
            }
        }

        return current;
    }

    public boolean hasUnresolvedPlaceholders(byte[] docxBytes) throws IOException {
        try (var in = new ByteArrayInputStream(docxBytes);
             var doc = new XWPFDocument(in)) {
            return poiReplacer.collectFullText(doc).map(PlaceholderPatterns::containsUnresolved).orElse(false);
        }
    }
}
