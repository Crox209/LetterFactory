package letterfactory;

import javafx.scene.control.Alert;
import javafx.scene.control.ButtonType;
import javafx.stage.Window;

public final class Dialogs {
    private Dialogs() {}

    public static void showInfo(Window owner, String title, String message) {
        var alert = new Alert(Alert.AlertType.INFORMATION);
        alert.initOwner(owner);
        alert.setTitle(title);
        alert.setHeaderText(null);
        alert.setContentText(message);
        alert.showAndWait();
    }

    public static void showError(Window owner, String title, String message) {
        var alert = new Alert(Alert.AlertType.ERROR);
        alert.initOwner(owner);
        alert.setTitle(title);
        alert.setHeaderText(null);
        alert.setContentText(message);
        alert.showAndWait();
    }

    public static boolean confirm(Window owner, String title, String message) {
        var alert = new Alert(Alert.AlertType.CONFIRMATION, message, ButtonType.YES, ButtonType.NO);
        alert.initOwner(owner);
        alert.setTitle(title);
        alert.setHeaderText(null);
        var result = alert.showAndWait();
        return result.isPresent() && result.get() == ButtonType.YES;
    }
}
