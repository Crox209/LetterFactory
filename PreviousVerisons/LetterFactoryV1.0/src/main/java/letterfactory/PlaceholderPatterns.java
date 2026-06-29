package letterfactory;

import java.util.LinkedHashSet;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Shared [[placeholder]] syntax used across template scan and replacement. */
public final class PlaceholderPatterns {
    public static final Pattern PLACEHOLDER = Pattern.compile("\\[\\[([^\\]]+?)\\]\\]");
    public static final Pattern UNRESOLVED = Pattern.compile("\\[\\[[^\\]]+?\\]\\]");

    private PlaceholderPatterns() {}

    public static String token(String innerName) {
        return "[[" + innerName + "]]";
    }

    public static Set<String> scanText(String text) {
        var out = new LinkedHashSet<String>();
        if (text == null || text.isBlank()) return out;
        Matcher m = PLACEHOLDER.matcher(text);
        while (m.find()) {
            String inner = m.group(1);
            if (inner == null) continue;
            String trimmed = inner.trim();
            if (!trimmed.isEmpty()) out.add(trimmed);
        }
        return out;
    }

    public static boolean containsUnresolved(String text) {
        return text != null && UNRESOLVED.matcher(text).find();
    }
}
