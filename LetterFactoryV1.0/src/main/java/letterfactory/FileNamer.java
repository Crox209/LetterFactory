package letterfactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

public final class FileNamer {
    private FileNamer() {}

    public static String sanitize(String s) {
        if (s == null) return "";
        // Windows illegal: \ / : * ? " < > |
        String sanitized = s.replaceAll("[\\\\/:*?\"<>|]", "_");
        sanitized = sanitized.replaceAll("_+", "_");
        return sanitized.replaceAll("^_+|_+$", "").trim();
    }

    public static String buildNameFromParts(List<String> placeholderParts, Map<String, String> values) {
        if (placeholderParts == null || placeholderParts.isEmpty()) return "";
        StringBuilder sb = new StringBuilder();
        for (String p : placeholderParts) {
            sb.append(sanitize(values.getOrDefault(p, "")));
        }
        return sb.toString();
    }

    public static String ensureUnique(String base, Set<String> used, int indexWidth) {
        String b = (base == null || base.isBlank()) ? "Document" : base;
        String name = b;
        int i = 1;
        while (used.contains(name)) {
            name = b + "_" + String.format("%0" + indexWidth + "d", i++);
        }
        used.add(name);
        return name;
    }

    public static List<String> normalizeSelectedParts(List<String> selected, List<String> templateOrder) {
        if (selected == null || selected.isEmpty()) return List.of();
        if (templateOrder == null || templateOrder.isEmpty()) return new ArrayList<>(selected);
        var out = new ArrayList<String>();
        for (String p : templateOrder) {
            if (selected.contains(p)) out.add(p);
        }
        return out;
    }
}

