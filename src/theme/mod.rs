use egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeId {
    Light,
    Rust,
    Coal,
    Navy,
    Ayu,
}

impl ThemeId {
    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Light => "Light",
            ThemeId::Rust  => "Rust",
            ThemeId::Coal  => "Coal",
            ThemeId::Navy  => "Navy",
            ThemeId::Ayu   => "Ayu",
        }
    }
}

pub struct Theme {
    pub id:   ThemeId,
    pub name: &'static str,

    // Surfaces
    pub bg:          Color32,  // central panel / preview background
    pub sidebar_bg:  Color32,  // sidebar + outline panel
    pub toolbar_bg:  Color32,  // top toolbar + status bar
    pub tab_bar_bg:  Color32,  // tab strip

    // Text
    pub fg:             Color32,  // body text
    pub fg_muted:       Color32,  // secondary / placeholder text
    pub sidebar_fg:     Color32,  // sidebar labels
    pub sidebar_active: Color32,  // active item accent

    // Interactive
    pub link:         Color32,  // hyperlinks
    pub selection_bg: Color32,  // text selection

    // Code
    pub code_bg:        Color32,  // code block frame
    pub inline_code_fg: Color32,  // `inline code` text

    // Structure
    pub separator: Color32,  // hr, dividers
    pub quote_bg:  Color32,  // blockquote tint
}

pub const THEMES: &[Theme] = &[
    // ── Light — "Manuscript" ─────────────────────────────────────────────────
    // Warm cream paper. The writing desk at golden hour.
    // Sapphire accent pops against the warm neutrals without jarring.
    Theme {
        id:   ThemeId::Light,
        name: "Light",
        bg:          Color32::from_rgb(252, 249, 243), // warm cream, not sterile white
        sidebar_bg:  Color32::from_rgb(241, 237, 228), // a notch darker, same warmth
        toolbar_bg:  Color32::from_rgb(232, 228, 218), // toolbar sits visually below sidebar
        tab_bar_bg:  Color32::from_rgb(222, 217, 206), // distinct strip
        fg:          Color32::from_rgb(30, 27, 22),    // warm near-black ink
        fg_muted:    Color32::from_rgb(110, 104, 92),  // warm mid-tone
        sidebar_fg:  Color32::from_rgb(45, 42, 36),
        sidebar_active: Color32::from_rgb(12, 88, 188), // deep sapphire
        link:           Color32::from_rgb(12, 88, 188),
        selection_bg:   Color32::from_rgb(185, 215, 255),
        code_bg:        Color32::from_rgb(230, 226, 216),
        inline_code_fg: Color32::from_rgb(172, 20, 92), // distinctive crimson
        separator:      Color32::from_rgb(200, 195, 184),
        quote_bg:       Color32::from_rgb(230, 226, 216),
    },

    // ── Rust — "Forge" ───────────────────────────────────────────────────────
    // Charred wood sidebar, amber parchment content.
    // The feeling of a blacksmith's workshop in late afternoon light.
    Theme {
        id:   ThemeId::Rust,
        name: "Rust",
        bg:          Color32::from_rgb(244, 238, 220), // warm amber parchment
        sidebar_bg:  Color32::from_rgb(50, 29, 14),    // charred deep brown
        toolbar_bg:  Color32::from_rgb(62, 38, 20),    // slightly lighter toolbar
        tab_bar_bg:  Color32::from_rgb(38, 22, 10),    // near-black brown strip
        fg:          Color32::from_rgb(40, 30, 12),    // dark warm ink
        fg_muted:    Color32::from_rgb(105, 82, 56),   // warm tan-gray
        sidebar_fg:  Color32::from_rgb(238, 210, 165), // warm golden cream
        sidebar_active: Color32::from_rgb(225, 128, 30), // cooling ember orange
        link:           Color32::from_rgb(165, 58, 10),  // rusty red
        selection_bg:   Color32::from_rgb(200, 148, 72),
        code_bg:        Color32::from_rgb(226, 215, 192),
        inline_code_fg: Color32::from_rgb(150, 28, 55),  // warm crimson
        separator:      Color32::from_rgb(188, 165, 132),
        quote_bg:       Color32::from_rgb(226, 215, 192),
    },

    // ── Coal — "Graphite" ────────────────────────────────────────────────────
    // Refined warm charcoal. Not the cold blue-gray that plagues dark modes.
    // Like artist charcoal on smooth black paper.
    Theme {
        id:   ThemeId::Coal,
        name: "Coal",
        bg:          Color32::from_rgb(18, 18, 20),   // near-black with warmth
        sidebar_bg:  Color32::from_rgb(26, 26, 29),   // subtle lift for sidebar
        toolbar_bg:  Color32::from_rgb(22, 22, 24),   // between bg and sidebar
        tab_bar_bg:  Color32::from_rgb(13, 13, 15),   // darkest surface
        fg:          Color32::from_rgb(240, 236, 228), // warm off-white
        fg_muted:    Color32::from_rgb(130, 124, 114), // warm mid-gray
        sidebar_fg:  Color32::from_rgb(212, 208, 200),
        sidebar_active: Color32::from_rgb(70, 165, 255), // sharp electric blue
        link:           Color32::from_rgb(70, 165, 255),
        selection_bg:   Color32::from_rgb(36, 82, 132),
        code_bg:        Color32::from_rgb(23, 23, 26),
        inline_code_fg: Color32::from_rgb(232, 168, 58), // warm amber on dark
        separator:      Color32::from_rgb(38, 38, 44),
        quote_bg:       Color32::from_rgb(23, 23, 26),
    },

    // ── Navy — "Midnight Deep" ───────────────────────────────────────────────
    // Rich saturated ocean. Not washed-out steel blue — committed deep navy.
    // Bright cyan cuts through like bioluminescence.
    Theme {
        id:   ThemeId::Navy,
        name: "Navy",
        bg:          Color32::from_rgb(18, 30, 58),    // saturated deep navy
        sidebar_bg:  Color32::from_rgb(10, 18, 38),    // darker abyss
        toolbar_bg:  Color32::from_rgb(13, 23, 45),    // between
        tab_bar_bg:  Color32::from_rgb(7, 13, 28),     // near-black navy
        fg:          Color32::from_rgb(215, 232, 255),  // light blue-tinted white
        fg_muted:    Color32::from_rgb(108, 142, 195),  // desaturated mid blue
        sidebar_fg:  Color32::from_rgb(186, 214, 252),
        sidebar_active: Color32::from_rgb(50, 198, 255), // bioluminescent cyan
        link:           Color32::from_rgb(50, 198, 255),
        selection_bg:   Color32::from_rgb(26, 66, 122),
        code_bg:        Color32::from_rgb(11, 20, 42),
        inline_code_fg: Color32::from_rgb(144, 214, 255), // softer cyan
        separator:      Color32::from_rgb(30, 46, 82),
        quote_bg:       Color32::from_rgb(13, 23, 48),
    },

    // ── Ayu — "Ember" ────────────────────────────────────────────────────────
    // Near-void darkness with golden amber warmth. Staring into dying embers.
    // The contrast between cold darkness and warm fire is the whole point.
    Theme {
        id:   ThemeId::Ayu,
        name: "Ayu",
        bg:          Color32::from_rgb(10, 14, 20),    // deep void, slight blue-dark
        sidebar_bg:  Color32::from_rgb(15, 20, 29),    // subtle rise
        toolbar_bg:  Color32::from_rgb(11, 16, 23),    // between
        tab_bar_bg:  Color32::from_rgb(7, 10, 14),     // near-black
        fg:          Color32::from_rgb(196, 192, 184),  // warm star-white
        fg_muted:    Color32::from_rgb(78, 88, 106),    // cool-dark mid
        sidebar_fg:  Color32::from_rgb(165, 170, 186),
        sidebar_active: Color32::from_rgb(255, 152, 26), // golden fire amber
        link:           Color32::from_rgb(46, 180, 222),  // cold cyan (contrast)
        selection_bg:   Color32::from_rgb(26, 43, 68),
        code_bg:        Color32::from_rgb(13, 18, 25),
        inline_code_fg: Color32::from_rgb(255, 152, 26),  // amber matches accent
        separator:      Color32::from_rgb(20, 28, 42),
        quote_bg:       Color32::from_rgb(13, 18, 25),
    },
];

pub fn theme_by_id(id: ThemeId) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap()
}
