package letterfactory;

import javafx.application.Platform;
import javafx.collections.FXCollections;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Scene;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.ScrollPane;
import javafx.scene.control.TextField;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.GridPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.stage.FileChooser;
import javafx.stage.Modality;
import javafx.stage.Stage;
import javafx.stage.Window;

import java.io.File;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Consumer;
import java.util.function.Function;

public final class BulkModeWindow {
    private static final DateTimeFormatter TS = DateTimeFormatter.ofPattern("yyyy-MM-dd_HHmmss");

    private final Stage stage;

    private final Label selectedFileLabel = new Label("No file selected");
    private final Label previewLine1 = new Label("Documents found: 0");
    private final Label previewLine2 = new Label("Upload an Excel file to begin.");

    private final GridPane placeholderGrid = new GridPane();
    private final ScrollPane placeholderScroll = new ScrollPane(placeholderGrid);
    private final Label previewFilenameLabel = new Label("Preview: (upload Excel)");

    private final CheckBox mergeAllCheck = new CheckBox("Merge all into one file");
    private final TextField mergedFilenameField = new TextField();

    private final Button generateAllButton = new Button("Generate All");
    private final ProgressBar progressBar = new ProgressBar(0);
    private final Label progressLabel = new Label("");

    private File excelFile;
    private int docCount;
    private Set<String> templatePlaceholders = Set.of();
    private List<String> selectedNameParts = new ArrayList<>();

    // Wiring for the engine
    private final Function<BulkUiState, Void> onGenerate;

    public BulkModeWindow(Window owner, Set<String> templatePlaceholders, Function<BulkUiState, Void> onGenerate, Consumer<BulkResult> onClose) {
        this.templatePlaceholders = templatePlaceholders != null ? templatePlaceholders : Set.of();
        this.onGenerate = onGenerate;

        stage = new Stage();
        stage.initOwner(owner);
        stage.initModality(Modality.APPLICATION_MODAL);
        stage.setTitle("Letter Factory — Bulk Mode");
        stage.setWidth(700);
        stage.setHeight(600);

        var root = new BorderPane();
        root.getStyleClass().add("bulk-root");
        root.setPadding(new Insets(25));

        root.setTop(buildTop());
        root.setCenter(buildCenter());
        root.setBottom(buildBottom(onClose));

        var scene = new Scene(root);
        scene.getStylesheets().add(BulkModeWindow.class.getResource("/ui/theme.css").toExternalForm());
        stage.setScene(scene);

        // Defaults
        mergedFilenameField.setText("Merged_" + TS.format(LocalDateTime.now()));
        mergedFilenameField.setDisable(true);
        placeholderGrid.setHgap(10);
        placeholderGrid.setVgap(12);
        placeholderScroll.setFitToWidth(true);
        placeholderScroll.setPrefHeight(220);

        progressBar.setVisible(false);
        progressLabel.setVisible(false);
        generateAllButton.setDisable(true);

        mergeAllCheck.selectedProperty().addListener((obs, oldV, newV) -> {
            placeholderScroll.setDisable(newV);
            mergedFilenameField.setDisable(!newV);
        });
    }

    public void show() {
        stage.showAndWait();
    }

    public void setPreview(int documentsFound, String message, boolean mismatch) {
        this.docCount = documentsFound;
        previewLine1.setText("Documents found: " + documentsFound);
        previewLine2.setText(message);
        previewLine2.getStyleClass().removeAll("danger-text", "success-text");
        previewLine2.getStyleClass().add(mismatch ? "danger-text" : "success-text");
    }

    public void setGenerateEnabled(boolean enabled) {
        generateAllButton.setDisable(!enabled);
    }

    public void updateProgress(int done, int total) {
        progressBar.setVisible(true);
        progressLabel.setVisible(true);
        progressLabel.setText("Progress: " + done + " / " + total);
        progressBar.setProgress(total <= 0 ? 0 : (done / (double) total));
    }

    public void generationFinished(boolean success, String message) {
        generateAllButton.setDisable(false);
        progressBar.setVisible(false);
        progressLabel.setVisible(false);
        previewLine2.setText(message);
        previewLine2.getStyleClass().removeAll("danger-text", "success-text");
        previewLine2.getStyleClass().add(success ? "success-text" : "danger-text");
    }

    private VBox buildTop() {
        var box = new VBox();
        box.setSpacing(10);

        var dropZone = new VBox();
        dropZone.getStyleClass().add("bulk-drop-zone");
        dropZone.setSpacing(8);
        dropZone.setAlignment(Pos.CENTER);

        var dropTitle = new Label("Drag and drop your Excel (.xlsx) here");
        dropTitle.getStyleClass().add("empty-state-title");

        var row = new HBox();
        row.setAlignment(Pos.CENTER);
        row.setSpacing(15);

        var uploadBtn = new Button("Upload Excel");
        uploadBtn.getStyleClass().add("surface-button");
        uploadBtn.setMinHeight(36);
        uploadBtn.setOnAction(e -> chooseExcel());

        selectedFileLabel.getStyleClass().add("status-label");

        row.getChildren().addAll(uploadBtn, selectedFileLabel);
        dropZone.getChildren().addAll(dropTitle, row);

        FileDropHelper.enableFileDrop(dropZone, ".xlsx", this::loadExcel);
        FileDropHelper.enableFileDrop(uploadBtn, ".xlsx", this::loadExcel);

        var preview = new VBox(previewLine1, previewLine2);
        preview.getStyleClass().add("bulk-preview");
        preview.setSpacing(6);
        preview.setPadding(new Insets(10));

        previewLine1.getStyleClass().add("bulk-preview-line1");
        previewLine2.getStyleClass().add("bulk-preview-line2");

        box.getChildren().addAll(dropZone, preview);
        return box;
    }

    private VBox buildCenter() {
        var center = new VBox();
        center.setSpacing(12);
        center.setPadding(new Insets(10, 0, 10, 0));

        var title = new Label("File Naming");
        title.getStyleClass().add("section-title");
        var desc = new Label("Select placeholders to build each filename.");
        desc.getStyleClass().add("status-label");

        previewFilenameLabel.getStyleClass().add("bulk-preview-filename");

        center.getChildren().addAll(title, desc, placeholderScroll, previewFilenameLabel);

        return center;
    }

    private VBox buildBottom(Consumer<BulkResult> onClose) {
        var bottom = new VBox();
        bottom.setSpacing(10);

        var mergeBox = new VBox();
        mergeBox.setSpacing(6);

        mergeAllCheck.getStyleClass().add("bulk-merge-check");

        var mergeHint = new Label("Combines all documents into a single .docx or .pdf");
        mergeHint.getStyleClass().add("status-label");

        var mergedRow = new HBox();
        mergedRow.setAlignment(Pos.CENTER_LEFT);
        mergedRow.setSpacing(10);

        var mergedLabel = new Label("Merged Filename");
        mergedLabel.getStyleClass().add("control-label");
        mergedFilenameField.getStyleClass().add("field-input");
        mergedFilenameField.setPrefWidth(300);
        mergedRow.getChildren().addAll(mergedLabel, mergedFilenameField);

        mergeBox.getChildren().addAll(mergeAllCheck, mergeHint, mergedRow);

        var actions = new HBox();
        actions.setAlignment(Pos.CENTER_LEFT);
        actions.setSpacing(15);

        generateAllButton.getStyleClass().addAll("action-button", "primary-button");
        generateAllButton.setMinHeight(44);
        generateAllButton.setMinWidth(180);
        generateAllButton.setOnAction(e -> {
            if (excelFile == null) return;
            if (onGenerate == null) return;
            generateAllButton.setDisable(true);
            progressBar.setVisible(true);
            progressLabel.setVisible(true);
            progressLabel.setText("Progress: 0 / 0");
            progressBar.setProgress(0);

            onGenerate.apply(new BulkUiState(
                    excelFile,
                    new ArrayList<>(selectedNameParts),
                    mergeAllCheck.isSelected(),
                    mergedFilenameField.getText()
            ));
        });

        var closeBtn = new Button("Close");
        closeBtn.getStyleClass().add("surface-button");
        closeBtn.setMinHeight(44);
        closeBtn.setMinWidth(100);
        closeBtn.setOnAction(e -> {
            stage.close();
            if (onClose != null) onClose.accept(new BulkResult(excelFile, docCount));
        });

        var spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        actions.getChildren().addAll(generateAllButton, spacer, closeBtn);

        progressBar.setPrefWidth(650);
        progressLabel.getStyleClass().add("status-label");

        bottom.getChildren().addAll(mergeBox, actions, progressLabel, progressBar);
        return bottom;
    }

    private void chooseExcel() {
        var chooser = new FileChooser();
        chooser.setTitle("Select an Excel file (.xlsx)");
        chooser.getExtensionFilters().add(new FileChooser.ExtensionFilter("Excel Workbook (*.xlsx)", "*.xlsx"));
        var f = chooser.showOpenDialog(stage);
        if (f == null) return;
        loadExcel(f);
    }

    private void loadExcel(File f) {
        if (f == null || !f.getName().toLowerCase().endsWith(".xlsx")) {
            Dialogs.showError(stage, "Invalid file", "Please drop or choose an .xlsx Excel file.");
            return;
        }
        excelFile = f;
        selectedFileLabel.setText(f.getName());

        buildPlaceholderChecklist(templatePlaceholders);
        docCount = 0;
        previewLine1.setText("Documents found: (validate on Generate)");
        previewLine2.setText("Ready to generate.");
        previewLine2.getStyleClass().removeAll("danger-text", "success-text");
        previewLine2.getStyleClass().add("success-text");
        generateAllButton.setDisable(false);
    }

    private void buildPlaceholderChecklist(Set<String> placeholders) {
        placeholderGrid.getChildren().clear();
        selectedNameParts = new ArrayList<>();

        var list = new ArrayList<>(placeholders != null ? placeholders : Set.<String>of());
        list.sort(String::compareTo);

        int cols = list.size() > 10 ? 3 : 1;
        int rows = (int) Math.ceil(list.size() / (double) cols);

        for (int i = 0; i < list.size(); i++) {
            String p = list.get(i);
            int col = i / rows;
            int row = i % rows;

            var cb = new CheckBox(p);
            cb.getStyleClass().add("bulk-placeholder-cb");
            cb.selectedProperty().addListener((obs, oldV, newV) -> {
                if (newV) {
                    selectedNameParts.add(p);
                } else {
                    selectedNameParts.remove(p);
                }
                updatePreviewFilename();
            });
            placeholderGrid.add(cb, col, row);
        }

        updatePreviewFilename();
    }

    private void updatePreviewFilename() {
        if (selectedNameParts.isEmpty()) {
            previewFilenameLabel.setText("Preview: (default naming)");
        } else {
            previewFilenameLabel.setText("Preview: " + String.join("", selectedNameParts) + ".docx");
        }
    }

    public record BulkResult(File excelFile, int documentsFound) {}

    public record BulkUiState(
            File excelFile,
            List<String> filenameParts,
            boolean mergeAllIntoOne,
            String mergedFilename
    ) {}
}

