# AD HyperOptimize Design System

## 1. Atmosphere & Identity

A focused Windows utility dashboard. The first screen presents an ordered eight-step performance guide. Each step opens a focused tool; the guide does not apply changes automatically. App removal remains available through the Cleanup category. Dark layered surfaces and a cool blue accent match the existing app.

## 2. Color

The existing CSS tokens in `src/styles.css` are authoritative: `--bg` and `--bg2` for the shell, `--card` and `--card2` for surfaces, `--border` for separation, `--text` and `--muted` for hierarchy, `--accent` for the primary action, and `--green`, `--yellow`, `--red` for status. Featured cards use these tokens and do not introduce new colors.

## 3. Typography

Use the existing Segoe UI/system stack and the existing 14px body text. The home heading uses the current 22px scale; featured action names use 16px semibold, descriptions 12.5px, and sequence labels 11px. Long descriptions wrap naturally.

## 4. Spacing & Layout

Base unit: 4px. Home sections use 24px vertical separation, featured cards use 20px inner padding and 16px gaps. The eight-step guide uses two columns on wide windows and one column at narrow widths so explanations remain readable. The content scroll container remains `.content`.

## 5. Components

### Featured action

- Structure: button containing step number, title, short explanation, and directional arrow.
- States: default, hover, pressed, focus visible, and hidden when its tool is not available in the current mode.
- Accessibility: the entire card is a button with the tool title as its accessible name; keyboard focus is visible.
- Layout: responsive grid in the home content area.

### Existing cards

The shared `Card` and `Badge` components in `src/components/ui.tsx`, `.tool-card`, and `.category-card` remain the common primitives. Category cards lead to all tools; search leads directly to a tool or feature.

## 6. Motion & Interaction

The existing short transform and border transitions signal hover and press. Featured cards follow the same 160ms timing. Reduced-motion preference disables spatial transitions. Clicking a featured action opens the tool directly; categories and browser search retain their behavior.

## 7. Depth & Surface

Mixed tonal surfaces and one-pixel borders follow the existing shell. The featured actions are visually stronger through contrast and scale, without a new gradient or decorative animation.

## 8. Accessibility Constraints & Accepted Debt

Target WCAG 2.2 AA for the changed home controls: visible keyboard focus, readable text, semantic buttons, and one-column reflow at narrow widths. Existing emoji icons, legacy inline styles, and unmeasured whole-app accessibility are inherited debt outside this menu change; they are not evidence that every screen passes WCAG.
