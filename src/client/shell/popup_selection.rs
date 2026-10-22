use super::*;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

pub(super) struct PopupSelectionFrame {
    terminal_id: String,
    frame: FrameData,
    width: u16,
    height: u16,
}

impl ClientShellState {
    pub(super) fn clear_popup_selection(&mut self) -> bool {
        let Some(captured) = self.popup_selection_frame.take() else {
            return false;
        };
        if self
            .selection
            .as_ref()
            .is_some_and(|s| s.pane_id == captured.terminal_id)
        {
            self.selection = None;
        }
        self.stop_selection_autoscroll();
        true
    }

    pub(super) fn selected_popup_frame(&self, terminal_id: &str) -> Option<&FrameData> {
        self.popup_selection_frame
            .as_ref()
            .filter(|captured| {
                captured.terminal_id == terminal_id
                    && self
                        .selection
                        .as_ref()
                        .is_some_and(|s| s.pane_id == terminal_id)
            })
            .map(|captured| &captured.frame)
    }

    pub(super) fn copy_popup_selection(&mut self, outcome: &mut ClientShellInput) -> bool {
        let Some(captured) = self.popup_selection_frame.as_ref() else {
            return false;
        };
        let Some(selection) = self
            .selection
            .as_ref()
            .filter(|s| s.pane_id == captured.terminal_id)
        else {
            return false;
        };
        let ((start_row, start_col), (end_row, end_col)) = selection.ordered_cells();
        let mut lines = Vec::new();
        for row in start_row..=end_row.min(u32::from(captured.height.saturating_sub(1))) {
            let from = if row == start_row { start_col } else { 0 };
            let through = if row == end_row {
                end_col
            } else {
                captured.width.saturating_sub(1)
            };
            let mut line = String::new();
            let mut col = 0;
            while col < captured.width && col <= through {
                let index = row as usize * usize::from(captured.frame.width) + usize::from(col);
                let Some(cell) = captured.frame.cells.get(index) else {
                    break;
                };
                let width = u16::try_from(UnicodeWidthStr::width(cell.symbol.as_str()))
                    .unwrap_or(u16::MAX)
                    .max(1);
                if col <= through && col.saturating_add(width) > from {
                    line.push_str(&cell.symbol);
                }
                col = col.saturating_add(width);
            }
            lines.push(line.trim_end_matches(' ').to_owned());
        }
        let text = lines.join("\n");
        if !text.is_empty() {
            outcome
                .actions
                .push(ClientShellAction::ClipboardWrite(text.into_bytes()));
            outcome.repaint |= self.show_copy_feedback(std::time::Instant::now());
        }
        true
    }

    pub(super) fn handle_popup_selection_mouse(
        &mut self,
        hit: &PaneHit,
        mouse: MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        let selecting = self
            .selection
            .as_ref()
            .is_some_and(|s| s.pane_id == hit.pane_id && s.is_in_progress());
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) if selecting => {
                if let Some(selection) = self.selection.as_mut() {
                    selection.drag(mouse.column, mouse.row, hit.inner_rect, None);
                }
                outcome.repaint |= self.request_selection_drag_repaint(std::time::Instant::now());
                true
            }
            MouseEventKind::Up(MouseButton::Left) if selecting => {
                let copied = self
                    .selection
                    .as_mut()
                    .is_some_and(crate::selection::Selection::finish);
                if copied && self.config.copy_on_select {
                    self.request_selection_copy(outcome, true);
                    self.clear_popup_selection();
                } else if !copied {
                    self.clear_popup_selection();
                }
                outcome.repaint = true;
                true
            }
            MouseEventKind::Down(_)
            | MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => {
                outcome.repaint |= self.clear_popup_selection();
                if mouse.kind != MouseEventKind::Down(MouseButton::Left)
                    || hit.mouse_reporting
                    || !super::contains(hit.inner_rect, (mouse.column, mouse.row))
                {
                    return false;
                }
                let Some(popup) = self
                    .pane_surface
                    .as_ref()
                    .and_then(|s| s.popup.as_deref())
                    .filter(|p| p.terminal_id == hit.pane_id)
                else {
                    return true;
                };
                // Capture once per gesture, not per render. Output keeps running, but the
                // selected viewport stays stable until copied, typed into, or scrolled.
                self.popup_selection_frame = Some(PopupSelectionFrame {
                    terminal_id: hit.pane_id.clone(),
                    frame: popup.frame.clone(),
                    width: hit.inner_rect.width.min(popup.frame.width),
                    height: hit.inner_rect.height.min(popup.frame.height),
                });
                self.selection = Some(crate::selection::Selection::anchor(
                    hit.pane_id.clone(),
                    mouse.row.saturating_sub(hit.inner_rect.y),
                    mouse.column.saturating_sub(hit.inner_rect.x),
                    None,
                ));
                outcome.repaint = true;
                true
            }
            _ => false,
        }
    }
}
