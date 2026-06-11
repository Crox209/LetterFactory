package letterfactory;

import java.awt.Desktop;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.Objects;

public final class PrintService {
    private final ExportService exportService = new ExportService();

    public void print(File templateDocx, Map<String, String> replacements, ExportService.OutputFormat format) throws IOException {
        Objects.requireNonNull(templateDocx, "templateDocx");
        Objects.requireNonNull(replacements, "replacements");
        Objects.requireNonNull(format, "format");

        Path tmpDir = Files.createTempDirectory("letterfactory-print-");
        tmpDir.toFile().deleteOnExit();

        Path out = tmpDir.resolve("print" + (format == ExportService.OutputFormat.PDF ? ".pdf" : ".docx"));
        if (format == ExportService.OutputFormat.PDF) {
            exportService.writePdf(templateDocx, replacements, out);
        } else {
            exportService.writeDocx(templateDocx, replacements, out);
        }

        if (!Desktop.isDesktopSupported()) {
            throw new IOException("Desktop integration not supported on this system.");
        }
        Desktop desktop = Desktop.getDesktop();
        if (!desktop.isSupported(Desktop.Action.PRINT)) {
            throw new IOException("Printing not supported on this system.");
        }
        desktop.print(out.toFile());
    }
}

