package letterfactory;

import javafx.scene.Node;
import javafx.scene.input.DragEvent;
import javafx.scene.input.Dragboard;
import javafx.scene.input.TransferMode;

import java.io.File;
import java.util.function.Consumer;

public final class FileDropHelper {
    private FileDropHelper() {}

    public static void enableFileDrop(Node node, String extension, Consumer<File> onFile) {
        node.setOnDragOver(e -> {
            if (hasMatchingFile(e.getDragboard(), extension)) {
                e.acceptTransferModes(TransferMode.COPY);
            }
            e.consume();
        });
        node.setOnDragEntered(e -> {
            if (hasMatchingFile(e.getDragboard(), extension)) {
                if (!node.getStyleClass().contains("drop-target-active")) {
                    node.getStyleClass().add("drop-target-active");
                }
            }
            e.consume();
        });
        node.setOnDragExited(e -> {
            node.getStyleClass().remove("drop-target-active");
            e.consume();
        });
        node.setOnDragDropped(e -> {
            node.getStyleClass().remove("drop-target-active");
            File f = firstMatchingFile(e.getDragboard(), extension);
            if (f != null) {
                onFile.accept(f);
            }
            e.consume();
        });
    }

    private static boolean hasMatchingFile(Dragboard db, String extension) {
        return firstMatchingFile(db, extension) != null;
    }

    private static File firstMatchingFile(Dragboard db, String extension) {
        if (db == null || !db.hasFiles()) return null;
        String ext = extension.startsWith(".") ? extension.toLowerCase() : ("." + extension).toLowerCase();
        for (File f : db.getFiles()) {
            if (f.isFile() && f.getName().toLowerCase().endsWith(ext)) {
                return f;
            }
        }
        return null;
    }
}
