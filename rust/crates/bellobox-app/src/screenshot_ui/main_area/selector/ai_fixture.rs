//! Generated pixels enter the actual coordinator and inline editor. Area uses
//! the ordinary frozen-selection model. Window begins with an already-generated
//! fixed frame: it does not claim to exercise the physical Window crop, native
//! catalog, refresh or capture. No caller can supply an image or destination.
use super::*;
use crate::transport::image_ocr::fixture::{self, FixturePermit, ResponseMode};
use bellobox_core::screenshot::ai_ocr::ProviderAuthority;

pub(super) struct Authority {
    pub authority: ProviderAuthority,
    pub permit: FixturePermit,
}
enum Policy {
    Area(FrozenAreaSession),
    FixedWindow(Option<ScreenshotDocument>),
}
pub(in super::super) struct Selection {
    policy: Policy,
    display: AreaDisplayGeometry,
    authority: Option<Authority>,
    binding: FixturePermit,
}
pub(super) struct Commit {
    session: ScreenshotEditSession,
    rect: Rect,
    fixed: bool,
    authority: Authority,
}
impl Commit {
    pub(super) fn rect(&self) -> Rect {
        self.rect
    }
    pub(super) fn prepare(self) -> Result<PreparedEditor, String> {
        if !self
            .authority
            .permit
            .check_document(self.session.document())
        {
            return Err("Generated image ownership changed before inline preparation.".into());
        }
        Ok(PreparedEditor {
            session: super::super::super::prepare_session(self.session),
            fixed_frame: self.fixed.then_some(self.rect),
            fixture: Some(self.authority),
            token: None,
            refresh: None,
        })
    }
}
impl Selection {
    pub(super) fn preview_rect(&self) -> Option<Rect> {
        match &self.policy {
            Policy::Area(area) => area.preview_rect(),
            Policy::FixedWindow(_) => Some(self.display.local_bounds()),
        }
    }
    pub(super) fn begin(
        &mut self,
        point: Point,
        display: AreaDisplayGeometry,
    ) -> Result<(), String> {
        match &mut self.policy {
            Policy::Area(area) => area.begin_drag(point, display).map_err(|e| e.to_string()),
            Policy::FixedWindow(_) => Err("Generated Window frame is already fixed.".into()),
        }
    }
    pub(super) fn movement(
        &mut self,
        point: Point,
        display: AreaDisplayGeometry,
    ) -> Result<(), String> {
        match &mut self.policy {
            Policy::Area(area) if area.phase() == AreaPhase::Dragging => {
                area.update_drag(point, display).map_err(|e| e.to_string())
            }
            _ => Ok(()),
        }
    }
    pub(super) fn end(
        &mut self,
        point: Point,
        display: AreaDisplayGeometry,
    ) -> Result<Option<super::Commit>, String> {
        let Policy::Area(area) = &mut self.policy else {
            return Ok(None);
        };
        if area.phase() != AreaPhase::Dragging {
            return Ok(None);
        }
        let Some(selected) = area.end_drag(point, display).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        Ok(Some(super::Commit::AiFixture(Commit {
            session: selected.editor,
            rect: selected.selection_local_points,
            fixed: false,
            authority: self
                .authority
                .take()
                .ok_or("Generated authority was already consumed.")?,
        })))
    }
    pub(super) fn fixed_window_commit(&mut self) -> Option<super::Commit> {
        let Policy::FixedWindow(document) = &mut self.policy else {
            return None;
        };
        Some(super::Commit::AiFixture(Commit {
            session: ScreenshotEditSession::new(document.take()?),
            rect: self.display.local_bounds(),
            fixed: true,
            authority: self.authority.take()?,
        }))
    }
    pub(super) fn accepts(
        &mut self,
        editor: &PreparedEditor,
        display: AreaDisplayGeometry,
    ) -> bool {
        let policy = match &mut self.policy {
            Policy::Area(area) => {
                area.phase() == AreaPhase::Committed && area.validate_topology(display).is_ok()
            }
            Policy::FixedWindow(document) => document.is_none() && display == self.display,
        };
        policy && self.authority.is_none() && self.binding.check_document(editor.session.document())
    }
}

pub(in super::super) fn begin(
    requester: AnyWindowHandle,
    cx: &mut App,
    fixed_window: bool,
) -> Result<(), String> {
    // Busy is checked before even binding the private fixture's numeric listener.
    if cx.default_global::<NativeCaptureVisibility>().busy {
        return Err("Another capture is still finishing.".into());
    }
    let layout = host::MainDisplayOverlayLayout {
        display: capture::CaptureDisplay {
            id: 1,
            bounds: capture::CaptureRect::new(0., 0., 960., 540.),
            pixels: capture::CapturePixelSize {
                width: 960,
                height: 540,
            },
        },
        cocoa_frame: capture::CaptureRect::new(0., 0., 960., 540.),
        rotation_degrees: 0,
        backing_scale: 1.,
    };
    begin_supplied_prepared(requester, layout, cx, move |cancellation| {
        if cancellation.load(Ordering::Acquire) {
            return Err("Generated review was cancelled.".into());
        }
        let fixture = fixture::start(ResponseMode::Success)?;
        let display = geometry(layout);
        let tiles = fixture.document.render_preview_tiles()?;
        let policy = if fixed_window {
            Policy::FixedWindow(Some(fixture.document))
        } else {
            let mut area = FrozenAreaSession::new(display).map_err(|e| e.to_string())?;
            let token = area
                .freeze_token()
                .ok_or("Generated freeze was consumed.")?;
            if !area
                .accept_frozen(token, fixture.document, display)
                .map_err(|e| e.to_string())?
            {
                return Err("Generated Area freeze was superseded.".into());
            }
            Policy::Area(area)
        };
        Ok(PreparedArea {
            selection: super::Selection::AiFixture(Selection {
                policy,
                display,
                binding: fixture.permit.clone(),
                authority: Some(Authority {
                    authority: fixture.authority,
                    permit: fixture.permit,
                }),
            }),
            tiles,
        })
    })
}

#[cfg(test)]
mod tests;
