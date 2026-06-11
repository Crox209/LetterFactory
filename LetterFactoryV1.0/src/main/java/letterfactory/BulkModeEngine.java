package letterfactory;

import org.apache.pdfbox.io.MemoryUsageSetting;
import org.apache.pdfbox.io.RandomAccessStreamCache;
import org.apache.pdfbox.multipdf.PDFMergerUtility;
import org.apache.poi.ss.usermodel.Cell;
import org.apache.poi.ss.usermodel.DataFormatter;
import org.apache.poi.ss.usermodel.Row;
import org.apache.poi.xssf.usermodel.XSSFWorkbook;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.BiConsumer;

public final class BulkModeEngine {
    private static final DateTimeFormatter TS = DateTimeFormatter.ofPattern("yyyy-MM-dd_HHmmss");
    private static final DataFormatter FMT = new DataFormatter();

    private final ExportService exportService = new ExportService();

    public record ValidationResult(int documentsFound, List<String> warnings, boolean hasMismatch) {}

    public ValidationResult validateExcel(File excelXlsx, Set<String> templatePlaceholders) throws IOException {
        Set<String> tpl = templatePlaceholders != null ? templatePlaceholders : Set.of();
        List<String> warnings = new ArrayList<>();
        boolean mismatch = false;

        try (var in = new FileInputStream(excelXlsx);
             var wb = new XSSFWorkbook(in)) {
            var sheet = wb.getSheetAt(0);
            Row header = sheet.getRow(0);
            if (header == null) {
                warnings.add("Missing header row (Row 1).");
                return new ValidationResult(0, warnings, true);
            }

            Set<String> excelHeaders = new LinkedHashSet<>();
            for (Cell c : header) {
                String v = FMT.formatCellValue(c).trim();
                if (!v.isEmpty()) excelHeaders.add(v);
            }

            for (String h : excelHeaders) {
                String inner = unwrap(h);
                if (inner == null) continue;
                if (!tpl.contains(inner)) {
                    warnings.add("Warning: " + h + " not found in template.");
                    mismatch = true;
                }
            }

            for (String p : tpl) {
                String wrapped = "[[" + p + "]]";
                if (!excelHeaders.contains(wrapped)) {
                    warnings.add("Warning: template placeholder missing in Excel: " + wrapped);
                    mismatch = true;
                }
            }

            int docs = countDataRows(sheet, header.getLastCellNum());
            return new ValidationResult(docs, warnings, mismatch);
        }
    }

    /**
     * Generate bulk outputs.
     *
     * @param onProgress callback (done, total)
     */
    public void generateAll(
            File templateDocx,
            File excelXlsx,
            Path outputFolder,
            ExportService.OutputFormat format,
            List<String> filenamePlaceholders,
            boolean mergeAllIntoOne,
            String mergedFilename,
            BiConsumer<Integer, Integer> onProgress
    ) throws IOException {
        Files.createDirectories(outputFolder);

        try (var in = new FileInputStream(excelXlsx);
             var wb = new XSSFWorkbook(in)) {
            var sheet = wb.getSheetAt(0);
            Row header = sheet.getRow(0);
            if (header == null) throw new IOException("Missing header row (Row 1).");

            List<String> headers = new ArrayList<>();
            for (Cell c : header) headers.add(FMT.formatCellValue(c).trim());

            int total = countDataRows(sheet, header.getLastCellNum());

            if (mergeAllIntoOne) {
                if (format == ExportService.OutputFormat.PDF) {
                    mergeAllPdf(templateDocx, sheet, headers, outputFolder, safeMergedName(mergedFilename) + ".pdf", onProgress, total);
                } else {
                    mergeAllDocx(templateDocx, sheet, headers, outputFolder, safeMergedName(mergedFilename) + ".docx", onProgress, total);
                }
                return;
            }

            Set<String> usedNames = new HashSet<>();
            int done = 0;

            for (int r = 1; r <= sheet.getLastRowNum(); r++) {
                Row row = sheet.getRow(r);
                if (!isDataRow(row, headers.size())) continue;

                Map<String, String> values = rowToValues(headers, row);
                String base = FileNamer.buildNameFromParts(filenamePlaceholders, values);
                base = FileNamer.sanitize(base);
                base = FileNamer.ensureUnique(base.isBlank() ? "Document" : base, usedNames, 3);

                Path out = outputFolder.resolve(base + (format == ExportService.OutputFormat.PDF ? ".pdf" : ".docx"));
                if (format == ExportService.OutputFormat.PDF) {
                    exportService.writePdf(templateDocx, values, out);
                } else {
                    exportService.writeDocx(templateDocx, values, out);
                }

                done++;
                if (onProgress != null) onProgress.accept(done, total);
            }
        }
    }

    private void mergeAllPdf(
            File templateDocx,
            org.apache.poi.ss.usermodel.Sheet sheet,
            List<String> headers,
            Path outputFolder,
            String outName,
            BiConsumer<Integer, Integer> onProgress,
            int total
    ) throws IOException {
        Path tmpDir = Files.createTempDirectory("letterfactory-bulkpdf-");
        tmpDir.toFile().deleteOnExit();

        var merger = new PDFMergerUtility();
        Path out = outputFolder.resolve(outName);
        merger.setDestinationFileName(out.toString());

        int done = 0;
        try {
            for (int r = 1; r <= sheet.getLastRowNum(); r++) {
                Row row = sheet.getRow(r);
                if (!isDataRow(row, headers.size())) continue;

                Map<String, String> values = rowToValues(headers, row);
                Path pdf = tmpDir.resolve("row_" + r + ".pdf");
                exportService.writePdf(templateDocx, values, pdf);
                merger.addSource(pdf.toFile());
                done++;
                if (onProgress != null) onProgress.accept(done, total);
            }

            if (done == 0) {
                throw new IOException("No data rows found in the Excel sheet.");
            }

            RandomAccessStreamCache.StreamCacheCreateFunction cache =
                    MemoryUsageSetting.setupTempFileOnly().streamCache;
            merger.mergeDocuments(cache);
        } catch (Exception e) {
            throw new IOException("PDF merge failed: " + e.getMessage(), e);
        } finally {
            try {
                Files.walk(tmpDir)
                        .sorted(java.util.Comparator.reverseOrder())
                        .forEach(p -> {
                            try {
                                Files.deleteIfExists(p);
                            } catch (IOException ignored) {
                            }
                        });
            } catch (IOException ignored) {
            }
        }
    }

    private void mergeAllDocx(
            File templateDocx,
            org.apache.poi.ss.usermodel.Sheet sheet,
            List<String> headers,
            Path outputFolder,
            String outName,
            BiConsumer<Integer, Integer> onProgress,
            int total
    ) throws IOException {
        Path out = outputFolder.resolve(outName);
        Path tmpDir = Files.createTempDirectory("letterfactory-blocks-");
        List<Path> blocks = new ArrayList<>();
        int done = 0;

        try {
            for (int r = 1; r <= sheet.getLastRowNum(); r++) {
                Row row = sheet.getRow(r);
                if (!isDataRow(row, headers.size())) continue;

                Map<String, String> values = rowToValues(headers, row);
                Path block = tmpDir.resolve(String.format("gen_%04d.docx", done));
                Files.write(block, exportService.generateDocxBytes(templateDocx, values));
                blocks.add(block);
                done++;
                if (onProgress != null) onProgress.accept(done, total);
            }

            if (blocks.isEmpty()) {
                throw new IOException("No data rows found in the Excel sheet.");
            }

            DocxPackageMerger.mergeDocumentFiles(blocks, out);
        } catch (IOException e) {
            throw e;
        } catch (Exception e) {
            throw new IOException("DOCX merge failed: " + rootMessage(e), e);
        } finally {
            deleteRecursively(tmpDir);
        }
    }

    private static void deleteRecursively(Path root) {
        if (root == null || !Files.exists(root)) {
            return;
        }
        try {
            Files.walk(root)
                    .sorted(java.util.Comparator.reverseOrder())
                    .forEach(p -> {
                        try {
                            Files.deleteIfExists(p);
                        } catch (IOException ignored) {
                        }
                    });
        } catch (IOException ignored) {
        }
    }

    private Map<String, String> rowToValues(List<String> headers, Row row) {
        Map<String, String> out = new HashMap<>();
        for (int i = 0; i < headers.size(); i++) {
            String h = headers.get(i);
            String inner = unwrap(h);
            if (inner == null) continue;
            Cell c = row.getCell(i);
            String v = c == null ? "" : FMT.formatCellValue(c);
            out.put(inner, v == null ? "" : v);
        }
        return out;
    }

    private static int countDataRows(org.apache.poi.ss.usermodel.Sheet sheet, int columnCount) {
        int count = 0;
        for (int r = 1; r <= sheet.getLastRowNum(); r++) {
            if (isDataRow(sheet.getRow(r), columnCount)) {
                count++;
            }
        }
        return count;
    }

    /**
     * Skip blank Excel rows (common when the sheet has a large formatted range but few real records).
     */
    private static boolean isDataRow(Row row, int columnCount) {
        if (row == null) return false;
        int cols = Math.max(columnCount, 0);
        for (int i = 0; i < cols; i++) {
            Cell c = row.getCell(i);
            String v = c == null ? "" : FMT.formatCellValue(c).trim();
            if (!v.isEmpty()) return true;
        }
        return false;
    }

    private static String unwrap(String headerCell) {
        if (headerCell == null) return null;
        String s = headerCell.trim();
        if (!s.startsWith("[[") || !s.endsWith("]]")) return null;
        String inner = s.substring(2, s.length() - 2).trim();
        return inner.isEmpty() ? null : inner;
    }

    private static String safeMergedName(String name) {
        String base = (name == null || name.isBlank())
                ? "Merged_" + TS.format(LocalDateTime.now())
                : name;
        base = FileNamer.sanitize(base);
        return base.isBlank() ? "Merged_" + TS.format(LocalDateTime.now()) : base;
    }

    private static String rootMessage(Throwable t) {
        Throwable c = t;
        while (c.getCause() != null && c.getCause() != c) {
            c = c.getCause();
        }
        String msg = c.getMessage();
        return msg != null && !msg.isBlank() ? msg : c.getClass().getSimpleName();
    }
}

