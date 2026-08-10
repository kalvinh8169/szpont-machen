use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

pub const HEIGHT: u16 = 6;
pub const WIDTH: u16 = 21;

pub fn carton(flapped: bool) -> Vec<Line<'static>> {
    let wings = Style::new().fg(Color::White);
    let body = Style::new().fg(Color::Yellow);
    let (wing_1, wing_2, wing_3) = if flapped {
        (
            "                    ",
            "       _  _         ",
            "       \\ \\/ \\       ",
        )
    } else {
        (
            "      __    __      ",
            "      \\ \\  / /      ",
            "       \\ \\/ /       ",
        )
    };
    vec![
        Line::from(Span::styled(wing_1, wings)),
        Line::from(Span::styled(wing_2, wings)),
        Line::from(Span::styled(wing_3, wings)),
        Line::from(Span::styled("     .-=(o o)=-.    ", body)),
        Line::from(stripe_line("  ==[ ", " ]==>")),
        Line::from(Span::styled("     '-=(___)=-'    ", body)),
    ]
}

fn stripe_line(open: &'static str, close: &'static str) -> Vec<Span<'static>> {
    let body = Style::new().fg(Color::Yellow);
    let stripe_a = Style::new().fg(Color::Yellow).bold();
    let stripe_b = Style::new().fg(Color::DarkGray).bold();
    let mut spans = vec![Span::styled(open, body)];
    for i in 0..5 {
        let style = if i % 2 == 0 { stripe_a } else { stripe_b };
        spans.push(Span::styled("≡≡", style));
    }
    spans.push(Span::styled(close, body));
    spans
}

pub fn compact() -> Span<'static> {
    Span::styled(
        " szpont machen ",
        Style::new().bold().fg(Color::Black).bg(Color::Yellow),
    )
}
