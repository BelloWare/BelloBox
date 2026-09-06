# Bello Box

A macOS command palette for everyday utilities and developer tools.

Select text in **any** app — a note, an email, a code comment, a web form — and a
small Bello Box toolbar appears next to your selection. Press **⌃⌥⌘B** to open
the searchable command palette, with or without a selection. Relevant tools are
suggested from the selected text; use **Use Clipboard** to bring in clipboard
text explicitly. Search by name or keyword, navigate with **↑/↓**, and press
**Return** to open a tool. Star favorites, use **⌘K** to return to all commands,
or **Esc** to go back and close. The compact palette focuses search immediately
and dismisses when you click elsewhere. Drafts survive tool switching until the
palette is closed; recent commands remember tool names only.

Opening Bello Box from the Dock or Finder brings up **Home**, a workspace with
Overview, Developer, Capture, and Text & AI categories. Use **⌘1–⌘4** to switch
categories and **⌘K** to search all tools. The global shortcut opens the compact
palette from other apps. Home and the tools share adaptive graphite/indigo
surfaces, consistent controls, and color-coded icons in light and dark themes.
Choose **System**, **Light**, or **Dark** in Settings → General. Theme previews
show each option, and changes apply to windows that are already open. Shared
accent and status text colors are checked for contrast on both themes.

Long selections show a short preview and a character count while keeping the
complete input, up to 500 KB. Larger selections show a clear size notice and
open tools with empty input; they are never silently truncated. Clear the
selection or use clipboard text directly from the palette.

The floating toolbar and palette also provide:

- **Fix Spelling & Grammar**, **Improve Writing**, **Make Shorter**, switch to a
  **Professional** or **Friendly** tone, **Summarize**, **Explain**, or
  **Translate to English** — one click each.
- Capture an **area**, **window**, **screen**, or **scrolling page**, then
  annotate, redact, OCR, copy, or save the result.
- Start **screen recordings** from the same capture overlay, with configurable
  audio, cursor, click, keystroke, privacy, countdown, and quality defaults.
- Type your own instruction ("make this a bullet list", "rephrase as a tweet", …).
- Watch the answer stream in, then **Copy** it or **Replace** the original
  selection in place.

Open **Bello Box** from the menu bar for searchable access to screenshots, scrolling
capture, recording, World Clock, QR codes, and Text Tools. QR and Text Tools also
work with text you type or paste, without selecting text in another app.

**World Clock** follows the current time until you choose a planning time. Enter
an exact time, compare date changes across locations, then use **Copy Times** to
share the comparison. **Now** returns to the live clock.
The meeting planner and location cards use text and symbols as well as color
to distinguish working hours, fringe hours, and night. Press **⌘L** to add a
location, then type, use **↑/↓**, and press **Return**; **Esc** cancels. **⌘N**
returns to live time and **⇧⌘C** copies the comparison.

In the screenshot editor, **⌥⌘1–9** selects annotation tools, **⇧⌘C** copies the
image, and **⌘S** saves it. The image-options menu shows pixel dimensions and an
undoable **Reset Crop** action. The Text Reader can save recognized text or
Markdown to a file. Text Tools offers **Use as Input** for chaining transforms,
and **Reset** restores the original input.

## Developer tools

All transformations run locally. Results can be copied, chained with **Use as
Input** where appropriate, or used to replace a captured selection. Automatic
selection capture uses Accessibility; apps that do not expose their selection
can use copy followed by **Use Clipboard**.

| Tool | What it does |
| --- | --- |
| JSON Tools | Pretty-print, minify, validate, sort keys, and retain large numbers exactly. |
| Compare Text & JSON | Line, word, or JSON-field differences; pin text for comparison with a later selection. |
| Inspect JWT | Decode headers and claims and explain expiry dates. Decoding does **not** verify the signature; encrypted JWE is not supported. |
| Regex Tester | Live ICU matches, capture groups, highlighting, extraction, and replacement. |
| URL & Query Editor | Edit scheme, host, port, path, fragment, and repeated query parameters without dropping flag parameters. |
| Timestamp Converter | Unix seconds/milliseconds, ISO dates, time zones, and differences between two times. |
| Cron Schedule | Standard five-field cron, names/ranges/steps, and the next five runs in a chosen time zone, including daylight-saving transitions. |
| Convert JSON, YAML & CSV | Convert structured data, choose a CSV delimiter, and preview rows as a table. |
| Snippets & Templates | Save local templates and fill custom fields plus `{{selection}}`, `{{date}}`, `{{timestamp}}`, and `{{uuid}}`. |
| HTTP & cURL | Import a literal cURL command, edit URL/method/headers/body, and explicitly choose **Send**. Inspect responses and redirects; cancel in-flight requests. |
| Developer Generators | UUIDs, secure random strings, Unix timestamps, and synthetic records in line, JSON, or CSV form. |

CSV requires unique column names in its first row. Values remain strings unless
type inference is enabled; null/missing cells export as empty and nested values
as JSON. YAML conversion rejects duplicate/non-string keys, recursive aliases,
and merge keys instead of silently changing their meaning. JSON/YAML/CSV input
is bounded to keep the palette responsive.

cURL import supports common request/header/data/JSON/URL/authentication/cookie
options, including attached values. It never executes a shell and does not read
files. Unsupported options produce an error. HTTP requests run only on **Send**,
show redirects without following them, limit the response preview to 2 MB, and
do not save request or response history. Snippets are saved only when requested
in `~/Library/Application Support/BelloBox/Snippets.json`.

## Bring your own AI

Bello Box does not ship a model or an API key. In **Settings** you choose:

- **Provider** — OpenAI-compatible, Anthropic-compatible, or local Codex app-server.
- **Endpoint** — any base URL. Works with the OpenAI and Anthropic APIs as well
  as OpenRouter, Groq, together.ai, and local servers like Ollama or LM Studio.
- **Model** and **API key** (the key is stored in your macOS Keychain).
- **Codex options** — model and reasoning effort through `codex app-server`,
  using your existing Codex login.
- The **system prompt** that shapes every transformation.
- Optional **auto hint**, **launch at login**, and global shortcuts for the command
  palette, screenshots, and recordings.

A **Test connection** button verifies your configuration. For screenshot OCR,
Mac OCR runs locally with Apple Vision. LLM OCR is optional and always asks
before uploading the redaction-aware image to your configured provider.

## Permissions

Bello Box needs **Accessibility** access to read the selected text and paste
replacements back. macOS will prompt on first launch, or grant it under
*System Settings → Privacy & Security → Accessibility*.

Screenshot capture needs **Screen Recording** permission. Bello Box requests it
lazily when you use the screenshot tool. Annotation, scrolling stitch, and Mac
OCR stay on your Mac.

## Requirements

- macOS 13 (Ventura) or later.

## Updates

Bello Box updates itself via [Sparkle](https://sparkle-project.org). Use
*Check for Updates…* in the menu-bar menu.

---

© 2026 Belloware.
