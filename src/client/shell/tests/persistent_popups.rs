use super::*;
use crate::persistent_popup::{PersistentPopupOptions, PopupEdge, PopupProjection, PopupStyle};

pub(super) fn persistent_state(style: PopupStyle, edge: PopupEdge) -> ClientShellState {
    let config: Config = toml::from_str(
        r#"
[[keys.command]]
key = "f12"
type = "persistent_popup"
command = "zsh"
"#,
    )
    .unwrap();
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    let mut projection = snapshot();
    projection
        .commands
        .push(crate::protocol::ClientShellCommand {
            command_id: "cmd_scratch".into(),
            binding_label: "f12".into(),
            binding_labels: vec!["f12".into()],
            action: crate::protocol::ClientShellCommandAction::Popup,
            description: None,
        });
    let boot_id = projection.boot_id.clone();
    state.set_snapshot(Box::new(projection));
    state.active_snapshot_generation = Some(1);
    state.set_persistent_popup_projection(
        &state.active_endpoint_id.clone(),
        1,
        PopupProjection {
            boot_id,
            commands: vec![("cmd_scratch".into(), vec!["f12".into()])],
            visible: Some((
                "terminal-popup".into(),
                PersistentPopupOptions {
                    style,
                    edge,
                    ..Default::default()
                },
            )),
        },
    );
    state.set_pane_surface(surface_with_popup());
    state
}

#[test]
fn persistent_popup_f12_invokes_toggle_instead_of_forwarding_key() {
    let mut state = persistent_state(PopupStyle::Overlay, PopupEdge::Bottom);
    let input = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::F(12),
        KeyModifiers::empty(),
    ))]);
    assert!(input.requests.is_empty());
    assert!(
        matches!(&input.actions[..], [ClientShellAction::Endpoint { request, .. }]
        if matches!(&request.method, crate::api::schema::Method::CommandInvoke(params) if params.command_id == "cmd_scratch"))
    );
    assert!(state.popup_pending);
    state.set_pane_surface(surface());
    assert!(!state.popup_pending);
    assert!(matches!(
        &state.handle_input_bytes(b"x").requests[..],
        [ClientMessage::ClientShellPaneInput { .. }]
    ));
}

#[test]
fn persistent_popup_other_input_still_goes_to_shell() {
    let mut state = persistent_state(PopupStyle::Overlay, PopupEdge::Bottom);
    for bytes in [b"x".as_slice(), b"\x1b".as_slice()] {
        assert!(matches!(
            &state.handle_input_bytes(bytes).requests[..],
            [ClientMessage::ClientShellPopupInput { .. }]
        ));
    }
}

#[test]
fn persistent_popup_drawer_client_geometry_uses_projected_edge() {
    for edge in [PopupEdge::Top, PopupEdge::Bottom] {
        let mut state = persistent_state(PopupStyle::Drawer, edge);
        state.compose(106, 20).unwrap();
        let rect = state.hits.popup.as_ref().unwrap().rect;
        // The pane surface excludes client-owned chrome; the drawer spans that surface.
        assert!(rect.width > 12);
        if edge == PopupEdge::Top {
            assert!(rect.y < 5);
        } else {
            assert_eq!(rect.bottom(), 20);
        }
    }
}

#[test]
fn persistent_popup_stale_generation_does_not_intercept_shell_keys() {
    let mut state = persistent_state(PopupStyle::Overlay, PopupEdge::Bottom);
    state.active_snapshot_generation = Some(2);
    let input = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::F(12),
        KeyModifiers::empty(),
    ))]);
    assert!(input.actions.is_empty());
    assert!(matches!(
        &input.requests[..],
        [ClientMessage::ClientShellPopupInput { .. }]
    ));
}
