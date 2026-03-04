use egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeId {
    Light,
    Coal,
    Navy,
    Ayu,
}

impl ThemeId {
    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Light => "Light",
            ThemeId::Coal  => "Coal",
            ThemeId::Navy  => "Navy",
            ThemeId::Ayu   => "Ayu",
        }
    }
}

pub struct Theme {
    pub id: ThemeId,

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
}

pub const THEMES: &[Theme] = &[
    // ── Light — "Manuscript" ─────────────────────────────────────────────────
    // Warm cream paper. The writing desk at golden hour.
    // Sapphire accent pops against the warm neutrals without jarring.
    Theme {
        id: ThemeId::Light,
        bg:          Color32::from_rgb(252, 249, 243), // warm cream, not sterile white
        sidebar_bg:  Color32::from_rgb(241, 237, 228), // a notch darker, same warmth
        toolbar_bg:  Color32::from_rgb(232, 228, 218), // toolbar sits visually below sidebar
        tab_bar_bg:  Color32::from_rgb(222, 217, 206), // distinct strip
        fg:          Color32::from_rgb(30, 27, 22),    // warm near-black ink
        fg_muted:    Color32::from_rgb(94, 88, 76),    // warm mid-tone — darker for better contrast
        sidebar_fg:  Color32::from_rgb(45, 42, 36),
        sidebar_active: Color32::from_rgb(14, 94, 200), // deep sapphire — slightly more vivid
        link:           Color32::from_rgb(14, 94, 200),
        selection_bg:   Color32::from_rgb(185, 215, 255),
        code_bg:        Color32::from_rgb(220, 216, 206), // more distinct from bg
        inline_code_fg: Color32::from_rgb(180, 26, 100),  // crimson — slightly brighter
        separator:      Color32::from_rgb(198, 193, 182),
    },

    // ── Coal — "Graphite" ────────────────────────────────────────────────────
    // Refined warm charcoal. Not the cold blue-gray that plagues dark modes.
    // Like artist charcoal on smooth black paper.
    Theme {
        id: ThemeId::Coal,
        bg:          Color32::from_rgb(17, 17, 19),   // near-black with warmth
        sidebar_bg:  Color32::from_rgb(27, 27, 31),   // subtle lift — slightly cooler tint
        toolbar_bg:  Color32::from_rgb(22, 22, 25),   // between bg and sidebar
        tab_bar_bg:  Color32::from_rgb(12, 12, 14),   // darkest surface
        fg:          Color32::from_rgb(240, 236, 228), // warm off-white
        fg_muted:    Color32::from_rgb(138, 132, 122), // slightly brighter than before
        sidebar_fg:  Color32::from_rgb(212, 208, 200),
        sidebar_active: Color32::from_rgb(92, 182, 255), // softened blue — less harsh
        link:           Color32::from_rgb(92, 182, 255),
        selection_bg:   Color32::from_rgb(36, 82, 132),
        code_bg:        Color32::from_rgb(21, 21, 24), // slightly darker for contrast
        inline_code_fg: Color32::from_rgb(240, 184, 72), // warmer amber
        separator:      Color32::from_rgb(40, 40, 46),
    },

    // ── Navy — "Midnight Deep" ───────────────────────────────────────────────
    // Rich saturated ocean. Not washed-out steel blue — committed deep navy.
    // Bright cyan cuts through like bioluminescence.
    Theme {
        id: ThemeId::Navy,
        bg:          Color32::from_rgb(15, 26, 52),   // deeper navy — more saturated
        sidebar_bg:  Color32::from_rgb(8, 14, 30),    // abyss — more contrast vs bg
        toolbar_bg:  Color32::from_rgb(11, 19, 40),   // between
        tab_bar_bg:  Color32::from_rgb(5, 10, 22),    // near-black navy
        fg:          Color32::from_rgb(215, 232, 255),  // light blue-tinted white
        fg_muted:    Color32::from_rgb(128, 164, 218),  // lifted for proper WCAG contrast
        sidebar_fg:  Color32::from_rgb(186, 214, 252),
        sidebar_active: Color32::from_rgb(42, 202, 252), // more vivid bioluminescent cyan
        link:           Color32::from_rgb(42, 202, 252),
        selection_bg:   Color32::from_rgb(26, 66, 122),
        code_bg:        Color32::from_rgb(9, 17, 36),  // darker for clear contrast
        inline_code_fg: Color32::from_rgb(108, 206, 255), // distinct from fg — not just white
        separator:      Color32::from_rgb(24, 40, 72),
    },

    // ── Ayu — "Ember" ────────────────────────────────────────────────────────
    // Near-void darkness with golden amber warmth. Staring into dying embers.
    // The contrast between cold darkness and warm fire is the whole point.
    Theme {
        id: ThemeId::Ayu,
        bg:          Color32::from_rgb(10, 14, 20),    // deep void, slight blue-dark
        sidebar_bg:  Color32::from_rgb(16, 21, 30),    // subtle rise — slightly more lift
        toolbar_bg:  Color32::from_rgb(12, 17, 24),    // between
        tab_bar_bg:  Color32::from_rgb(7, 10, 14),     // near-black
        fg:          Color32::from_rgb(200, 196, 188),  // warm star-white — slightly brighter
        fg_muted:    Color32::from_rgb(110, 122, 148),  // FIXED: was (78,88,106) — too low contrast
        sidebar_fg:  Color32::from_rgb(168, 174, 190),
        sidebar_active: Color32::from_rgb(255, 168, 48), // brighter fire amber
        link:           Color32::from_rgb(58, 196, 234),  // brighter cold cyan
        selection_bg:   Color32::from_rgb(28, 46, 72),
        code_bg:        Color32::from_rgb(13, 18, 26),
        inline_code_fg: Color32::from_rgb(255, 168, 48),  // amber matches accent
        separator:      Color32::from_rgb(22, 30, 46),
    },
];

pub fn theme_by_id(id: ThemeId) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).expect("ThemeId has no matching entry in THEMES")
}
