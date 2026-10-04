//! Pure capture-overlay and text-label geometry ported from the Swift editor.
//!
//! Selection adjustments are opt-in for a display-backed capture overlay. Popup
//! editors do not enable them. Geometry uses top-left coordinates except where
//! a conversion explicitly names Cocoa's bottom-left display coordinates.

use super::{Point, Rect, ScreenshotEditSession, valid_coordinate};

/// The eight grab points, clockwise from the top-left of a capture selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl SelectionHandle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    pub fn moves_left_edge(self) -> bool {
        matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
    }

    pub fn moves_right_edge(self) -> bool {
        matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
    }

    pub fn moves_top_edge(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }

    pub fn moves_bottom_edge(self) -> bool {
        matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
    }

    pub fn is_corner(self) -> bool {
        (self.moves_left_edge() != self.moves_right_edge())
            && (self.moves_top_edge() != self.moves_bottom_edge())
    }

    pub fn position(self, rect: Rect) -> Result<Point, String> {
        let rect = checked_rect(rect)?;
        let middle_x = rect.x + rect.width / 2.;
        let middle_y = rect.y + rect.height / 2.;
        Ok(match self {
            Self::TopLeft => Point::new(rect.x, rect.y),
            Self::Top => Point::new(middle_x, rect.y),
            Self::TopRight => Point::new(rect.right(), rect.y),
            Self::Right => Point::new(rect.right(), middle_y),
            Self::BottomRight => Point::new(rect.right(), rect.bottom()),
            Self::Bottom => Point::new(middle_x, rect.bottom()),
            Self::BottomLeft => Point::new(rect.x, rect.bottom()),
            Self::Left => Point::new(rect.x, middle_y),
        })
    }
}

/// Resize from the original pointer-down rectangle, never from the last preview.
/// Edges cannot cross their opposite edge. An invalid initial selection is first
/// fitted to the bounds, and minimum dimensions cannot exceed those bounds.
pub fn resized_rect(
    start: Rect,
    handle: SelectionHandle,
    translation: Point,
    bounds: Rect,
    minimum_size: f32,
) -> Result<Rect, String> {
    checked_point(translation)?;
    checked_value(minimum_size)?;
    let bounds = checked_rect(bounds)?;
    let start = clamped_rect(start, bounds, (minimum_size, minimum_size))?;
    let minimum_width = minimum_size.max(1.).min(bounds.width);
    let minimum_height = minimum_size.max(1.).min(bounds.height);
    let (mut x, mut right, mut y, mut bottom) = (start.x, start.right(), start.y, start.bottom());
    if handle.moves_left_edge() {
        x = clamp(
            start.x + translation.x,
            bounds.x,
            start.right() - minimum_width,
        );
    }
    if handle.moves_right_edge() {
        right = clamp(
            start.right() + translation.x,
            start.x + minimum_width,
            bounds.right(),
        );
    }
    if handle.moves_top_edge() {
        y = clamp(
            start.y + translation.y,
            bounds.y,
            start.bottom() - minimum_height,
        );
    }
    if handle.moves_bottom_edge() {
        bottom = clamp(
            start.bottom() + translation.y,
            start.y + minimum_height,
            bounds.bottom(),
        );
    }
    checked_rect(Rect::new(x, y, right - x, bottom - y))
}

/// Move a selection without changing its size, except to fit a smaller display.
pub fn moved_rect(start: Rect, translation: Point, bounds: Rect) -> Result<Rect, String> {
    let start = checked_rect(start)?;
    let bounds = checked_rect(bounds)?;
    checked_point(translation)?;
    let width = start.width.min(bounds.width);
    let height = start.height.min(bounds.height);
    checked_rect(Rect::new(
        clamp(start.x + translation.x, bounds.x, bounds.right() - width),
        clamp(start.y + translation.y, bounds.y, bounds.bottom() - height),
        width,
        height,
    ))
}

/// Intersect with the bounds and grow a degenerate selection in place. As in
/// Swift, an empty intersection resets both dimensions before minimum sizing.
pub fn clamped_rect(rect: Rect, bounds: Rect, minimum_size: (f32, f32)) -> Result<Rect, String> {
    let rect = checked_rect(rect)?;
    let bounds = checked_rect(bounds)?;
    checked_value(minimum_size.0)?;
    checked_value(minimum_size.1)?;
    if bounds.width == 0. || bounds.height == 0. {
        return Ok(bounds);
    }
    let mut result = rect.intersection(bounds).unwrap_or_else(|| {
        Rect::new(
            clamp(rect.x, bounds.x, bounds.right()),
            clamp(rect.y, bounds.y, bounds.bottom()),
            0.,
            0.,
        )
    });
    let minimum_width = minimum_size.0.max(1.).min(bounds.width);
    let minimum_height = minimum_size.1.max(1.).min(bounds.height);
    if result.width < minimum_width {
        result.x = clamp(result.x, bounds.x, bounds.right() - minimum_width);
        result.width = minimum_width;
    }
    if result.height < minimum_height {
        result.y = clamp(result.y, bounds.y, bounds.bottom() - minimum_height);
        result.height = minimum_height;
    }
    checked_rect(result)
}

/// Four nonoverlapping dim bands in top, bottom, left, right order. With no
/// intersecting selection, the first band covers everything and the rest are empty.
pub fn dim_bands(bounds: Rect, selection: Option<Rect>) -> Result<[Rect; 4], String> {
    let bounds = checked_rect(bounds)?;
    let selection = selection.map(checked_rect).transpose()?;
    let Some(selection) = selection.and_then(|selection| selection.intersection(bounds)) else {
        return Ok([bounds, Rect::default(), Rect::default(), Rect::default()]);
    };
    Ok([
        Rect::new(bounds.x, bounds.y, bounds.width, selection.y - bounds.y),
        Rect::new(
            bounds.x,
            selection.bottom(),
            bounds.width,
            bounds.bottom() - selection.bottom(),
        ),
        Rect::new(
            bounds.x,
            selection.y,
            selection.x - bounds.x,
            selection.height,
        ),
        Rect::new(
            selection.right(),
            selection.y,
            bounds.right() - selection.right(),
            selection.height,
        ),
    ])
}

/// Convert Cocoa display coordinates to top-left image pixels. Independent axis
/// scales use the actual captured image dimensions; edges round outwards.
pub fn cocoa_rect_to_image_pixel_rect(
    rect: Rect,
    screen_frame: Rect,
    image_size: (f32, f32),
) -> Result<Rect, String> {
    let rect = checked_rect(rect)?;
    let screen = checked_rect(screen_frame)?;
    checked_size(image_size)?;
    let x_scale = if image_size.0 > 0. && screen.width > 0. {
        image_size.0 / screen.width
    } else {
        1.
    };
    let y_scale = if image_size.1 > 0. && screen.height > 0. {
        image_size.1 / screen.height
    } else {
        x_scale
    };
    let x = ((rect.x - screen.x) * x_scale).floor();
    let right = ((rect.right() - screen.x) * x_scale).ceil();
    let y = ((screen.bottom() - rect.bottom()) * y_scale).floor();
    let bottom = ((screen.bottom() - rect.y) * y_scale).ceil();
    // Check intermediate edges before max(1, ..) can hide a nonfinite result.
    for value in [x, y, right, bottom] {
        checked_value(value)?;
    }
    checked_rect(Rect::new(x, y, (right - x).max(1.), (bottom - y).max(1.)))
}

/// Convert top-left display pixels back to Cocoa display coordinates. Like the
/// Swift helper, finite nonpositive scales fall back to one.
pub fn display_pixel_rect_to_cocoa_rect(
    rect: Rect,
    screen_frame: Rect,
    scale: f32,
) -> Result<Rect, String> {
    let rect = checked_rect(rect)?;
    let screen = checked_rect(screen_frame)?;
    checked_value(scale)?;
    let scale = if scale > 0. { scale } else { 1. };
    checked_rect(Rect::new(
        screen.x + rect.x / scale,
        screen.bottom() - rect.bottom() / scale,
        rect.width / scale,
        rect.height / scale,
    ))
}

/// Crop for a whole-display document, preserving the original display pixels.
/// A full-display selection returns `None`, matching Swift's unset crop.
pub fn display_selection_crop(
    selection_cocoa_rect: Rect,
    screen_frame: Rect,
    image_size: (u32, u32),
) -> Result<Option<Rect>, String> {
    super::validate_image_size(image_size.0, image_size.1)?;
    let selection = checked_rect(selection_cocoa_rect)?;
    let screen = checked_rect(screen_frame)?;
    let bounded = selection
        .intersection(screen)
        .ok_or("Selection is outside the display.")?;
    if bounded.width < 1. || bounded.height < 1. {
        return Err("Selection must cover at least one display point on each axis.".into());
    }
    let full = Rect::new(0., 0., image_size.0 as f32, image_size.1 as f32);
    let pixels = cocoa_rect_to_image_pixel_rect(bounded, screen, (full.width, full.height))?;
    let crop = pixels
        .intersection(full)
        .ok_or("Selection is outside the captured image.")?
        .integral();
    Ok((crop != full).then_some(crop))
}

/// Overlay minimum: eight display points scaled to pixels, with a four-pixel floor.
pub fn minimum_selection_pixel_size(scale: f32) -> Result<(f32, f32), String> {
    checked_value(scale)?;
    let edge = (8. * scale.max(1.)).round().max(4.);
    checked_value(edge)?;
    Ok((edge, edge))
}

/// One opt-in capture-overlay drag. Updates only change the draft; committing
/// publishes one crop edit. Dropping the draft cancels without touching history.
/// Callers can render `preview_rect` without changing annotation coordinates.
pub struct SelectionAdjustmentDraft {
    image: std::sync::Arc<image::RgbaImage>,
    revision: u64,
    original_crop: Option<Rect>,
    bounds: Rect,
    minimum_size: (f32, f32),
    preview: Rect,
}

impl SelectionAdjustmentDraft {
    /// `allows_selection_adjustment` must come from the overlay host, never from
    /// the image format or a popup editor's currently selected tool.
    pub fn begin(
        session: &ScreenshotEditSession,
        allows_selection_adjustment: bool,
        scale: f32,
    ) -> Result<Option<Self>, String> {
        if !allows_selection_adjustment {
            return Ok(None);
        }
        let document = session.document();
        Ok(Some(Self {
            image: document.base_image.clone(),
            revision: session.revision(),
            original_crop: document.crop_rect(),
            bounds: Rect::new(0., 0., document.width() as f32, document.height() as f32),
            minimum_size: minimum_selection_pixel_size(scale)?,
            preview: document.visible_rect(),
        }))
    }

    pub fn preview_rect(&self) -> Rect {
        self.preview
    }

    pub fn minimum_size(&self) -> (f32, f32) {
        self.minimum_size
    }

    pub fn update(&mut self, rect: Rect) -> Result<bool, String> {
        let preview = clamped_rect(rect, self.bounds, self.minimum_size)?
            .integral()
            .intersection(self.bounds)
            .ok_or("Selection does not intersect the screenshot.")?;
        let changed = self.preview != preview;
        self.preview = preview;
        Ok(changed)
    }

    pub fn commit(self, session: &mut ScreenshotEditSession) -> Result<bool, String> {
        if !std::sync::Arc::ptr_eq(&self.image, &session.document.base_image)
            || session.revision() != self.revision
            || session.document().crop_rect() != self.original_crop
        {
            return Err(
                "The screenshot changed during selection adjustment; start the drag again.".into(),
            );
        }
        session.set_crop((self.preview != self.bounds).then_some(self.preview))
    }
}

/// The source editor's interaction frame, independent of shaped text ink bounds.
pub fn text_label_frame(origin: Point, max_width: f32, font_size: f32) -> Result<Rect, String> {
    checked_point(origin)?;
    checked_value(max_width)?;
    checked_value(font_size)?;
    if max_width <= 0. || font_size < 0. {
        return Err("Text label width must be positive and font size nonnegative.".into());
    }
    checked_rect(Rect::new(
        origin.x,
        origin.y,
        max_width,
        (font_size + 16.).max(34.),
    ))
}

pub fn visible_text_label_frame(
    document_origin: Point,
    max_width: f32,
    font_size: f32,
    crop_origin: Point,
) -> Result<Rect, String> {
    checked_point(document_origin)?;
    checked_point(crop_origin)?;
    text_label_frame(
        document_origin.offset(-crop_origin.x, -crop_origin.y),
        max_width,
        font_size,
    )
}

/// Clamp a label's origin to the visible image; oversized labels sit at zero.
pub fn clamped_visible_text_origin(
    origin: Point,
    max_width: f32,
    font_size: f32,
    visible_size: (f32, f32),
) -> Result<Point, String> {
    checked_point(origin)?;
    checked_size(visible_size)?;
    let frame = text_label_frame(Point::default(), max_width, font_size)?;
    Ok(Point::new(
        origin.x.clamp(0., (visible_size.0 - frame.width).max(0.)),
        origin.y.clamp(0., (visible_size.1 - frame.height).max(0.)),
    ))
}

/// Clamp in visible coordinates, then restore the crop's document-pixel offset.
pub fn clamped_document_text_origin(
    origin: Point,
    max_width: f32,
    font_size: f32,
    visible_rect: Rect,
) -> Result<Point, String> {
    checked_point(origin)?;
    let visible = checked_rect(visible_rect)?;
    let clamped = clamped_visible_text_origin(
        origin.offset(-visible.x, -visible.y),
        max_width,
        font_size,
        (visible.width, visible.height),
    )?;
    checked_point(clamped.offset(visible.x, visible.y))
}

fn checked_value(value: f32) -> Result<(), String> {
    if valid_coordinate(value) {
        Ok(())
    } else {
        Err("Selection geometry must be finite and within supported coordinate bounds.".into())
    }
}

fn checked_point(point: Point) -> Result<Point, String> {
    checked_value(point.x)?;
    checked_value(point.y)?;
    Ok(point)
}

fn checked_size(size: (f32, f32)) -> Result<(), String> {
    checked_value(size.0)?;
    checked_value(size.1)?;
    if size.0 < 0. || size.1 < 0. {
        Err("Geometry dimensions must be nonnegative.".into())
    } else {
        Ok(())
    }
}

fn checked_rect(rect: Rect) -> Result<Rect, String> {
    if !rect.valid() || !rect.standardized().valid() {
        return Err(
            "Selection rectangles must be finite and within supported coordinate bounds.".into(),
        );
    }
    Ok(rect.standardized())
}

fn clamp(value: f32, lower: f32, upper: f32) -> f32 {
    if lower > upper {
        lower
    } else {
        value.clamp(lower, upper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screenshot::{AnnotationKind, AnnotationStyle, ScreenshotDocument};
    use image::{Rgba, RgbaImage};

    const BOUNDS: Rect = Rect::new(0., 0., 1_000., 600.);
    const START: Rect = Rect::new(200., 150., 300., 200.);

    fn session() -> ScreenshotEditSession {
        let mut document = ScreenshotDocument::from_rgba(RgbaImage::from_pixel(
            400,
            200,
            Rgba([255, 255, 255, 255]),
        ))
        .unwrap();
        document.crop_rect = Some(Rect::new(40., 60., 100., 80.));
        ScreenshotEditSession::new(document)
    }

    fn draft(session: &ScreenshotEditSession) -> SelectionAdjustmentDraft {
        SelectionAdjustmentDraft::begin(session, true, 2.)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn all_handle_positions_and_corner_flags_match_the_source() {
        let positions = [
            (200., 150.),
            (350., 150.),
            (500., 150.),
            (500., 250.),
            (500., 350.),
            (350., 350.),
            (200., 350.),
            (200., 250.),
        ];
        for (index, (handle, (x, y))) in SelectionHandle::ALL.into_iter().zip(positions).enumerate()
        {
            assert_eq!(handle.position(START).unwrap(), Point::new(x, y));
            assert_eq!(handle.is_corner(), index % 2 == 0);
        }
    }

    #[test]
    fn each_handle_moves_only_its_selected_edges() {
        let expected = [
            Rect::new(240., 170., 260., 180.),
            Rect::new(200., 170., 300., 180.),
            Rect::new(200., 170., 340., 180.),
            Rect::new(200., 150., 340., 200.),
            Rect::new(200., 150., 340., 220.),
            Rect::new(200., 150., 300., 220.),
            Rect::new(240., 150., 260., 220.),
            Rect::new(240., 150., 260., 200.),
        ];
        for (handle, expected) in SelectionHandle::ALL.into_iter().zip(expected) {
            assert_eq!(
                resized_rect(START, handle, Point::new(40., 20.), BOUNDS, 8.).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn outward_top_left_drag_and_reverse_rectangle_are_standardized() {
        let reverse = Rect::new(500., 350., -300., -200.);
        assert_eq!(
            resized_rect(
                reverse,
                SelectionHandle::TopLeft,
                Point::new(-50., -30.),
                BOUNDS,
                8.
            )
            .unwrap(),
            Rect::new(150., 120., 350., 230.),
        );
        assert_eq!(
            SelectionHandle::BottomRight.position(reverse).unwrap(),
            Point::new(500., 350.)
        );
    }

    #[test]
    fn resize_clamps_to_display_and_does_not_cross_opposite_edges() {
        assert_eq!(
            resized_rect(
                START,
                SelectionHandle::BottomRight,
                Point::new(5_000., 5_000.),
                BOUNDS,
                8.
            )
            .unwrap(),
            Rect::new(200., 150., 800., 450.),
        );
        assert_eq!(
            resized_rect(
                START,
                SelectionHandle::Left,
                Point::new(900., 0.),
                BOUNDS,
                8.
            )
            .unwrap(),
            Rect::new(492., 150., 8., 200.),
        );
        assert_eq!(
            resized_rect(
                START,
                SelectionHandle::Bottom,
                Point::new(0., -900.),
                BOUNDS,
                8.
            )
            .unwrap(),
            Rect::new(200., 150., 300., 8.),
        );
    }

    #[test]
    fn every_handle_stays_inside_offset_bounds_for_extreme_drags_and_minima() {
        let bounds = Rect::new(-400., 50., 300., 200.);
        for start in [
            Rect::new(-350., 90., 100., 60.),
            Rect::new(-450., -50., 500., 400.),
        ] {
            for handle in SelectionHandle::ALL {
                for delta in [Point::new(-5_000., 5_000.), Point::new(5_000., -5_000.)] {
                    for minimum in [-10., 8., 600.] {
                        let result = resized_rect(start, handle, delta, bounds, minimum).unwrap();
                        assert!(result.x >= bounds.x && result.y >= bounds.y);
                        assert!(
                            result.right() <= bounds.right() && result.bottom() <= bounds.bottom()
                        );
                        assert!(result.width >= minimum.max(1.).min(bounds.width));
                        assert!(result.height >= minimum.max(1.).min(bounds.height));
                    }
                }
            }
        }
    }

    #[test]
    fn move_preserves_dimensions_and_clamps_to_bounds() {
        assert_eq!(
            moved_rect(START, Point::new(30., -20.), BOUNDS).unwrap(),
            Rect::new(230., 130., 300., 200.)
        );
        assert_eq!(
            moved_rect(START, Point::new(5_000., -5_000.), BOUNDS).unwrap(),
            Rect::new(700., 0., 300., 200.)
        );
        assert_eq!(
            moved_rect(BOUNDS, Point::new(5., 5.), Rect::new(20., 30., 80., 40.)).unwrap(),
            Rect::new(20., 30., 80., 40.)
        );
    }

    #[test]
    fn clamp_grows_degenerate_rectangles_and_intersects_partial_selections() {
        let bounds = Rect::new(0., 0., 400., 300.);
        assert_eq!(
            clamped_rect(Rect::new(395., 295., 1., 1.), bounds, (16., 16.)).unwrap(),
            Rect::new(384., 284., 16., 16.)
        );
        assert_eq!(
            clamped_rect(Rect::new(-50., -50., 100., 100.), bounds, (16., 16.)).unwrap(),
            Rect::new(0., 0., 50., 50.)
        );
        assert_eq!(
            clamped_rect(Rect::new(500., 250., 20., 20.), bounds, (16., 16.)).unwrap(),
            Rect::new(384., 250., 16., 16.)
        );
        assert_eq!(
            clamped_rect(Rect::new(50., 60., 0., 40.), bounds, (16., 16.)).unwrap(),
            Rect::new(50., 60., 16., 16.)
        );
    }

    #[test]
    fn reverse_drag_clamp_and_empty_bounds_are_safe() {
        assert_eq!(
            clamped_rect(Rect::new(80., 90., -60., -50.), BOUNDS, (8., 8.)).unwrap(),
            Rect::new(20., 40., 60., 50.)
        );
        let empty = Rect::new(25., 30., 0., 0.);
        assert_eq!(clamped_rect(START, empty, (8., 8.)).unwrap(), empty);
        assert_eq!(
            resized_rect(
                START,
                SelectionHandle::TopLeft,
                Point::new(-900., 900.),
                empty,
                8.
            )
            .unwrap(),
            empty
        );
    }

    #[test]
    fn dim_bands_tile_everything_outside_selection_without_overlap() {
        let bands = dim_bands(BOUNDS, Some(START)).unwrap();
        let area: f32 = bands.iter().map(|rect| rect.width * rect.height).sum();
        assert_eq!(
            area,
            BOUNDS.width * BOUNDS.height - START.width * START.height
        );
        for (index, band) in bands.iter().enumerate() {
            assert_eq!(band.intersection(BOUNDS), Some(*band));
            assert!(band.intersection(START).is_none());
            for other in &bands[index + 1..] {
                assert!(band.intersection(*other).is_none());
            }
        }
    }

    #[test]
    fn dim_bands_handle_absent_outside_partial_and_full_selections() {
        for selection in [
            None,
            Some(Rect::new(2_000., 0., 10., 10.)),
            Some(Rect::new(20., 30., 0., 40.)),
        ] {
            assert_eq!(
                dim_bands(BOUNDS, selection).unwrap(),
                [BOUNDS, Rect::default(), Rect::default(), Rect::default()]
            );
        }
        let full = dim_bands(BOUNDS, Some(BOUNDS)).unwrap();
        assert!(full.iter().all(|r| r.width == 0. || r.height == 0.));
        let partial = dim_bands(BOUNDS, Some(Rect::new(-50., -60., 100., 100.))).unwrap();
        assert_eq!(
            partial.iter().map(|r| r.width * r.height).sum::<f32>(),
            600_000. - 2_000.
        );
    }

    #[test]
    fn capture_conversion_matches_retina_selection_crop() {
        assert_eq!(
            display_selection_crop(
                Rect::new(20., 30., 50., 40.),
                Rect::new(0., 0., 200., 100.),
                (400, 200)
            )
            .unwrap(),
            Some(Rect::new(40., 60., 100., 80.))
        );
        assert_eq!(
            display_selection_crop(
                Rect::new(0., 0., 200., 100.),
                Rect::new(0., 0., 200., 100.),
                (400, 200)
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn capture_conversion_uses_actual_pixels_and_outward_fractional_edges() {
        let screen = Rect::new(0., 0., 100., 80.);
        assert_eq!(
            cocoa_rect_to_image_pixel_rect(Rect::new(10., 20., 30., 15.), screen, (250., 200.))
                .unwrap(),
            Rect::new(25., 112., 75., 38.)
        );
        assert_eq!(
            cocoa_rect_to_image_pixel_rect(Rect::new(10.2, 20.1, 30.2, 15.2), screen, (250., 200.))
                .unwrap(),
            Rect::new(25., 111., 76., 39.)
        );
        assert_eq!(
            cocoa_rect_to_image_pixel_rect(Rect::new(10., 20., 30., 15.), screen, (200., 240.))
                .unwrap(),
            Rect::new(20., 135., 60., 45.)
        );
    }

    #[test]
    fn capture_conversion_round_trips_a_negative_origin_secondary_display() {
        let screen = Rect::new(-1_200., 80., 1_200., 700.);
        let rect = Rect::new(-1_000., 200., 400., 180.);
        let pixels = cocoa_rect_to_image_pixel_rect(rect, screen, (2_400., 1_400.)).unwrap();
        assert_eq!(pixels, Rect::new(400., 800., 800., 360.));
        assert_eq!(
            display_pixel_rect_to_cocoa_rect(pixels, screen, 2.).unwrap(),
            rect
        );
    }

    #[test]
    fn conversion_fallbacks_are_finite_and_capture_selection_clips_to_the_screen() {
        let screen = Rect::new(0., 0., 100., 80.);
        let rect = Rect::new(10., 20., 30., 15.);
        assert_eq!(
            cocoa_rect_to_image_pixel_rect(rect, screen, (0., 0.)).unwrap(),
            Rect::new(10., 45., 30., 15.)
        );
        assert_eq!(
            display_pixel_rect_to_cocoa_rect(Rect::new(10., 45., 30., 15.), screen, 0.).unwrap(),
            rect
        );
        assert_eq!(
            display_selection_crop(Rect::new(-20., -10., 50., 30.), screen, (200, 160)).unwrap(),
            Some(Rect::new(0., 120., 60., 40.))
        );
        assert!(display_selection_crop(Rect::new(150., 10., 5., 5.), screen, (200, 160)).is_err());
        assert!(display_selection_crop(Rect::new(10., 10., 0.5, 10.), screen, (200, 160)).is_err());
    }

    #[test]
    fn minimum_selection_matches_swift_scale_rounding() {
        for (scale, edge) in [
            (-1., 8.),
            (0., 8.),
            (0.5, 8.),
            (1., 8.),
            (1.2, 10.),
            (2., 16.),
        ] {
            assert_eq!(minimum_selection_pixel_size(scale).unwrap(), (edge, edge));
        }
    }

    #[test]
    fn many_draft_updates_commit_as_one_undo_step() {
        let mut session = session();
        let original = session.document().crop_rect();
        let mut draft = draft(&session);
        assert_eq!(draft.minimum_size(), (16., 16.));
        draft.update(Rect::new(40., 60., 120., 80.)).unwrap();
        draft.update(Rect::new(40., 60., 160., 90.)).unwrap();
        assert_eq!(session.document().crop_rect(), original);
        assert!(!session.can_undo());
        assert!(draft.commit(&mut session).unwrap());
        assert_eq!(
            session.document().crop_rect(),
            Some(Rect::new(40., 60., 160., 90.))
        );
        assert_eq!(session.document().visible_dimensions(), (160, 90));
        assert!(session.undo());
        assert_eq!(session.document().crop_rect(), original);
        assert!(!session.can_undo());
        assert!(session.redo());
        assert_eq!(session.document().visible_dimensions(), (160, 90));
    }

    #[test]
    fn unchanged_and_cancelled_drafts_leave_no_history() {
        let mut session = session();
        let mut unchanged = draft(&session);
        assert!(!unchanged.update(unchanged.preview_rect()).unwrap());
        assert!(!unchanged.commit(&mut session).unwrap());
        let mut cancelled = draft(&session);
        cancelled.update(Rect::new(5., 5., 20., 20.)).unwrap();
        drop(cancelled);
        assert!(!session.can_undo());
        assert!(!session.has_edits());
    }

    #[test]
    fn adjustment_clamps_to_image_and_minimum_then_clears_full_crop() {
        let mut session = session();
        let mut draft = draft(&session);
        draft.update(Rect::new(300., 150., 500., 500.)).unwrap();
        assert_eq!(draft.preview_rect(), Rect::new(300., 150., 100., 50.));
        draft.update(Rect::new(10., 10., 1., 1.)).unwrap();
        assert_eq!(draft.preview_rect(), Rect::new(10., 10., 16., 16.));
        draft.update(Rect::new(0., 0., 400., 200.)).unwrap();
        assert!(draft.commit(&mut session).unwrap());
        assert_eq!(session.document().crop_rect(), None);
        assert_eq!(session.document().visible_dimensions(), (400, 200));
    }

    #[test]
    fn adjustment_preserves_annotations_in_document_pixels() {
        let mut session = session();
        session
            .add_visible_annotation(
                AnnotationKind::Rectangle(Rect::new(10., 10., 20., 20.)),
                AnnotationStyle::default(),
            )
            .unwrap();
        let annotation = session.document().annotations()[0].clone();
        assert_eq!(
            annotation.kind,
            AnnotationKind::Rectangle(Rect::new(50., 70., 20., 20.))
        );
        let mut draft = draft(&session);
        draft.update(Rect::new(30., 50., 100., 80.)).unwrap();
        draft.commit(&mut session).unwrap();
        assert_eq!(session.document().annotations()[0], annotation);
        assert_eq!(
            session.document().visible_annotations()[0].kind,
            AnnotationKind::Rectangle(Rect::new(20., 20., 20., 20.))
        );
        assert_eq!(session.document().dimensions(), (400, 200));
    }

    #[test]
    fn popup_hosts_cannot_begin_selection_adjustment() {
        let session = session();
        assert!(
            SelectionAdjustmentDraft::begin(&session, false, 2.)
                .unwrap()
                .is_none()
        );
        assert!(!session.can_undo());
    }

    #[test]
    fn stale_and_cross_document_drafts_cannot_overwrite_newer_work() {
        let mut edit = session();
        let mut pending = draft(&edit);
        pending.update(Rect::new(10., 20., 30., 40.)).unwrap();
        edit.set_crop(None).unwrap();
        assert!(pending.commit(&mut edit).is_err());
        assert_eq!(edit.document().crop_rect(), None);
        let pending = draft(&session());
        let mut other = session();
        assert!(pending.commit(&mut other).is_err());
        assert!(!other.can_undo());
    }

    #[test]
    fn text_label_frame_uses_source_height_without_changing_max_width() {
        assert_eq!(
            text_label_frame(Point::new(10., 20.), 260., 12.).unwrap(),
            Rect::new(10., 20., 260., 34.)
        );
        assert_eq!(
            text_label_frame(Point::new(10., 20.), 260., 32.).unwrap(),
            Rect::new(10., 20., 260., 48.)
        );
        assert_eq!(
            visible_text_label_frame(Point::new(50., 70.), 260., 32., Point::new(40., 60.))
                .unwrap(),
            Rect::new(10., 10., 260., 48.)
        );
    }

    #[test]
    fn text_drag_clamps_origin_in_visible_coordinates_at_crop_offsets() {
        assert_eq!(
            clamped_visible_text_origin(Point::new(5_000., 5_000.), 260., 32., (400., 200.))
                .unwrap(),
            Point::new(140., 152.)
        );
        assert_eq!(
            clamped_visible_text_origin(Point::new(-10., -20.), 260., 32., (400., 200.)).unwrap(),
            Point::default()
        );
        let visible = Rect::new(40., 60., 400., 200.);
        assert_eq!(
            clamped_document_text_origin(Point::new(5_000., 5_000.), 260., 32., visible).unwrap(),
            Point::new(180., 212.)
        );
        assert_eq!(
            clamped_document_text_origin(Point::new(-10., -20.), 260., 32., visible).unwrap(),
            Point::new(40., 60.)
        );
    }

    #[test]
    fn labels_larger_than_the_crop_pin_to_its_origin() {
        assert_eq!(
            clamped_visible_text_origin(Point::new(90., 90.), 260., 32., (100., 30.)).unwrap(),
            Point::default()
        );
        assert_eq!(
            clamped_document_text_origin(
                Point::new(900., 900.),
                260.,
                32.,
                Rect::new(40., 60., 100., 30.)
            )
            .unwrap(),
            Point::new(40., 60.)
        );
    }

    #[test]
    fn nonfinite_and_oversized_inputs_are_rejected_without_mutation() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1_000_001.] {
            assert!(
                SelectionHandle::Top
                    .position(Rect::new(bad, 0., 1., 1.))
                    .is_err()
            );
            assert!(
                resized_rect(START, SelectionHandle::Top, Point::new(bad, 0.), BOUNDS, 8.).is_err()
            );
            assert!(moved_rect(START, Point::new(0., bad), BOUNDS).is_err());
            assert!(clamped_rect(START, BOUNDS, (bad, 8.)).is_err());
            assert!(dim_bands(BOUNDS, Some(Rect::new(0., bad, 10., 10.))).is_err());
            assert!(text_label_frame(Point::default(), 260., bad).is_err());
            assert!(clamped_visible_text_origin(Point::default(), 260., 18., (bad, 200.)).is_err());
            assert!(cocoa_rect_to_image_pixel_rect(START, BOUNDS, (bad, 200.)).is_err());
            assert!(display_pixel_rect_to_cocoa_rect(START, BOUNDS, bad).is_err());
            assert!(minimum_selection_pixel_size(bad).is_err());
            let session = session();
            let mut draft = draft(&session);
            let previous = draft.preview_rect();
            assert!(draft.update(Rect::new(bad, 0., 20., 20.)).is_err());
            assert_eq!(draft.preview_rect(), previous);
            assert!(!session.can_undo());
        }
        assert!(text_label_frame(Point::new(999_990., 0.), 260., 18.).is_err());
        assert!(clamped_visible_text_origin(Point::default(), -10., 18., (400., 200.)).is_err());
        assert!(clamped_visible_text_origin(Point::default(), 260., 18., (-10., 200.)).is_err());
        assert!(
            cocoa_rect_to_image_pixel_rect(
                Rect::new(1., 1., 2., 2.),
                Rect::new(0., 0., f32::MIN_POSITIVE, 1.),
                (1_000., 1_000.)
            )
            .is_err()
        );
    }
}
