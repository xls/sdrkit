//! Draws the screens from the state of the application.

use crate::api::{CommandDecl, State, Type, Value};
use crate::app::{App, EditFor, Live, Mode, Owner, Screen, ENTRY_PANE, TABLES_PANE};
use crate::find::Find;
use crate::format;
use crate::radio_form::{Part, RadioForm, Step};
use crate::results::Results;
use crate::theme::{
    self, Focus, ACCENT, BAR, ERROR, GOOD, HINT, STRONG, TABLE_HEAD, TEXT, VALUE, WARNING,
};
use crate::views;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use std::fmt::Write;

/// The mark before the selected line of a list or a table, and the blank before every other line.
const MARK: &str = "▶ ";
const NO_MARK: &str = "  ";

/// Draws the whole screen: the bar of screens, the screen, the status line and the keys of the pane.
pub fn draw(f: &mut Frame, app: &App) {
    let [bar, main, status, hint] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    bar_line(f, bar, app);
    match app.screen {
        Screen::Stacks => stacks(f, main, app),
        Screen::Radios => radios(f, main, app),
        Screen::Session => session(f, main, app),
        Screen::Settings => settings(f, main, app),
        Screen::Events => events(f, main, app),
        Screen::Commands => commands(f, main, app),
        Screen::Services => services(f, main, app),
        Screen::Load => load(f, main, app),
    }
    let mut status_line = vec![Span::styled(format!(" {} ", keys_owner(app)), ACCENT)];
    match &app.mode {
        // The search line stands in place of the status while the user types it.
        Mode::Find { find, .. } => {
            let count = find.count(&app.find_lines());
            status_line.push(Span::raw(format!(" /{}", find.text)));
            status_line.push(Span::styled("█", ACCENT));
            status_line.push(Span::styled(format!("  {count}"), HINT));
        }
        _ if app.status.error => {
            status_line.push(Span::styled(format!(" ✗ {}", app.status.text), ERROR));
        }
        _ => status_line.push(Span::raw(format!(" {}", app.status.text))),
    }
    // The samples that the session lost and the gaps of its input, in the warning colour while they rise.
    if app.live.is_some() && app.losses != (0, 0) {
        let style = if app.losses_rising() { WARNING } else { HINT };
        let (lost, gaps) = app.losses;
        status_line.push(Span::styled(format!("  lost {lost}, gaps {gaps}"), style));
    }
    f.render_widget(Paragraph::new(Line::from(status_line)), status);
    hint_line(f, hint, app);
    match &app.mode {
        Mode::Pick {
            options, sel, find, ..
        } => pick(f, main, options, *sel, find),
        Mode::Edit { what, text } => edit_box(f, main, app, *what, text),
        Mode::Normal | Mode::Find { .. } => {}
    }
    if let Some(form) = &app.radio_form {
        radio_form(f, main, form);
    }
    if app.help {
        keys(f, main);
    }
    theme::finish(f.buffer_mut());
}

/// Returns what the keys act on, for the label at the start of the status line: a form or a list over the screen, or
/// the pane of the screen with the heavy frame.
fn keys_owner(app: &App) -> &'static str {
    if app.help {
        return "Keys";
    }
    if app.radio_form.is_some() {
        return "Open a radio";
    }
    match &app.mode {
        Mode::Edit { .. } => return "Edit",
        Mode::Pick { .. } => return "Choose",
        Mode::Find { .. } => return "Find",
        Mode::Normal => {}
    }
    match (app.screen, app.pane) {
        (Screen::Stacks, _) => "Stacks",
        (Screen::Radios, _) => "Open radios",
        (Screen::Session, 0) => "Controls",
        (Screen::Session, ENTRY_PANE) if app.on_entry() => "Text",
        (Screen::Services, 0) => "Services",
        (Screen::Session | Screen::Services, _) => "Tables",
        (Screen::Settings | Screen::Commands, 0) => "Objects",
        (Screen::Settings, _) => "Settings",
        (Screen::Commands, _) if app.form.is_some() => "Command",
        (Screen::Commands, _) => "Commands",
        (Screen::Events, _) => "Event types",
        (Screen::Load, _) => "Load",
    }
}

/// The keys that every pane has, for the right end of the last line of the screen.
const KEYS_OF_A_SCREEN: &str = "/: find  1-8: screen  ?: keys  q: quit";

/// Draws the last line of the screen: the keys of the pane at the left and the keys of every pane at the right. The
/// keys of every pane are not drawn while a form or a list is open, or while the keys go into the entry line.
fn hint_line(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(Paragraph::new("").style(BAR), area);
    let over = app.help || app.radio_form.is_some() || !matches!(app.mode, Mode::Normal);
    let mut left = area;
    if !over && !app.on_entry() {
        let mut text = String::new();
        if app.panes() > 1 {
            text.push_str("Tab: next pane  ");
        }
        text.push_str(KEYS_OF_A_SCREEN);
        let line = theme::keys(&text);
        let width = (line.width() as u16 + 2).min(area.width / 2);
        let [l, r] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(width)]).areas(area);
        left = l;
        f.render_widget(Paragraph::new(line.right_aligned()).style(BAR), r);
    }
    let mut keys = theme::keys(&hints(app));
    keys.spans.insert(0, Span::raw(" "));
    f.render_widget(Paragraph::new(keys).style(BAR), left);
}

/// Returns the keys of the pane that the keys act on, for the last line of the screen.
fn hints(app: &App) -> String {
    let fixed = fixed_hints(app);
    if !matches!(app.mode, Mode::Normal) || app.help || app.radio_form.is_some() {
        return fixed.to_owned();
    }
    let entry = app.entry_command().is_some() && app.screen == Screen::Session;
    match app.pane {
        _ if !entry => fixed.to_owned(),
        ENTRY_PANE => {
            "Type the text  Enter: run the command  ←→ Home End: move  Esc: leave the line, the text stays  Tab: next pane"
                .to_owned()
        }
        TABLES_PANE => {
            let args: Vec<String> = app.entry_columns().iter().map(|(a, _)| a.label.to_lowercase()).collect();
            if args.is_empty() {
                format!("{fixed}  i: type a text")
            } else {
                let takes = format!("Enter: use its {} for the text", args.join(" and "));
                format!("↑↓: row  {takes}  ←→: table  , .: columns  i: type a text  s: stop")
            }
        }
        _ => "↑↓: control  Enter: change or run it  i: type a text  a: advanced  f: frequency  [ ]: table  s: stop"
            .to_owned(),
    }
}

/// Returns the keys of the pane that do not depend on the stack of the session.
fn fixed_hints(app: &App) -> &'static str {
    if app.help {
        return "Any key closes the list of keys";
    }
    if app.radio_form.is_some() {
        return "Esc: close the form";
    }
    match &app.mode {
        Mode::Edit { .. } => return "Type the value  Enter: apply  Backspace: delete  Esc: cancel",
        Mode::Pick { .. } => {
            return "↑↓: choose  type: find an option  Tab: next match  Enter: apply  Esc: cancel"
        }
        Mode::Find { .. } => {
            return "Type the text to find  Tab: next match  Shift-Tab: previous match  Enter: stay on the line  Esc: back"
        }
        Mode::Normal => {}
    }
    match (app.screen, app.pane) {
        (Screen::Stacks, _) => {
            "↑↓: stack  Enter: start it  a: choose the source  g: region  c: channel  f: frequency"
        }
        (Screen::Radios, _) => {
            "↑↓: radio  a: open one  u: start the stacks on it  s: its session  c: close it  Enter: its settings"
        }
        (Screen::Session, 0) => {
            "↑↓: control  Enter: change or run it  a: advanced  f: frequency  c: channel  g: region  [ ]: table  s: stop"
        }
        (Screen::Session, _) => {
            "↑↓: row  ←→: table  , .: columns  Enter: select the row  f: frequency  s: stop"
        }
        (Screen::Settings | Screen::Commands, 0) => "↑↓: object  → or Enter: its list",
        (Screen::Settings, _) => "↑↓: setting  Enter: change it  ←: objects",
        (Screen::Commands, _) if app.form.is_some() => {
            "↑↓: argument  Enter: edit  d: default  p: paste the chosen row  r: run  Esc: close"
        }
        (Screen::Commands, _) => "↑↓: command  Enter: fill its arguments  ←: objects",
        (Screen::Events, _) => "↑↓: event type  c: clear the events",
        (Screen::Services, 0) => "↑↓: service  →: its tables  [ ]: table  Enter: its settings",
        (Screen::Services, _) => "↑↓: row  ←→: table  , .: columns  Enter: choose the row",
        (Screen::Load, _) => "The load of the session, read once a second",
    }
}

/// Returns the rectangle of `width` by `height` cells in the middle of `area`, cut to `area`.
fn middle(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Returns a list of `items` with the selected line marked, in the frame of a pane with `focus`.
fn marked_list<'a>(items: Vec<ListItem<'a>>, title: &'a str, focus: Focus) -> List<'a> {
    List::new(items)
        .block(theme::frame(theme::title(title, focus), focus))
        .highlight_style(theme::selected(focus == Focus::Here))
        .highlight_symbol(MARK)
}

/// Returns the first line to show so that line `sel` of `len` lines is visible in `height` lines.
fn window(sel: usize, len: usize, height: usize) -> usize {
    if height == 0 || len <= height {
        return 0;
    }
    sel.saturating_sub(height / 2).min(len - height)
}

/// Returns the frame of a box that shows text and takes no keys.
fn block(title: &str) -> Block<'_> {
    theme::frame(theme::title(title, Focus::Never), Focus::Never)
}

/// Returns the frame of a pane that takes keys. `focused` tells that the keys act on it.
fn pane(title: &str, focused: bool) -> Block<'_> {
    let focus = Focus::of(focused);
    theme::frame(theme::title(title, focus), focus)
}

/// Returns the mark and the style of state `s`.
fn state_mark(s: State) -> (&'static str, Style) {
    match s {
        State::Off => ("○", HINT),
        State::Searching => ("◌", WARNING),
        State::Tracking => ("●", WARNING),
        State::Locked => ("●", GOOD),
    }
}

/// Draws the first line of the screen: the screens, with the shown one marked, and at the right the stack, the radio
/// and the state of the shown session.
fn bar_line(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(Paragraph::new("").style(BAR), area);
    let name = Span::styled(" sdrkit ", ACCENT);
    let titles: Vec<String> = Screen::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{} {}", i + 1, s.title()))
        .collect();
    let names: Vec<&str> = titles.iter().map(String::as_str).collect();
    let shown = Screen::ALL.iter().position(|s| *s == app.screen);
    let mut right = Vec::new();
    match &app.live {
        Some(live) => {
            let r = &live.stack.results;
            let (mark, style) = state_mark(r.state);
            right.push(Span::styled(live.stack_id.clone(), STRONG));
            right.push(Span::styled(format!(" on {}  ", live.radio), TEXT));
            right.push(Span::styled(
                format!("{mark} {} ", state_text(r.state)),
                style,
            ));
        }
        None => right.push(Span::styled("no session ", HINT)),
    }
    let line = Line::from(right);
    let width = (line.width() as u16).min(area.width / 3);
    let [n, t, _, r] = Layout::horizontal([
        Constraint::Length(name.width() as u16 + 1),
        Constraint::Min(1),
        Constraint::Length(2),
        Constraint::Length(width),
    ])
    .areas(area);
    f.render_widget(Paragraph::new(name).style(BAR), n);
    let tabs = theme::tabs(&names, shown.unwrap_or(0), usize::from(t.width));
    f.render_widget(Paragraph::new(tabs).style(BAR), t);
    f.render_widget(Paragraph::new(line.right_aligned()).style(BAR), r);
}

/// Draws the box that edits the line `text` for `what`: the text, then the value as it is now, how to type it, and the
/// range, the default and the help of the value.
fn edit_box(f: &mut Frame, area: Rect, app: &App, what: EditFor, text: &str) {
    let d = app.edit_decl(what);
    let tunes = matches!(what, EditFor::Frequency | EditFor::StartFrequency);
    let mut lines = vec![
        Line::from(vec![
            Span::raw(" "),
            Span::styled(text, STRONG),
            Span::styled("█", ACCENT),
        ]),
        Line::raw(""),
    ];
    if app.status.error {
        lines.push(Line::styled(format!(" ✗ {}", app.status.text), ERROR));
    }
    if let Some(now) = app.edit_now(what).filter(|n| !n.is_empty()) {
        lines.push(Line::styled(format!(" Now: {now}"), TEXT));
    }
    if tunes {
        let form = " A number is in MHz. A suffix k, M or G is accepted: 105.6  105.6M  919875k";
        lines.push(Line::styled(form, HINT));
    } else if d.is_some_and(|d| d.ty == Type::Hz) {
        let form = " A suffix k, M or G is accepted: 2.4M  200k";
        lines.push(Line::styled(form, HINT));
    }
    if what == EditFor::StartFrequency {
        let empty = " An empty line: each stack starts on its own frequency.";
        lines.push(Line::styled(empty, HINT));
    }
    let mut wrapped = 0;
    if let Some(d) = d {
        let range = views::range(d);
        if !range.is_empty() && !tunes {
            lines.push(Line::styled(format!(" Range: {range}"), TEXT));
        }
        if d.default != Value::None && !tunes {
            let v = format::value_text(&d.default, d);
            lines.push(Line::styled(format!(" Default: {v}"), TEXT));
        }
        if !d.help.is_empty() {
            wrapped = d.help.chars().count() / 66;
            lines.push(Line::styled(format!(" {}", d.help), TEXT));
        }
    }
    let r = middle(area, 82, (lines.len() + wrapped) as u16 + 2);
    let title = match what {
        EditFor::StartFrequency => "Frequency of the stack that starts",
        EditFor::Region => "Region",
        EditFor::Channel | EditFor::StartChannel => "Channel",
        _ => d.map_or("Value", |d| d.label.as_str()),
    };
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(pane(title, true)),
        r,
    );
}

fn pick(f: &mut Frame, area: Rect, options: &[String], sel: usize, find: &Find) {
    let title = if find.text.is_empty() {
        "Choose one".to_owned()
    } else {
        format!("Find: {} {}", find.text, find.count(options))
    };
    let widest = options.iter().map(|o| o.chars().count()).max();
    let width = widest.unwrap_or(10).max(title.chars().count());
    let r = middle(area, (width as u16 + 6).max(24), options.len() as u16 + 2);
    let items: Vec<ListItem> = options.iter().map(|o| ListItem::new(o.as_str())).collect();
    let mut state = ListState::default().with_selected(Some(sel));
    f.render_widget(Clear, r);
    f.render_stateful_widget(marked_list(items, &title, Focus::Here), r, &mut state);
}

/// Returns the shown text of part `p`: the typed text, or what an empty text means.
fn part_text(p: &Part, selected: bool) -> Vec<Span<'_>> {
    let chosen = matches!(p.decl.ty, Type::Bool | Type::Enum) || !p.rows.is_empty();
    let mut spans = Vec::new();
    if p.text.is_empty() {
        let text = if p.decl.required {
            "(required)"
        } else {
            "(not set)"
        };
        spans.push(Span::styled(text, HINT));
    } else {
        spans.push(Span::raw(p.text.as_str()));
    }
    if selected && chosen {
        spans.insert(0, Span::styled("◀ ", HINT));
        spans.push(Span::styled(" ▶", HINT));
    } else if selected {
        spans.push(Span::styled("█", HINT));
    }
    spans
}

/// Draws the form that opens a radio over `area`.
fn radio_form(f: &mut Frame, area: Rect, form: &RadioForm) {
    let kind = form.kinds.get(form.kind);
    match form.step {
        Step::Kind => {
            let r = middle(area, 96, form.kinds.len() as u16 + 6);
            f.render_widget(Clear, r);
            let title = if form.find.text.is_empty() {
                "Open a radio: choose the kind".to_owned()
            } else {
                let lines = form.kind_lines();
                format!(
                    "Open a radio: find {} {}",
                    form.find.text,
                    form.find.count(&lines)
                )
            };
            let outer = pane(&title, true);
            let inner = outer.inner(r);
            f.render_widget(outer, r);
            let [list, forms, hint] = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .areas(inner);
            let key_width = form.kinds.iter().map(|k| k.key.len()).max().unwrap_or(0);
            let items: Vec<ListItem> = form
                .kinds
                .iter()
                .map(|k| ListItem::new(format!("{:key_width$}  {}", k.key, k.name)))
                .collect();
            let mut state = ListState::default().with_selected(Some(form.kind));
            f.render_stateful_widget(
                List::new(items)
                    .highlight_style(theme::selected(true))
                    .highlight_symbol(MARK),
                list,
                &mut state,
            );
            let text = kind.map_or("", |k| k.forms.as_str());
            f.render_widget(
                Paragraph::new(vec![
                    Line::raw(""),
                    Line::from(vec![Span::styled("Address forms: ", TEXT), Span::raw(text)]),
                ])
                .wrap(Wrap { trim: true }),
                forms,
            );
            let keys = "↑↓: choose  type: find a kind  Tab: next match  Enter: next  Esc: cancel";
            f.render_widget(Paragraph::new(theme::keys(keys)), hint);
        }
        Step::Parts => {
            let r = middle(area, 96, form.parts.len() as u16 + 10);
            f.render_widget(Clear, r);
            let title = match kind {
                Some(k) if !k.key.is_empty() => format!("Open a radio: {} ({})", k.name, k.key),
                _ => "Open a radio".to_owned(),
            };
            let outer = pane(&title, true);
            let inner = outer.inner(r);
            f.render_widget(outer, r);
            let label_width = form.parts.iter().map(|p| p.decl.label.chars().count());
            let label_width = label_width.max().unwrap_or(0);
            let mut lines = Vec::new();
            for (i, p) in form.parts.iter().enumerate() {
                let selected = i == form.part;
                let label = format!(
                    "{}{}  ",
                    if selected { MARK } else { NO_MARK },
                    fit(&p.decl.label, label_width)
                );
                let mut spans = vec![Span::styled(
                    label,
                    if selected {
                        theme::selected(true)
                    } else {
                        TEXT
                    },
                )];
                spans.push(Span::raw(" "));
                spans.extend(part_text(p, selected));
                lines.push(Line::from(spans));
            }
            lines.push(Line::raw(""));
            if let Some(p) = form.parts.get(form.part) {
                let mut about = p.decl.help.clone();
                let range = views::range(&p.decl);
                if !range.is_empty() {
                    let _ = write!(about, "  Range: {range}.");
                }
                if p.decl.default != Value::None {
                    let default = format::value_text(&p.decl.default, &p.decl);
                    let _ = write!(about, "  Default: {default}.");
                }
                lines.push(Line::styled(about, TEXT));
            }
            lines.push(Line::raw(""));
            lines.push(match &form.address {
                Ok(a) => Line::from(vec![
                    Span::styled("Address: ", TEXT),
                    Span::styled(a.as_str(), STRONG),
                ]),
                Err(e) => Line::styled(format!("Address: none yet. {e}"), HINT),
            });
            lines.push(Line::styled(form.error.as_str(), ERROR));
            let [body, hint] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body);
            let keys = "↑↓ or Tab: part  type: value  ←→: change a choice  Enter: open  Esc: back";
            f.render_widget(Paragraph::new(theme::keys(keys)), hint);
        }
    }
}

/// The keys of the application, in the order of the list of keys.
const KEYS: &[(&str, &str)] = &[
    ("1 to 7", "show a screen"),
    ("Tab, Shift-Tab", "next, previous pane of the screen"),
    (
        "↑ ↓ PgUp PgDn Home End",
        "move the selection of the pane with the heavy frame",
    ),
    (
        "← →",
        "tables pane: previous, next table; list at the left: the pane at its right",
    ),
    ("[ ]", "previous, next table, from every pane of the screen"),
    (", .", "in the tables pane: scroll the columns left, right"),
    ("Enter", "act on the selected line"),
    ("/", "find a line of the list: type a part of its text"),
    (
        "",
        "while finding: Tab, Shift-Tab: next, previous match  Enter: stay  Esc: back",
    ),
    ("Esc", "close a form or a list"),
    ("m", "mute or unmute the sound"),
    ("?", "this list"),
    ("q, Ctrl-C", "quit"),
    ("", ""),
    (
        "Stacks",
        "a: choose the source  f: the frequency to start on  Enter: start the stack",
    ),
    (
        "",
        "g: choose the region from the list of the data  c: choose a channel of the region",
    ),
    (
        "Radios",
        "a: open a radio  u: start the stacks on it  s: show its session",
    ),
    ("", "c: close it  Enter: its settings"),
    (
        "Session",
        "f: type a frequency  c: choose a channel of the region  g: choose the region",
    ),
    ("", "Enter: change a control, select a row"),
    ("", "n: next session  s: stop"),
    (
        "",
        "i: type a text for the first command of the stack  Enter: run it  Esc: leave",
    ),
    (
        "",
        "Enter on a row that has a value for the text (a channel): use it",
    ),
    (
        "Settings",
        "choose an object on the left, then Enter on a setting changes it",
    ),
    ("Events", "c: clear"),
    (
        "Commands",
        "choose an object on the left; Enter: fill the arguments  r: run",
    ),
    (
        "Services",
        "choose a service on the left; → or Tab: its tables  Enter: its settings",
    ),
];

/// Draws the list of keys over `area`.
fn keys(f: &mut Frame, area: Rect) {
    let r = middle(area, 104, KEYS.len() as u16 + 4);
    let mut lines: Vec<Line> = KEYS
        .iter()
        .map(|(key, does)| {
            Line::from(vec![
                Span::styled(format!(" {key:<24}"), ACCENT),
                Span::raw(*does),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(" Any key closes this list.", HINT));
    f.render_widget(Clear, r);
    f.render_widget(Paragraph::new(lines).block(pane("Keys", true)), r);
}

/// The widest a column is drawn. A longer cell is cut and ends in "…".
const MAX_COLUMN: usize = 28;

/// Pads or cuts `text` to `width` characters.
fn fit(text: &str, width: usize) -> String {
    let n = text.chars().count();
    if n <= width {
        let mut s = String::with_capacity(width);
        s.push_str(text);
        s.extend(std::iter::repeat_n(' ', width - n));
        s
    } else {
        let mut s: String = text.chars().take(width.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

/// Returns the columns that fit in `width`: column 0, then the columns from `1 + first_col` on, each with its drawn
/// width. A column is as wide as its longest shown cell, up to [`MAX_COLUMN`]; the last shown column is as wide as
/// its longest cell, up to the width that is left.
fn shown_columns(
    header: &[String],
    rows: &[Vec<String>],
    first_col: usize,
    width: usize,
) -> Vec<(usize, usize)> {
    let width_of = |c: usize| {
        let cells = rows.iter().filter_map(|r| r.get(c));
        std::iter::once(&header[c])
            .chain(cells)
            .map(|t| t.chars().count())
            .max()
            .unwrap_or(0)
            .max(1)
    };
    let mut shown = Vec::new();
    let mut used = 0;
    let scrolled = (1 + first_col).min(header.len().saturating_sub(1).max(1));
    for c in std::iter::once(0).chain(scrolled..header.len()) {
        if c >= header.len() {
            break;
        }
        let w = width_of(c);
        let gap = usize::from(!shown.is_empty());
        if gap == 1 && used + gap + w.min(MAX_COLUMN) > width {
            break;
        }
        shown.push((c, w, used + gap));
        used += gap + w.min(MAX_COLUMN);
    }
    // Every column but the last is cut to MAX_COLUMN. The last one is as wide as its cells, up to the width left.
    let last = shown.len().saturating_sub(1);
    shown
        .iter()
        .enumerate()
        .map(|(i, &(c, w, at))| {
            let w = if i == last {
                w.min(width.saturating_sub(at)).max(1)
            } else {
                w.min(MAX_COLUMN)
            };
            (c, w)
        })
        .collect()
}

/// Draws `rows` with `header` in `frame` in `area`. `first` is the index of the first row in `rows`. With `sel`, line
/// `sel` is a filled bar and every line starts with the mark column; without it, no line can be selected. Column 0
/// stays in place and the other columns start at `1 + first_col`. Columns that do not fit in `area` are not drawn.
/// If there are no rows, the hint `empty` stands in their place.
#[allow(clippy::too_many_arguments)]
fn text_table(
    f: &mut Frame,
    area: Rect,
    frame: Block,
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    first: usize,
    sel: Option<usize>,
    focused: bool,
    first_col: usize,
    empty: &str,
) {
    let inner = frame.inner(area);
    f.render_widget(frame, area);
    if header.is_empty() {
        return;
    }
    let mark_width = if sel.is_some() {
        MARK.chars().count()
    } else {
        0
    };
    let width = usize::from(inner.width).saturating_sub(mark_width);
    let columns = shown_columns(&header, &rows, first_col, width);
    let line_of = |cells: &[String], selected: bool| {
        let mut s = String::with_capacity(width + mark_width);
        if sel.is_some() {
            s.push_str(if selected { MARK } else { NO_MARK });
        }
        for (i, &(c, w)) in columns.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&fit(cells.get(c).map_or("", String::as_str), w));
        }
        s
    };
    let mut lines = Vec::with_capacity(rows.len() + 1);
    let head = fit(&line_of(&header, false), width + mark_width);
    lines.push(Line::styled(head, TABLE_HEAD));
    for (i, r) in rows.iter().enumerate() {
        let selected = Some(first + i) == sel;
        let text = fit(&line_of(r, selected), width + mark_width);
        lines.push(if selected {
            Line::styled(text, theme::selected(focused))
        } else {
            Line::styled(text, TEXT)
        });
    }
    if rows.is_empty() && !empty.is_empty() {
        lines.push(Line::styled(format!("{NO_MARK}{empty}"), HINT));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// Draws table `t` of `tables` with line `sel` marked and the columns scrolled by `first_col`. Each table is the
/// results that hold it and its id there. `focused` tells that the keys act on the table. `note` goes into the bottom line
/// of the frame, before the counts.
#[allow(clippy::too_many_arguments)]
fn results_table(
    f: &mut Frame,
    area: Rect,
    tables: &[(&Results, usize)],
    t: usize,
    sel: usize,
    focused: bool,
    first_col: usize,
    note: Option<&str>,
) {
    let Some(&(r, id)) = tables.get(t) else {
        f.render_widget(
            Paragraph::new("This object has no tables.").block(pane("Tables", focused)),
            area,
        );
        return;
    };
    let len = r.tables[id].rows.len();
    let columns = r.decl.tables[id].columns.len();
    let note = note.map(|n| format!("{n} · ")).unwrap_or_default();
    let counts = match len {
        0 => format!(" no rows · {columns} columns "),
        _ => format!(
            " {note}row {} of {len} · {columns} columns ",
            sel.min(len - 1) + 1
        ),
    };
    let names: Vec<&str> = tables
        .iter()
        .map(|(r, id)| {
            let d = &r.decl.tables[*id];
            if d.label.is_empty() { &d.key } else { &d.label }.as_str()
        })
        .collect();
    let frame = pane("Tables", focused).title_bottom(Line::styled(counts, TEXT).right_aligned());
    let inner = frame.inner(area);
    f.render_widget(frame, area);
    let [tabs, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    let row = theme::tabs(&names, t, usize::from(tabs.width));
    f.render_widget(Paragraph::new(row), tabs);
    let height = usize::from(body.height.saturating_sub(1));
    let first = window(sel, len, height);
    let (header, rows) = views::table(r, id, first, height);
    let empty = format!("No {} yet", names[t].to_lowercase());
    text_table(
        f,
        body,
        Block::default(),
        header,
        rows,
        first,
        Some(sel),
        focused,
        first_col,
        &empty,
    );
}

fn fields_list(f: &mut Frame, area: Rect, title: &str, owners: &[&Results]) {
    let mut lines = Vec::new();
    for r in owners {
        for (label, value) in views::fields(r) {
            lines.push(Line::from(vec![
                Span::styled(format!("{label}: "), TEXT),
                Span::raw(value),
            ]));
        }
    }
    f.render_widget(Paragraph::new(lines).block(block(title)), area);
}

fn state_text(s: State) -> &'static str {
    match s {
        State::Off => "off",
        State::Searching => "searching",
        State::Tracking => "tracking",
        State::Locked => "locked",
    }
}

/// Adds the state, the SNR, the level, the offset and the summary of `r` to `out`.
fn summary(out: &mut Vec<Span>, r: &Results) {
    if r.counter == 0 {
        out.push(Span::styled("no results yet", HINT));
        return;
    }
    let (mark, style) = state_mark(r.state);
    out.push(Span::styled(
        format!("{mark} {}", state_text(r.state)),
        style,
    ));
    let mut value = |label: &str, text: String| {
        out.push(Span::styled(format!("   {label} "), TEXT));
        out.push(Span::styled(text, STRONG));
    };
    if r.snr_db.is_finite() {
        value("SNR", format!("{:.1} dB", r.snr_db));
    }
    if r.level_dbfs.is_finite() {
        value("level", format!("{:.1} dBFS", r.level_dbfs));
    }
    if r.offset_hz.is_finite() {
        value("offset", format!("{:.0} Hz", r.offset_hz));
    }
    if !r.summary.is_empty() {
        out.push(Span::styled(format!("   {}", r.summary), STRONG));
    }
    // What is wrong with the session, and what stops it from transmitting.
    for key in ["session.problem", crate::app::TX_BLOCKED] {
        if let Some(text) = crate::app::field_text(r, key).filter(|t| !t.is_empty()) {
            out.push(Span::styled(format!("   ! {text}"), WARNING));
        }
    }
}

fn stacks(f: &mut Frame, area: Rect, app: &App) {
    let [top, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
    let source = app.source_text();
    let mut frequency = String::new();
    match app.freq_hz {
        Some(hz) => format::hz(&mut frequency, hz),
        None => frequency.push_str("the frequency of each stack"),
    }
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Source ", TEXT),
            Span::styled(source, STRONG),
            Span::styled("     Frequency ", TEXT),
            Span::styled(frequency, STRONG),
        ])),
        top,
    );
    let height = usize::from(list.height.saturating_sub(3));
    let first = window(app.sel.stack, app.stacks.len(), height);
    let rows = app
        .stacks
        .iter()
        .skip(first)
        .take(height)
        .map(|s| {
            vec![
                s.id.clone(),
                s.name.clone(),
                s.group.clone(),
                s.signals.clone(),
                s.blurb.clone(),
            ]
        })
        .collect();
    let header = ["id", "name", "group", "signal types", "about"]
        .map(String::from)
        .to_vec();
    let n = app.stacks.len();
    let at = format!(" stack {} of {n} ", (app.sel.stack + 1).min(n));
    let frame = pane("Stacks", true).title_bottom(Line::styled(at, TEXT).right_aligned());
    let sel = Some(app.sel.stack);
    text_table(f, list, frame, header, rows, first, sel, true, 0, "");
}

fn radios(f: &mut Frame, area: Rect, app: &App) {
    let [drivers, open] =
        Layout::vertical([Constraint::Percentage(40), Constraint::Min(5)]).areas(area);
    let r = &app.engine.results;
    if let Some(t) = r.table_id("drivers") {
        let height = usize::from(drivers.height.saturating_sub(3));
        let (header, rows) = views::table(r, t, 0, height);
        text_table(
            f,
            drivers,
            block("Kinds of radio"),
            header,
            rows,
            0,
            None,
            false,
            0,
            "",
        );
    }
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Min(20)]).areas(open);
    let items: Vec<ListItem> = app
        .radios
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let mut s = vec![Span::styled(o.name.clone(), STRONG)];
            let address = app.radio_address(i).unwrap_or_default();
            if let Some(e) = &o.dead {
                let closed = format!("  closed ({}): c removes it", e.name());
                s.push(Span::styled(closed, ERROR));
                return ListItem::new(Line::from(s));
            }
            s.push(Span::raw(format!("  {address}")));
            if !address.is_empty() && address == app.source {
                s.push(Span::styled("  source of the stacks", VALUE));
            }
            match app.session_on(&o.name) {
                Some((stack, true)) => {
                    s.push(Span::styled(format!("  runs {stack}, shown"), GOOD));
                }
                Some((stack, false)) => {
                    s.push(Span::styled(format!("  runs {stack}"), GOOD));
                }
                None => {}
            }
            s.push(Span::raw("  "));
            summary(&mut s, &o.results);
            ListItem::new(Line::from(s))
        })
        .collect();
    let mut state =
        ListState::default().with_selected((!app.radios.is_empty()).then_some(app.sel.radio));
    f.render_stateful_widget(
        marked_list(items, "Open radios", Focus::Here),
        list,
        &mut state,
    );
    match app.radios.get(app.sel.radio) {
        Some(o) => fields_list(f, detail, &format!("Readings of {}", o.name), &[&o.results]),
        None => f.render_widget(
            Paragraph::new("No radio is open. Press a to open one.").block(block("Readings")),
            detail,
        ),
    }
}

fn session(f: &mut Frame, area: Rect, app: &App) {
    let Some(live) = &app.live else {
        f.render_widget(
            Paragraph::new("No session runs. Choose a stack on the Stacks screen and press Enter.")
                .block(block("Session")),
            area,
        );
        return;
    };
    let command = live.entry_command();
    let [head, sound, fall, middle, entry, table] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(10),
        Constraint::Percentage(35),
        Constraint::Length(if command.is_some() { 3 } else { 0 }),
        Constraint::Min(5),
    ])
    .areas(area);
    if let Some(c) = command {
        entry_box(f, entry, app, live, c);
    }
    let mut s = vec![
        Span::styled(format!(" {}", live.stack_id), ACCENT),
        Span::styled(" on ", TEXT),
        Span::styled(live.radio.clone(), STRONG),
        Span::styled(format!(" ({})   ", live.address), TEXT),
    ];
    if !app.parked.is_empty() {
        let n = app.parked.len() + 1;
        s.push(Span::styled(
            format!("session 1 of {n}, n: the next   "),
            TEXT,
        ));
    }
    if live.closing {
        s.push(Span::styled("stopping", WARNING));
    } else {
        summary(&mut s, &live.stack.results);
    }
    if let Some(e) = &live.session.dead {
        let dead = format!("   session handle dead: {}", e.name());
        s.push(Span::styled(dead, ERROR));
    }
    f.render_widget(Paragraph::new(Line::from(s)), head);
    f.render_widget(Paragraph::new(sound_text(app, live)), sound);
    let mut frame = block("Spectrum");
    for (i, text) in axis_texts(live).into_iter().enumerate() {
        let line = Line::styled(text, TEXT);
        frame = frame.title_bottom(match i {
            0 => line.left_aligned(),
            1 => line.centered(),
            _ => line.right_aligned(),
        });
    }
    let inner = frame.inner(fall);
    f.render_widget(frame, fall);
    if live.waterfall.is_empty() {
        f.render_widget(
            Paragraph::new("waiting for spectrum frames").style(HINT),
            inner,
        );
    } else {
        f.render_widget(&live.waterfall, inner);
    }
    let [controls, fields, log] = Layout::horizontal([
        Constraint::Percentage(36),
        Constraint::Percentage(30),
        Constraint::Min(20),
    ])
    .areas(middle);
    controls_pane(f, controls, app);
    fields_list(
        f,
        fields,
        "Results",
        &[&live.stack.results, &live.session.results],
    );
    let r = &live.stack.results;
    let lines = usize::from(log.height.saturating_sub(2));
    let from = r.log.len().saturating_sub(lines);
    let text: Vec<Line> = r.log[from..]
        .iter()
        .map(|l| Line::raw(l.as_str()))
        .collect();
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(block("Log")),
        log,
    );
    results_table(
        f,
        table,
        &app.shown_tables(),
        app.sel.table,
        app.sel.row,
        app.pane == TABLES_PANE,
        app.sel.column,
        app.table_note().as_deref(),
    );
}

/// Draws the entry line of the session for its command `c`. The title names the command and the chosen values. An
/// empty line shows what to type, or why a text cannot go on air now. The bottom line of the frame shows why the
/// command refused the text, or else why a text cannot go on air now.
fn entry_box(f: &mut Frame, area: Rect, app: &App, live: &Live, c: &CommandDecl) {
    let e = &live.entry;
    let focused = app.on_entry();
    let mut title = c.label.clone();
    for a in c.args.iter().skip(1) {
        if let Some(v) = e.chosen(&a.key) {
            let _ = write!(title, " · {}: {v}", a.label.to_lowercase());
        }
    }
    let blocked = app.entry_blocked();
    let mut frame = pane(&title, focused);
    if !e.error.is_empty() {
        frame = frame.title_bottom(Line::styled(format!(" ✗ {} ", e.error), ERROR));
    } else if let Some(why) = blocked.as_ref().filter(|_| !e.text.is_empty()) {
        frame = frame.title_bottom(Line::styled(format!(" {why} "), HINT));
    }
    let width = usize::from(area.width.saturating_sub(4));
    let cursor = e.cursor.min(e.text.chars().count());
    let first = (cursor + 1).saturating_sub(width);
    let mut chars = e.text.chars().skip(first).take(width);
    let mut line = vec![Span::raw(" ")];
    if focused {
        let before: String = chars.by_ref().take(cursor - first).collect();
        line.push(Span::styled(before, STRONG));
        match chars.next() {
            Some(under) => line.push(Span::styled(under.to_string(), theme::selected(true))),
            None => line.push(Span::styled("█", ACCENT)),
        }
    }
    line.push(Span::styled(chars.collect::<String>(), STRONG));
    if e.text.is_empty() {
        let hint = match (blocked, focused) {
            (Some(why), _) => why,
            (None, true) => "Type the text and press Enter".to_owned(),
            (None, false) => "i: type the text here".to_owned(),
        };
        line.push(Span::styled(format!(" {hint}"), HINT));
    }
    f.render_widget(Paragraph::new(Line::from(line)).block(frame), area);
}

/// The widest the name of a control is drawn.
const MAX_CONTROL_NAME: usize = 28;

/// Draws the controls of the session: its frequency, the scan, and the commands and the settings of the stack.
fn controls_pane(f: &mut Frame, area: Rect, app: &App) {
    let all = app.controls();
    let texts: Vec<(String, String)> = all.iter().map(|c| app.control_text(*c)).collect();
    let names = texts.iter().map(|(name, _)| name.chars().count());
    let width = names.max().unwrap_or(0).min(MAX_CONTROL_NAME);
    let height = usize::from(area.height.saturating_sub(2));
    let first = window(app.sel.control, all.len(), height);
    let items: Vec<ListItem> = texts
        .iter()
        .skip(first)
        .take(height)
        .map(|(name, value)| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{}  ", fit(name, width))),
                Span::styled(value.as_str(), VALUE),
            ]))
        })
        .collect();
    let sel = (!all.is_empty()).then(|| app.sel.control.min(all.len() - 1) - first);
    let mut state = ListState::default().with_selected(sel);
    f.render_stateful_widget(
        marked_list(items, "Controls", Focus::of(app.pane == 0)),
        area,
        &mut state,
    );
}

/// The number of cells of the sound meter. A cell is 3 dB; the last cell ends at full scale.
const METER_CELLS: usize = 20;

/// Returns the line of the sound: the format of the speaker, a meter of the peak level and whether the sound is muted.
fn sound_text(app: &App, live: &Live) -> Line<'static> {
    let mut s = vec![Span::styled(" sound ", TEXT)];
    match live.speaker.as_ref().and_then(|sp| sp.format()) {
        Some((rate_hz, channels)) => {
            s.push(Span::raw(format!("{rate_hz:.0} Hz, {channels} ch  ")));
            let db = app.meter.peak_dbfs();
            let lit = db.map_or(0, |db| {
                ((db + 60.0) / 3.0).clamp(0.0, METER_CELLS as f32) as usize
            });
            // The meter is green below -18 dBFS, yellow below -6 dBFS and red above.
            for (from, to, style) in [(0, 14, GOOD), (14, 18, WARNING), (18, METER_CELLS, ERROR)] {
                let n = lit.clamp(from, to) - from;
                s.push(Span::styled("━".repeat(n), style));
            }
            s.push(Span::styled("─".repeat(METER_CELLS - lit), HINT));
            s.push(match db {
                Some(db) => Span::raw(format!("  peak {db:.1} dBFS")),
                None => Span::styled("  silence", HINT),
            });
        }
        None if live.sound_failed => s.push(Span::styled("no speaker", ERROR)),
        None => s.push(Span::styled("waiting for audio", HINT)),
    }
    if app.meter.muted() {
        s.push(Span::styled("   muted (m)", WARNING));
    }
    Line::from(s)
}

/// Returns the lowest, the middle and the highest frequency of the spectrum as text, for the frame of the spectrum.
/// Returns no text before the first frame.
fn axis_texts(live: &Live) -> Vec<String> {
    let w = &live.waterfall;
    if w.span_hz <= 0.0 {
        return Vec::new();
    }
    [-0.5, 0.0, 0.5]
        .iter()
        .map(|part| {
            let mut s = String::from(" ");
            format::hz(&mut s, w.centre_hz + part * w.span_hz);
            s.push(' ');
            s
        })
        .collect()
}

/// Returns the kind and the name of the object of the settings and commands screens.
fn object_title(app: &App) -> String {
    match app.objects().get(app.object_index()) {
        Some(o) if o.name.is_empty() => o.kind.to_lowercase(),
        Some(o) => format!("{} {}", o.kind.to_lowercase(), o.name),
        None => String::new(),
    }
}

/// The width of the objects pane.
const OBJECTS_WIDTH: u16 = 38;

/// Draws the objects that have settings and commands, with the object of the screen selected.
fn objects_pane(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .objects()
        .iter()
        .map(|o| ListItem::new(format!("{:<8}{}", o.kind, o.name)))
        .collect();
    let mut state = ListState::default().with_selected(Some(app.object_index()));
    f.render_stateful_widget(
        marked_list(items, "Objects", Focus::of(app.pane == 0)),
        area,
        &mut state,
    );
}

fn settings(f: &mut Frame, area: Rect, app: &App) {
    let [left, right] =
        Layout::horizontal([Constraint::Length(OBJECTS_WIDTH), Constraint::Min(30)]).areas(area);
    objects_pane(f, left, app);
    let focused = app.pane == 1;
    let title = format!("Settings of the {}", object_title(app));
    let Some(o) = app.target().filter(|o| !o.settings.is_empty()) else {
        f.render_widget(
            Paragraph::new("This object has no settings.").block(pane(&title, focused)),
            right,
        );
        return;
    };
    let [list, help] = Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(right);
    let height = usize::from(list.height.saturating_sub(3));
    let first = window(app.sel.setting, o.settings.len(), height);
    // Two settings of an object can have one label in two sections, so the section is a column of its own.
    let sections = o.settings.iter().any(|d| !d.section.is_empty());
    let rows = o
        .settings
        .iter()
        .zip(&o.values)
        .skip(first)
        .take(height)
        .map(|(d, v)| {
            let note = match (d.fixed, d.restart) {
                (true, _) => "cannot be changed",
                (false, true) => "a change restarts the object",
                (false, false) => "",
            };
            let mut row = vec![
                d.label.clone(),
                format::value_text(v, d),
                views::range(d),
                note.to_owned(),
            ];
            if sections {
                row.insert(0, d.section.clone());
            }
            row
        })
        .collect();
    let mut header = ["setting", "value", "range", ""].map(String::from).to_vec();
    if sections {
        header.insert(0, "section".to_owned());
    }
    let sel = Some(app.sel.setting);
    let frame = pane(&title, focused);
    text_table(f, list, frame, header, rows, first, sel, focused, 0, "");
    help_box(f, help, o, app.sel.setting);
}

fn help_box(f: &mut Frame, area: Rect, o: &Owner, sel: usize) {
    let Some(d) = o.settings.get(sel) else {
        return;
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(d.key.as_str(), ACCENT),
        Span::styled(
            format!("  default {}", format::value_text(&d.default, d)),
            TEXT,
        ),
        Span::styled(format!("  {}", d.section), TEXT),
    ])];
    if !d.help.is_empty() {
        lines.push(Line::raw(d.help.as_str()));
    }
    if !d.options.is_empty() {
        let opts: Vec<&str> = d.options.iter().map(|o| o.label.as_str()).collect();
        lines.push(Line::styled(format!("options: {}", opts.join(", ")), TEXT));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(block("Help")),
        area,
    );
}

fn events(f: &mut Frame, area: Rect, app: &App) {
    let [kinds, list] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Min(30)]).areas(area);
    let height = usize::from(kinds.height.saturating_sub(2));
    let first = window(app.sel.event_type, app.event_types.len(), height);
    let items: Vec<ListItem> = app
        .event_types
        .iter()
        .skip(first)
        .take(height)
        .map(|t| ListItem::new(format!("{} ({})", t.key, t.fields.len())))
        .collect();
    let mut state = ListState::default().with_selected(Some(app.sel.event_type - first));
    let title = format!("Event types ({})", app.event_types.len());
    f.render_stateful_widget(marked_list(items, &title, Focus::Here), kinds, &mut state);
    let lines = usize::from(list.height.saturating_sub(2));
    let from = app.events.len().saturating_sub(lines);
    let text: Vec<Line> = app
        .events
        .iter()
        .skip(from)
        .map(|e| {
            let ty = app.event_types.get(e.ty as usize);
            let mut spans = vec![
                Span::styled(format!("{:>9.3} {} ", e.time, e.source), TEXT),
                Span::styled(ty.map_or("?", |t| t.key.as_str()), STRONG),
            ];
            let mut s = String::new();
            if e.freq_hz > 0.0 {
                s.push_str(" on ");
                format::hz(&mut s, e.freq_hz);
            }
            if let Some(t) = ty {
                for (d, v) in t.fields.iter().zip(&e.fields) {
                    if *v != Value::None {
                        let _ = write!(s, " {}=", d.key);
                        format::value(&mut s, v, d);
                    }
                }
            }
            spans.push(Span::raw(s));
            Line::from(spans)
        })
        .collect();
    f.render_widget(
        Paragraph::new(text).block(block(&format!("Events ({})", app.events.len()))),
        list,
    );
}

fn commands(f: &mut Frame, area: Rect, app: &App) {
    let [left, list, right] = Layout::horizontal([
        Constraint::Length(OBJECTS_WIDTH),
        Constraint::Percentage(30),
        Constraint::Min(30),
    ])
    .areas(area);
    objects_pane(f, left, app);
    let focused = app.pane == 1;
    let title = format!("Commands of the {}", object_title(app));
    let Some(o) = app.target().filter(|o| !o.commands.is_empty()) else {
        f.render_widget(
            Paragraph::new("This object has no commands.").block(pane(&title, focused)),
            list,
        );
        return;
    };
    let height = usize::from(list.height.saturating_sub(2));
    let first = window(app.sel.command, o.commands.len(), height);
    let items: Vec<ListItem> = o
        .commands
        .iter()
        .skip(first)
        .take(height)
        .map(|c| ListItem::new(format!("{}  {}", c.key, c.label)))
        .collect();
    let mut state = ListState::default()
        .with_selected((!o.commands.is_empty()).then(|| app.sel.command - first));
    // While the form of a command is open, the keys act on the form and Tab does not leave it.
    let focus = match (focused, app.form.is_some()) {
        (true, false) => Focus::Here,
        (false, false) => Focus::Reachable,
        (_, true) => Focus::Never,
    };
    f.render_stateful_widget(marked_list(items, &title, focus), list, &mut state);
    let Some(c) = o.commands.get(app.sel.command) else {
        return;
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(c.key.as_str(), ACCENT),
        Span::styled(format!("  {}", c.label), STRONG),
    ])];
    if !c.help.is_empty() {
        lines.push(Line::raw(c.help.as_str()));
    }
    lines.push(Line::raw(""));
    match &app.form {
        Some(form) if form.command == app.sel.command => {
            for (i, (d, v)) in c.args.iter().zip(&form.values).enumerate() {
                let value = match v {
                    Some(v) => format::value_text(v, d),
                    None if d.required => "(required)".to_owned(),
                    None => format!("(default {})", format::value_text(&d.default, d)),
                };
                let (mark, style) = if i == form.sel {
                    (MARK, theme::selected(true))
                } else {
                    (NO_MARK, TEXT)
                };
                let name = d.ty.name();
                let text = format!("{mark}{}: {value}   [{name}] {}", d.label, d.help);
                lines.push(Line::styled(text, style));
            }
        }
        _ => {
            for d in &c.args {
                lines.push(Line::raw(format!(
                    "{} [{}] {}",
                    d.label,
                    d.ty.name(),
                    d.help
                )));
            }
        }
    }
    if !app.reply.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Reply", STRONG));
        for (label, value) in &app.reply {
            lines.push(Line::raw(format!("{label}: {value}")));
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(if app.form.is_some() {
                pane("Command", true)
            } else {
                block("Command")
            }),
        right,
    );
}

fn services(f: &mut Frame, area: Rect, app: &App) {
    let [list, right] =
        Layout::horizontal([Constraint::Percentage(25), Constraint::Min(30)]).areas(area);
    let all = app.services();
    let items: Vec<ListItem> = all.iter().map(|(_, n)| ListItem::new(n.as_str())).collect();
    let mut state =
        ListState::default().with_selected((!all.is_empty()).then_some(app.sel.service));
    f.render_stateful_widget(
        marked_list(items, "Services", Focus::of(app.pane == 0)),
        list,
        &mut state,
    );
    let Some(o) = &app.service else {
        f.render_widget(
            Paragraph::new("The engine has no service, or the service could not be read.")
                .block(block("Service")),
            right,
        );
        return;
    };
    let [head, fields, table] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Percentage(35),
        Constraint::Min(5),
    ])
    .areas(right);
    let mut s = vec![Span::styled(format!(" {}   ", o.name), ACCENT)];
    summary(&mut s, &o.results);
    f.render_widget(Paragraph::new(Line::from(s)), head);
    fields_list(f, fields, "Results", &[&o.results]);
    let focused = app.pane == 1;
    let (t, row, column) = (app.sel.table, app.sel.row, app.sel.column);
    results_table(f, table, &app.shown_tables(), t, row, focused, column, None);
}

/// The text of each row of table `key` of `r` in the order the engine wrote them: the row key, then the columns `cols`.
/// Absent cells are empty.
fn load_rows(r: &Results, key: &str, cols: &[&str]) -> Vec<Vec<String>> {
    let Some(t) = r.table_id(key) else {
        return Vec::new();
    };
    let ids: Vec<Option<usize>> = cols.iter().map(|c| r.column_id(t, c)).collect();
    let decls = &r.decl.tables[t].columns;
    // The copy holds the rows newest first.
    r.tables[t]
        .rows
        .iter()
        .rev()
        .map(|row| {
            let mut cells = Vec::with_capacity(cols.len() + 1);
            cells.push(row.key.clone());
            for id in &ids {
                let text = id
                    .and_then(|c| Some(format::value_text(row.cells.get(c)?, decls.get(c)?)))
                    .unwrap_or_default();
                cells.push(text);
            }
            cells
        })
        .collect()
}

/// The number in a cell of [`load_rows`], for sorting.
fn number(cell: &str) -> f64 {
    cell.split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0.0)
}

/// Draws the load of the live session: the line of the cores it needs, its components with the largest share first,
/// its stages with their thread and the fill of their queue, and its connections with their fill and waits. The screen
/// reads the copy of the results that the last tick read.
fn load(f: &mut Frame, area: Rect, app: &App) {
    let Some(live) = &app.live else {
        f.render_widget(
            Paragraph::new("No session runs. Start a stack on the Stacks screen.").block(pane("Load", false)),
            area,
        );
        return;
    };
    let r = &live.session.results;
    let text = |key| crate::views::field(r, key).map(|v| match v {
        Value::F64(x) => format!("{x:.2}"),
        Value::I64(n) => n.to_string(),
        Value::String(t) => t.clone(),
        _ => String::new(),
    });
    let mut head = vec![Span::styled(" ", TEXT)];
    match text("session.load_problem") {
        Some(p) => head.push(Span::styled(p, WARNING)),
        None => {
            let need = text("session.cores_needed").unwrap_or_else(|| "-".into());
            let free = text("session.cores_free").unwrap_or_else(|| "-".into());
            head.push(Span::styled(format!("needs about {need} cores, {free} free"), VALUE));
        }
    }
    for (label, key) in [
        ("share", "session.share"),
        ("threads", "session.threads"),
        ("lost", "session.dropped"),
        ("gaps", "session.gaps"),
        ("stage waits", "session.stage_behind"),
    ] {
        if let Some(v) = text(key) {
            head.push(Span::styled(format!("   {label} {v}"), HINT));
        }
    }
    let [top, comps, stages, conns] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
    ])
    .areas(area);
    f.render_widget(Paragraph::new(Line::from(head)), top);
    let mut rows = load_rows(
        r,
        "session.components",
        &["stage", "thread", "share", "share_total", "calls", "mean_us", "worst_us", "units"],
    );
    rows.sort_by(|a, b| number(&b[3]).total_cmp(&number(&a[3])));
    let header = ["Component", "Stage", "Thread", "Share", "Since start", "Calls", "Mean", "Worst", "Units"];
    text_table(
        f,
        comps,
        block("Components by share"),
        header.map(str::to_owned).to_vec(),
        rows,
        0,
        None,
        false,
        0,
        "The stack of this session is no graph, or the first measurement has not come.",
    );
    let rows = load_rows(
        r,
        "session.stages",
        &["thread", "cut", "share", "cost", "declared", "queue_fill", "queue_units", "behind"],
    );
    let header = ["Stage", "Thread", "Own thread", "Share", "Cost", "Declared", "Queue fill", "Waiting", "Behind"];
    text_table(
        f,
        stages,
        block("Stages"),
        header.map(str::to_owned).to_vec(),
        rows,
        0,
        None,
        false,
        0,
        "",
    );
    let rows = load_rows(
        r,
        "session.connections",
        &[
            "format", "items_per_s", "fill", "held_max_bytes", "wait_mean_us", "wait_worst_us", "full", "empty",
            "cut", "lost",
        ],
    );
    let header = [
        "Connection", "Format", "Items", "Fill", "Held most", "Wait", "Wait worst", "Full", "Empty", "Cut", "Lost",
    ];
    text_table(
        f,
        conns,
        block("Connections"),
        header.map(str::to_owned).to_vec(),
        rows,
        0,
        None,
        false,
        0,
        "",
    );
}
