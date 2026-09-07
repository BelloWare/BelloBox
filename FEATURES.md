# Bello Box — Features

Bello Box is a macOS menu-bar **toolbox for the text you have selected**. Select
text in any app and a small floating toolbar appears next to it; each button is a
tool. Results can be copied or pasted straight back in place.

This document lists what Bello Box can do and how you interact with each feature.

---

## The core interaction

1. **Select text** in any app — a note, an email, a code comment, a web form.
2. A small **floating toolbar** appears just above your selection with one button
   per tool: **AI ✨**, **Screenshot 📸**, **Record ⏺**, **QR ▦**, and
   **Text Tools 🔧**.
3. **Click a tool** (or dismiss by clicking elsewhere). A popup opens next to the
   selection.
4. Act on the result — most tools offer **Copy** and **Replace** (Replace pastes
   the result back over your original selection in the source app).

You can also summon the AI tool on the current selection from anywhere with the
global hotkey **⌃⌥⌘B**, or run any tool from the menu-bar menu.

> The toolbar and popups never steal focus from the app you're working in, so
> your selection stays put.

---

## Tool 1 — AI Assistant ✨

Ask a configurable AI to act on the selected text. The answer **streams in live**.

**One-click actions:**
- Fix Spelling & Grammar
- Improve Writing
- Make Shorter
- Professional Tone
- Friendly Tone
- Summarize
- Explain
- Translate to English

**Custom instruction:** type anything into the "Ask Bello Box to…" field
(e.g. "make this a bullet list", "rephrase as a friendly reply") and run it.

**After a result:**
- **Copy** the result to the clipboard, or
- **Replace** your original selection with it (⌘↩).

**Bring your own AI** (set up once in Settings or onboarding):
- **API format:** OpenAI-compatible, Anthropic-compatible, or the local **Codex app-server**.
- **Endpoint:** any base URL — works with OpenAI, Anthropic, OpenRouter, Groq,
  together.ai, and local servers like Ollama or LM Studio.
- **Codex app-server:** runs your local `codex app-server` via your login shell
  with your existing Codex login. Bello Box passes model, reasoning effort,
  sandbox, and approval policy per request; an optional command field is there
  if you need a specific binary.
- **Load models:** fetch the available models from the endpoint and pick one
  (Codex offers preset models).
- **Model** and **API key** (the key is stored in your macOS Keychain).
- **System prompt:** editable text that shapes every transformation.
- **Test connection:** runs a quick "say hi" to confirm it works.

---

## Tool 2 — QR Code ▦

Turn the selection into a scannable QR code.

- A **live QR code** renders from the selected text.
- An **editable text field** lets you tweak the content; the QR updates as you
  type.
- **Copy** the QR image to the clipboard, or **Save…** it as a PNG.

Great for sharing a link, address, Wi-Fi string, or any text to a phone.

---

## Tool 3 — Text Tools 🔧

Offline utilities — no AI key, no network, instant. Pick a category at the top of
the popup; the input is pre-filled with your selection and is editable. Most
categories offer **Copy** and **Replace**.

- **Case** — UPPERCASE, lowercase, Title Case, Sentence case, camelCase,
  PascalCase, snake_case, kebab-case, CONSTANT_CASE.
- **Encode** — Base64, URL, HTML entities, Hex.
- **Decode** — **auto-detects** Base64 / URL / HTML / Hex (with a manual override
  if it guesses wrong).
- **Pretty** — **auto-detects** the language and reindents: JSON, XML/HTML, and
  brace-style code (CSS / JS).
- **Hash** — MD5, SHA-1, SHA-256, SHA-512, shown together; copy any.
- **Lines** — sort A→Z / Z→A, reverse, remove duplicates, remove empty lines,
  trim each line.
- **Count** — characters, characters without spaces, words, lines, and a
  **model-aware token estimate**. The token model can be changed **right in the
  popup** (provider toggle + model field + preset menu); the estimate updates
  live and the choice is remembered.

> The token count is a fast, word-aware estimate (not an exact tokenizer), and it
> shows which model and tokenizer family it is based on.

---

## Tool 4 — Screenshot 📸

Capture an **area**, **window**, **screen**, or **scrolling page**. Bello Box
opens the image in an annotation editor with pen, arrows, rectangles,
highlights, text labels, crop, masks, an eraser, undo/redo, OCR, copy image,
and save PNG.

- **Mask** hides content behind an opaque fill. Pick charcoal, black, white,
  orange, teal or plum (or any custom colour) and a solid, striped or dotted
  finish; whatever you choose, the export and OCR upload never let the pixels
  underneath show through.
- **Eraser** is a brush: drag it over part of an arrow, line, shape, label or
  mask and only that part disappears, revealing the screenshot beneath. One
  drag is one undo step; annotations drawn afterwards over the erased spot are
  untouched; nothing outside the brush is ever removed. To delete a whole
  label, right-click it.
- **Zoom** in the editor window: Fit, Fit Width, Actual Size (⌘0), Fit (⌘9),
  ⌘+/⌘− steps, and scrolling to pan. Tall scrolling captures open fitted to
  the width so they read like the page.

For area, window, and screen captures the displays are frozen the moment you
trigger the capture, before the dim overlay appears, so hover-only UI such as
menus, tooltips, and hovered links stays visible in the overlay and in the
result. A window that another window was covering is re-captured live so the
covered part is filled in. Scroll-to-capture samples the live content.

**Capture modes:**
- **Area** — drag a screen region; tiny pointer jitter still behaves like a click.
  After the capture, drag the corner or edge handles to resize the selection,
  or drag inside it with the Select tool to move it. The editor re-crops the
  frozen display instantly, annotations stay on the pixels they were drawn on,
  and one undo reverts a whole drag.
- **Window** — hover a window to highlight it, then click to capture it.
- **Screen** — click blank space to capture the display; the selection handles
  work here too.
- **Scrolling Capture** — after an area or window capture, press
  **Scrolling Capture** in the editor toolbar (or start from Screenshot →
  Scrolling). The selection turns live and a card beside it shows which mode
  is active (**Manual**: you scroll inside the orange frame; **Auto-scroll**:
  Bello Box scrolls until the content ends), how much is captured (screens,
  pixel height, frames used of the maximum) and a preview of the stitched
  result growing. It warns when a jump skipped more than a screen. **Finish**
  stitches the frames into one tall screenshot (duplicated overlap removed,
  fixed headers and footers kept once) and opens it in the editor, fitted to
  the width and with any capture notes shown above the image; **Cancel**
  returns to the editor with the original capture. Every control explains
  itself with a tooltip.

**OCR:**
- **Mac OCR** uses Apple Vision locally and works offline.
- **Improve with LLM OCR…** can use your configured OpenAI-compatible or
  Anthropic-compatible vision model for difficult screenshots, tables,
  handwriting, and messy layouts.
- LLM OCR always shows a confirmation with provider, model, upload dimensions,
  size, and thumbnail before sending anything.
- The uploaded image applies crop and blur/redaction first, and decorative
  annotations are excluded by default.

OCR text is selectable and copyable as plain text or Markdown. Local OCR can
also show line boxes over the image without baking those boxes into the export.

## Tool 5 — Recording ⏺

Start a screen recording from the toolbar, capture chooser, menu bar, or global
recording shortcut.

- Capture an **area**, **window**, or **screen**.
- Choose the output: a **Movie** (.mov with audio) or a **GIF** (silent; the
  movie is kept too). GIF frame rate, longest edge and looping are set before
  recording; trimming happens afterwards. Loop off writes a GIF that plays
  once in browsers and viewers; loop on repeats forever.
- Choose no audio, microphone, Mac audio, or mic + Mac audio.
- Configure cursor, click rings, keystroke overlays, secure-field redaction,
  countdown, quality, and whether Bello Box windows are excluded, all on one
  compact card.
- Pause/resume, change click/key tracking, or stop from the recording HUD
  (which shows a GIF badge in GIF mode); review the movie or the GIF
  afterwards to save, copy, reveal, or discard it. **Make GIF…** in the review
  turns any recording into a GIF with your own clip, frame rate and width.
- **Video to GIF** (palette, Home → Capture, menu bar) converts a movie you
  choose from disk. Everything is decoded and written locally; progress is
  real, cancelling writes nothing, and an existing file is only replaced once
  the new GIF is complete.

---

## Home window

Opening Bello Box (from Finder or Launchpad, or **Open Bello Box** in the menu)
shows a home window with:

- The app version and a one-line description.
- Live **status**: whether an AI provider is configured and whether Accessibility
  is granted, each with a quick action to fix it.
- A short how-to.
- Buttons for **Settings**, the **Setup Guide**, and **Check for Updates**.

It opens centered, and re-opening the app brings it back. The app uses a regular
Dock icon whenever the main app is running.

## Appearance / Themes

- Choose **System**, **Light**, or **Dark** in **Settings → Appearance**.
- "System" follows your macOS setting; Light/Dark force Bello Box either way.

## Onboarding

- On first launch a short guide walks you through what Bello Box does, granting
  Accessibility and Screen Recording, launch-at-login, auto hint behavior,
  global shortcuts for the tool board, screenshots, and recordings, and
  connecting your AI provider.
- You can **Skip** the setup and configure it later.
- Reopen it anytime from the menu bar → **Set Up Bello Box…**.

## Menu bar

The ✨ menu-bar icon gives you:
- **Open Bello Box** (the home window)
- **Ask Bello Box About Selection**, **Capture Screenshot…**,
  **Capture Scrolling Screenshot…**, **Record Screen…**, **Stop Recording**,
  **Convert Video to GIF…**, **Generate QR Code from Selection**, and
  **Text Tools on Selection**
- **Set Up Bello Box…**, **Settings…**
- **Check for Updates…**
- **Quit**

## Updates

Bello Box updates itself via [Sparkle](https://sparkle-project.org). Use
**Check for Updates…**, or let automatic checks handle it.

## Permissions & privacy

- Bello Box needs **Accessibility** access to read the selected text and to paste
  replacements. It only reads the selection when you ask it to.
- Screenshot capture needs **Screen Recording** permission. Capture, annotation,
  scrolling stitch, and Mac OCR stay on your Mac.
- Text is sent only to the **AI endpoint you configure**. Screenshot pixels are
  sent only when you explicitly choose LLM OCR and confirm the redaction-aware
  upload. QR and Text Tools run entirely offline.
- Your API key is stored in the macOS **Keychain**.

## Requirements

- macOS 13 (Ventura) or later.
- Your own OpenAI-compatible or Anthropic-compatible endpoint + key for the AI
  tool (the other tools work without one).
