package letterfactory;

import org.docx4j.XmlUtils;
import org.docx4j.jaxb.Context;
import org.docx4j.openpackaging.packages.WordprocessingMLPackage;
import org.docx4j.wml.Body;
import org.docx4j.wml.ObjectFactory;
import org.docx4j.wml.P;
import org.docx4j.wml.PPr;
import org.docx4j.wml.PPrBase;
import org.docx4j.wml.R;
import org.docx4j.wml.SectPr;
import org.docx4j.wml.Text;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Merges completed DOCX files that were all generated from the same template.
 * <p>
 * Each personalized letter is created in full first (Gen 1, Gen 2, …). The merge copies every
 * letter's body content into one document so the spacing, sections, images and footers inside
 * each letter are exactly what was generated. Letters are separated by a next-page section break
 * so each recipient starts on a fresh page.
 * <p>
 * Word templates often end with vestigial trailing filler (empty paragraphs with very large
 * "space after" plus a final section) that renders as blank space only at the very end of a
 * document. When another letter is appended after it, that filler suddenly becomes extra blank
 * pages. So for every letter except the last, trailing empty filler paragraphs are removed and
 * the letter's real final section becomes the page break to the next recipient.
 * <p>
 * Because every file comes from the same template, all of them share identical relationship ids
 * (images, footers), so copied content resolves against the first document's parts without any
 * relationship remapping. The result is a finalized .docx that opens correctly everywhere.
 */
public final class DocxPackageMerger {
    private static final ObjectFactory WML = Context.getWmlObjectFactory();
    private static final BigInteger LARGE_SPACE_AFTER = BigInteger.valueOf(2000);

    private DocxPackageMerger() {}

    public static void mergeDocuments(List<byte[]> documents, Path outputPath) throws Exception {
        if (documents == null || documents.isEmpty()) {
            throw new IOException("No documents to merge.");
        }
        mergeBlocks(documents, outputPath);
    }

    public static void mergeDocumentFiles(List<Path> documentFiles, Path outputPath) throws Exception {
        if (documentFiles == null || documentFiles.isEmpty()) {
            throw new IOException("No documents to merge.");
        }
        List<byte[]> blocks = new ArrayList<>(documentFiles.size());
        for (Path file : documentFiles) {
            blocks.add(Files.readAllBytes(file));
        }
        mergeBlocks(blocks, outputPath);
    }

    private static void mergeBlocks(List<byte[]> documents, Path outputPath) throws Exception {
        WordprocessingMLPackage merged = WordprocessingMLPackage.load(new ByteArrayInputStream(documents.get(0)));
        Body mergedBody = merged.getMainDocumentPart().getJaxbElement().getBody();

        if (documents.size() == 1) {
            merged.save(outputPath.toFile());
            return;
        }

        int lastIndex = documents.size() - 1;

        // First letter: it is already the host body. Turn its tail into a page break to letter 2.
        SectPr firstBodySect = mergedBody.getSectPr();
        mergedBody.setSectPr(null);
        closeLetterWithPageBreak(mergedBody.getContent(), firstBodySect);

        for (int i = 1; i < documents.size(); i++) {
            WordprocessingMLPackage next = WordprocessingMLPackage.load(new ByteArrayInputStream(documents.get(i)));
            Body nextBody = next.getMainDocumentPart().getJaxbElement().getBody();

            List<Object> letter = new ArrayList<>();
            for (Object item : nextBody.getContent()) {
                letter.add(XmlUtils.deepCopy(item));
            }

            if (i == lastIndex) {
                // Keep the final letter verbatim so it renders exactly like a single export.
                mergedBody.getContent().addAll(letter);
                if (nextBody.getSectPr() != null) {
                    mergedBody.setSectPr(XmlUtils.deepCopy(nextBody.getSectPr()));
                }
            } else {
                closeLetterWithPageBreak(letter, nextBody.getSectPr());
                mergedBody.getContent().addAll(letter);
            }
        }

        merged.save(outputPath.toFile());
    }

    /**
     * Prepares one letter's content so the next recipient starts on a new page:
     * drops trailing empty filler paragraphs, then makes the letter's last section a next-page break.
     */
    private static void closeLetterWithPageBreak(List<Object> content, SectPr bodySectPr) {
        removeTrailingFillerParagraphs(content);

        P tail = lastParagraph(content);
        if (tail != null && hasSectionProperties(tail)) {
            // The letter's real final section becomes the boundary to the next letter.
            SectPr sectPr = tail.getPPr().getSectPr();
            setNextPage(sectPr);
            zeroSpaceAfter(tail);
            return;
        }

        // No usable trailing section: append a minimal next-page break carrying the page setup.
        P breakParagraph = WML.createP();
        PPr pPr = WML.createPPr();
        pPr.setSectPr(nextPageBreak(bodySectPr));
        breakParagraph.setPPr(pPr);
        content.add(breakParagraph);
    }

    /** Removes trailing empty paragraphs that carry no section break (template end-of-page filler). */
    private static void removeTrailingFillerParagraphs(List<Object> content) {
        while (!content.isEmpty()) {
            Object last = content.get(content.size() - 1);
            if (last instanceof P p && isEmptyParagraph(p) && !hasSectionProperties(p)) {
                content.remove(content.size() - 1);
                continue;
            }
            break;
        }
    }

    private static P lastParagraph(List<Object> content) {
        for (int i = content.size() - 1; i >= 0; i--) {
            if (content.get(i) instanceof P p) {
                return p;
            }
            return null;
        }
        return null;
    }

    private static boolean hasSectionProperties(P paragraph) {
        return paragraph.getPPr() != null && paragraph.getPPr().getSectPr() != null;
    }

    private static boolean isEmptyParagraph(P paragraph) {
        return paragraphText(paragraph).isEmpty();
    }

    private static void zeroSpaceAfter(P paragraph) {
        PPr pPr = paragraph.getPPr();
        if (pPr == null) {
            return;
        }
        PPrBase.Spacing spacing = pPr.getSpacing();
        if (spacing != null && spacing.getAfter() != null
                && spacing.getAfter().compareTo(LARGE_SPACE_AFTER) >= 0) {
            spacing.setAfter(BigInteger.ZERO);
        }
    }

    private static SectPr nextPageBreak(SectPr basis) {
        SectPr sectPr = basis != null ? XmlUtils.deepCopy(basis) : WML.createSectPr();
        setNextPage(sectPr);
        return sectPr;
    }

    private static void setNextPage(SectPr sectPr) {
        if (sectPr == null) {
            return;
        }
        SectPr.Type type = WML.createSectPrType();
        type.setVal("nextPage");
        sectPr.setType(type);
    }

    private static String paragraphText(P paragraph) {
        StringBuilder sb = new StringBuilder();
        collectText(paragraph, sb);
        return sb.toString().trim();
    }

    private static void collectText(Object node, StringBuilder sb) {
        Object value = XmlUtils.unwrap(node);
        if (value instanceof Text text && text.getValue() != null) {
            sb.append(text.getValue());
            return;
        }
        if (value instanceof R r) {
            for (Object child : r.getContent()) {
                collectText(child, sb);
            }
        } else if (value instanceof P p) {
            for (Object child : p.getContent()) {
                collectText(child, sb);
            }
        } else if (value instanceof org.docx4j.wml.ContentAccessor ca) {
            for (Object child : ca.getContent()) {
                collectText(child, sb);
            }
        }
    }
}
