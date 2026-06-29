package letterfactory;

import org.docx4j.TraversalUtil;
import org.docx4j.finders.ClassFinder;
import org.docx4j.openpackaging.packages.WordprocessingMLPackage;
import org.docx4j.openpackaging.parts.JaxbXmlPart;
import org.docx4j.openpackaging.parts.Part;
import org.docx4j.wml.DelText;
import org.docx4j.wml.P;
import org.docx4j.wml.Text;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * Replaces [[placeholders]] anywhere in the OPC package (body, headers, footers, notes, SDTs, tables…).
 * Split-run safe: only runs that belong to a matched placeholder are modified.
 */
public final class Docx4jPlaceholderReplacer {

    public Set<String> collectPlaceholders(WordprocessingMLPackage pkg) {
        var out = new LinkedHashSet<String>();
        Objects.requireNonNull(pkg, "pkg");
        forEachParagraph(pkg, paragraph -> out.addAll(PlaceholderPatterns.scanText(combineParagraphText(paragraph))));
        return out;
    }

    public void apply(WordprocessingMLPackage pkg, Map<String, String> replacements) {
        Objects.requireNonNull(pkg, "pkg");
        Objects.requireNonNull(replacements, "replacements");
        if (replacements.isEmpty()) return;
        forEachParagraph(pkg, paragraph -> replaceInParagraph(paragraph, replacements));
    }

    private void forEachParagraph(WordprocessingMLPackage pkg, java.util.function.Consumer<P> action) {
        for (Part part : pkg.getParts().getParts().values()) {
            if (!(part instanceof JaxbXmlPart<?> jaxbPart)) continue;
            Object root;
            try {
                root = jaxbPart.getContents();
            } catch (Exception e) {
                continue;
            }
            if (root == null) continue;

            ClassFinder finder = new ClassFinder(P.class);
            new TraversalUtil(root, finder);
            for (Object o : finder.results) {
                if (o instanceof P paragraph) {
                    action.accept(paragraph);
                }
            }
        }
    }

    private void replaceInParagraph(P paragraph, Map<String, String> replacements) {
        List<TextSlice> slices = collectTextSlices(paragraph);
        if (slices.isEmpty()) return;

        boolean changed;
        do {
            changed = false;
            String combined = combine(slices);
            if (combined.isEmpty()) break;

            for (Map.Entry<String, String> entry : replacements.entrySet()) {
                String token = PlaceholderPatterns.token(entry.getKey());
                int idx = combined.indexOf(token);
                if (idx >= 0) {
                    String repl = entry.getValue() == null ? "" : entry.getValue();
                    replaceTokenRange(slices, idx, idx + token.length(), repl);
                    changed = true;
                    break;
                }
            }
        } while (changed);
    }

    private static String combineParagraphText(P paragraph) {
        return combine(collectTextSlices(paragraph));
    }

    private static List<TextSlice> collectTextSlices(P paragraph) {
        List<TextSlice> slices = new ArrayList<>();
        new TraversalUtil(paragraph, new TraversalUtil.CallbackImpl() {
            @Override
            public List<Object> apply(Object o) {
                if (o instanceof Text text) {
                    slices.add(new TextSlice(text));
                } else if (o instanceof DelText delText) {
                    slices.add(new TextSlice(delText));
                }
                return null;
            }
        });
        return slices;
    }

    private static String combine(List<TextSlice> slices) {
        StringBuilder sb = new StringBuilder();
        for (TextSlice slice : slices) {
            String value = slice.getValue();
            if (value != null && !value.isEmpty()) {
                sb.append(value);
            }
        }
        return sb.toString();
    }

    private static void replaceTokenRange(List<TextSlice> slices, int start, int end, String replacement) {
        int charPos = 0;
        int beginIdx = -1;
        int beginChar = 0;
        int endIdx = -1;
        int endChar = 0;

        for (int i = 0; i < slices.size(); i++) {
            String value = nullToEmpty(slices.get(i).getValue());
            int len = value.length();

            if (beginIdx < 0 && start < charPos + len) {
                beginIdx = i;
                beginChar = start - charPos;
            }
            if (beginIdx >= 0 && end <= charPos + len) {
                endIdx = i;
                endChar = end - charPos - 1;
                break;
            }
            charPos += len;
        }

        if (beginIdx < 0 || endIdx < 0) return;

        if (beginIdx == endIdx) {
            String value = nullToEmpty(slices.get(beginIdx).getValue());
            String updated = value.substring(0, Math.min(beginChar, value.length()))
                    + replacement
                    + value.substring(Math.min(endChar + 1, value.length()));
            slices.get(beginIdx).setValue(updated);
            return;
        }

        String beginValue = nullToEmpty(slices.get(beginIdx).getValue());
        String endValue = nullToEmpty(slices.get(endIdx).getValue());
        String beginPrefix = beginValue.substring(0, Math.min(beginChar, beginValue.length()));
        String endSuffix = endValue.substring(Math.min(endChar + 1, endValue.length()));
        slices.get(beginIdx).setValue(beginPrefix + replacement + endSuffix);

        for (int i = beginIdx + 1; i <= endIdx; i++) {
            slices.get(i).setValue("");
        }
    }

    private static String nullToEmpty(String s) {
        return s == null ? "" : s;
    }

    private static final class TextSlice {
        private final Object node;

        TextSlice(Object node) {
            this.node = node;
        }

        String getValue() {
            if (node instanceof Text text) return text.getValue();
            if (node instanceof DelText delText) return delText.getValue();
            return "";
        }

        void setValue(String value) {
            if (node instanceof Text text) text.setValue(value);
            else if (node instanceof DelText delText) delText.setValue(value);
        }
    }
}
