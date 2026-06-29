# Variadic Text Builder Node

## Symptom

The current `Append Text` primitive is too narrow for practical prompt and
terminal-input construction. It supports one incoming `text`, one `appendix`,
and a simple separator. Users need a more ergonomic way to combine multiple
pieces of text with predictable formatting.

## Current Status

Implemented today:

- `Append Text` has two inputs: `text` and `appendix`.
- Runtime behavior is `text + separator + appendix`.
- If no `appendix` input is wired, the node uses configured `appendix` text.
- First-pass `Build Text` is implemented as a separate primitive node.
- `Build Text` starts with `part_1`, `part_2`, and `part_3` text inputs.
- The inspector exposes `input_count`, separator preset, custom separator,
  skip-empty, trim-parts, and final-newline options.
- Changing `input_count` in the inspector reshapes visible input pins to
  `part_1..part_N`.
- Runtime joins connected/literal parts in port order.

## Design Direction

Keep `Append Text` as the simple suffix primitive, but introduce or evolve a
separate `Build Text` / `Join Text` node for multi-input composition.

Desired behavior:

- Variable number of text inputs.
- Global separator presets:
  - none
  - space
  - newline
  - blank line
  - custom
- Per-input prefix/suffix.
- Skip empty inputs.
- Trim inputs.
- Optional final newline for terminal input.
- Stable serialization of dynamic inputs so graph files remain portable.

Editor interaction:

- Users should be able to expand/collapse the number of inputs without editing
  JSON by hand.
- Possible UX:
  - drag the bottom of the node to reveal/add more input rows;
  - `+ input` and `- input` controls in the inspector;
  - context menu action on the node body.
- The experience should be inspired by expandable dataflow nodes such as
  LabVIEW-style function blocks, but adapted to DragonGUI's canvas editor.

## Open Questions

- Should the simple `Append Text` node become variadic, or should `Build Text`
  be a separate node?
- Should dynamic inputs be stored as ordinary `NodeGraphPort` records on the
  node, or generated from `node.data.config.inputs` at render/runtime time?
- Should per-input labels be user-editable and shown next to pins?
- Should output type stay `text`, or should there be a `terminal_input` profile
  option that enforces final newline behavior?

## First Step

Added `Build Text` as a separate node instead of overloading `Append Text`.

Implemented:

- Start with 3 input ports: `part_1`, `part_2`, `part_3`.
- Inspector fields:
  - input count
  - separator preset
  - custom separator
  - skip empty
  - trim parts
  - final newline
- Runtime combines connected values in port order.
- Inspector `+` / `-` controls on the `Inputs` row can nudge `input_count`
  without typing the number manually. Arrow keys also nudge the active
  `Inputs` field.

Remaining:

- Consider a node body resize/drag affordance for adding inputs.
- Consider per-input labels and per-input prefix/suffix.
- Decide whether `Build Text` should expose a terminal-input profile preset.
