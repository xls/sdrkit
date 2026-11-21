//! The colours, the styles, the frames and the tab rows of the screens. The colours are indexes of the 256-colour
//! table. A filled label is the chosen tab or the selected line and nothing else. With the variable `NO_COLOR` set,
//! the filled labels are drawn in reverse video.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};
use std::sync::OnceLock;

/// The colour of the pane that the keys act on, of the chosen tab and of the names of keys.
const ACCENT_COLOUR: Color = Color::Indexed(75);

/// Body text: the foreground colour of the terminal.
pub const TEXT: Style = Style::new();
/// A heading and a value that stands out.
pub const STRONG: Style = Style::new().add_modifier(Modifier::BOLD);
/// A hint, a placeholder and an item that is off. No other text is dim.
pub const HINT: Style = Style::new().fg(Color::Indexed(246));
/// The name of a key, the title of the pane that the keys act on, and the name of the shown object.
pub const ACCENT: Style = Style::new().fg(ACCENT_COLOUR).add_modifier(Modifier::BOLD);
/// A value that the user sets, in a list.
pub const VALUE: Style = Style::new().fg(ACCENT_COLOUR);
/// The frame of the pane that the keys act on.
pub const FOCUS_BORDER: Style = Style::new().fg(ACCENT_COLOUR).add_modifier(Modifier::BOLD);
/// The frame of every other pane, and the line between two tabs.
pub const BORDER: Style = Style::new();
/// A tab that is not chosen.
pub const TAB_IDLE: Style = Style::new();
/// The header line of a table.
pub const TABLE_HEAD: Style = Style::new()
    .add_modifier(Modifier::BOLD)
    .add_modifier(Modifier::UNDERLINED);
/// A state that is as it should be.
pub const GOOD: Style = Style::new().fg(Color::Indexed(114));
/// A state that is not final, and a level near the limit.
pub const WARNING: Style = Style::new().fg(Color::Indexed(221));
/// An error.
pub const ERROR: Style = Style::new().fg(Color::Indexed(203));
/// The background of the first and the last line of the screen.
pub const BAR: Style = Style::new().bg(Color::Indexed(236));

/// Returns whether the terminal shows no colours: the variable `NO_COLOR` is set and not empty.
fn plain() -> bool {
    static PLAIN: OnceLock<bool> = OnceLock::new();
    *PLAIN.get_or_init(|| std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()))
}

/// Ends the drawing of a screen in `buffer`. With `NO_COLOR` set, takes every colour out of the screen: the terminal
/// library then sends an empty colour change, and that change would end the bold or the reverse video of a cell.
pub fn finish(buffer: &mut Buffer) {
    if plain() {
        let area = buffer.area;
        buffer.set_style(area, Style::new().fg(Color::Reset).bg(Color::Reset));
    }
}

/// Returns the style of a filled label with text colour `fg` on `bg`, or in reverse video if `plain`.
const fn filled(fg: u8, bg: Color, plain: bool) -> Style {
    if plain {
        Style::new()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new()
            .fg(Color::Indexed(fg))
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    }
}

/// Returns the style of the chosen tab on a terminal with colours, or without them if `plain`.
const fn tab_active_on(plain: bool) -> Style {
    filled(16, ACCENT_COLOUR, plain)
}

/// Returns the style of the selected line on a terminal with colours, or without them if `plain`. `focused` tells
/// that the keys act on the pane of the line.
const fn selected_on(focused: bool, plain: bool) -> Style {
    match (focused, plain) {
        (true, _) => filled(231, Color::Indexed(25), plain),
        (false, false) => filled(255, Color::Indexed(240), false),
        (false, true) => Style::new()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    }
}

/// Returns the style of the chosen tab: a filled label.
pub fn tab_active() -> Style {
    tab_active_on(plain())
}

/// Returns the style of the selected line of a list or a table: a filled bar. `focused` tells that the keys act on
/// the pane of the line; the bar of another pane is grey.
pub fn selected(focused: bool) -> Style {
    selected_on(focused, plain())
}

/// Where the keys are, for the frame of a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The keys act on the pane.
    Here,
    /// Tab moves the keys to the pane.
    Reachable,
    /// The pane takes no keys.
    Never,
}

impl Focus {
    /// Returns the focus of a pane that takes keys: [`Focus::Here`] if `focused`, else [`Focus::Reachable`].
    pub fn of(focused: bool) -> Self {
        if focused {
            Self::Here
        } else {
            Self::Reachable
        }
    }
}

/// Returns `text` as the title of a pane with `focus`: bold text in the accent colour for the pane that the keys act
/// on, and body text for every other pane.
pub fn title(text: &str, focus: Focus) -> Line<'_> {
    let style = match focus {
        Focus::Here => ACCENT,
        Focus::Reachable | Focus::Never => TEXT,
    };
    Line::from(Span::styled(format!(" {text} "), style))
}

/// Returns the frame of a pane with `title`. The pane that the keys act on has a heavy frame in the accent colour.
/// Every other pane has a thin frame; a pane that Tab reaches shows "Tab" at the right of its top line.
pub fn frame(title: Line<'_>, focus: Focus) -> Block<'_> {
    let b = Block::default().borders(Borders::ALL).title(title);
    match focus {
        Focus::Here => b.border_type(BorderType::Thick).border_style(FOCUS_BORDER),
        Focus::Reachable => b
            .border_type(BorderType::Rounded)
            .border_style(BORDER)
            .title_top(Line::styled(" Tab ", HINT).right_aligned()),
        Focus::Never => b.border_type(BorderType::Rounded).border_style(BORDER),
    }
}

/// The line between two tabs.
const TAB_LINE: &str = "│";
/// The marks that tell that tabs are left out before the first shown tab and after the last one.
const TABS_BEFORE: &str = "‹";
const TABS_AFTER: &str = "›";

/// Returns the first and the last tab to draw so that tab `active` shows in `width` cells. The row starts at the
/// first tab when the chosen tab then fits; else it starts at the first tab that leaves the chosen tab inside.
fn tab_window(names: &[&str], active: usize, width: usize) -> (usize, usize) {
    let cells = |i: usize| names[i].chars().count() + 2;
    // The cells of tabs `from..=to` with the lines between them and the marks of the tabs that are left out.
    let used = |from: usize, to: usize| {
        let tabs: usize = (from..=to).map(cells).sum();
        tabs + (to - from) + usize::from(from > 0) + usize::from(to + 1 < names.len())
    };
    let from = (0..=active)
        .find(|&from| used(from, active) <= width)
        .unwrap_or(active);
    let mut to = active;
    while to + 1 < names.len() && used(from, to + 1) <= width {
        to += 1;
    }
    (from, to)
}

/// Returns a row of tabs with the names `names` in `width` cells. Tab `active` is a filled label; every other tab
/// is body text; a line stands between two tabs. If the tabs do not all fit, the row shows the tabs around the
/// chosen one, and "‹" or "›" stands where tabs are left out. Every tab row of the application is drawn by this
/// function.
pub fn tabs(names: &[&str], active: usize, width: usize) -> Line<'static> {
    if names.is_empty() {
        return Line::default();
    }
    let active = active.min(names.len() - 1);
    let (from, to) = tab_window(names, active, width);
    let mut spans = Vec::with_capacity(2 * (to - from) + 3);
    if from > 0 {
        spans.push(Span::styled(TABS_BEFORE, HINT));
    }
    for (i, name) in names.iter().enumerate().take(to + 1).skip(from) {
        if i > from {
            // No line stands next to the filled label: its edge is the limit.
            let line = if i == active || i == active + 1 {
                " "
            } else {
                TAB_LINE
            };
            spans.push(Span::styled(line, BORDER));
        }
        let style = if i == active { tab_active() } else { TAB_IDLE };
        spans.push(Span::styled(format!(" {name} "), style));
    }
    if to + 1 < names.len() {
        spans.push(Span::styled(TABS_AFTER, HINT));
    }
    Line::from(spans)
}

/// Returns `text` as a line of keys. `text` holds items separated by two or more spaces; the part of an item before
/// ": " is the name of a key, drawn in the accent colour, and the rest tells what the key does, in body text.
pub fn keys(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for item in text.split("  ").map(str::trim).filter(|i| !i.is_empty()) {
        if !spans.is_empty() {
            spans.push(Span::raw("   "));
        }
        match item.split_once(": ") {
            Some((key, does)) => {
                spans.push(Span::styled(key.to_owned(), ACCENT));
                spans.push(Span::styled(format!(" {does}"), TEXT));
            }
            None => spans.push(Span::styled(item.to_owned(), TEXT)),
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts<'a>(line: &'a Line) -> Vec<&'a str> {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_line_of_keys_marks_the_part_before_the_colon() {
        let line = keys("↑↓: row  Enter: select the row   Type the text");
        assert_eq!(
            texts(&line),
            [
                "↑↓",
                " row",
                "   ",
                "Enter",
                " select the row",
                "   ",
                "Type the text"
            ]
        );
        assert_eq!(line.spans[0].style, ACCENT);
        assert_eq!(line.spans[1].style, TEXT);
        assert_eq!(line.spans[6].style, TEXT);
    }

    #[test]
    fn the_chosen_tab_is_filled_and_no_other_tab_is() {
        let line = tabs(&["Packets", "Nodes", "Messages"], 1, 80);
        assert_eq!(
            texts(&line),
            [" Packets ", " ", " Nodes ", " ", " Messages "]
        );
        assert_eq!(line.spans[2].style, tab_active());
        assert_eq!(line.spans[0].style, TAB_IDLE);
        assert_eq!(line.spans[4].style, TAB_IDLE);
        assert_ne!(tab_active(), TAB_IDLE);
    }

    #[test]
    fn the_chosen_tab_differs_from_the_others_with_and_without_colours() {
        let coloured = tab_active_on(false);
        assert!(coloured.bg.is_some() && coloured.fg.is_some());
        assert!(coloured.add_modifier.contains(Modifier::BOLD));
        let plain = tab_active_on(true);
        assert_eq!((plain.fg, plain.bg), (None, None));
        assert!(plain.add_modifier.contains(Modifier::REVERSED));
        assert_ne!(coloured, TAB_IDLE);
        assert_ne!(plain, TAB_IDLE);
    }

    #[test]
    fn the_selected_line_is_a_bar_with_and_without_colours() {
        assert!(selected_on(true, false).bg.is_some());
        assert!(selected_on(false, false).bg.is_some());
        assert_ne!(selected_on(true, false), selected_on(false, false));
        assert!(selected_on(true, true)
            .add_modifier
            .contains(Modifier::REVERSED));
        assert_ne!(selected_on(false, true), TEXT);
    }

    #[test]
    fn a_line_stands_between_two_tabs_that_are_not_chosen() {
        let line = tabs(&["a", "b", "c"], 0, 80);
        assert_eq!(texts(&line), [" a ", " ", " b ", "│", " c "]);
    }

    #[test]
    fn a_row_of_tabs_that_does_not_fit_shows_the_chosen_tab() {
        let names = ["Packets", "Nodes", "Messages", "Readings", "Stations"];
        // The first tabs fit: the row starts at the first tab and marks the tabs that are left out after.
        let line = tabs(&names, 0, 20);
        assert_eq!(texts(&line), [" Packets ", " ", " Nodes ", "›"]);
        assert!(line.width() <= 20);
        // The last tab is chosen: the row ends with it and marks the tabs that are left out before.
        let line = tabs(&names, 4, 24);
        assert_eq!(texts(&line), ["‹", " Readings ", " ", " Stations "]);
        assert!(line.width() <= 24);
        // A tab in the middle is chosen and the row has tabs left out on both sides.
        let line = tabs(&names, 2, 14);
        assert_eq!(texts(&line), ["‹", " Messages ", "›"]);
        assert_eq!(line.spans[1].style, tab_active());
    }

    #[test]
    fn a_tab_wider_than_the_row_is_still_drawn() {
        let line = tabs(&["Packets", "Nodes"], 1, 3);
        assert_eq!(texts(&line), ["‹", " Nodes "]);
        assert!(tabs(&[], 0, 10).spans.is_empty());
    }
}
