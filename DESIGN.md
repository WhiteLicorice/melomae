---
name: Melomae
description: A quiet repair bench for a music library the user already owns.
colors:
  canvas-light: "#F4F1EC"
  canvas-dark: "#171614"
  surface-light: "#FBF9F6"
  surface-dark: "#201E1B"
  ink-light: "#1C1A17"
  ink-dark: "#F2EDE6"
  ink-muted-light: "#5E5A54"
  ink-muted-dark: "#B3ACA2"
  border-light: "#D8D2C8"
  border-dark: "#3A3732"
  control-edge-light: "#857D71"
  control-edge-dark: "#7C746A"
  accent-light: "#9A4708"
  accent-dark: "#E9A66B"
  accent-ink-light: "#FFFFFF"
  accent-ink-dark: "#171614"
  focus-light: "#7C3806"
  focus-dark: "#FFC598"
  cutoff-light: "#0F6E7A"
  cutoff-dark: "#7FD6DE"
  ok-light: "#2F6B4F"
  ok-dark: "#7FB694"
  warn-light: "#8A5A00"
  warn-dark: "#D9A441"
  danger-light: "#A23B2A"
  danger-dark: "#E08374"
typography:
  headline:
    fontFamily: 'system-ui, "Segoe UI", Cantarell, Ubuntu, "Noto Sans", sans-serif'
    fontSize: "20px"
    fontWeight: 600
    lineHeight: "28px"
  title:
    fontFamily: 'system-ui, "Segoe UI", Cantarell, Ubuntu, "Noto Sans", sans-serif'
    fontSize: "15px"
    fontWeight: 600
    lineHeight: "22px"
  body:
    fontFamily: 'system-ui, "Segoe UI", Cantarell, Ubuntu, "Noto Sans", sans-serif'
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "18px"
  label:
    fontFamily: 'system-ui, "Segoe UI", Cantarell, Ubuntu, "Noto Sans", sans-serif'
    fontSize: "11px"
    fontWeight: 600
    lineHeight: "16px"
    letterSpacing: "0.04em"
  mono:
    fontFamily: 'ui-monospace, "Cascadia Mono", Consolas, "Liberation Mono", "DejaVu Sans Mono", monospace'
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "18px"
rounded:
  sm: "4px"
  md: "6px"
  pill: "999px"
spacing:
  "2xs": "4px"
  xs: "8px"
  sm: "12px"
  md: "16px"
  lg: "24px"
  xl: "32px"
  "2xl": "48px"
---

# Design System: Melomae

## Overview

**Creative North Star: "The Repair Bench"**

A repair bench holds the tool, the part, and the measurement where the
technician can see them at once. The bench is quiet, the light is even, and
nothing on it is decoration. Melomae takes that posture. The file list, the
measured cutoff, and the plain verdict carry the screen. The chrome stays out
of the way.

The palette is a warm neutral range with one ember accent and one instrument
teal. The accent marks the action the user takes. The teal marks the
measurement the app records. Neither color fills a large area. Depth comes
from a tonal step and a 1 px edge, never from a shadow. The type is the
system font, so the app looks native on Windows and on Linux and needs no
font file.

The system must read as a local tool that measures audio. It must not read as
a streaming service, a media browser, or an "AI" product.

**Key Characteristics:**

- A warm neutral canvas, one accent, and one measurement color.
- A measurement first, and a verdict in plain words after it.
- Flat surfaces with 1 px edges and no shadow.
- The system font, tabular numerals, and a mono face for technical strings.
- Short motion, and no transforms when the OS asks for less motion.

## Colors

The palette is warm and low in chroma, so the cutoff meter and the status
colors stay the only strong signals on the screen.

Each color token carries a light value and a dark value. The `-light` and
`-dark` suffixes name the two themes. The theme follows the OS setting and has
no manual override (B-051 holds the override).

### Primary

- **Warm Ember** (`accent`): the fill of the primary button, the text of a
  focused link, and the left marker of the selected row. It is the only
  saturated color on an action.
- **Ember Ink** (`accent-ink`): the label on a primary button. It is the only
  text color that may sit on `accent`.
- **Banked Ember** (`focus`): the focus ring. It is the accent taken one step
  deeper in the light theme and one step paler in the dark theme, so the ring
  stays visible on the canvas beside an accent-filled button.

### Secondary

- **Instrument Teal** (`cutoff`): the cutoff marker, the measured scale, and
  the kHz value in a triage row. It belongs to the measurement and to nothing
  else. Do not use it for an action, because an action is not a measurement.

### Neutral

- **Warm Canvas** (`canvas`): the window background behind every surface.
- **Raised Surface** (`surface`): panels, table rows, fields, and dialogs.
- **Warm Ink** (`ink`): primary text.
- **Quiet Ink** (`ink-muted`): secondary text, the relative folder path under a
  file name, and the unit beside a value.
- **Divider** (`border`): a rule between rows, between table columns, and
  between a header and its content. It is a divider and nothing else.
- **Control Edge** (`control-edge`): the outline of an input, a select, a
  checkbox, a secondary button, and a chip. It reaches 3:1 against both the
  canvas and the surface.
- **Restored Green** (`ok`): a done job and a restored file.
- **Caution Amber** (`warn`): a low-benefit verdict, a skipped file, and a
  measurement that is still running.
- **Fault Rust** (`danger`): a failed job and an unreadable file.

### Named Rules

**The Edge Rule.** `border` divides content. `control-edge` outlines a control.
A divider is never the only edge of something a user can operate.

**The Word and Shape Rule.** No state is carried by color alone. Every verdict
and every result also has a word and a shape.

### Contrast pairs

| Pair | Foreground | Background | Minimum |
| --- | --- | --- | --- |
| Body text on canvas | ink | canvas | 4.5 |
| Body text on surface | ink | surface | 4.5 |
| Secondary text on canvas | ink-muted | canvas | 4.5 |
| Secondary text on surface | ink-muted | surface | 4.5 |
| Action text on canvas | accent | canvas | 4.5 |
| Action text on surface | accent | surface | 4.5 |
| Label on a primary button | accent-ink | accent | 4.5 |
| Cutoff text on canvas | cutoff | canvas | 4.5 |
| Cutoff text on surface | cutoff | surface | 4.5 |
| Done text on canvas | ok | canvas | 4.5 |
| Done text on surface | ok | surface | 4.5 |
| Caution text on canvas | warn | canvas | 4.5 |
| Caution text on surface | warn | surface | 4.5 |
| Failure text on canvas | danger | canvas | 4.5 |
| Failure text on surface | danger | surface | 4.5 |
| Control edge on canvas | control-edge | canvas | 3 |
| Control edge on surface | control-edge | surface | 3 |
| Focus ring on canvas | focus | canvas | 3 |
| Focus ring on surface | focus | surface | 3 |

This table is the contract. `tests/design.test.ts` reads the rows and the
frontmatter, computes each pair in both themes, and fails below the stated
minimum. A new color token needs a row here before it ships.

## Typography

**UI Font:** the system stack, `system-ui`, then `Segoe UI` on Windows and
`Cantarell` or `Ubuntu` on Linux, then `Noto Sans`, then `sans-serif`.
**Mono Font:** the system mono stack, led by `ui-monospace`, then `Cascadia
Mono` on Windows and `Liberation Mono` or `DejaVu Sans Mono` on Linux, then
`monospace`.

**Character:** the UI face is the one the user already reads all day, so the
app inherits the desktop and never looks foreign. Numbers carry the screen, so
they use tabular figures. The mono face appears only where a string is
technical: a path, a hash, a sample rate, and the provenance line.

The app bundles no font file and requests no font from a network (SRS §3.4).

### Hierarchy

- **Headline** (600, 20/28): the screen title, once on a screen.
- **Title** (600, 15/22): a section title and the file name in the table.
- **Body** (400, 13/18): table text, field labels, and every sentence of copy.
- **Label** (600, 11/16, 0.04em, upright case): a column header, a chip, and the
  device or model state in the rail footer.
- **Mono** (400, 12/18): a path, a hash, a bitrate, and the provenance string.

### Named Rules

**The Tabular Rule.** The cutoff, duration, ETA, count, and progress columns use
tabular figures. A number that shifts its width while it counts is a defect.

## Layout

The window opens at 1100 x 720 and never goes below 800 x 600. The rail is
216 px wide at every size and does not collapse. The work area is fluid and
stops at 1280 px, so a long table stays readable on a wide display.

Two widths govern the layout. 800 px is the floor, and every screen must work
there without horizontal scrolling. 1280 px is the ceiling, where the work
area stops growing. The stylesheet needs no other width breakpoint. No column
is hidden and no control is dropped at the narrow end. A long file name
truncates with an ellipsis and keeps the full name in its title.

The spacing rhythm is 4 px. Named steps run 4, 8, 12, 16, 24, 32, and 48 px.
A table row is 36 px high. A control keeps a 44 px hit area, even where its
drawn shape is smaller.

The layout also works at 200% zoom at 800 x 600. That is the same problem as
a narrow window, and the same rule answers it: reflow, do not clip.

## Elevation & Depth

The system is flat. It has no shadow token and no state that adds one. Depth
comes from two tonal steps and one edge: `canvas` sits behind `surface`, and a
1 px `border` tells a panel from the page. A panel that must sit above the page
uses `surface` and a full 1 px outline in `border`.

**The Flat Rule.** A control does not lift on hover. It changes color, and the
change completes in 80 ms.

## Shapes

Corners are gentle and small. A button and an input use a 4 px radius. A panel
and a dialog use 6 px. A status chip is a full pill. A table, a rail, and a
meter track keep square corners.

Every edge is 1 px. A focus ring is 2 px wide with a 2 px offset, so it never
touches the shape it marks. The cutoff meter is a 3 px track with a 1 px
marker.

## Components

### Buttons

- **Shape:** 4 px radius (`sm`), 1 px outline, 32 px drawn height.
- **Primary:** `accent` fill, `accent-ink` label, no outline.
- **Secondary:** `surface` fill, `control-edge` outline, `ink` label.
- **Hover and press:** an 80 ms color change. No transform and no shadow.
- **Focus:** a 2 px `focus` ring with a 2 px offset.

### Fields

- **Shape:** `surface` fill, 1 px `control-edge` outline, 4 px radius.
- **Focus:** the outline stays, and a 2 px `focus` ring follows it at a 2 px
  offset.
- **Error:** a 1 px `danger` outline, a `danger` message under the field, and
  the field keeps its label and its value.

### Chips

- **Shape:** a full pill, 1 px `control-edge` outline, label type.
- **Verdict chip:** a shape, a word, and a color. The shape carries the same
  meaning as the word, so the chip survives a grayscale screen.
- **Selected filter:** `accent` text with a 1 px `accent` outline.

### Rows

- **Shape:** 36 px high, `surface` fill, a 1 px `border` rule between rows.
- **Selected:** the row keeps its fill and gains a 2 px `accent` marker on the
  leading edge.
- **Unreadable:** the row is not selectable, shows `Can't read` in
  `danger`, and keeps the decoder reason on the row.

### Motion

Motion is short and quiet. The A/B switch in the compare player has no
crossfade, because the comparison must land on the same sample (SRS §10.2).

- `motion-fast`: 80 ms, for hover and press.
- `motion-panel`: 140 ms, for a panel change and a step change in first run.
- `ease-out`: `cubic-bezier(0, 0, 0.2, 1)`, for both durations.

**The Reduced Motion Rule.** When the OS asks for reduced motion, every
transform is removed. Opacity and color changes stay, because they carry the
state and they do not move. No component is exempt.

## Do's and Don'ts

### Do:

- **Do** use `control-edge` for the outline of every input, select, checkbox,
  chip, and secondary button. Leave `border` for dividers.
- **Do** pair every state color with a word and a shape.
- **Do** use the system font stacks and tabular figures in a numeric column.
- **Do** keep a focus ring at 2 px with a 2 px offset on every control.
- **Do** keep motion at 80 ms for hover and press and 140 ms for a panel.
- **Do** mark the restored side with the words "Generated detail".

### Don't:

- **Don't** use album art as a hero image or as a browsing surface.
- **Don't** use a waveform or an equalizer as decoration.
- **Don't** describe the output as "AI restored", "lossless", "original
  quality", or "recovered" (SRS §1.4).
- **Don't** use a shadow to lift a control or a panel.
- **Don't** show a state in color alone.
- **Don't** animate a transform when the OS asks for reduced motion.
- **Don't** bundle a font file or request one from a network.
