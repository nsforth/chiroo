# Branding

Product branding is configured two ways: **values** via `BRAND_*` environment
variables in `server/.env`, and **assets** (logos, icons, fonts) as files at fixed
paths under `server/assets/branding/`. The backend exposes the result via
`GET /api/branding`; the frontend keeps no branding assets or defaults of its own,
so applying branding never requires a frontend rebuild.

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
| `BRAND_FONT_FAMILY` | `Josefin Sans` | CSS family name of the main (screen) typeface |
| `BRAND_MONO_FONT_FAMILY` | `Source Code Pro` | CSS family name of the monospace typeface |

Only the font *family labels* are overridable. Override them when you replace the
shipped font files with your own typeface, so the name shown in the browser matches
the actual font while debugging. There are no path variables — all asset files live
at fixed locations (see below) and are rebranded by replacing the file in place.

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

## Asset files

All asset files live under `server/assets/branding/` (resolved relative to the
server working directory, i.e. `assets/branding/…` when the server runs from
`server/`). To rebrand, replace the file at its fixed path — no configuration
needed.

| File | Role |
|---|---|
| `logo.png` | Sidebar logo. Keep it compact — it renders at a fixed height with no width limit |
| `favicon.png` | Browser favicon |
| `assistant.png` | Assistant/mascot icon next to the home greeting |
| `font.ttf` | Main (screen) typeface — drives the whole UI (`--font-sans`) |
| `font-italic.ttf` | Optional italic face of the main typeface |
| `font-mono.ttf` | Monospace typeface for code (`--font-mono`) |
| `scenarios/…` | Optional per-scenario icons (see below) |

## Fonts

The app ships default typefaces bundled under `server/assets/branding/` — Josefin
Sans (`font.ttf`) with an italic face (`font-italic.ttf`), and Source Code Pro
(`font-mono.ttf`). All are variable fonts (`.ttf`). The main and mono typefaces are
**required**: they are loaded at startup and a missing/unreadable file aborts
startup. Only the italic face is optional.

- **Main (screen) typeface** — `font.ttf`, family label `BRAND_FONT_FAMILY`
  (default `Josefin Sans`). Drives `--font-sans`. To switch the typeface, replace
  `font.ttf` and, if you have one, `font-italic.ttf`.
- **Italic** — purely file-presence based: if `font-italic.ttf` exists it is served
  as a second `@font-face` (`font-style: italic`) and the browser uses the real
  italic glyphs for emphasis (`<em>` in markdown); if the file is absent, the
  browser falls back to a synthesized oblique. When you replace `font.ttf` with a
  different typeface, also replace or remove `font-italic.ttf` accordingly.
- **Monospace** — `font-mono.ttf`, family label `BRAND_MONO_FONT_FAMILY` (default
  `Source Code Pro`). Used for code (`--font-mono`); no italic variant.

Supported formats for replaced files: `.ttf`, `.otf`, `.woff`, `.woff2`.

## Scenario icons

Scenario icons are opt-in. Drop an icon file into `server/assets/branding/scenarios/`
named after the scenario id: `<id>.<ext>` (e.g. `simple.png`, `deep_research.svg`).
Supported extensions, in priority order when several match: `png`, `svg`, `webp`,
`jpg`, `jpeg`. Icons are resolved once at server startup; a scenario without a
matching file has no custom icon and falls back to its built-in icon. The scenario
list itself lives in code, not in configuration.
