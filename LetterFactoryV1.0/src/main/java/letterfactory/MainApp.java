package letterfactory;

import javafx.application.Application;
import javafx.scene.Scene;
import javafx.stage.Stage;

public final class MainApp extends Application {

    @Override
    public void start(Stage stage) {
        var root = new MainWindow().getRoot();
        var scene = new Scene(root, 900, 700);
        scene.getStylesheets().add(MainApp.class.getResource("/ui/theme.css").toExternalForm());

        stage.setTitle("LetterFactory");
        stage.setMinWidth(900);
        stage.setMinHeight(700);
        stage.setScene(scene);
        stage.show();
    }

    public static void main(String[] args) {
        launch(args);
    }
}

