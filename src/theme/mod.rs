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
            ThemeId::Rust => "Rust",
            ThemeId::Coal => "Coal",
            ThemeId::Navy => "Navy",
            ThemeId::Ayu => "Ayu",
        }
    }
}

pub struct Theme {
    pub id: ThemeId,
    pub name: &'static str,

    // Surfaces
    pub bg: Color32,              // central panel / preview background
    pub sidebar_bg: Color32,      // sidebar + outline panel background
    pub toolbar_bg: Color32,      // top toolbar background
    pub tab_bar_bg: Color32,      // tab strip background

    // Text
    pub fg: Color32,              // body text
    pub fg_muted: Color32,        // muted / secondary text
    pub sidebar_fg: Color32,      // sidebar file/folder labels
    pub sidebar_active: Color32,  // currently open file highlight

    // Interactive
    pub link: Color32,            // hyperlinks
    pub selection_bg: Color32,    // text selection background

    // Code blocks
    pub code_bg: Color32,         // fenced code block frame background
    pub inline_code_fg: Color32,  // `inline code` text color

    // Structural
    pub separator: Color32,       // dividers, rule (---) lines
    pub quote_bg: Color32,        // blockquote left-bar tint
}

pub const THEMES: &[Theme] = &[
    // Light — clean white, high contrast (WCAG AA)
    Theme {
        id: ThemeId::Light,
        name: "Light",
        bg: Color32::from_rgb(255, 255, 255),
        sidebar_bg: Color32::from_rgb(248, 248, 248),
        toolbar_bg: Color32::from_rgb(242, 242, 242),
        tab_bar_bg: Color32::from_rgb(235, 235, 235),
        fg: Color32::from_rgb(25, 25, 25),          // Darker for better contrast
        fg_muted: Color32::from_rgb(75, 75, 75),   // Darker muted
        sidebar_fg: Color32::from_rgb(25, 25, 25), // Darker
        sidebar_active: Color32::from_rgb(0, 100, 200), // Deeper blue
        link: Color32::from_rgb(0, 100, 200),
        selection_bg: Color32::from_rgb(150, 200, 255),
        code_bg: Color32::from_rgb(242, 244, 247),
        inline_code_fg: Color32::from_rgb(180, 20, 100),
        separator: Color32::from_rgb(200, 200, 200),
        quote_bg: Color32::from_rgb(240, 248, 255),
    },
    // Rust — warm parchment (WCAG AA)
    Theme {
        id: ThemeId::Rust,
        name: "Rust",
        bg: Color32::from_rgb(233, 230, 220),
        sidebar_bg: Color32::from_rgb(70, 55, 48),
        toolbar_bg: Color32::from_rgb(80, 65, 55),
        tab_bar_bg: Color32::from_rgb(60, 48, 40),
        fg: Color32::from_rgb(20, 20, 20),         // Much darker for contrast
        fg_muted: Color32::from_rgb(80, 75, 65),  // Darker muted
        sidebar_fg: Color32::from_rgb(240, 240, 230), // Much lighter for dark bg
        sidebar_active: Color32::from_rgb(255, 180, 100),
        link: Color32::from_rgb(30, 110, 160),
        selection_bg: Color32::from_rgb(200, 160, 110),
        code_bg: Color32::from_rgb(220, 215, 205),
        inline_code_fg: Color32::from_rgb(130, 50, 80),
        separator: Color32::from_rgb(180, 170, 160),
        quote_bg: Color32::from_rgb(230, 225, 215),
    },
    // Coal — near-black dark mode (WCAG AA)
    Theme {
        id: ThemeId::Coal,
        name: "Coal",
        bg: Color32::from_rgb(25, 27, 30),
        sidebar_bg: Color32::from_rgb(50, 54, 60),
        toolbar_bg: Color32::from_rgb(40, 44, 50),
        tab_bar_bg: Color32::from_rgb(35, 39, 45),
        fg: Color32::from_rgb(230, 235, 240),     // Much lighter for dark bg
        fg_muted: Color32::from_rgb(180, 185, 190),
        sidebar_fg: Color32::from_rgb(220, 225, 230),
        sidebar_active: Color32::from_rgb(100, 180, 255),
        link: Color32::from_rgb(100, 180, 255),
        selection_bg: Color32::from_rgb(70, 110, 150),
        code_bg: Color32::from_rgb(40, 44, 50),
        inline_code_fg: Color32::from_rgb(220, 230, 240),
        separator: Color32::from_rgb(80, 90, 100),
        quote_bg: Color32::from_rgb(45, 50, 60),
    },
    // Navy — blue-tinted dark mode (WCAG AA)
    Theme {
        id: ThemeId::Navy,
        name: "Navy",
        bg: Color32::from_rgb(30, 38, 60),
        sidebar_bg: Color32::from_rgb(50, 60, 85),
        toolbar_bg: Color32::from_rgb(40, 50, 75),
        tab_bar_bg: Color32::from_rgb(35, 45, 70),
        fg: Color32::from_rgb(230, 235, 245),     // Much lighter
        fg_muted: Color32::from_rgb(180, 190, 210),
        sidebar_fg: Color32::from_rgb(220, 230, 245),
        sidebar_active: Color32::from_rgb(120, 200, 255),
        link: Color32::from_rgb(120, 200, 255),
        selection_bg: Color32::from_rgb(70, 120, 180),
        code_bg: Color32::from_rgb(40, 50, 75),
        inline_code_fg: Color32::from_rgb(220, 240, 255),
        separator: Color32::from_rgb(80, 100, 130),
        quote_bg: Color32::from_rgb(50, 65, 95),
    },
    // Ayu — minimal dark with warm accent (WCAG AA)
    Theme {
        id: ThemeId::Ayu,
        name: "Ayu",
        bg: Color32::from_rgb(20, 27, 35),
        sidebar_bg: Color32::from_rgb(30, 40, 50),
        toolbar_bg: Color32::from_rgb(25, 35, 45),
        tab_bar_bg: Color32::from_rgb(20, 30, 40),
        fg: Color32::from_rgb(225, 225, 225),     // Much lighter
        fg_muted: Color32::from_rgb(160, 170, 180),
        sidebar_fg: Color32::from_rgb(220, 230, 240),
        sidebar_active: Color32::from_rgb(255, 200, 100),
        link: Color32::from_rgb(0, 170, 230),
        selection_bg: Color32::from_rgb(60, 90, 130),
        code_bg: Color32::from_rgb(30, 40, 50),
        inline_code_fg: Color32::from_rgb(255, 200, 100),
        separator: Color32::from_rgb(50, 70, 90),
        quote_bg: Color32::from_rgb(35, 48, 65),
    },
];

pub fn theme_by_id(id: ThemeId) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap()
}
