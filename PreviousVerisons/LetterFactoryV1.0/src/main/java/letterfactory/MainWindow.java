package letterfactory;

import javafx.collections.FXCollections;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Parent;
import javafx.scene.control.Button;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Label;
import javafx.scene.control.ScrollPane;
import javafx.scene.control.TextField;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.stage.DirectoryChooser;
import javafx.stage.FileChooser;
import javafx.stage.Window;

import java.io.File;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

public final class MainWindow {
    private final BorderPane root;

    private final ComboBox<String> formatDropdown;
    private final Button templateButton;
    private final Button outputFolderButton;
    private final Button bulkModeButton;
    private final Button exportButton;
    private final Button printButton;
    private final Label bulkStatusLabel;

    private final VBox inputsContainer;
    private final ScrollPane inputsScrollPane;
    private final VBox emptyState;

    private File templateFile;
    private File outputFolder;

    private final Map<String, TextField> inputByPlaceholder = new LinkedHashMap<>();

    public MainWindow() {
        root = new BorderPane();
        root.getStyleClass().add("app-root");
        root.setTop(buildHeader());

        inputsContainer = new VBox();
        inputsContainer.getStyleClass().add("inputs-container");
        inputsContainer.setSpacing(20);

        inputsScrollPane = new ScrollPane(inputsContainer);
        inputsScrollPane.getStyleClass().add("inputs-scroll");
        inputsScrollPane.setFitToWidth(true);

        emptyState = buildEmptyState();
        root.setCenter(emptyState);

        var bottom = new VBox();
        bottom.getStyleClass().add("bottom-panel");
        bottom.setPadding(new Insets(15, 20, 15, 20));
        bottom.setSpacing(12);
        bottom.setMinHeight(140);

        var topRow = new HBox();
        topRow.setAlignment(Pos.CENTER_LEFT);
        topRow.setSpacing(20);

        formatDropdown = new ComboBox<>(FXCollections.observableArrayList(".docx", ".pdf"));
        formatDropdown.getSelectionModel().select(0);
        formatDropdown.getStyleClass().add("dropdown");
        formatDropdown.setPrefWidth(120);
        var formatWrap = labeledControl("Format", formatDropdown);

        templateButton = new Button("Template");
        templateButton.getStyleClass().add("surface-button");
        templateButton.setOnAction(e -> chooseTemplate());
        var templateWrap = labeledControl("Template", templateButton);
        FileDropHelper.enableFileDrop(templateButton, ".docx", this::loadTemplate);
        FileDropHelper.enableFileDrop(templateWrap, ".docx", this::loadTemplate);

        outputFolderButton = new Button("Output Folder");
        outputFolderButton.getStyleClass().add("surface-button");
        outputFolderButton.setOnAction(e -> chooseOutputFolder());
        var outputWrap = labeledControl("Output Folder", outputFolderButton);

        bulkModeButton = new Button("Bulk Mode");
        bulkModeButton.getStyleClass().add("surface-button");
        bulkModeButton.setOnAction(e -> openBulkMode());
        var bulkWrap = labeledControl("Bulk Mode", bulkModeButton);

        topRow.getChildren().addAll(formatWrap, templateWrap, outputWrap, bulkWrap);

        var bottomRow = new HBox();
        bottomRow.setAlignment(Pos.CENTER_LEFT);
        bottomRow.setSpacing(20);

        exportButton = new Button("Export");
        exportButton.getStyleClass().addAll("action-button", "primary-button");
        exportButton.setDisable(true);
        exportButton.setOnAction(e -> exportSingle());

        printButton = new Button("Print");
        printButton.getStyleClass().addAll("action-button", "print-button");
        printButton.setDisable(true);
        printButton.setOnAction(e -> printSingle());

        var spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        bulkStatusLabel = new Label("Bulk Status: N/A");
        bulkStatusLabel.getStyleClass().add("status-label");

        bottomRow.getChildren().addAll(exportButton, printButton, spacer, bulkStatusLabel);

        bottom.getChildren().addAll(topRow, bottomRow);
        root.setBottom(bottom);

        FileDropHelper.enableFileDrop(emptyState, ".docx", this::loadTemplate);
        FileDropHelper.enableFileDrop(root, ".docx", this::loadTemplate);

        updateActionButtonsEnabled();
    }

    public Parent getRoot() {
        return root;
    }

    private Window owner() {
        return root.getScene() != null ? root.getScene().getWindow() : null;
    }

    private boolean requireOutputFolder() {
        if (outputFolder != null && outputFolder.isDirectory()) {
            return true;
        }
        Dialogs.showInfo(owner(), "Output folder required", "You need to select an output folder before exporting.");
        return false;
    }

    private Parent buildHeader() {
        var bar = new HBox();
        bar.getStyleClass().add("top-bar");
        bar.setPadding(new Insets(0, 20, 0, 20));
        bar.setMinHeight(60);
        bar.setAlignment(Pos.CENTER_LEFT);

        var title = new Label("LetterFactory");
        title.getStyleClass().add("app-title");

        var spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        var helpBtn = new Button("?");
        helpBtn.getStyleClass().addAll("icon-button", "help-button");
        helpBtn.setMinSize(30, 30);
        helpBtn.setPrefSize(30, 30);
        helpBtn.setMaxSize(30, 30);
        helpBtn.setOnAction(e -> HelpWindow.show(owner()));

        bar.getChildren().addAll(title, spacer, helpBtn);
        return bar;
    }

    private VBox buildEmptyState() {
        var box = new VBox();
        box.getStyleClass().add("empty-state");
        box.setAlignment(Pos.CENTER);
        box.setSpacing(10);
        box.setPadding(new Insets(30));

        var icon = new Label("📄");
        icon.getStyleClass().add("empty-state-icon");

        var title = new Label("Load a template to begin");
        title.getStyleClass().add("empty-state-title");

        var subtitle = new Label("Click Template or drag and drop a .docx file here");
        subtitle.getStyleClass().add("empty-state-subtitle");

        box.getChildren().addAll(icon, title, subtitle);
        return box;
    }

    private VBox labeledControl(String label, Parent control) {
        var wrap = new VBox();
        wrap.setSpacing(4);
        var l = new Label(label);
        l.getStyleClass().add("control-label");
        wrap.getChildren().addAll(l, control);
        return wrap;
    }

    private void chooseTemplate() {
        var chooser = new FileChooser();
        chooser.setTitle("Select a .docx template");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("Word Document (*.docx)", "*.docx"));
        var file = chooser.showOpenDialog(owner());
        if (file == null) return;
        loadTemplate(file);
    }

    private void loadTemplate(File file) {
        if (file == null || !file.getName().toLowerCase().endsWith(".docx")) {
            Dialogs.showError(owner(), "Invalid template", "Please choose a .docx file (not a Word lock file starting with ~$).");
            return;
        }
        if (file.getName().startsWith("~$")) {
            Dialogs.showError(owner(), "Invalid template", "That file is a temporary Word lock file. Open the real .docx template instead.");
            return;
        }

        templateFile = file;
        templateButton.setText(truncateMiddle(file.getName(), 16));

        try {
            var placeholders = new TemplateParser().detectPlaceholders(file);
            if (placeholders.isEmpty()) {
                Dialogs.showInfo(owner(), "No placeholders", "No placeholders found. Add [[brackets]] to your template to create input fields.");
            }
            renderPlaceholders(placeholders);
        } catch (Exception ex) {
            inputByPlaceholder.clear();
            inputsContainer.getChildren().clear();
            root.setCenter(emptyState);
            Dialogs.showError(owner(), "Template error", "Could not read template: " + ex.getMessage());
        }
        updateActionButtonsEnabled();
    }

    private void chooseOutputFolder() {
        var chooser = new DirectoryChooser();
        chooser.setTitle("Select output folder");
        var dir = chooser.showDialog(owner());
        if (dir == null) return;

        outputFolder = dir;
        outputFolderButton.setText(truncateMiddle(dir.getName(), 16));
        outputFolderButton.setTooltip(new javafx.scene.control.Tooltip(dir.getAbsolutePath()));
        updateActionButtonsEnabled();
    }

    private void openBulkMode() {
        if (templateFile == null) {
            Dialogs.showInfo(owner(), "Template required", "Load a .docx template before opening Bulk Mode.");
            return;
        }
        if (!requireOutputFolder()) {
            return;
        }

        var placeholders = inputByPlaceholder.keySet();
        final BulkModeWindow[] bulkRef = new BulkModeWindow[1];

        bulkRef[0] = new BulkModeWindow(owner(), placeholders, state -> {
            var engine = new BulkModeEngine();
            var fmt = selectedFormat();

            var outDir = outputFolder.toPath();

            new Thread(() -> {
                try {
                    Files.createDirectories(outDir);

                    var vr = engine.validateExcel(state.excelFile(), placeholders);
                    javafx.application.Platform.runLater(() -> {
                        String msg = vr.warnings().isEmpty() ? "All headers match template." : String.join("\n", vr.warnings());
                        bulkRef[0].setPreview(vr.documentsFound(), msg, vr.hasMismatch());
                        bulkRef[0].setGenerateEnabled(true);
                    });

                    engine.generateAll(
                            templateFile,
                            state.excelFile(),
                            outDir,
                            fmt,
                            state.filenameParts(),
                            state.mergeAllIntoOne(),
                            state.mergedFilename(),
                            (done, total) -> javafx.application.Platform.runLater(() -> bulkRef[0].updateProgress(done, total))
                    );

                    javafx.application.Platform.runLater(() -> {
                        bulkRef[0].generationFinished(true, "Bulk generation complete — " + vr.documentsFound() + " documents");
                        bulkStatusLabel.setText("Bulk Status: Ready — " + vr.documentsFound() + " documents");
                        bulkStatusLabel.getStyleClass().removeAll("danger-text", "success-text");
                        bulkStatusLabel.getStyleClass().add("success-text");
                    });
                } catch (Exception ex) {
                    javafx.application.Platform.runLater(() -> {
                        bulkRef[0].generationFinished(false, "Error: " + ex.getMessage());
                        bulkStatusLabel.setText("Bulk Status: Error");
                        bulkStatusLabel.getStyleClass().removeAll("danger-text", "success-text");
                        bulkStatusLabel.getStyleClass().add("danger-text");
                    });
                }
            }, "bulk-generate").start();

            return null;
        }, r -> {});
        bulkRef[0].show();
    }

    private void exportSingle() {
        if (templateFile == null) return;
        if (!requireOutputFolder()) return;

        var replacements = currentReplacements();
        try {
            var svc = new ExportService();
            svc.export(templateFile, replacements, outputFolder.toPath(), selectedFormat());
        } catch (Exception ex) {
            Dialogs.showError(owner(), "Export failed", ex.getMessage());
        }
    }

    private void printSingle() {
        if (templateFile == null) return;
        if (!requireOutputFolder()) return;

        var replacements = currentReplacements();
        try {
            var svc = new PrintService();
            svc.print(templateFile, replacements, selectedFormat());
        } catch (Exception ex) {
            Dialogs.showError(owner(), "Print failed", ex.getMessage());
        }
    }

    private ExportService.OutputFormat selectedFormat() {
        return ".pdf".equals(formatDropdown.getSelectionModel().getSelectedItem())
                ? ExportService.OutputFormat.PDF
                : ExportService.OutputFormat.DOCX;
    }

    private void updateActionButtonsEnabled() {
        boolean hasTemplate = templateFile != null;
        boolean hasOutput = outputFolder != null && outputFolder.isDirectory();
        boolean hasAllFields = hasTemplate
                && !inputByPlaceholder.isEmpty()
                && inputByPlaceholder.values().stream().allMatch(tf -> !tf.getText().isBlank());

        exportButton.setDisable(!hasAllFields || !hasOutput);
        printButton.setDisable(!hasAllFields || !hasOutput);
    }

    private Map<String, String> currentReplacements() {
        var m = new LinkedHashMap<String, String>();
        for (var e : inputByPlaceholder.entrySet()) {
            m.put(e.getKey(), e.getValue().getText());
        }
        return m;
    }

    private void renderPlaceholders(Set<String> placeholders) {
        inputByPlaceholder.clear();
        inputsContainer.getChildren().clear();

        if (placeholders == null || placeholders.isEmpty()) {
            root.setCenter(emptyState);
            return;
        }

        for (var p : placeholders) {
            var label = new Label(p);
            label.getStyleClass().add("field-label");

            var tf = new TextField();
            tf.getStyleClass().add("field-input");
            tf.setPromptText("Enter " + p + "...");
            tf.textProperty().addListener((obs, oldV, newV) -> updateActionButtonsEnabled());

            var row = new VBox(label, tf);
            row.setSpacing(6);
            inputsContainer.getChildren().add(row);
            inputByPlaceholder.put(p, tf);
        }

        var centerWrap = new BorderPane(inputsScrollPane);
        centerWrap.setPadding(new Insets(30));
        root.setCenter(centerWrap);
    }

    private static String truncateMiddle(String s, int maxLen) {
        if (s == null) return "";
        if (s.length() <= maxLen) return s;
        int keep = Math.max(2, (maxLen - 3) / 2);
        return s.substring(0, keep) + "..." + s.substring(s.length() - keep);
    }
}
