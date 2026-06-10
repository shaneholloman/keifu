//! Event loop and input handling

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, MouseEventKind};

/// Poll for events (100ms timeout) and drain everything already queued.
///
/// Processing the whole batch before the next render keeps scrolling
/// responsive: rendering once per event made queued scroll events burst
/// out at unpredictable speed (issue #12).
pub fn poll_events() -> Result<Vec<Event>> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(Vec::new());
    }
    let mut events = vec![event::read()?];
    while event::poll(Duration::ZERO)? {
        events.push(event::read()?);
    }
    Ok(coalesce_scroll_events(events))
}

fn is_scroll(kind: MouseEventKind) -> bool {
    matches!(kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown)
}

/// Collapse runs of consecutive same-direction scroll events into one.
///
/// Terminals emit a different number of scroll events per wheel notch
/// (1 in Windows Terminal, ~3 in kitty, ~8 in Ghostty), and there is no
/// protocol-level notion of a "notch". A notch's events arrive as one burst,
/// so within a drained batch one same-direction run ≈ one notch. Collapsing
/// the run normalizes wheel speed to one step per notch everywhere
/// (issue #12).
fn coalesce_scroll_events(events: Vec<Event>) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    for event in events {
        if let (Event::Mouse(current), Some(Event::Mouse(prev))) = (&event, out.last()) {
            if is_scroll(current.kind) && current.kind == prev.kind {
                continue;
            }
        }
        out.push(event);
    }
    out
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

    use super::*;

    fn scroll(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn collapses_same_direction_scroll_bursts() {
        let burst = vec![
            scroll(MouseEventKind::ScrollDown),
            scroll(MouseEventKind::ScrollDown),
            scroll(MouseEventKind::ScrollDown),
        ];
        assert_eq!(coalesce_scroll_events(burst).len(), 1);
    }

    #[test]
    fn keeps_direction_changes_and_other_events() {
        let key = Event::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        let events = vec![
            scroll(MouseEventKind::ScrollDown),
            scroll(MouseEventKind::ScrollDown),
            scroll(MouseEventKind::ScrollUp),
            key.clone(),
            scroll(MouseEventKind::ScrollDown),
        ];
        let out = coalesce_scroll_events(events);
        assert_eq!(out.len(), 4); // down, up, key, down
    }
}
