package letterfactory;

import java.io.IOException;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;

/**
 * Loads JavaFX for the current OS from the fat jar, then re-launches with {@code --module-path}.
 */
public final class JavaFxBootstrap {
    private static final String BOOTSTRAPPED = "letterfactory.javafx.bootstrapped";
    private static final String[] MODULES = {"javafx-base.jar", "javafx-graphics.jar", "javafx-controls.jar"};
    private static final long MIN_MODULE_BYTES = 200_000L;

    private JavaFxBootstrap() {}

    public static void init(String[] args) throws Exception {
        if (Boolean.getBoolean(BOOTSTRAPPED)) {
            return;
        }

        String platform = detectPlatform();
        Path jarPath = resolveJarPath();
        Path modulePath = ensureJavaFxModules(jarPath, platform);
        relaunch(jarPath, modulePath, args);
    }

    public static String detectPlatform() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);

        if (os.contains("win")) {
            return "win";
        }
        if (os.contains("mac")) {
            return (arch.contains("aarch64") || arch.contains("arm64") || arch.contains("arm")) ? "mac-aarch64" : "mac";
        }
        if (os.contains("nux") || os.contains("nix")) {
            return (arch.contains("aarch64") || arch.contains("arm64") || arch.contains("arm"))
                    ? "linux-aarch64"
                    : "linux";
        }
        throw new UnsupportedOperationException("Unsupported platform: " + os + " / " + arch);
    }

    public static void showLaunchError(Throwable error) {
        String message = rootMessage(error);
        logLine("Launch failed: " + message);
        logThrowable(error);

        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("mac")) {
            macAlert("LetterFactory could not start.\n\n" + message + "\n\nDetails: " + logFilePath());
        } else if (os.contains("win")) {
            try {
                javax.swing.JOptionPane.showMessageDialog(
                        null,
                        message + "\n\nSee log: " + logFilePath(),
                        "LetterFactory",
                        javax.swing.JOptionPane.ERROR_MESSAGE
                );
            } catch (Throwable ignored) {
                // headless
            }
        }
    }

    private static Path ensureJavaFxModules(Path jarPath, String platform) throws IOException {
        IOException last = null;
        for (String candidate : platformCandidates(platform)) {
            Path cacheDir = cacheDirectory(jarPath, candidate);
            try {
                Files.createDirectories(cacheDir);
                extractModulesFromJar(jarPath, candidate, cacheDir);
                verifyModules(cacheDir);
                return cacheDir;
            } catch (IOException e) {
                last = e;
                wipeCache(cacheDir);
            }
        }
        throw last != null ? last : new IOException("No embedded JavaFX runtime for platform: " + platform);
    }

    private static void wipeCache(Path cacheDir) {
        try {
            if (!Files.isDirectory(cacheDir)) {
                return;
            }
            for (String module : MODULES) {
                Files.deleteIfExists(cacheDir.resolve(module));
            }
        } catch (IOException ignored) {
        }
    }

    private static Path cacheDirectory(Path jarPath, String platform) {
        Path besideJar = jarPath.getParent().resolve(".letterfactory-javafx").resolve(platform);
        if (Files.isWritable(jarPath.getParent())) {
            return besideJar;
        }
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("mac")) {
            return Paths.get(System.getProperty("user.home"), "Library", "Application Support", "LetterFactory", "javafx", platform);
        }
        if (os.contains("win")) {
            return Paths.get(System.getenv().getOrDefault("LOCALAPPDATA", System.getProperty("user.home")),
                    "LetterFactory", "javafx", platform);
        }
        return Paths.get(System.getProperty("user.home"), ".letterfactory", "javafx", platform);
    }

    private static void extractModulesFromJar(Path jarPath, String platform, Path cacheDir) throws IOException {
        try (JarFile jar = new JarFile(jarPath.toFile())) {
            for (String module : MODULES) {
                Path dest = cacheDir.resolve(module);
                if (isValidModule(dest)) {
                    continue;
                }
                String entryName = "javafx-libs/" + platform + "/" + module;
                JarEntry entry = jar.getJarEntry(entryName);
                if (entry == null) {
                    throw new IOException("Missing entry in jar: " + entryName);
                }
                Files.createDirectories(dest.getParent());
                try (var in = jar.getInputStream(entry)) {
                    Files.copy(in, dest, StandardCopyOption.REPLACE_EXISTING);
                }
                if (!isValidModule(dest)) {
                    throw new IOException("Extracted module is invalid: " + dest);
                }
            }
        }
    }

    private static void verifyModules(Path cacheDir) throws IOException {
        for (String module : MODULES) {
            Path file = cacheDir.resolve(module);
            if (!isValidModule(file)) {
                throw new IOException("JavaFX module missing or corrupt: " + file);
            }
        }
    }

    private static boolean isValidModule(Path file) {
        try {
            return Files.isRegularFile(file) && Files.size(file) >= MIN_MODULE_BYTES;
        } catch (IOException e) {
            return false;
        }
    }

    private static void relaunch(Path jarPath, Path modulePath, String[] args) throws Exception {
        String javaBin = resolveJavaBinary();
        List<String> cmd = new ArrayList<>();
        cmd.add(javaBin);
        cmd.add("-D" + BOOTSTRAPPED + "=true");
        cmd.add("--enable-native-access=javafx.graphics");
        cmd.add("--module-path");
        cmd.add(modulePath.toAbsolutePath().toString());
        cmd.add("--add-modules");
        cmd.add("javafx.controls");
        cmd.add("-jar");
        cmd.add(jarPath.toAbsolutePath().toString());
        if (args != null) {
            for (String arg : args) {
                cmd.add(arg);
            }
        }

        Path logFile = logFilePath();
        Files.createDirectories(logFile.getParent());
        logLine("Relaunching: " + String.join(" ", cmd));

        ProcessBuilder builder = new ProcessBuilder(cmd)
                .redirectErrorStream(true)
                .redirectOutput(ProcessBuilder.Redirect.appendTo(logFile.toFile()));

        Process process = builder.start();

        // Finder / double-click launches have no console; waiting would block the stub and can kill the UI child.
        if (System.console() == null) {
            logLine("Detached GUI process (pid=" + process.pid() + ")");
            Runtime.getRuntime().halt(0);
        }

        int code = process.waitFor();
        logLine("Child process exited with code " + code);
        System.exit(code);
    }

    private static String resolveJavaBinary() {
        return ProcessHandle.current()
                .info()
                .command()
                .orElseGet(() -> {
                    String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
                    String bin = Path.of(System.getProperty("java.home"), "bin", "java").toString();
                    return os.contains("win") ? bin + ".exe" : bin;
                });
    }

    private static Path resolveJarPath() throws Exception {
        var codeSource = JavaFxBootstrap.class.getProtectionDomain().getCodeSource();
        if (codeSource == null || codeSource.getLocation() == null) {
            throw new IllegalStateException("Could not locate the application jar.");
        }
        Path path = Paths.get(codeSource.getLocation().toURI());
        if (Files.isDirectory(path)) {
            throw new IllegalStateException(
                    "LetterFactory is running from compiled classes, not a jar. "
                            + "Use: mvn -Djavafx.platform=" + detectPlatform() + " javafx:run"
            );
        }
        return path.toAbsolutePath().normalize();
    }

    private static List<String> platformCandidates(String platform) {
        return switch (platform) {
            case "linux-aarch64" -> List.of("linux-aarch64", "linux");
            default -> List.of(platform);
        };
    }

    private static Path logFilePath() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("mac")) {
            return Paths.get(System.getProperty("user.home"), "Library", "Logs", "LetterFactory", "launch.log");
        }
        if (os.contains("win")) {
            return Paths.get(System.getenv().getOrDefault("LOCALAPPDATA", System.getProperty("user.home")),
                    "LetterFactory", "launch.log");
        }
        return Paths.get(System.getProperty("user.home"), ".letterfactory", "launch.log");
    }

    private static void logLine(String line) {
        try {
            Path log = logFilePath();
            Files.createDirectories(log.getParent());
            String entry = java.time.Instant.now() + " " + line + System.lineSeparator();
            Files.writeString(log, entry, java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
        } catch (IOException ignored) {
        }
    }

    private static void logThrowable(Throwable error) {
        StringWriter sw = new StringWriter();
        error.printStackTrace(new PrintWriter(sw));
        logLine(sw.toString());
    }

    private static void macAlert(String message) {
        String escaped = message.replace("\\", "\\\\").replace("\"", "\\\"");
        try {
            new ProcessBuilder("osascript", "-e", "display alert \"LetterFactory\" message \"" + escaped + "\" as critical")
                    .start();
        } catch (IOException ignored) {
        }
    }

    private static String rootMessage(Throwable error) {
        Throwable c = error;
        while (c.getCause() != null && c.getCause() != c) {
            c = c.getCause();
        }
        String msg = c.getMessage();
        return msg != null && !msg.isBlank() ? msg : c.getClass().getSimpleName();
    }
}
