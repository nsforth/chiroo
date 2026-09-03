# Branding

Product branding (name, label, tagline, colors, logos, fonts) is configured entirely
through `BRAND_*` environment variables in `server/.env`. The backend exposes the
result via `GET /api/branding`; the frontend keeps no branding assets or defaults of
its own, so applying branding never requires a frontend rebuild.

All branding assets are read **once at server startup**. Changing any `BRAND_*`
value or replacing an asset file requires a server restart.

## Variables

| Variable | Default | Description |
|---|---|---|
| `BRAND_NAME` | `Chiroo` | Product name: browser title, Open Graph title, default assistant name |
| `BRAND_LABEL` | — | Text shown next to the logo in the sidebar. Unset hides the label; set (even empty) renders it |
| `BRAND_TAGLINE` | — | Slogan shown under the sidebar label; used as meta description fallback |
| `BRAND_ASSISTANT_NAME` | `BRAND_NAME` | Assistant name injected into the LLM system prompt |
| `BRAND_PRIMARY_COLOR` | — | Light-theme accent hex (`#RRGGBB`); must be paired with `BRAND_NEUTRAL_COLOR` |
| `BRAND_NEUTRAL_COLOR` | — | Light-theme neutral hex |
| `BRAND_PRIMARY_DARK_COLOR` | `BRAND_PRIMARY_COLOR` | Dark-theme accent hex |
| `BRAND_NEUTRAL_DARK_COLOR` | `BRAND_NEUTRAL_COLOR` | Dark-theme neutral hex |
| `BRAND_LOGO_PATH` | `assets/branding/logo.png` | Filesystem path to the product logo served by the backend |
| `BRAND_FAVICON_PATH` | `assets/branding/favicon.png` | Filesystem path to the favicon served by the backend |
| `BRAND_ASSISTANT_ICON_PATH` | `assets/branding/assistant.png` | Filesystem path to the assistant/mascot icon |
| `BRAND_SCENARIO_ICONS_DIR` | — | Directory with per-scenario icon files (opt-in) |
| `BRAND_SANS_FONT_FAMILY` | — | CSS family name for the sans typeface (pair with `BRAND_SANS_FONT_PATH`) |
| `BRAND_SANS_FONT_PATH` | — | Path to a variable font file (`.ttf`/`.otf`/`.woff`/`.woff2`) |
| `BRAND_MONO_FONT_FAMILY` | — | CSS family name for the monospace typeface (pair with `BRAND_MONO_FONT_PATH`) |
| `BRAND_MONO_FONT_PATH` | — | Path to a variable font file (`.ttf`/`.otf`/`.woff`/`.woff2`) |

## Name, label, tagline

- `BRAND_NAME` — the product name. Drives the browser title, the Open Graph title,
  and (unless overridden) the assistant name in the system prompt.
- `BRAND_LABEL` — optional text next to the sidebar logo. It has no fallback: leave
  it unset to hide the label entirely (for example when the logo already contains
  the name), or set it — even to an empty string — to render the label span.
- `BRAND_TAGLINE` — optional slogan shown under the label; also the meta description
  fallback.

## Colors

Provide `BRAND_PRIMARY_COLOR` and `BRAND_NEUTRAL_COLOR` together (both or neither).
Dark variants default to their light counterparts. All values are hex colors
(`#RRGGBB` or `#RGB`). When no colors are configured, the default Nuxt UI theme is
used. The full UI palette is derived from these seeds at runtime on the frontend.

## Assets

`BRAND_LOGO_PATH`, `BRAND_FAVICON_PATH` and `BRAND_ASSISTANT_ICON_PATH` point at
image files served by the backend. Relative paths are resolved against the server
working directory. Keep the sidebar logo reasonably compact — it is rendered at a
fixed height with no width limit.

## Fonts

Each font is configured as a `FAMILY`/`PATH` pair, which must be set together.
`FAMILY` is the CSS family name; `PATH` points at a variable font file served by the
backend. Supported formats: `.ttf`, `.otf`, `.woff`, `.woff2`.

## Scenario icons

`BRAND_SCENARIO_ICONS_DIR` optionally points at a directory of per-scenario icon
files, combined with `icon_file` on each scenario defined in
`server/src/services/scenarios/mod.rs`. The scenario list itself lives in code, not
in configuration.
