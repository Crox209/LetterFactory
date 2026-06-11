package letterfactory;

import java.io.File;
import java.net.URI;
import java.nio.file.Path;

public final class AppPaths {
    private AppPaths() {}

    public static Path appDir() {
        try {
            URI uri = AppPaths.class.getProtectionDomain().getCodeSource().getLocation().toURI();
            File f = new File(uri);
            File dir = f.isFile() ? f.getParentFile() : f;
            if (dir != null && dir.exists()) return dir.toPath();
        } catch (Exception ignore) {
        }
        return Path.of(System.getProperty("user.dir", ".")).toAbsolutePath().normalize();
    }
}

