# Bello Box

A tiny macOS menu-bar assistant for the text you already have in front of you.

Select text in **any** app — a note, an email, a code comment, a web form — and a
small Bello Box toolbar appears next to your selection. Click it (or press
**⌃⌥⌘B**) and a popup opens where you can:

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

Open **Bello Box** from the menu bar for direct access to screenshots, scrolling
capture, recording, World Clock, QR codes, and Text Tools. QR and Text Tools also
work with text you type or paste, without selecting text in another app.

**World Clock** follows the current time until you choose a planning time. Enter
an exact time, compare date changes across locations, then use **Copy Times** to
share the comparison. **Now** returns to the live clock.

In the screenshot editor, **⌥⌘1–9** selects annotation tools, **⇧⌘C** copies the
image, and **⌘S** saves it. The image-options menu shows pixel dimensions and an
undoable **Reset Crop** action. The Text Reader can save recognized text or
Markdown to a file. Text Tools offers **Use as Input** for chaining transforms,
and **Reset** restores the original input.

## Bring your own AI

Bello Box does not ship a model or an API key. In **Settings** you choose:

- **Provider** — OpenAI-compatible, Anthropic-compatible, or local Codex app-server.
- **Endpoint** — any base URL. Works with the OpenAI and Anthropic APIs as well
  as OpenRouter, Groq, together.ai, and local servers like Ollama or LM Studio.
- **Model** and **API key** (the key is stored in your macOS Keychain).
- **Codex options** — model and reasoning effort through `codex app-server`,
  using your existing Codex login.
- The **system prompt** that shapes every transformation.
- Optional **auto hint**, **launch at login**, and global shortcuts for the tool
  board, screenshots, and recordings.

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
