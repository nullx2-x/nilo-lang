# Nilo brand guide

This guide defines the core visual identity for the Nilo programming language and its official mascot, Niro.

## Brand idea

> **Write simply. Create freely.**

Nilo should feel readable, modern, friendly, and capable. The visual system combines a dark technical foundation with bright emerald accents.

## Names

- **Nilo**: the programming language and primary product brand
- **Niro**: the official black-cat mascot

Keep the spellings distinct in documentation and generated assets.

## Repository assets

| Asset | Path | Primary use |
| --- | --- | --- |
| Nilo wordmark | `docs/assets/nilo-logo.svg` | README headers, documentation, presentations |
| Nilo app icon | `docs/assets/nilo-icon.svg` | avatars, extension icons, favicons, small square placements |
| Niro mascot | `docs/assets/niro-mascot.svg` | README, tutorials, community and release communication |
| Niro prompt | `docs/prompts/niro-mascot.md` | consistent generation of new poses and expressions |

## Colors

| Name | Hex | Use |
| --- | --- | --- |
| Electric Emerald | `#00E5C0` | primary highlights and the N symbol |
| Deep Emerald | `#00BFA6` | gradients and secondary highlights |
| Night | `#071317` | dark surfaces and icon backgrounds |
| Ink | `#02080B` | deepest background and contrast |
| Cloud | `#F7FBFB` | light wordmarks and high-contrast text |
| Outline | `#245C58` | subtle borders and mascot outlines |

## Logo usage

- Prefer the full wordmark when the available width is at least 280 px.
- Use the square icon at small sizes, for avatars, favicons, and extension marketplaces.
- Keep clear space around the logo equal to at least one quarter of the icon height.
- Do not rotate, stretch, recolor, add shadows to, or redraw the N geometry.
- On light backgrounds, place the current light wordmark on a dark panel or use the icon by itself.

## Mascot usage

Niro is friendly, curious, technically capable, and slightly playful. Niro may introduce tutorials, highlight tips, announce releases, or represent debugging and discovery.

Required identifying features:

- black fur with blue-black highlights
- emerald eyes, inner ears, paw pads, and tail tip
- one geometric glowing N on the forehead
- a silver hexagonal pendant with an emerald N
- compact chibi proportions and an expressive face

Avoid using Niro in violent, frightening, political, adult, or misleading contexts. Do not alter the forehead symbol into another letter or logo.

## Typography

Use readable system-first sans-serif fonts for documentation and UI:

```text
Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
"Segoe UI", sans-serif
```

Use a modern monospace stack for code:

```text
"SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace
```

No font files are stored in this repository.

## Recommended exports

- README and docs: SVG preferred
- Social and marketplace images: PNG, sRGB
- App icon master: 1024 x 1024 PNG plus SVG source
- Favicon: 32 x 32 and 16 x 16 derived from the square icon
- Transparent mascot: PNG or SVG with clear padding around the silhouette

## AI-generated variations

Use `docs/prompts/niro-mascot.md` and an approved reference image. AI-generated output must be reviewed manually for character consistency, malformed anatomy, incorrect symbols, embedded text, and licensing or platform disclosure requirements.
