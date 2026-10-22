use super::*;

impl ClientShellState {
    pub(crate) fn set_persistent_popup_projection(
        &mut self,
        endpoint_id: &ClientEndpointId,
        generation: u64,
        projection: crate::persistent_popup::PopupProjection,
    ) {
        if self
            .persistent_popup_projections
            .get(endpoint_id)
            .is_some_and(|(current, _)| *current > generation)
        {
            return;
        }
        self.persistent_popup_projections
            .insert(endpoint_id.clone(), (generation, projection));
    }

    fn persistent_popup_projection(&self) -> Option<&crate::persistent_popup::PopupProjection> {
        let (generation, projection) = self
            .persistent_popup_projections
            .get(&self.active_endpoint_id)?;
        let snapshot = self.snapshot.as_deref()?;
        (self.active_snapshot_generation == Some(*generation)
            && snapshot.boot_id == projection.boot_id)
            .then_some(projection)
    }

    pub(super) fn persistent_popup_options(
        &self,
        terminal_id: &str,
    ) -> Option<crate::persistent_popup::PersistentPopupOptions> {
        let (id, options) = self.persistent_popup_projection()?.visible.as_ref()?;
        (id == terminal_id).then_some(*options)
    }

    pub(super) fn route_persistent_popup_toggle(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        let Some(terminal_id) = self.popup_terminal_id.as_deref() else {
            return false;
        };
        if self.persistent_popup_options(terminal_id).is_none() {
            return false;
        }
        let prefixed = self.persistent_popup_prefix;
        let dispatch = if prefixed {
            self.persistent_popup_prefix = false;
            crate::input::KeybindDispatch::Prefix
        } else {
            if self.config.keybinds.matches_prefix(key) {
                self.persistent_popup_prefix = true;
                return true;
            }
            crate::input::KeybindDispatch::Direct
        };
        let Some(command) =
            crate::input::resolve_custom_command(&self.config.keybinds.keybinds, key, dispatch)
        else {
            return prefixed;
        };
        let labels = command.bindings.labels();
        let persistent = self
            .persistent_popup_projection()
            .is_some_and(|projection| {
                projection.commands.iter().any(|(id, bindings)| {
                    id == &command.command
                        || (!labels.is_empty()
                            && labels.iter().all(|label| bindings.contains(label)))
                })
            });
        if !persistent {
            return prefixed;
        }
        if key.kind != crossterm::event::KeyEventKind::Press {
            return true;
        }
        self.record_binding(crate::input::KeybindMatch::Command(command), outcome);
        true
    }
}
