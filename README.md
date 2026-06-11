# LetterFactory

Portable desktop app to generate personalized documents from a single `.docx` template using `[[Placeholders]]`, with optional bulk generation from Excel.

## Features

- **Template input**: load `.docx` templates only
- **Placeholder detection**: scans for `[[...]]` placeholders (case-sensitive) across:
  - body paragraphs
  - tables
  - headers/footers
  - Word text boxes (best-effort)
- **Single mode**: fill fields → export `.docx` / `.pdf` or print
- **Bulk mode**: upload `.xlsx`, validate headers, generate many files, custom filenames, merge into one (`.docx` or `.pdf`)

## Requirements

- Java 21+ installed on the machine that runs the app (works through Java 26).

- `LetterFactory.app` — macOS app, double-click to run
- `LetterFactory.jar` — universal, runs on any OS with `java -jar`
- `LetterFactory.exe` — self-contained Windows launcher (built via Launch4j)

## Bulk Excel format

- Row 1 must contain placeholder headers **including brackets**, e.g. `[[Name]]`, `[[Date]]`
- Each subsequent row generates one document
