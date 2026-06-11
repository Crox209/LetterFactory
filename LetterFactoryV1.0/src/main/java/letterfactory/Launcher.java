package letterfactory;

import javafx.application.Application;

public final class Launcher {
    private Launcher() {}

    public static void main(String[] args) {
        try {
            JavaFxBootstrap.init(args);
            Application.launch(MainApp.class, args);
        } catch (Exception e) {
            JavaFxBootstrap.showLaunchError(e);
            System.exit(1);
        }
    }
}
