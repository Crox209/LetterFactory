package letterfactory;

import org.docx4j.Docx4J;
import org.docx4j.convert.out.FOSettings;
import org.docx4j.fonts.BestMatchingMapper;
import org.docx4j.fonts.IdentityPlusMapper;
import org.docx4j.fonts.Mapper;
import org.docx4j.fonts.PhysicalFonts;
import org.docx4j.openpackaging.packages.WordprocessingMLPackage;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Map;
import java.util.Objects;

public final class ExportService {
    private static final DateTimeFormatter TS = DateTimeFormatter.ofPattern("yyyy-MM-dd_HHmmss");

    private final UniversalDocxProcessor docxProcessor = new UniversalDocxProcessor();

    static {
        try {
            PhysicalFonts.discoverPhysicalFonts();
        } catch (Exception ignored) {
        }
        System.setProperty("docx4j.Log4j.configurator.disabled", "true");
    }

    public enum OutputFormat { DOCX, PDF }

    public Path export(File templateDocx, Map<String, String> replacements, Path outputFolder, OutputFormat format) throws IOException {
        Objects.requireNonNull(templateDocx, "templateDocx");
        Objects.requireNonNull(replacements, "replacements");
        Objects.requireNonNull(format, "format");
        Objects.requireNonNull(outputFolder, "outputFolder");

        Files.createDirectories(outputFolder);

        String baseName = stripExt(templateDocx.getName());
        String fileName = baseName + "_" + TS.format(LocalDateTime.now()) + (format == OutputFormat.PDF ? ".pdf" : ".docx");
        Path outPath = outputFolder.resolve(fileName);

        if (format == OutputFormat.DOCX) {
            writeDocx(templateDocx, replacements, outPath);
        } else {
            writePdf(templateDocx, replacements, outPath);
        }

        return outPath;
    }

    public byte[] generateDocxBytes(File templateDocx, Map<String, String> replacements) throws IOException {
        try {
            return docxProcessor.fillTemplate(templateDocx, replacements);
        } catch (Exception e) {
            throw new IOException("Could not generate document: " + rootMessage(e), e);
        }
    }

    public void writeDocx(File templateDocx, Map<String, String> replacements, Path outPath) throws IOException {
        Files.write(outPath, generateDocxBytes(templateDocx, replacements));
    }

    public void writePdf(File templateDocx, Map<String, String> replacements, Path outPath) throws IOException {
        try {
            WordprocessingMLPackage pkg = WordprocessingMLPackage.load(new ByteArrayInputStream(
                    generateDocxBytes(templateDocx, replacements)));
            convertPackageToPdf(pkg, outPath);
        } catch (Exception e) {
            throw new IOException("DOCX to PDF conversion failed: " + rootMessage(e), e);
        }
    }

    private void convertPackageToPdf(WordprocessingMLPackage pkg, Path pdfPath) throws Exception {
        configureFontMapper(pkg);

        try (OutputStream os = Files.newOutputStream(pdfPath)) {
            FOSettings foSettings = Docx4J.createFOSettings();
            foSettings.setOpcPackage(pkg);
            foSettings.setApacheFopMime(FOSettings.MIME_PDF);
            Docx4J.toFO(foSettings, os, Docx4J.FLAG_EXPORT_PREFER_XSL);
        } catch (Exception first) {
            Files.deleteIfExists(pdfPath);
            try (OutputStream os = Files.newOutputStream(pdfPath)) {
                Docx4J.toPDF(pkg, os, Docx4J.FLAG_EXPORT_PREFER_XSL);
            } catch (Exception second) {
                throw new Exception(
                        "PDF export failed. " + rootMessage(first) + " | " + rootMessage(second),
                        second
                );
            }
        }
    }

    private static void configureFontMapper(WordprocessingMLPackage pkg) throws Exception {
        Mapper fontMapper;
        try {
            fontMapper = new BestMatchingMapper();
        } catch (Exception e) {
            fontMapper = new IdentityPlusMapper();
        }
        mapIfPresent(fontMapper, "Calibri");
        mapIfPresent(fontMapper, "Arial");
        mapIfPresent(fontMapper, "Times New Roman");
        mapIfPresent(fontMapper, "Helvetica");
        mapIfPresent(fontMapper, "Segoe UI");
        pkg.setFontMapper(fontMapper);
    }

    private static void mapIfPresent(Mapper fontMapper, String name) {
        try {
            if (PhysicalFonts.get(name) != null) {
                fontMapper.put(name, PhysicalFonts.get(name));
            }
        } catch (Exception ignored) {
        }
    }

    private static String rootMessage(Throwable t) {
        Throwable c = t;
        while (c.getCause() != null && c.getCause() != c) {
            c = c.getCause();
        }
        String msg = c.getMessage();
        return msg != null && !msg.isBlank() ? msg : c.getClass().getSimpleName();
    }

    private static String stripExt(String name) {
        int idx = name.lastIndexOf('.');
        return idx > 0 ? name.substring(0, idx) : name;
    }
}
