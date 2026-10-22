use super::*;

fn popup_state(copy_on_select: bool) -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.config.copy_on_select = copy_on_select;
    state.set_snapshot(Box::new(snapshot()));
    let mut surface = surface_with_popup();
    let popup = surface.popup.as_mut().unwrap();
    popup.mouse_reporting = false;
    popup.width = Some(crate::protocol::ClientShellPopupSize::Cells(24));
    popup.height = Some(crate::protocol::ClientShellPopupSize::Cells(7));
    popup.frame = FrameData::from_ratatui_buffer(
        &Buffer::with_lines(["alpha   ", "  bravo", "charlie"]),
        None,
    );
    state.set_pane_surface(surface);
    state.compose(106, 20).unwrap();
    state
}

fn mouse(
    state: &mut ClientShellState,
    kind: MouseEventKind,
    col: u16,
    row: u16,
) -> ClientShellInput {
    let rect = state.hits.popup.as_ref().unwrap().inner_rect;
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind,
        column: rect.x + col,
        row: rect.y + row,
        modifiers: KeyModifiers::empty(),
    })])
}

#[test]
fn popup_selection_copies_multiline_text_without_chrome_or_endpoint_requests() {
    let mut state = popup_state(true);
    let down = mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 2, 0);
    assert!(down.requests.is_empty() && down.actions.is_empty());
    mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 2, 2);
    let selected = state.compose(106, 20).unwrap();
    let rect = state.hits.popup.as_ref().unwrap().inner_rect;
    let highlighted = &selected.cells[usize::from(rect.y) * 106 + usize::from(rect.x + 2)];
    assert_ne!(
        highlighted.bg,
        crate::protocol::color_to_u32(ratatui::style::Color::Reset)
    );
    let up = mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 2, 2);
    assert!(
        matches!(&up.actions[..], [ClientShellAction::ClipboardWrite(text)] if text == b"pha\n  bravo\ncha")
    );
    assert!(up.requests.is_empty());
    assert!(state.selection.is_none() && state.popup_selection_frame.is_none());
}

#[test]
fn popup_selection_retains_rendered_text_while_output_continues() {
    let mut state = popup_state(false);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 0, 0);
    mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 4, 0);
    let mut newer = state.pane_surface.as_ref().unwrap().clone();
    newer.popup.as_mut().unwrap().frame =
        FrameData::from_ratatui_buffer(&Buffer::with_lines(["new output"]), None);
    state.set_pane_surface(newer);
    let frame = state.compose(106, 20).unwrap();
    let rect = state.hits.popup.as_ref().unwrap().inner_rect;
    assert_eq!(
        frame.cells[usize::from(rect.y) * 106 + usize::from(rect.x)].symbol,
        "a"
    );
    assert!(
        mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 4, 0)
            .actions
            .is_empty()
    );
    let copy = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ))]);
    assert!(
        matches!(&copy.actions[..], [ClientShellAction::ClipboardWrite(text)] if text == b"alpha")
    );
    assert!(copy.requests.is_empty());
    assert!(state.selection.is_none() && state.popup_selection_frame.is_none());
}

#[test]
fn popup_selection_works_in_drawers_and_overlays_and_clamps_outside_release() {
    use crate::persistent_popup::{PopupEdge, PopupStyle};
    for (style, edge) in [
        (PopupStyle::Drawer, PopupEdge::Top),
        (PopupStyle::Drawer, PopupEdge::Bottom),
        (PopupStyle::Overlay, PopupEdge::Bottom),
    ] {
        let mut state = super::persistent_popups::persistent_state(style, edge);
        let mut next = state.pane_surface.as_ref().unwrap().clone();
        next.popup.as_mut().unwrap().mouse_reporting = false;
        state.set_pane_surface(next);
        state.compose(106, 20).unwrap();
        mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 0, 0);
        let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 105,
            row: 19,
            modifiers: KeyModifiers::empty(),
        })]);
        assert!(drag.requests.is_empty());
        let up = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 105,
            row: 19,
            modifiers: KeyModifiers::empty(),
        })]);
        // The overlay reserves a terminal column; its last source glyph is clipped.
        let expected = if style == PopupStyle::Overlay {
            b"popup-liv\n\n".as_slice()
        } else {
            b"popup-live\n\n".as_slice()
        };
        assert!(
            matches!(&up.actions[..], [ClientShellAction::ClipboardWrite(text)] if text == expected),
            "{style:?} {edge:?}: {:?}",
            up.actions
        );
        assert!(state.selection.is_none() && state.popup_selection_frame.is_none());
    }
}

#[test]
fn popup_selection_does_not_steal_mouse_reporting_app_gestures() {
    let mut state = popup_state(true);
    let mut next = state.pane_surface.as_ref().unwrap().clone();
    next.popup.as_mut().unwrap().mouse_reporting = true;
    state.set_pane_surface(next);
    state.compose(106, 20).unwrap();
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let input = mouse(&mut state, kind, 0, 0);
        assert!(matches!(
            &input.requests[..],
            [ClientMessage::ClientShellPopupInput { .. }]
        ));
        assert!(input.actions.is_empty());
        assert!(state.selection.is_none() && state.popup_selection_frame.is_none());
    }
}

#[test]
fn popup_selection_click_alone_does_not_copy() {
    let mut state = popup_state(true);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 0, 0);
    let up = mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 0, 0);
    assert!(up.actions.is_empty() && up.requests.is_empty());
    assert!(state.selection.is_none() && state.popup_selection_frame.is_none());
}

#[test]
fn popup_selection_is_cleared_on_hide_resize_scroll_and_paste() {
    for action in 0..4 {
        let mut state = popup_state(false);
        mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 0, 0);
        mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 4, 0);
        match action {
            0 => state.set_pane_surface(surface()),
            1 => state.invalidate_pane_surface(),
            2 => {
                mouse(&mut state, MouseEventKind::ScrollUp, 0, 0);
            }
            _ => {
                let paste =
                    state.handle_raw_events(vec![RawInputEvent::Paste("paste\ntext".into())]);
                assert!(matches!(
                    &paste.requests[..],
                    [ClientMessage::ClientShellPopupInput { .. }]
                ));
            }
        }
        assert!(
            state.selection.is_none() && state.popup_selection_frame.is_none(),
            "action {action}"
        );
    }
}

#[test]
fn popup_selection_copies_wide_and_combining_graphemes_once_in_reverse() {
    let mut state = popup_state(true);
    let mut next = state.pane_surface.as_ref().unwrap().clone();
    next.popup.as_mut().unwrap().frame =
        FrameData::from_ratatui_buffer(&Buffer::with_lines(["a界e\u{301}🙂z", "second"]), None);
    state.set_pane_surface(next);
    state.compose(106, 20).unwrap();
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 2, 1);
    mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 2, 0);
    let up = mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 2, 0);
    assert!(
        matches!(&up.actions[..], [ClientShellAction::ClipboardWrite(text)] if text == "界e\u{301}🙂z\nsec".as_bytes())
    );
}
