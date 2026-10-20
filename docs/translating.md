# Translating ORQWA

ORQWA is written in English and translated with gettext files in `app/translations/`
(R24). German (`de.po`, Swiss spelling) is the first translation.
The German catalogue uses informal **du**, Swiss **ss** spelling, and the wording reviewed
individually in [issue #261](https://github.com/ORQWA/ORQWA/issues/261).
The approved wording is maintained directly in `de.po`, including retained wording, contexts
and subsequent corrections. German quotation marks are straight double quotes. Preserve
explicitly approved exceptions to the general terminology.

Rider language selection is described in the [wiki](https://github.com/ORQWA/ORQWA/wiki/Riders#profile).

## Adding or updating a language

1. In the dev container, refresh the template after code changes:
   `python3 scripts/i18n/extract.py` → `app/translations/orqwa.pot`.
2. Copy `orqwa.pot` to `<code>.po` (e.g. `fr.po`), set `"Language: fr\n"` in the header and
   fill in every `msgstr`. Keep placeholders like `%s`, `%d`, `%.1f` and `%%` — the check fails
   if they differ.
3. Add the file to `internationalization/locale/translations` in `app/project.godot` and the
   language to `ProfileDialog.LANGUAGES`.
4. `scripts/check.sh` runs `extract.py --check`: the template must be current and every
   translation complete.

## For developers

- Wrap user-facing text in `tr("…")` (or `TranslationServer.translate("…")` in static
  functions) *before* formatting: `tr("Saved %s") % name`.
- Texts in scenes (`text`, `tooltip_text`, `title`, …) are found automatically.
- Lists of names (zones, weather, figures sent from the core) go between `i18n-begin` and
  `i18n-end` comments — in GDScript and in Rust; the front end translates them when shown.
- Texts built at run time go into `app/translations/extra-msgids.txt`.
- Names of people, routes, courses and devices are never translated: switch automatic
  translation off on controls that show them.
- Use gettext contexts when one English word has different meanings:
  `tr("Ride", "ride_action")` is **Fahren**, while `tr("Ride", "ride_tab")` is **Fahrt**.
  Keep `msgctxt` in the PO entry. Extraction and completeness checks include the context.
- Core failures carry `failure::Failure`: a `Kind` with an English message key, untouched
  arguments and a separate original diagnostic. Choose the kind where the operation fails;
  never classify errors by matching their formatted English text. Register templates in
  `Kind::message`'s extraction region. The Godot boundary translates before inserting values.
- The existing `failed`, `account_failed` and `upload_failed` signals carry localized summaries.
  `failure_reported` carries `{message, args, details}` and `last_failure()` returns its latest
  payload for inline controls. `FailureDetails` retains up to 20 failures during the session
  and translates their summaries again if the rider changes language. Technical diagnostics
  remain untranslated and are logged; they are shown separately under **Technical details**.
- Most of the interface is built before the rider's profile chooses the language. Leave a
  label's automatic translation on with its English text (as `UiTheme.caption()` does, which
  upper-cases only when drawing), and redo formatted texts on `NOTIFICATION_TRANSLATION_CHANGED`.
- Run `python3 -m unittest discover -s scripts/i18n` in the dev container for context and
  escaping checks, and the `error_smoke.gd` Godot test for the boundary/UI.
