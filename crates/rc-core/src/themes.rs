//! Curated themes and the shared semantic palette.
//!
//! A theme is a named set of colours for the *browser chrome* — the toolbar,
//! tabs, new-tab page, error pages and the privacy dashboard. The palette uses
//! semantic slots rather than widget names, so the GTK shell (native widgets,
//! `@define-color` CSS) and the Win32 shell (`chrome.html` custom properties)
//! can each map the same colours into their own substrate without the logic
//! drifting apart.
//!
//! The two defaults (`light`, `dark`) use **exactly** the colours the 0.2.x
//! shells shipped with, slot for slot, so switching to the registry does not
//! restyle an existing user's browser. The extra themes are tasteful and
//! deliberately few. The tests here enforce that every theme fills every slot
//! with a valid `#rrggbb`, and that a theme's accent stays distinguishable
//! from its background (the progress bar, links and caret depend on it).

/// Semantic colour slots. Every theme fills every slot; a missing slot is a
/// test failure, because a shell that reads a missing colour would silently
/// restyle or, worse, emit an invalid value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Window / page background (light `#ffffff`, dark `#202124`).
    pub bg: &'static str,
    /// Headerbar (tab strip row) background.
    pub header: &'static str,
    /// Toolbar, bookmarks bar and menus background.
    pub toolbar: &'static str,
    /// Generic hover surface on the chrome (toolbar buttons, menu items).
    pub surface: &'static str,
    /// Headerbar controls hover (title buttons, "+" new-tab button).
    pub header_hover: &'static str,
    /// Inactive tab background (transparent in practice) and body text colour.
    pub tab: &'static str,
    /// Tab strip hover background.
    pub tab_hover: &'static str,
    /// Active tab background.
    pub tab_active: &'static str,
    /// Text on the active tab.
    pub tab_active_fg: &'static str,
    /// Close-tab button hover.
    pub close_hover: &'static str,
    /// URL field and input background.
    pub field: &'static str,
    /// Primary foreground: text, menu items, watched labels.
    pub fg: &'static str,
    /// Disabled control text.
    pub disabled: &'static str,
    /// Accent: caret, progress bar, links, focus ring.
    pub accent: &'static str,
    /// Hairlines and menu separators.
    pub line: &'static str,
    /// New-tab page search box background.
    pub nt_field: &'static str,
    /// New-tab page search box border + button hover border.
    pub nt_border: &'static str,
    /// New-tab page search button background.
    pub nt_btn: &'static str,
    /// New-tab page search button text.
    pub nt_btn_fg: &'static str,
    /// New-tab page footer text (version, github link).
    pub nt_foot: &'static str,
}

impl Palette {
    /// Build a GTK3 `@define-color` block from this palette. The GTK shell loads
    /// it together with its (theme-independent) rule sheet, so one CSS source
    /// works for every theme: only the colour definitions change.
    pub fn define_colors(&self) -> String {
        format!(
            "@define-color rc_bg {bg};\n\
             @define-color rc_header {header};\n\
             @define-color rc_toolbar {toolbar};\n\
             @define-color rc_surface {surface};\n\
             @define-color rc_header_hover {header_hover};\n\
             @define-color rc_tab {tab};\n\
             @define-color rc_tab_hover {tab_hover};\n\
             @define-color rc_tab_active {tab_active};\n\
             @define-color rc_tab_active_fg {tab_active_fg};\n\
             @define-color rc_close_hover {close_hover};\n\
             @define-color rc_field {field};\n\
             @define-color rc_fg {fg};\n\
             @define-color rc_disabled {disabled};\n\
             @define-color rc_accent {accent};\n\
             @define-color rc_line {line};\n\
             @define-color rc_nt_field {nt_field};\n\
             @define-color rc_nt_border {nt_border};\n\
             @define-color rc_nt_btn {nt_btn};\n\
             @define-color rc_nt_btn_fg {nt_btn_fg};\n\
             @define-color rc_nt_foot {nt_foot};\n",
            bg = self.bg,
            header = self.header,
            toolbar = self.toolbar,
            surface = self.surface,
            header_hover = self.header_hover,
            tab = self.tab,
            tab_hover = self.tab_hover,
            tab_active = self.tab_active,
            tab_active_fg = self.tab_active_fg,
            close_hover = self.close_hover,
            field = self.field,
            fg = self.fg,
            disabled = self.disabled,
            accent = self.accent,
            line = self.line,
            nt_field = self.nt_field,
            nt_border = self.nt_border,
            nt_btn = self.nt_btn,
            nt_btn_fg = self.nt_btn_fg,
            nt_foot = self.nt_foot,
        )
    }

    /// The same palette as CSS custom properties, for shells that render their
    /// chrome in HTML (`chrome.html` on Windows). Keys are the `--rc-*` names.
    pub fn css_vars(&self) -> String {
        format!(
            "--rc-bg:{bg};--rc-header:{header};--rc-toolbar:{toolbar};--rc-surface:{surface};\
             --rc-header-hover:{header_hover};--rc-tab:{tab};--rc-tab-hover:{tab_hover};\
             --rc-tab-active:{tab_active};--rc-tab-active-fg:{tab_active_fg};\
             --rc-close-hover:{close_hover};--rc-field:{field};--rc-fg:{fg};\
             --rc-disabled:{disabled};--rc-accent:{accent};--rc-line:{line};\
             --rc-nt-field:{nt_field};--rc-nt-border:{nt_border};--rc-nt-btn:{nt_btn};\
             --rc-nt-btn-fg:{nt_btn_fg};--rc-nt-foot:{nt_foot};",
            bg = self.bg,
            header = self.header,
            toolbar = self.toolbar,
            surface = self.surface,
            header_hover = self.header_hover,
            tab = self.tab,
            tab_hover = self.tab_hover,
            tab_active = self.tab_active,
            tab_active_fg = self.tab_active_fg,
            close_hover = self.close_hover,
            field = self.field,
            fg = self.fg,
            disabled = self.disabled,
            accent = self.accent,
            line = self.line,
            nt_field = self.nt_field,
            nt_border = self.nt_border,
            nt_btn = self.nt_btn,
            nt_btn_fg = self.nt_btn_fg,
            nt_foot = self.nt_foot,
        )
    }

    /// The new-tab page colours, in the tuple order the shipped template needs:
    /// `(bg, fg, field, field_border, btn_bg, btn_fg, foot)`. Distinct from the
    /// chrome-slots because a theme's page should read like the chrome without
    /// necessarily sharing exact field/button hues.
    pub fn new_tab_colors(
        &self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ) {
        (
            self.bg,
            self.fg,
            self.nt_field,
            self.nt_border,
            self.nt_btn,
            self.nt_btn_fg,
            self.nt_foot,
        )
    }

    /// A `chrome.html`-style CSS block scoped to `html[data-theme="<id>"]`.
    /// `chrome.html` (Windows) names its colours `--window`, `--headerbar`,
    /// etc., so this maps the shared palette onto those names. Used by the
    /// shell to inject every theme into the written chrome at startup; the page
    /// then flips themes by setting `data-theme` from the persisted id.
    pub fn windows_chrome_block(&self, theme_id: &str) -> String {
        format!(
            "html[data-theme=\"{id}\"]{{\n  --window: {bg};\n  --headerbar: {header};\n  \
             --toolbar: {toolbar};\n  --text: {fg};\n  --tab-text: {tab};\n  \
             --tab-hover: {tab_hover};\n  --tab-active-bg: {tab_active};\n  \
             --tab-active-fg: {tab_active_fg};\n  --close-hover: {close_hover};\n  \
             --url-bg: {field};\n  --url-fg: {fg};\n  --url-focus-bg: {bg};\n  \
             --url-border: {accent};\n  --btn-fg: {tab};\n  --btn-hover: {surface};\n  \
             --bm-fg: {tab_active_fg};\n  --bm-hover: {surface};\n  --menu-bg: {toolbar};\n  \
             --menu-fg: {fg};\n  --menu-hover: {surface};\n  --sep: {line};\n  \
             --accent: {accent};\n  --shadow: rgba(0,0,0,.28);\n}}\n",
            id = theme_id,
            bg = self.bg,
            header = self.header,
            toolbar = self.toolbar,
            surface = self.surface,
            tab = self.tab,
            tab_hover = self.tab_hover,
            tab_active = self.tab_active,
            tab_active_fg = self.tab_active_fg,
            close_hover = self.close_hover,
            field = self.field,
            fg = self.fg,
            accent = self.accent,
            line = self.line,
        )
    }
}

/// One selectable theme.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Stable id stored in `prefs.json`. Never change an id once shipped.
    pub id: &'static str,
    /// Human label for the theme picker.
    pub label: &'static str,
    pub is_dark: bool,
    pub palette: Palette,
}

/// The shipped themes, in the order the picker shows them. The first is the
/// default. Ids are stable; never reorder, remove or rename.
///
/// `light` and `dark` reproduce the 0.2.x shell colours exactly.
pub const THEMES: &[Theme] = &[
    Theme {
        id: "light",
        label: "Light",
        is_dark: false,
        palette: Palette {
            bg: "#ffffff",
            header: "#dee1e6",
            toolbar: "#ffffff",
            surface: "#f1f3f4",
            header_hover: "#cfd3d9",
            tab: "#5f6368",
            tab_hover: "#cfd3d9",
            tab_active: "#ffffff",
            tab_active_fg: "#202124",
            close_hover: "#dadce0",
            field: "#f1f3f4",
            fg: "#202124",
            disabled: "#bdc1c6",
            accent: "#1a73e8",
            line: "#e8eaed",
            nt_field: "#ffffff",
            nt_border: "#dfe1e5",
            nt_btn: "#f8f9fa",
            nt_btn_fg: "#3c4043",
            nt_foot: "#70757a",
        },
    },
    Theme {
        id: "dark",
        label: "Dark",
        is_dark: true,
        palette: Palette {
            bg: "#202124",
            header: "#202124",
            toolbar: "#35363a",
            surface: "#4a4d51",
            header_hover: "#35363a",
            tab: "#9aa0a6",
            tab_hover: "#2d2f31",
            tab_active: "#35363a",
            tab_active_fg: "#e8eaed",
            close_hover: "#4a4d51",
            field: "#202124",
            fg: "#e8eaed",
            disabled: "#5f6368",
            accent: "#8ab4f8",
            line: "#4a4d51",
            nt_field: "#202124",
            nt_border: "#5f6368",
            nt_btn: "#303134",
            nt_btn_fg: "#e8eaed",
            nt_foot: "#9aa0a6",
        },
    },
    Theme {
        id: "graphite",
        label: "Graphite",
        is_dark: true,
        palette: Palette {
            bg: "#2b2d31",
            header: "#292b2f",
            toolbar: "#313338",
            surface: "#3f434a",
            header_hover: "#383b40",
            tab: "#9aa4b0",
            tab_hover: "#363940",
            tab_active: "#3b3f46",
            tab_active_fg: "#e6e7ea",
            close_hover: "#454a52",
            field: "#222428",
            fg: "#e6e7ea",
            disabled: "#5f6672",
            accent: "#8ab4f8",
            line: "#3d4148",
            nt_field: "#222428",
            nt_border: "#3d4148",
            nt_btn: "#3f434a",
            nt_btn_fg: "#e6e7ea",
            nt_foot: "#9aa4b0",
        },
    },
    Theme {
        id: "sepia",
        label: "Sepia",
        is_dark: false,
        palette: Palette {
            bg: "#f4ecd8",
            header: "#e7dcc0",
            toolbar: "#efe4c9",
            surface: "#e3d7b5",
            header_hover: "#dccfa9",
            tab: "#6b5d43",
            tab_hover: "#e3d5ad",
            tab_active: "#f8f1de",
            tab_active_fg: "#433422",
            close_hover: "#d9cba3",
            field: "#ece2c8",
            fg: "#433422",
            disabled: "#a99a78",
            accent: "#a0522d",
            line: "#d8c9a8",
            nt_field: "#ece2c8",
            nt_border: "#cabb96",
            nt_btn: "#ded0ac",
            nt_btn_fg: "#433422",
            nt_foot: "#8a7a5c",
        },
    },
    Theme {
        id: "midnight",
        label: "Midnight",
        is_dark: true,
        palette: Palette {
            bg: "#10131c",
            header: "#0e1120",
            toolbar: "#181d2e",
            surface: "#232a3d",
            header_hover: "#1f2638",
            tab: "#8f9bb8",
            tab_hover: "#1d2336",
            tab_active: "#26304a",
            tab_active_fg: "#e8ecf4",
            close_hover: "#2b3550",
            field: "#0c0f16",
            fg: "#e8ecf4",
            disabled: "#4c5670",
            accent: "#5aa7ff",
            line: "#232a3d",
            nt_field: "#0c0f16",
            nt_border: "#2b3550",
            nt_btn: "#26304a",
            nt_btn_fg: "#e8ecf4",
            nt_foot: "#8f9bb8",
        },
    },
    Theme {
        id: "nord",
        label: "Nord",
        is_dark: true,
        palette: Palette {
            bg: "#2e3440",
            header: "#2b313c",
            toolbar: "#3b4252",
            surface: "#434c5e",
            header_hover: "#3a4252",
            tab: "#9ca8bd",
            tab_hover: "#37404f",
            tab_active: "#4c566a",
            tab_active_fg: "#eceff4",
            close_hover: "#4a5468",
            field: "#272c36",
            fg: "#eceff4",
            disabled: "#616e88",
            accent: "#88c0d0",
            line: "#3b4252",
            nt_field: "#272c36",
            nt_border: "#4a5468",
            nt_btn: "#434c5e",
            nt_btn_fg: "#eceff4",
            nt_foot: "#9ca8bd",
        },
    },
];

/// The theme a fresh install boots into.
pub const DEFAULT_ID: &str = "light";

/// Look a theme up by id. `None` for anything unknown, so callers can fall back
/// rather than accidentally load a half-initialised theme.
pub fn by_id(id: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.id == id)
}

/// Resolve any theme string to a known `Theme`, falling back to the default.
pub fn resolve(id: &str) -> &'static Theme {
    by_id(id).unwrap_or_else(|| by_id(DEFAULT_ID).expect("default theme exists"))
}

/// `true` when `id` is a known theme id.
pub fn known(id: &str) -> bool {
    by_id(id).is_some()
}

/// All theme ids, for the picker.
pub fn all_ids() -> Vec<&'static str> {
    THEMES.iter().map(|t| t.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_stable() {
        let mut seen = std::collections::HashSet::new();
        for t in THEMES {
            assert!(seen.insert(t.id), "duplicate or empty theme id: {:?}", t.id);
            assert!(t.id.len() >= 2, "theme id too short: {:?}", t.id);
        }
    }

    #[test]
    fn first_theme_is_the_default_and_is_light() {
        assert_eq!(THEMES[0].id, DEFAULT_ID);
        assert!(!THEMES[0].is_dark);
        assert_eq!(by_id(DEFAULT_ID).unwrap().id, DEFAULT_ID);
    }

    #[test]
    fn every_slot_is_a_hex_colour_in_every_theme() {
        for t in THEMES {
            let p = &t.palette;
            for (name, c) in [
                ("bg", p.bg),
                ("header", p.header),
                ("toolbar", p.toolbar),
                ("surface", p.surface),
                ("header_hover", p.header_hover),
                ("tab", p.tab),
                ("tab_hover", p.tab_hover),
                ("tab_active", p.tab_active),
                ("tab_active_fg", p.tab_active_fg),
                ("close_hover", p.close_hover),
                ("field", p.field),
                ("fg", p.fg),
                ("disabled", p.disabled),
                ("accent", p.accent),
                ("line", p.line),
                ("nt_field", p.nt_field),
                ("nt_border", p.nt_border),
                ("nt_btn", p.nt_btn),
                ("nt_btn_fg", p.nt_btn_fg),
                ("nt_foot", p.nt_foot),
            ] {
                assert_eq!(
                    c.len(),
                    7,
                    "theme {} slot {name} must be #rrggbb, got {c:?}",
                    t.id
                );
                assert!(
                    c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()),
                    "theme {} slot {name} is not a hex colour: {c:?}",
                    t.id
                );
            }
        }
    }

    #[test]
    fn accent_differs_from_background_in_every_theme() {
        fn rgb(hex: &str) -> (u8, u8, u8) {
            (
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
            )
        }
        for t in THEMES {
            let (ar, ag, ab) = rgb(t.palette.accent);
            let (br, bg_, bb) = rgb(t.palette.bg);
            let d2 = ((ar as i64 - br as i64).pow(2))
                + ((ag as i64 - bg_ as i64).pow(2))
                + ((ab as i64 - bb as i64).pow(2));
            assert!(
                d2 >= 10_000,
                "theme {}: accent {}/bg {} too similar (d2={d2})",
                t.id,
                t.palette.accent,
                t.palette.bg
            );
        }
    }

    #[test]
    fn light_and_dark_reproduce_historic_colours() {
        let light = by_id("light").unwrap();
        let dark = by_id("dark").unwrap();
        assert!(!light.is_dark);
        assert!(dark.is_dark);
        // Chrome slots are unchanged from 0.2.x.
        assert_eq!(light.palette.bg, "#ffffff");
        assert_eq!(light.palette.header, "#dee1e6");
        assert_eq!(light.palette.tab_active, "#ffffff");
        assert_eq!(light.palette.accent, "#1a73e8");
        assert_eq!(dark.palette.bg, "#202124");
        assert_eq!(dark.palette.header, "#202124");
        assert_eq!(dark.palette.toolbar, "#35363a");
        assert_eq!(dark.palette.tab_active, "#35363a");
        assert_eq!(dark.palette.accent, "#8ab4f8");
        // New-tab slots are unchanged from 0.2.x.
        assert_eq!(light.palette.new_tab_colors().3, "#dfe1e5"); // field border
        assert_eq!(light.palette.new_tab_colors().6, "#70757a"); // footer
        assert_eq!(dark.palette.new_tab_colors().2, "#202124"); // field
        assert_eq!(dark.palette.new_tab_colors().6, "#9aa0a6"); // footer
    }

    #[test]
    fn resolve_falls_back_to_default_and_unknown_ids_are_known_wrong() {
        assert_eq!(resolve("light").id, "light");
        assert_eq!(resolve("nope").id, DEFAULT_ID);
        assert_eq!(resolve("").id, DEFAULT_ID);
        assert!(!known("nope"));
        assert!(known("nord"));
        assert_eq!(all_ids().len(), THEMES.len());
    }

    #[test]
    fn exactly_two_light_themes() {
        // The default and sepia. This keeps the "Light" picker from being a
        // whole row of near-identical whites.
        let n_light = THEMES.iter().filter(|t| !t.is_dark).count();
        assert_eq!(n_light, 2);
    }

    #[test]
    fn emitters_mention_every_slot() {
        let t = by_id("nord").unwrap().palette;
        let block = t.define_colors();
        for slot in [
            "rc_bg",
            "rc_header",
            "rc_toolbar",
            "rc_surface",
            "rc_header_hover",
            "rc_tab",
            "rc_tab_hover",
            "rc_tab_active",
            "rc_tab_active_fg",
            "rc_close_hover",
            "rc_field",
            "rc_fg",
            "rc_disabled",
            "rc_accent",
            "rc_line",
            "rc_nt_field",
            "rc_nt_border",
            "rc_nt_btn",
            "rc_nt_btn_fg",
            "rc_nt_foot",
        ] {
            assert!(block.contains(slot), "GTK missing {slot} in {block}");
        }
        let vars = t.css_vars();
        for slot in [
            "--rc-bg",
            "--rc-header",
            "--rc-toolbar",
            "--rc-surface",
            "--rc-header-hover",
            "--rc-tab",
            "--rc-tab-hover",
            "--rc-tab-active",
            "--rc-tab-active-fg",
            "--rc-close-hover",
            "--rc-field",
            "--rc-fg",
            "--rc-disabled",
            "--rc-accent",
            "--rc-line",
            "--rc-nt-field",
            "--rc-nt-border",
            "--rc-nt-btn",
            "--rc-nt-btn-fg",
            "--rc-nt-foot",
        ] {
            assert!(vars.contains(slot), "HTML missing {slot} in {vars}");
        }
        assert!(block.contains("#2e3440")); // nord bg
    }

    #[test]
    fn new_tab_colors_are_dark_aware() {
        let dark = by_id("dark").unwrap();
        assert_eq!(dark.palette.new_tab_colors().0, "#202124"); // bg
        assert_eq!(dark.palette.new_tab_colors().1, "#e8eaed"); // fg
        let light = by_id("light").unwrap();
        assert_eq!(light.palette.new_tab_colors().0, "#ffffff");
        assert_eq!(light.palette.new_tab_colors().1, "#202124");
    }

    #[test]
    fn windows_chrome_block_matches_chrome_html_variable_names() {
        // These are the variable names chrome.html actually reads. If a mapping
        // here stops matching, the Windows chrome silently falls back to ITS
        // hardcoded light/dark blocks, so the test is the guard.
        let names = [
            "--window",
            "--headerbar",
            "--toolbar",
            "--text",
            "--tab-text",
            "--tab-hover",
            "--tab-active-bg",
            "--tab-active-fg",
            "--close-hover",
            "--url-bg",
            "--url-fg",
            "--url-focus-bg",
            "--url-border",
            "--btn-fg",
            "--btn-hover",
            "--bm-fg",
            "--bm-hover",
            "--menu-bg",
            "--menu-fg",
            "--menu-hover",
            "--sep",
            "--accent",
        ];
        for t in THEMES {
            let block = t.palette.windows_chrome_block(t.id);
            assert!(
                block.contains(&format!("html[data-theme=\"{}\"]", t.id)),
                "missing selector for {}",
                t.id
            );
            for name in names {
                assert!(
                    block.contains(name),
                    "theme {} block missing chrome variable {name}",
                    t.id
                );
            }
        }
        // The light block must reproduce the shipped light chrome colours.
        let light = by_id("light").unwrap();
        let block = light.palette.windows_chrome_block("light");
        assert!(
            block.contains("--window: #ffffff;"),
            "light bg drifted: {block}"
        );
        assert!(
            block.contains("--headerbar: #dee1e6;"),
            "light header drifted"
        );
        assert!(block.contains("--accent: #1a73e8;"), "light accent drifted");
        // Dark likewise.
        let dark = by_id("dark").unwrap();
        let block = dark.palette.windows_chrome_block("dark");
        assert!(block.contains("--window: #202124;"), "dark bg drifted");
        assert!(
            block.contains("--toolbar: #35363a;"),
            "dark toolbar drifted"
        );
        assert!(block.contains("--accent: #8ab4f8;"), "dark accent drifted");
    }
}
