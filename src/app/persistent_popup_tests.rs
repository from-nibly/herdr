use super::*;
use crate::persistent_popup::{PersistentPopupKey, PersistentPopupOptions, PopupOwner, PopupScope};

fn fixture(scope: PopupScope) -> (App, crate::config::CustomCommandKeybind, TerminalId) {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        &crate::config::Config::default(),
        crate::app::AppPolicy::TEST,
        None,
        rx,
        crate::api::EventHub::default(),
    );
    app.state.workspaces = vec![
        crate::workspace::Workspace::test_new("one"),
        crate::workspace::Workspace::test_new("two"),
    ];
    app.state.active = Some(0);
    app.state.selected = 0;
    for workspace in &app.state.workspaces {
        for tab in &workspace.tabs {
            for pane in tab.panes.values() {
                let id = pane.attached_terminal_id.clone();
                app.state
                    .terminals
                    .insert(id.clone(), TerminalState::new(id, PathBuf::from("/tmp")));
            }
        }
    }
    let options = PersistentPopupOptions {
        scope,
        ..Default::default()
    };
    let binding = crate::config::CustomCommandKeybind {
        bindings: crate::config::ActionKeybinds::from_labels(&["f12".into()]).unwrap(),
        label: "f12".into(),
        command: "zsh".into(),
        action: crate::config::CustomCommandAction::Popup,
        description: None,
        width: None,
        height: None,
        persistent_popup: Some(options),
    };
    let runtime = TerminalRuntime::test_with_screen_bytes(40, 12, b"SAVED_OUTPUT");
    let (_, terminal_id) = app.install_test_popup_runtime(runtime);
    let owner = match scope {
        PopupScope::Global => PopupOwner::Global,
        PopupScope::Workspace => PopupOwner::Workspace(app.state.workspaces[0].id.clone()),
        PopupScope::Tab => PopupOwner::Tab(app.public_tab_id(0, 0).unwrap()),
    };
    let key = PersistentPopupKey {
        label: binding.label.clone(),
        command: binding.command.clone(),
        owner,
    };
    let popup = app.state.popup_pane.as_mut().unwrap();
    popup.persistent_key = Some(key.clone());
    popup.options = Some(options);
    app.state.persistent_popups.insert(key, popup.clone());
    (app, binding, terminal_id)
}

#[tokio::test]
async fn persistent_popup_toggle_retains_terminal_and_runtime() {
    let (mut app, binding, terminal_id) = fixture(PopupScope::Workspace);
    let options = binding.persistent_popup.unwrap();
    for _ in 0..3 {
        app.toggle_persistent_popup(&binding, options).unwrap();
        assert!(app.state.popup_pane.is_none());
        assert!(app.state.terminals.contains_key(&terminal_id));
        assert!(app.terminal_runtimes.get(&terminal_id).is_some());
        app.state.assert_invariants_for_test();
        app.toggle_persistent_popup(&binding, options).unwrap();
        assert_eq!(
            app.state.popup_pane.as_ref().unwrap().terminal_id,
            terminal_id
        );
    }
    app.close_popup_pane();
    assert!(app.state.persistent_popups.is_empty());
    assert!(app.terminal_runtimes.get(&terminal_id).is_none());
}

#[tokio::test]
async fn persistent_popup_global_reuses_terminal_in_other_workspace() {
    let (mut app, binding, terminal_id) = fixture(PopupScope::Global);
    app.hide_persistent_popup();
    app.state.active = Some(1);
    app.toggle_persistent_popup(&binding, binding.persistent_popup.unwrap())
        .unwrap();
    assert_eq!(
        app.state.popup_pane.as_ref().unwrap().terminal_id,
        terminal_id
    );
    app.close_popup_pane();
}

#[tokio::test]
async fn persistent_popup_scope_cleanup_uses_identity_not_index() {
    for scope in [PopupScope::Workspace, PopupScope::Tab] {
        let (mut app, _, terminal_id) = fixture(scope);
        app.hide_persistent_popup();
        app.state.workspaces.swap(0, 1);
        app.reconcile_persistent_popups();
        assert!(app.state.terminals.contains_key(&terminal_id));
        app.state.workspaces.remove(1);
        app.state.active = Some(0);
        app.reconcile_persistent_popups();
        assert!(app.state.persistent_popups.is_empty());
        assert!(!app.state.terminals.contains_key(&terminal_id));
        assert!(app.terminal_runtimes.get(&terminal_id).is_none());
    }
}

#[tokio::test]
async fn persistent_popup_workspace_and_tab_instances_do_not_alias() {
    for scope in [PopupScope::Workspace, PopupScope::Tab] {
        let (mut app, binding, first) = fixture(scope);
        let options = binding.persistent_popup.unwrap();
        let owner = if scope == PopupScope::Tab {
            let tab_index = app.state.workspaces[0].test_add_tab(Some("second"));
            for pane in app.state.workspaces[0].tabs[tab_index].panes.values() {
                let id = pane.attached_terminal_id.clone();
                app.state
                    .terminals
                    .insert(id.clone(), TerminalState::new(id, PathBuf::from("/tmp")));
            }
            PopupOwner::Tab(app.public_tab_id(0, tab_index).unwrap())
        } else {
            PopupOwner::Workspace(app.state.workspaces[1].id.clone())
        };
        let first_pane = app.state.popup_pane.as_ref().unwrap().pane_id;
        app.hide_persistent_popup();
        let (_, second) = app
            .install_test_popup_runtime(TerminalRuntime::test_with_screen_bytes(40, 12, b"SECOND"));
        let key = PersistentPopupKey {
            label: binding.label.clone(),
            command: binding.command.clone(),
            owner,
        };
        let popup = app.state.popup_pane.as_mut().unwrap();
        popup.options = Some(options);
        popup.persistent_key = Some(key.clone());
        app.state.persistent_popups.insert(key, popup.clone());
        app.hide_persistent_popup();
        app.toggle_persistent_popup(&binding, options).unwrap();
        assert_eq!(app.state.popup_pane.as_ref().unwrap().terminal_id, first);
        if scope == PopupScope::Tab {
            app.state.workspaces[0].active_tab = 1;
        } else {
            app.state.active = Some(1);
        }
        app.toggle_persistent_popup(&binding, options).unwrap();
        assert_eq!(app.state.popup_pane.as_ref().unwrap().terminal_id, second);
        assert_ne!(first, second);
        assert_eq!(app.state.persistent_popups.len(), 2);
        app.state.assert_invariants_for_test();
        app.close_popup_pane();
        assert!(app.close_hidden_popup_for_pane(first_pane));
    }
}

#[tokio::test]
async fn persistent_popup_hidden_exit_cleans_up_without_closing_visible_popup() {
    let (mut app, _, terminal_id) = fixture(PopupScope::Workspace);
    let pane_id = app.state.popup_pane.as_ref().unwrap().pane_id;
    app.hide_persistent_popup();
    let (_, other) =
        app.install_test_popup_runtime(TerminalRuntime::test_with_screen_bytes(40, 12, b"OTHER"));
    assert!(app.close_hidden_popup_for_pane(pane_id));
    assert!(!app.state.terminals.contains_key(&terminal_id));
    assert_eq!(app.state.popup_pane.as_ref().unwrap().terminal_id, other);
    app.close_popup_pane();
}
