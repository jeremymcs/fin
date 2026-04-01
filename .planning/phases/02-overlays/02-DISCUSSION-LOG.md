# Phase 2: Overlays - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-01
**Phase:** 02-overlays
**Areas discussed:** Help overlay layout, Toast duration + stacking, Stage transition detection, Toast signal set

---

## Help Overlay Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Single-column grouped | Keybindings section then slash commands section vertically. Directly extends model picker pattern — zero new layout primitives, long command lines wrap cleanly, scroll resets on open. | ✓ |
| Two-column | Keybindings left, slash commands right. ~34 chars per column at 80 cols — /blueprint descriptions overflow. | |
| Tabbed/paged | Tab switches between pages. Conflicts with "any key dismisses" contract; adds stateful input routing complexity. | |

**User's choice:** Single-column grouped  
**Notes:** Confirmed with preview mockup. Mirrors model picker pattern exactly.

---

## Toast Duration + Stacking

| Option | Description | Selected |
|--------|-------------|----------|
| 5s TTL + queue of 2 | VecDeque capped at 2. Only front item renders. Prevents WorkflowComplete being wiped by simultaneous ToolError in same drain cycle. | ✓ |
| 5s TTL + replace-on-new | Simplest: 1 Option<(String, Instant)>. Risk: burst flush silently drops one signal. | |
| Tiered TTL: errors 8s, info 4s | Longer errors, shorter info toasts. Adds classification logic. Can be layered later. | |

**User's choice:** 5s TTL + queue of 2  
**Notes:** Confirmed with drain cycle visualization. Both signals survive the same tick.

---

## Stage Transition Detection

| Option | Description | Selected |
|--------|-------------|----------|
| TUI-local prev_stage tracking | `let prev = workflow_state.current_stage.clone()` before write, compare after. Zero enum changes. Phase 3 would need to duplicate. | |
| New AgentEvent::StageTransition { from, to } | Add variant to agent_io.rs, emit from auto_loop.rs before WorkflowUnitStart. ~15 lines across 3 files. Phase 3 gets clean signal. Consistent with 8 existing workflow variants. | ✓ |

**User's choice:** New AgentEvent::StageTransition { from, to }  
**Notes:** Forward-compat for Phase 3 auto-run panel was the deciding factor.

---

## Toast Signal Set

| Option | Description | Selected |
|--------|-------------|----------|
| WorkflowError: toast + output line | WorkflowError halts execution. User may have scrolled away. Toast + existing output line — additive. | ✓ |
| ModelChanged: toast + keep output line | Model switches during auto-runs are meaningful checkpoints. Output line stays as audit trail. | ✓ |
| None — TOAST-01/02/03 only | Stick exactly to requirements. WorkflowError and ModelChanged output-line only. | |

**User's choice:** Both WorkflowError and ModelChanged get toasts (additive to output lines)  
**Notes:** "Would you notice if you looked away 30 seconds?" test — WorkflowError and ModelChanged pass; others (AgentStart, TurnStart, ToolStart, etc.) do not.

---

## Claude's Discretion

- Toast content truncation threshold (exact char count)
- Exact toast Rect position math within layout.output
- Whether render_toast() goes in widgets.rs or stays inline in app.rs

## Deferred Ideas

- Tiered TTL (errors 8s, info 4s) — noted for Phase 3+ consideration
- TOAST-FUT-01 (true stacking / multiple visible toasts) — per requirements, deferred
