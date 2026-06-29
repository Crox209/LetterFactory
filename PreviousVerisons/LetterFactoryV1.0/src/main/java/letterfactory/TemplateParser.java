package letterfactory;

import org.apache.poi.xwpf.usermodel.XWPFDocument;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.util.Set;

public final class TemplateParser {
    private final UniversalDocxProcessor processor = new UniversalDocxProcessor();

    public Set<String> detectPlaceholders(File docxFile) throws IOException {
        return processor.findPlaceholders(docxFile);
    }

    public Set<String> detectPlaceholders(XWPFDocument doc) {
        return new DocxPlaceholderReplacer().collectPlaceholders(doc);
    }
}
