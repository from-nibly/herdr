//! Persistent terminal presentation and scope policy (independent of PTYs).
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::popup_size::{resolve_popup_geometry, PopupResolvedGeometry, PopupSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupScope {
    Global,
    #[default]
    Workspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupStyle {
    #[default]
    Overlay,
    Drawer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupEdge {
    Top,
    #[default]
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PersistentPopupOptions {
    pub scope: PopupScope,
    pub style: PopupStyle,
    pub edge: PopupEdge,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum PopupOwner {
    Global,
    Workspace(String),
    Tab(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PersistentPopupKey {
    pub label: String,
    pub command: String,
    pub owner: PopupOwner,
}

pub(crate) const PRESENTATION_KIND: &str = "endpoint.persistent-popups.v1";

/// Optional JSON companion; deliberately not part of a frozen binary codec.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PopupProjection {
    pub boot_id: String,
    pub commands: Vec<(String, Vec<String>)>,
    pub visible: Option<(String, PersistentPopupOptions)>,
}

pub(crate) fn resolve_geometry(
    width: Option<PopupSize>,
    height: Option<PopupSize>,
    options: Option<PersistentPopupOptions>,
    area: Rect,
) -> Option<PopupResolvedGeometry> {
    let drawer = options.filter(|options| options.style == PopupStyle::Drawer);
    let mut geometry = resolve_popup_geometry(
        if drawer.is_some() {
            Some(PopupSize::Percent(100))
        } else {
            width
        },
        height,
        area,
    )?;
    if let Some(options) = drawer {
        let y = match options.edge {
            PopupEdge::Top => area.y,
            PopupEdge::Bottom => area.bottom().saturating_sub(geometry.outer.height),
        };
        geometry.outer.y = y;
        geometry.inner.y = y + 1;
    }
    Some(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_popup_drawer_anchors_to_edges_and_fills_width() {
        let area = Rect::new(13, 7, 120, 40);
        for edge in [PopupEdge::Top, PopupEdge::Bottom] {
            let options = PersistentPopupOptions {
                style: PopupStyle::Drawer,
                edge,
                ..Default::default()
            };
            let geometry = resolve_geometry(
                Some(PopupSize::Percent(20)),
                Some(PopupSize::Percent(40)),
                Some(options),
                area,
            )
            .unwrap();
            assert_eq!(geometry.outer.width, area.width);
            assert_eq!(geometry.outer.height, 16);
            assert_eq!(geometry.outer.x, area.x);
            assert_eq!(
                geometry.outer.y,
                if edge == PopupEdge::Top {
                    area.y
                } else {
                    area.bottom() - 16
                }
            );
            assert_eq!(geometry.inner.y, geometry.outer.y + 1);
        }
    }

    #[test]
    fn persistent_popup_overlay_preserves_existing_geometry() {
        let area = Rect::new(4, 9, 120, 40);
        assert_eq!(
            resolve_geometry(None, None, Some(PersistentPopupOptions::default()), area),
            resolve_popup_geometry(None, None, area)
        );
    }

    #[test]
    fn persistent_popup_drawer_clamps_small_and_large_sizes() {
        let options = Some(PersistentPopupOptions {
            style: PopupStyle::Drawer,
            ..Default::default()
        });
        assert!(resolve_geometry(None, None, options, Rect::new(0, 0, 5, 3)).is_none());
        let area = Rect::new(2, 3, 20, 8);
        let geometry = resolve_geometry(None, Some(PopupSize::Cells(999)), options, area).unwrap();
        assert_eq!(geometry.outer, area);
        assert!(area.contains((geometry.inner.x, geometry.inner.y).into()));
    }
}
