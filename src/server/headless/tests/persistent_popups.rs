use super::*;
use crate::persistent_popup::{PersistentPopupKey, PersistentPopupOptions, PopupOwner, PopupScope};

fn server_with_persistent_command() -> HeadlessServer {
    let mut server = test_headless_server();
    let config: crate::config::Config = toml::from_str(
        r#"
[[keys.command]]
key = "f12"
type = "persistent_popup"
command = "test-shell"
scope = "global"
"#,
    )
    .unwrap();
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    server.app = crate::app::App::new(
        &config,
        crate::app::AppPolicy::TEST,
        None,
        rx,
        api::EventHub::default(),
    );
    install_shared_view_test_runtime(&mut server);
    server
}

#[tokio::test]
async fn persistent_popup_manifest_is_hidden_from_unaware_clients() {
    let mut server = server_with_persistent_command();
    let (control, _) = connect_matching_test_shell(&mut server, 71);
    let snapshots = control
        .try_iter()
        .filter_map(|bytes| match read_server_message(bytes) {
            ServerMessage::EndpointControl { kind, data }
                if kind == crate::protocol::endpoint::ENDPOINT_SNAPSHOT_KIND =>
            {
                Some(serde_json::from_str::<crate::protocol::ClientShellSnapshot>(&data).unwrap())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].commands.is_empty());
    server.clients.get_mut(&71).unwrap().shell_persistent_popups = true;
    server.render_and_stream();
    let projections = control
        .try_iter()
        .filter_map(|bytes| match read_server_message(bytes) {
            ServerMessage::EndpointControl { kind, data }
                if kind == crate::persistent_popup::PRESENTATION_KIND =>
            {
                Some(
                    serde_json::from_str::<crate::persistent_popup::PopupProjection>(&data)
                        .unwrap(),
                )
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(projections.len(), 1);
    assert_eq!(projections[0].commands.len(), 1);
    assert_eq!(projections[0].commands[0].1, vec!["f12"]);
    shutdown_test_runtimes(&mut server);
}

#[tokio::test]
async fn persistent_popup_global_toggle_transfers_to_invoking_client_tab_only() {
    let mut server = server_with_persistent_command();
    let second = server.app.state.workspaces[0].test_add_tab(Some("second"));
    let second_pane = server.app.state.workspaces[0].tabs[second].root_pane;
    server.app.state.workspaces[0].insert_test_runtime(
        second_pane,
        crate::terminal::TerminalRuntime::test_with_screen_bytes(80, 23, b"SECOND"),
    );
    let first_tab = server.app.public_tab_id(0, 0).unwrap();
    let second_tab = server.app.public_tab_id(0, second).unwrap();
    let (_c1, _r1) = connect_matching_test_shell(&mut server, 81);
    let (_c2, _r2) = connect_matching_test_shell(&mut server, 82);
    assert!(server.focus_shell_client_on_tab(82, &second_tab));
    let (_, terminal_id) = server.app.install_test_popup_runtime(
        crate::terminal::TerminalRuntime::test_with_screen_bytes(40, 12, b"GLOBAL"),
    );
    let options = PersistentPopupOptions {
        scope: PopupScope::Global,
        ..Default::default()
    };
    let key = PersistentPopupKey {
        label: "f12".into(),
        command: "test-shell".into(),
        owner: PopupOwner::Global,
    };
    let popup = server.app.state.popup_pane.as_mut().unwrap();
    popup.options = Some(options);
    popup.persistent_key = Some(key.clone());
    server
        .app
        .state
        .persistent_popups
        .insert(key, popup.clone());
    server.popup_owner_tab_id = Some(first_tab);
    let command_id = server.app.client_shell_command_manifest()[0]
        .command_id
        .clone();
    let (respond_to, response) = std::sync::mpsc::channel();
    server.handle_client_shell_api_request(
        82,
        api::ApiRequestMessage {
            request: api::schema::Request {
                id: "transfer".into(),
                method: api::schema::Method::CommandInvoke(api::schema::CommandInvokeParams {
                    command_id,
                    workspace_id: None,
                    tab_id: Some(second_tab.clone()),
                    pane_id: None,
                    selection: None,
                }),
            },
            respond_to,
            response_write_complete: None,
        },
    );
    let response: api::schema::SuccessResponse =
        serde_json::from_str(&response.recv().unwrap()).unwrap();
    assert_eq!(response.result, api::schema::ResponseResult::Ok {});
    assert_eq!(server.popup_owner_tab_id.as_ref(), Some(&second_tab));
    assert_eq!(
        server.app.state.popup_pane.as_ref().unwrap().terminal_id,
        terminal_id
    );
    let (respond_to, _response) = std::sync::mpsc::channel();
    server.handle_client_shell_api_request(
        81,
        api::ApiRequestMessage {
            request: api::schema::Request {
                id: "list".into(),
                method: api::schema::Method::WorkspaceList(api::schema::EmptyParams::default()),
            },
            respond_to,
            response_write_complete: None,
        },
    );
    assert_eq!(server.popup_owner_tab_id.as_ref(), Some(&second_tab));
    shutdown_test_runtimes(&mut server);
}
