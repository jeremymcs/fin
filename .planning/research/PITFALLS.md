// Fin — TUI Enhancement v1.1 Research: Domain Pitfalls
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

# Domain Pitfalls: Fin TUI Enhancement (v1.1)

**Domain:** Adding overlay, streaming-text, and layout-extension features to an existing ratatui/crossterm Rust TUI
**Researched:** 2026-04-01
**Confidence:** HIGH (code-grounded observations from live source) / MEDIUM (ratatui ecosystem patterns)

---

## Critical Pitfalls

Mistakes that cause incorrect rendering, scroll state corruption, or input lockout.

---

### Pitfall 1: Dual-Layout Chunk Indexing Breaks When New Panels Are Added

**What goes wrong:**
`app.rs` currently has two hard-coded layout paths: a 4-chunk path (normal) and a 5-chunk path (workflow active). Every index into `chunks[N]` is manually paired with a boolean guard (`if wf_active`). Adding the auto-run expansion rows or the side panel adds a third (or fourth) layout variant, but it is easy to miss updating every chunk index in the cursor-placement code, status-bar render, and input render.

The cursor line (near line 338 of `app.rs`) explicitly resolves `input_chunk` by index based on `wf_active`. A third layout variant — for example, auto panel + side panel both active — will silently use the wrong chunk index and draw the cursor in the wrong row.

**Why it happens:**
Hardcoded index arithmetic instead of named references. The pattern `chunks[3]` vs `chunks[2]` is brittle the moment layout count changes.

**Consequences:**
Cursor renders in a status bar or workflow panel cell. Input area visually overlaps the panel. Both are silent — no panic, just wrong output.

**Prevention:**
Replace the index-based lookup with named assignments immediately after every `Layout::split()` call. Assign `let input_chunk = chunks[...]` once with a clearly documented variable. Extract layout construction into a function that returns a named struct (`struct AppLayout { output: Rect, workflow: Option<Rect>, side: Option<Rect>, status: Rect, input: Rect }`). This eliminates stale index references when layouts change.

**Detection:**
Visually: cursor appears in wrong row. Write a debug render that prints chunk rects to a log file during development.

---

### Pitfall 2: Scroll Offset Becomes Invalid When Workflow Panel Is Toggled In or Out

**What goes wrong:**
`auto_scroll()` and `max_scroll()` in `app.rs` both receive `workflow_state.active` as a parameter and use it to adjust the available output height. When workflow panel visibility toggles mid-session — the panel appears when `WorkflowUnitStart` fires, and disappears on `WorkflowComplete`/`WorkflowBlocked`/`WorkflowError` — the effective output area shrinks or grows by 4 rows. The existing `scroll` offset is not recomputed.

If the user has scrolled up, the previously valid offset may now point past the line count for the new area size, leaving the output area blank or showing an empty tail.

**Why it happens:**
`scroll` is a persistent `u16` state. It is only updated by auto-scroll and keyboard events. Layout changes that alter the viewport height do not trigger a scroll recalculation.

**Consequences:**
Blank output area after workflow panel appears or disappears. The user sees nothing until they scroll manually.

**Prevention:**
After any state change that modifies panel visibility (`workflow_state.active`, `side_panel_active`), immediately call `auto_scroll()` unconditionally — or re-clamp `scroll` to the new `max_scroll()` value. The same applies when the side panel is toggled via Ctrl+P.

**Detection:**
Trigger `/auto` on a small blueprint, let the workflow panel appear, then complete the run and observe whether output re-anchors correctly.

---

### Pitfall 3: Toast Tick Mechanism Starves Under Agent Event Flood

**What goes wrong:**
Fin's event loop processes all pending `AgentEvent` items in a tight `while let Ok(evt) = agent_event_rx.try_recv()` drain before checking keyboard input and before drawing. During an active streaming turn, this can drain dozens of `TextDelta` events per 50ms tick. A toast timer that decrements on each `draw()` call will work correctly. But if the toast expiry check is placed inside the `AgentEvent` drain (e.g., "expire toasts on every event"), it will fire many times per frame and toasts will disappear almost immediately during heavy streaming.

**Why it happens:**
The frame rate and the agent event rate are decoupled. One draw cycle processes N agent events, not 1.

**Consequences:**
Toasts flash for a fraction of a second then vanish. Users miss error notifications during tool runs.

**Prevention:**
Use `Instant::now()` for toast expiry, not a frame counter. Store `Vec<(String, ToastLevel, Instant)>` in the TUI state. On each `draw()`, filter out toasts where `now > created_at + duration`. This is immune to event rate variation.

**Detection:**
Trigger a tool error while a long streaming response is active. Observe whether the error toast is visible for the full intended duration.

---

### Pitfall 4: Inline Markdown Parser Breaks on Mid-Stream Delimiter Splits

**What goes wrong:**
`render_output()` in `widgets.rs` already processes `OutputLine` items — each item is a fully buffered line. The current `TextDelta` handler in `app.rs` appends chunks to the last `Assistant` line and only finalizes lines on `\n`. This means by the time `render_output()` sees a line, it may be a partial token stream that ends mid-bold (`**foo` without the closing `**`).

A span-level parser that scans for `**text**` and `*text*` pairs will fail to find matches on in-progress lines. If the parser emits an unmatched `**` as a plain span it will re-parse it as styled text when the closing delimiter arrives on the same line — producing a flash of unstyled text followed by styled text, one frame apart.

The current code in `widgets.rs` also has an existing question-mark heuristic (`text.ends_with('?') → Cyan`) that will misfire if the last partial token happens to end with a `?`.

**Why it happens:**
`render_output()` is called every frame. Streaming lines are mutable. Parsing logic sees an intermediate state.

**Consequences:**
Rendered text flickers between styled and unstyled mid-response. The question-mark heuristic produces spurious cyan coloring on partial tokens.

**Prevention:**
Parse markdown spans only on lines that are finalized (after `\n` is received or after `AgentEnd`). For in-progress lines, render them raw (no span parsing). Track finalization state in `OutputLine` with a boolean flag (`is_final: bool`). Alternatively, parse eagerly but only emit styled spans when both opening and closing delimiters are present in the current line buffer.

Remove or bound the question-mark heuristic to complete lines only.

**Detection:**
Stream a response containing `**bold term**` — the bold should not flash. Stream a tool call description that happens to end with `?` mid-stream — it should not turn cyan prematurely.

---

### Pitfall 5: Keybindings Help Overlay Doesn't Clear Input Mode State on Close

**What goes wrong:**
The existing model picker overlay in `app.rs` uses a guard at the top of the key handler: `if model_picker_active { ... ; continue; }`. This completely swallows all key events while the picker is open. The help overlay will need the same guard. If Esc is the close key but the help overlay is opened while the user's `input_text` is non-empty, pressing Esc may clear the overlay but `input_text` and `cursor_pos` are unaffected — which is correct. However, if `?` is mapped to open the overlay but `?` is also a valid input character for normal chat, the `?` keypress will be consumed by the overlay-open logic even if the user was mid-sentence.

A subtler version: if the overlay guard uses `continue` inside the outer key match, but the outer match also matches `KeyCode::Char('?')` as a text input char, key ordering in the match determines which arm fires. In Rust match arms evaluate in order, so whichever `KeyCode::Char('?')` arm appears first wins.

**Why it happens:**
The existing pattern intercepts the _entire_ key event and uses `continue` to skip the rest of the match. Adding a new overlay open-key inside `KeyCode::Char(c)` catch-all requires careful arm ordering.

**Consequences:**
`?` is typed into `input_text` when the overlay should open, or the overlay opens when the user is typing. The overlay is shown but pressing Esc leaves the overlay guard stuck as `true` if the `Esc` arm is not present in the overlay's dedicated match.

**Prevention:**
Follow the same pattern as `model_picker_active`: add `help_overlay_active` as a boolean state variable. Place `if help_overlay_active { ... match key.code { Esc => help_overlay_active = false, _ => {} }; continue; }` at the top of the key handler, before the main match. Bind `?` to open the overlay by adding it as a named case in the main match above the `KeyCode::Char(c)` catch-all: `(KeyCode::Char('?'), KeyModifiers::NONE) => { help_overlay_active = true; }`.

**Detection:**
Type `?` in the input box — overlay must open, not insert `?`. While overlay is open, press every navigation key — none should modify `input_text`. Press Esc — overlay must close, `input_text` unchanged.

---

## Moderate Pitfalls

---

### Pitfall 6: Per-Message Token Display Requires Attribution at Streaming Start, Not End

**What goes wrong:**
`AgentEvent::AgentEnd` carries the `Usage` struct. The current code in `app.rs` appends a `System` line showing in/out tokens after `AgentEnd` fires. For per-message tracking, the token count must be associated with a specific message turn. If the goal is to display tokens inline per-response (e.g., a footnote line appended after each assistant turn), the `AgentEnd` approach works. But if the goal is per-streaming-block costs visible _during_ streaming, there is no token count available until the LLM finishes — providers do not stream usage incrementally.

A common mistake is to pre-allocate a mutable `System` line that reads "tokens: ..." at turn start and update it in place as events arrive. In-place mutation of `OutputLine` items via index access is error-prone because new lines may be inserted by `TextDelta` or `ToolStart` events between the pre-allocated slot and the current tail.

**Why it happens:**
The `output_lines` vec grows during streaming. An index captured at `AgentStart` becomes stale as new lines are pushed.

**Consequences:**
Token count is displayed on the wrong line, or the footer line is separated from the response it belongs to by tool call lines inserted after it.

**Prevention:**
Do not pre-allocate. Append a token summary line at `AgentEnd`, immediately after the last content line. This is already how the current per-turn summary is handled (`└─ N in / N out ────`). For per-message tracking add a message-level counter to `WorkflowState` or a separate `last_turn_usage: Option<Usage>` field, and display it in the status bar or as the footer line appended at `AgentEnd`.

---

### Pitfall 7: `render_workflow_panel()` Uses a Fixed 2-Line Inner Layout — Expansion Will Panic on Small Terminals

**What goes wrong:**
`render_workflow_panel()` in `widgets.rs` splits its inner area into exactly 2 rows (pipeline + progress bar). The auto-run panel expansion will add more rows (blueprint/model, action, last git commit, memory %). If the expanded panel requests 5+ inner rows but the terminal height is 30 and other panels consume most of it, `Layout::split()` may return `Rect`s with `height: 0`. Rendering a `Paragraph` into a zero-height `Rect` is safe (ratatui no-ops it), but computing layouts that exceed available area can produce empty or overlapping areas.

The existing guard `if inner.height < 2 { return; }` handles the current 2-row case. The expanded version needs a proportional guard that handles variable row counts.

**Why it happens:**
Hardcoded `Constraint::Length(1)` rows without a fallback for constrained terminals.

**Consequences:**
On an 80x24 terminal with both the workflow panel and side panel open, the output area may be squeezed to 0 rows. The screen looks corrupt.

**Prevention:**
Calculate the required panel height dynamically based on the number of rows to display. Use `Constraint::Min(0)` for the last row to absorb overflow gracefully. Guard at the panel entry: `if area.height < REQUIRED_ROWS { render_compact_fallback(); return; }`. Define a minimum terminal height constant (e.g., 20 rows) below which panels degrade to compact mode.

---

### Pitfall 8: Side Panel Toggle via Ctrl+P Conflicts With Common Terminal Shortcuts

**What goes wrong:**
`Ctrl+P` is the designated keybind for the side panel toggle. In many terminal emulators and multiplexers (tmux default is `Ctrl+B`, but some remap `Ctrl+P` for pane navigation), `Ctrl+P` may be intercepted before reaching the application. In macOS Terminal.app, `Ctrl+P` moves the cursor up (readline behavior in some modes). Under raw mode crossterm captures all key events, so in a true raw-mode alt-screen session `Ctrl+P` will reach the app. But users running Fin inside tmux or iTerm2 with custom keybinds may find the toggle unreachable.

**Why it happens:**
Raw mode does not protect against host-level multiplexer key interception. `Ctrl+P` has a long history of readline/Emacs cursor-up binding.

**Consequences:**
Side panel cannot be toggled in affected terminal environments.

**Prevention:**
Document the known conflict and offer an alternative binding (e.g., `/panel` slash command as a fallback). Validate the binding choice before shipping. At minimum note it in the help overlay.

---

### Pitfall 9: `Color::Rgb(r,g,b)` Falls Back Silently on 256-Color Terminals

**What goes wrong:**
The current `widgets.rs` uses named colors only (`Color::Cyan`, `Color::White`, `Color::DarkGray`, etc.) which map correctly in both 256-color and truecolor terminals. The visual theme consistency pass may introduce `Color::Rgb(r,g,b)` values for more precise palette control. On terminals without truecolor support, crossterm will attempt to approximate with the nearest ANSI-256 color — the result may be visually acceptable or may produce jarring color substitutions depending on the palette.

There is no runtime panic; the substitution is silent.

**Why it happens:**
crossterm does not detect terminal color capability at compile time. It writes escape sequences and lets the terminal handle what it cannot display.

**Consequences:**
Theme looks correct in the developer's terminal (likely truecolor) but looks wrong in CI terminals, SSH sessions, or older terminal emulators.

**Prevention:**
Check `COLORTERM` environment variable at startup: if it equals `truecolor` or `24bit`, use `Color::Rgb`. Otherwise, constrain the palette to named ANSI colors. Expose this as a fallback branch in a `ThemeConfig` struct. Do not introduce any `Color::Rgb` values in the theme without a corresponding ANSI fallback. The `termprofile` crate provides detection helpers if a dependency is acceptable (check binary size impact first — the constraint is ~3.7MB stripped).

**Detection:**
`TERM=xterm-256color COLORTERM= cargo run -- --tui` — run Fin without truecolor env var and verify the theme is acceptable.

---

### Pitfall 10: Auto-Scroll Height Calculation Must Account for All Active Panels

**What goes wrong:**
`auto_scroll()` and `max_scroll()` receive `workflow_state.active: bool` to subtract the workflow panel height from the available output rows. If a side panel is added horizontally (not vertically), it does not change the vertical output area and does not need to affect `max_scroll()`. But if the auto-run panel expansion adds rows vertically beyond the current 4-row block, `max_scroll()` will undercount available output rows and let `scroll` go too high, cutting off the top of the conversation.

**Why it happens:**
The panel height constant used in `auto_scroll`/`max_scroll` is currently hardcoded to the 4-row workflow panel. Any change to that panel's row count must be mirrored in the scroll calculation.

**Consequences:**
Scroll position can exceed the real max, blanking the top of the output area.

**Prevention:**
Replace the hardcoded panel height constant in the scroll functions with a computed value derived from the same constraints used in the layout. Either pass the panel height as a parameter or derive it from `WorkflowState` directly. When the panel height changes for auto mode, the scroll math updates automatically.

---

## Minor Pitfalls

---

### Pitfall 11: `is_numbered_list()` False Positives on LLM Output Like "1984. Great year"

**What goes wrong:**
The existing `is_numbered_list()` helper matches any string beginning with one or more digits followed by `. `. Sentences like "1984. Great year" or token costs like "3.50 USD" (if formatted differently) may trigger this path. The numbered list path also uses `text.find(". ")` which finds the first occurrence — on a line like `"10. 5 items processed"` the split will capture `"10"` as the list number and `"5 items processed"` as content, which is correct but fragile.

This is a pre-existing bug. The markdown enhancement phase should not make it worse by adding more pattern-matching on top without fixing the base case.

**Prevention:**
Tighten `is_numbered_list()` to require the number is followed by `. ` at the _start_ of the trimmed string (which it already does via `trimmed.find(". ")`), but also require the number is 1-3 digits maximum. Numbers longer than 3 digits are almost certainly not list items.

---

### Pitfall 12: The `?` Key Opens Help Overlay But `?` Also Appears in Prompts

**What goes wrong:**
Users frequently end prompts with `?` (e.g., `"What is the current status?"`). If `?` is bound at the top-level key handler with no modifier requirement, it fires when the user presses `?` in any input context, not just when `input_text` is empty.

**Prevention:**
Bind the help overlay only when `input_text.is_empty()` and no overlay is active. If input is non-empty, `?` should insert the character as normal. This requires a conditional at the binding site: `(KeyCode::Char('?'), _) if input_text.is_empty() && !model_picker_active => { help_overlay_active = true; }`. Rust match guards support this directly.

---

### Pitfall 13: `Clear` Widget Must Be Rendered Before the Overlay Content, Not After

**What goes wrong:**
The model picker in `app.rs` correctly renders `f.render_widget(ratatui::widgets::Clear, picker_area)` before the picker content. Any new overlay (help, toast) must follow the same pattern. A common mistake when copying overlay code is to render the content block first and then `Clear` — the `Clear` will erase the content just rendered because ratatui renders in draw order.

**Why it happens:**
`Clear` sets cells to default background. If rendered after the overlay content, it blanks the overlay.

**Consequences:**
Overlay appears blank.

**Prevention:**
Enforce a code review rule: every overlay render must begin with `f.render_widget(Clear, overlay_area)`. Apply this to toasts, help overlay, and any future popup.

---

### Pitfall 14: Toast Stack Position Interacts Badly With Workflow Panel Visibility

**What goes wrong:**
If toasts are positioned relative to the bottom-right of the terminal area (a common pattern), and the workflow panel appears or disappears, the toast stack visually jumps. More critically, if toasts are rendered at a fixed `y` offset from the bottom and the workflow panel is also at the bottom, they overlap.

**Prevention:**
Anchor toasts to the top-right corner of the output area, not the terminal area. The output area is stable (it only shrinks when the workflow panel appears, it does not move). Toast width should be bounded to a maximum (e.g., 50 columns) and capped at half the terminal width on narrow terminals. Render all toasts after the workflow panel and side panel so z-order is correct.

---

## Phase-Specific Warnings

| Feature | Phase | Likely Pitfall | Mitigation |
|---------|-------|---------------|------------|
| Auto-run panel expansion | Phase 1 | Chunk index breakage (Pitfall 1), scroll invalidation (Pitfall 2) | Named layout struct, recalc scroll on panel toggle |
| Toast notifications | Phase 2 | Timer starvation under event flood (Pitfall 3), z-order (Pitfall 13), position clash (Pitfall 14) | Use `Instant` not frame counter, render last, anchor top-right of output |
| Inline markdown bold/italic | Phase 3 | Mid-stream parsing flicker (Pitfall 4), `is_numbered_list` regressions (Pitfall 11) | Parse only finalized lines, tighten existing helper |
| Keybindings help overlay | Phase 4 | Input conflict (Pitfall 5, Pitfall 12), Clear render order (Pitfall 13) | Guard `?` with `input_text.is_empty()`, follow model picker pattern exactly |
| Per-message token tracking | Phase 5 | Index invalidation during stream (Pitfall 6) | Append at `AgentEnd`, do not pre-allocate slot |
| Toggle-able side panel | Phase 6 | Ctrl+P multiplexer conflict (Pitfall 8), scroll recalc (Pitfall 2), terminal width enforcement (Pitfall 7) | Slash command fallback, clamp scroll on toggle |
| Visual theme consistency | Phase 7 | Silent color fallback on 256-color terminals (Pitfall 9) | Check COLORTERM, named-color fallback |

---

## Sources

- ratatui source code + issue tracker: [ratatui/ratatui](https://github.com/ratatui/ratatui)
- ratatui overlay recipe (Clear widget z-order): [Popups — overwrite regions](https://ratatui.rs/recipes/render/overwrite-regions/)
- Unicode width calculation issue: [Fix unicode text width calculation #1271](https://github.com/ratatui/ratatui/issues/1271)
- Multiline scroll rendering issue: [Multiline list items cause late rendering when scrolling #1514](https://github.com/ratatui/ratatui/issues/1514)
- Scrollbar viewport length assumption: [Scrollbar viewport_length function assumption #966](https://github.com/ratatui/ratatui/issues/966)
- Color palette and terminal compatibility: [How to choose good colors for different terminal emulators? #877](https://github.com/ratatui/ratatui/discussions/877)
- ratatui-toolkit ToastManager: [ratatui-toolkit on lib.rs](https://lib.rs/crates/ratatui-toolkit)
- Terminal truecolor detection: [termprofile on GitHub](https://github.com/aschey/termprofile)
- Color enum documentation: [Color in ratatui::style](https://docs.rs/ratatui/latest/ratatui/style/enum.Color.html)
- Live code reference: `/Users/jeremymcspadden/Github/fin/src/tui/app.rs` (2153 lines, reviewed lines 1–1013)
- Live code reference: `/Users/jeremymcspadden/Github/fin/src/tui/widgets.rs` (527 lines, reviewed in full)
