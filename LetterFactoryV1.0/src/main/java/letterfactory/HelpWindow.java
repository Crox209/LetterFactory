package letterfactory;

import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Scene;
import javafx.scene.control.Label;
import javafx.scene.control.ScrollPane;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.VBox;
import javafx.stage.Modality;
import javafx.stage.Stage;
import javafx.stage.Window;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

public final class HelpWindow {
    private HelpWindow() {}

    public static void show(Window owner) {
        var stage = new Stage();
        stage.initOwner(owner);
        stage.initModality(Modality.NONE);
        stage.setTitle("Letter Factory — Help");
        stage.setWidth(800);
        stage.setHeight(700);

        var root = new BorderPane();
        root.getStyleClass().add("help-root");

        var header = new VBox();
        header.setAlignment(Pos.CENTER);
        header.setPadding(new Insets(20, 20, 10, 20));
        header.setSpacing(6);

        var title = new Label("LetterFactory");
        title.getStyleClass().add("help-title");
        var subtitle = new Label("User Guide");
        subtitle.getStyleClass().add("help-subtitle");
        header.getChildren().addAll(title, subtitle);

        var content = new Label(loadHelpText());
        content.getStyleClass().add("help-content");
        content.setWrapText(true);
        content.setStyle("-fx-text-fill: #000000; -fx-background-color: #FFFFFF;");

        var scroll = new ScrollPane(content);
        scroll.getStyleClass().add("help-scroll");
        scroll.setFitToWidth(true);
        scroll.setPadding(new Insets(0));

        var footer = new VBox();
        footer.setAlignment(Pos.CENTER);
        footer.setPadding(new Insets(14));
        footer.getStyleClass().add("help-footer");

        var credit = new Label("Developed by Ethan Silvio");
        credit.getStyleClass().add("help-credit");
        var version = new Label("Version 1.0");
        version.getStyleClass().add("help-version");
        footer.getChildren().addAll(credit, version);

        root.setTop(header);
        root.setCenter(scroll);
        root.setBottom(footer);

        var scene = new Scene(root);
        scene.getStylesheets().add(HelpWindow.class.getResource("/ui/theme.css").toExternalForm());
        stage.setScene(scene);
        stage.show();
    }

    private static String loadHelpText() {
        try (InputStream in = HelpWindow.class.getResourceAsStream("/help/user_guide.txt")) {
            if (in == null) return "Help content not found.";
            return new String(in.readAllBytes(), StandardCharsets.UTF_8);
        } catch (IOException e) {
            return "Unable to load help content: " + e.getMessage();
        }
    }
}

