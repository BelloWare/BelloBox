use super::*;
use image::{Rgba, RgbaImage};

fn white(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
}
fn striped(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([
            (x * 17 % 256) as u8,
            (y * 23 % 256) as u8,
            ((x + y) * 11 % 256) as u8,
            255,
        ])
    })
}
fn session(image: RgbaImage) -> ScreenshotEditSession {
    ScreenshotEditSession::new(ScreenshotDocument::from_rgba(image).unwrap())
}
fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn arrow(width: f32) -> (AnnotationKind, AnnotationStyle) {
    (
        AnnotationKind::Arrow {
            start: p(10., 50.),
            end: p(190., 50.),
        },
        AnnotationStyle {
            line_width: width,
            ..AnnotationStyle::default()
        },
    )
}
fn rgba(png: &[u8]) -> RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8()
}
fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}
fn reconstruct_preview_tiles(tiles: &[PreviewTile], width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let mut next = (0, 0);
    let mut total_png_bytes = 0;
    for tile in tiles {
        assert_eq!((tile.x, tile.y), next, "Tiles must cover each pixel once");
        assert_eq!(tile.width, MAX_PREVIEW_TILE_EDGE.min(width - tile.x));
        assert_eq!(tile.height, MAX_PREVIEW_TILE_EDGE.min(height - tile.y));
        assert!((1..=1_024).contains(&tile.width));
        assert!((1..=1_024).contains(&tile.height));
        let decoded = rgba(&tile.png);
        assert_eq!(decoded.dimensions(), (tile.width, tile.height));
        image::imageops::replace(&mut image, &decoded, i64::from(tile.x), i64::from(tile.y));
        total_png_bytes += tile.png.len();
        next = if tile.x + tile.width == width {
            (0, tile.y + tile.height)
        } else {
            (tile.x + tile.width, tile.y)
        };
    }
    assert_eq!(next, (0, height), "Preview must contain the complete crop");
    assert!(total_png_bytes <= MAX_PREVIEW_PNG_BYTES);
    image
}
fn optional_test_font() -> Option<Vec<u8>> {
    // No bundled fonts or hidden fallback. Test the app's explicit-byte contract
    // only when the runner has a locally installed, licensed font available.
    [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ]
    .iter()
    .find_map(|path| std::fs::read(path).ok())
}

#[test]
fn png_round_trip_retains_top_left_coordinates_and_native_dimensions() {
    let image = striped(100, 90);
    let source = render::encode_png(&image).unwrap();
    let doc = ScreenshotDocument::from_png(&source).unwrap();
    assert_eq!(doc.dimensions(), (100, 90));
    assert_eq!(rgba(&doc.render_png().unwrap()), image);
    let mut edit = ScreenshotEditSession::new(doc);
    edit.set_crop(Some(Rect::new(10., 12., 40., 30.))).unwrap();
    let crop = rgba(&edit.render_png().unwrap());
    assert_eq!(crop.dimensions(), (40, 30));
    assert_eq!(pixel(&crop, 0, 0), pixel(&image, 10, 12));
    assert_eq!(pixel(&crop, 39, 29), pixel(&image, 49, 41));
}
#[test]
fn preview_tiles_reconstruct_export_across_masks_erasures_crop_and_tile_edges() {
    // The nonzero crop offset puts preview tile edges at document x=1037 and
    // y=1043. Both redaction patterns and antialiased eraser strokes cross them.
    for pattern in MaskPattern::ALL {
        let base = striped(1_300, 1_300);
        let mut edit = session(base.clone());
        edit.add_annotation(
            AnnotationKind::Blur(Rect::new(990.3, 990.3, 190.1, 190.1)),
            AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, pattern),
        )
        .unwrap();
        edit.add_annotation(
            AnnotationKind::Highlight(Rect::new(20., 20., 1_200., 1_200.)),
            AnnotationStyle::highlight(),
        )
        .unwrap();
        edit.add_annotation(
            AnnotationKind::Freehand {
                points: vec![p(20., 1_043.), p(1_250., 1_043.)],
            },
            AnnotationStyle {
                line_width: 14.,
                ..AnnotationStyle::default()
            },
        )
        .unwrap();
        edit.add_annotation(
            AnnotationKind::Rectangle(Rect::new(1_037., 40., 100., 1_160.)),
            AnnotationStyle::default(),
        )
        .unwrap();
        edit.erase_stroke(vec![p(1_010., 1_015.), p(1_070., 1_075.)], 10.)
            .unwrap();
        edit.set_crop(Some(Rect::new(13., 19., 1_250., 1_240.)))
            .unwrap();
        let snapshot = edit.render_snapshot();
        let expected = rgba(&snapshot.render_png().unwrap());
        let tiles = snapshot.render_preview_tiles().unwrap();
        assert_eq!(tiles.len(), 4);
        let preview = reconstruct_preview_tiles(&tiles, 1_250, 1_240);
        assert_eq!(preview, expected);
        assert_eq!(pixel(&preview, 1_024, 1_024), pixel(&base, 1_037, 1_043));
        assert_eq!(pixel(&preview, 1_100, 1_100)[3], 255);
        assert_ne!(pixel(&preview, 1_100, 1_100), pixel(&base, 1_113, 1_119));
    }
}
#[test]
fn tall_preview_uses_small_tiles_without_downsampling_export() {
    let mut edit = session(striped(2, 40_000));
    edit.add_annotation(
        AnnotationKind::Blur(Rect::new(0., 900., 2., 35_000.)),
        AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, MaskPattern::Dots),
    )
    .unwrap();
    edit.erase_stroke(vec![p(1., 1_022.), p(1., 1_026.)], 2.)
        .unwrap();
    let snapshot = edit.render_snapshot();
    let tiles = snapshot.render_preview_tiles().unwrap();
    assert_eq!(tiles.len(), 40);
    assert_eq!(tiles.last().unwrap().height, 64);
    let preview = reconstruct_preview_tiles(&tiles, 2, 40_000);
    let export = rgba(&snapshot.render_png().unwrap());
    assert_eq!(export.dimensions(), (2, 40_000));
    assert_eq!(preview, export);
}
#[test]
fn preview_tiles_preserve_transparent_pixels_and_partial_edge_tiles() {
    let base = RgbaImage::from_fn(1_025, 3, |x, y| {
        Rgba([(x % 256) as u8, (y * 23) as u8, 93, (x % 255) as u8])
    });
    let document = ScreenshotDocument::from_rgba(base.clone()).unwrap();
    let tiles = document.render_preview_tiles().unwrap();
    assert_eq!(tiles.len(), 2);
    assert_eq!((tiles[1].width, tiles[1].height), (1, 3));
    assert_eq!(reconstruct_preview_tiles(&tiles, 1_025, 3), base);
    assert_eq!(rgba(&document.render_png().unwrap()), base);
}
#[test]
fn preview_encoded_budget_is_cumulative_and_fails_without_partial_tiles() {
    let image = striped(1_025, 2);
    let tiles = render::encode_preview_tiles(&image, MAX_PREVIEW_PNG_BYTES).unwrap();
    assert_eq!(tiles.len(), 2);
    let total = tiles.iter().map(|tile| tile.png.len()).sum::<usize>();
    assert!(tiles.iter().all(|tile| tile.png.len() < total - 1));
    assert_eq!(render::encode_preview_tiles(&image, total).unwrap(), tiles);
    // Each PNG alone fits, but the combined budget must also be enforced.
    assert!(
        render::encode_preview_tiles(&image, total - 1)
            .unwrap_err()
            .contains("cumulative encoded-preview limit")
    );
    assert!(render::encode_preview_tiles(&image, 0).is_err());
    assert!(render::encode_preview_tiles(&RgbaImage::new(0, 1), MAX_PREVIEW_PNG_BYTES).is_err());
}
#[test]
fn png_input_rejects_oversized_header_before_pixel_decode() {
    let mut png = render::encode_png(&white(1, 1)).unwrap();
    png[16..20].copy_from_slice(&60_001_u32.to_be_bytes());
    let mut crc = 0xffff_ffff_u32;
    for byte in &png[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            }
        }
    }
    png[29..33].copy_from_slice(&(!crc).to_be_bytes());
    let error = ScreenshotDocument::from_png(&png).unwrap_err();
    assert!(error.contains("supported bounds"), "{error}");
    assert!(ScreenshotDocument::from_png(b"not a PNG").is_err());
}
#[test]
fn rectangle_pen_tap_and_translucent_highlight_are_real_pixels() {
    let mut edit = session(white(80, 80));
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: vec![p(15., 15.)],
        },
        AnnotationStyle::default(),
    )
    .unwrap();
    edit.add_annotation(
        AnnotationKind::Rectangle(Rect::new(30., 10., 35., 35.)),
        AnnotationStyle::default(),
    )
    .unwrap();
    edit.add_annotation(
        AnnotationKind::Highlight(Rect::new(10., 50., 40., 20.)),
        AnnotationStyle::highlight(),
    )
    .unwrap();
    let image = edit.document.render_rgba().unwrap();
    assert_ne!(pixel(&image, 15, 15), [255; 4]);
    assert_ne!(pixel(&image, 30, 20), [255; 4]);
    let highlight = pixel(&image, 20, 60);
    assert_ne!(highlight, [255; 4]);
    assert!(highlight[0] > 100 && highlight[2] > 100);
    assert_eq!(pixel(&image, 40, 25), [255; 4]);
}
#[test]
fn every_mask_pattern_is_opaque_and_independent_of_underlying_pixels() {
    for pattern in MaskPattern::ALL {
        for (_, fill) in AnnotationStyle::MASK_FILL_PRESETS {
            let mut a = session(striped(80, 80));
            let mut b = session(white(80, 80));
            let style = AnnotationStyle {
                fill_color: Some(RgbaColor {
                    alpha: 0.01,
                    ..fill
                }),
                opacity: 0.01,
                mask_pattern: pattern,
                ..AnnotationStyle::redaction()
            };
            for edit in [&mut a, &mut b] {
                edit.add_annotation(AnnotationKind::Blur(Rect::new(20., 20., 20., 30.)), style)
                    .unwrap();
            }
            let one = a.document.render_rgba().unwrap();
            let two = b.document.render_rgba().unwrap();
            for y in 20..50 {
                for x in 20..40 {
                    assert_eq!(pixel(&one, x, y), pixel(&two, x, y));
                    assert_eq!(pixel(&one, x, y)[3], 255);
                }
            }
            assert_eq!(
                pixel(&one, 40, 30),
                pixel(a.document.base_image.as_ref(), 40, 30)
            );
            let annotation = &a.document.annotations()[0];
            assert_eq!(annotation.style.opacity, 1.);
            assert_eq!(annotation.style.mask_fill().alpha, 1.);
        }
    }
}
#[test]
fn masks_cover_all_annotations_even_when_added_first() {
    for pattern in MaskPattern::ALL {
        let mut expected = session(striped(200, 100));
        expected
            .add_annotation(
                AnnotationKind::Blur(Rect::new(0., 0., 200., 100.)),
                AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, pattern),
            )
            .unwrap();
        let reference = expected.document.render_rgba().unwrap();
        for first in [true, false] {
            let mut edit = session(striped(200, 100));
            let mask = |edit: &mut ScreenshotEditSession| {
                edit.add_annotation(
                    AnnotationKind::Blur(Rect::new(0., 0., 200., 100.)),
                    AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, pattern),
                )
                .unwrap();
            };
            if first {
                mask(&mut edit)
            }
            edit.add_annotation(
                AnnotationKind::Rectangle(Rect::new(10., 30., 100., 30.)),
                AnnotationStyle::default(),
            )
            .unwrap();
            edit.add_annotation(
                AnnotationKind::Freehand {
                    points: vec![p(10., 60.), p(190., 60.)],
                },
                AnnotationStyle::default(),
            )
            .unwrap();
            edit.add_annotation(
                AnnotationKind::Highlight(Rect::new(0., 0., 200., 100.)),
                AnnotationStyle::highlight(),
            )
            .unwrap();
            if !first {
                mask(&mut edit)
            }
            assert_eq!(edit.document.render_rgba().unwrap(), reference);
        }
    }
}
#[test]
fn fractional_mask_bounds_cover_whole_protected_pixels() {
    let mut edit = session(striped(20, 20));
    edit.add_annotation(
        AnnotationKind::Blur(Rect::new(3.3, 4.3, 6.1, 5.1)),
        AnnotationStyle::mask(RgbaColor::new(0., 0., 0., 0.), MaskPattern::Solid),
    )
    .unwrap();
    let image = edit.document.render_rgba().unwrap();
    for y in 4..10 {
        for x in 3..10 {
            assert_eq!(pixel(&image, x, y), [0, 0, 0, 255]);
        }
    }
    assert_eq!(
        pixel(&image, 2, 4),
        pixel(edit.document.base_image.as_ref(), 2, 4)
    );
}
#[test]
fn mask_patterns_do_not_paint_outside_their_bounds_or_change_with_crop() {
    for pattern in MaskPattern::ALL {
        let mut edit = session(white(80, 80));
        edit.add_annotation(
            AnnotationKind::Blur(Rect::new(10., 10., 20., 40.)),
            AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, pattern),
        )
        .unwrap();
        let full = edit.document.render_rgba().unwrap();
        for y in 0..80 {
            for x in 30..80 {
                assert_eq!(pixel(&full, x, y), [255; 4]);
            }
        }
        edit.set_crop(Some(Rect::new(15., 15., 30., 30.))).unwrap();
        let crop = edit.document.render_rgba().unwrap();
        assert_eq!(
            crop,
            image::imageops::crop_imm(&full, 15, 15, 30, 30).to_image()
        );
    }
}
#[test]
fn external_and_local_ocr_exclude_decoration_and_apply_crop_and_masks() {
    let base = striped(80, 80);
    let mut edit = session(base.clone());
    edit.add_annotation(
        AnnotationKind::Rectangle(Rect::new(10., 10., 40., 40.)),
        AnnotationStyle::default(),
    )
    .unwrap();
    edit.add_annotation(
        AnnotationKind::Blur(Rect::new(20., 20., 20., 20.)),
        AnnotationStyle::mask(RgbaColor::new(0., 0., 0., 0.), MaskPattern::Solid),
    )
    .unwrap();
    edit.set_crop(Some(Rect::new(10., 10., 50., 50.))).unwrap();
    let ocr = rgba(&edit.render_for_external_ocr_png().unwrap());
    assert_eq!(ocr.dimensions(), (50, 50));
    assert_eq!(pixel(&ocr, 0, 0), pixel(&base, 10, 10));
    assert_eq!(pixel(&ocr, 15, 15), [0, 0, 0, 255]);
    assert_eq!(
        edit.render_for_external_ocr_png().unwrap(),
        edit.render_for_local_ocr_png().unwrap()
    );
    let target = rgba(
        &edit
            .document
            .render_for_external_ocr_target_png(Rect::new(0., 0., 80., 80.))
            .unwrap(),
    );
    assert_eq!(
        target, ocr,
        "An explicit OCR target must not override the active crop"
    );
    assert!(
        edit.document
            .render_for_external_ocr_target_png(Rect::new(70., 70., 10., 10.))
            .is_err()
    );
}
#[test]
fn erasing_middle_of_arrow_preserves_ends_head_and_base() {
    let base = striped(200, 100);
    let mut edit = session(base.clone());
    let (kind, style) = arrow(6.);
    edit.add_annotation(kind, style).unwrap();
    let before = edit.document.render_rgba().unwrap();
    edit.erase_stroke(vec![p(100., 30.), p(100., 50.), p(100., 70.)], 20.)
        .unwrap();
    assert_eq!(edit.document.annotations()[0].erasures.len(), 1);
    assert_eq!(edit.document.annotations()[0].erasures[0].points.len(), 3);
    let after = edit.document.render_rgba().unwrap();
    assert_eq!(pixel(&after, 100, 50), pixel(&base, 100, 50));
    assert_eq!(pixel(&after, 102, 48), pixel(&base, 102, 48));
    for x in [40, 160, 186] {
        assert_eq!(pixel(&after, x, 50), pixel(&before, x, 50));
    }
    assert_eq!(pixel(&after, 100, 10), pixel(&base, 100, 10));
}
#[test]
fn narrow_eraser_retains_thick_stroke_edges() {
    let mut edit = session(white(200, 100));
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: vec![p(20., 50.), p(180., 50.)],
        },
        AnnotationStyle {
            line_width: 14.,
            ..AnnotationStyle::default()
        },
    )
    .unwrap();
    edit.erase_stroke(vec![p(40., 50.), p(160., 50.)], 6.)
        .unwrap();
    let image = edit.document.render_rgba().unwrap();
    for x in [60, 100, 140] {
        assert_eq!(pixel(&image, x, 50), [255; 4]);
        assert_ne!(pixel(&image, x, 45), [255; 4]);
        assert_ne!(pixel(&image, x, 55), [255; 4]);
    }
    assert_eq!(edit.document.annotations().len(), 1);
    assert_ne!(pixel(&image, 25, 50), [255; 4]);
}
#[test]
fn erasing_shaft_does_not_remove_arrowhead_or_delete_annotation() {
    let mut edit = session(white(200, 100));
    let (kind, style) = arrow(6.);
    edit.add_annotation(kind, style).unwrap();
    edit.erase_stroke(vec![p(20., 50.), p(150., 50.)], 20.)
        .unwrap();
    let image = edit.document.render_rgba().unwrap();
    assert_eq!(edit.document.annotations().len(), 1);
    for x in [30, 80, 140] {
        assert_eq!(pixel(&image, x, 50), [255; 4]);
    }
    assert_ne!(pixel(&image, 186, 50), [255; 4]);
    assert_ne!(pixel(&image, 178, 46), [255; 4]);
}
#[test]
fn grazing_filled_rectangle_erases_outer_stroke_only() {
    let mut edit = session(white(200, 100));
    edit.add_annotation(
        AnnotationKind::Rectangle(Rect::new(50., 30., 60., 40.)),
        AnnotationStyle {
            line_width: 8.,
            fill_color: Some(RgbaColor::new(0.2, 0.4, 0.9, 1.)),
            ..AnnotationStyle::default()
        },
    )
    .unwrap();
    assert!(
        edit.erase_stroke(vec![p(44., 35.), p(44., 65.)], 8.)
            .unwrap()
    );
    let image = edit.document.render_rgba().unwrap();
    assert_eq!(pixel(&image, 47, 50), [255; 4]);
    assert_ne!(pixel(&image, 80, 50), [255; 4]);
    assert_ne!(pixel(&image, 108, 50), [255; 4]);
    assert!(
        !edit
            .erase_stroke(vec![p(30., 35.), p(30., 65.)], 8.)
            .unwrap()
    );
    assert_eq!(edit.document.annotations()[0].erasures.len(), 1);
}
#[test]
fn crossing_erasures_never_restore_pixels() {
    let mut edit = session(white(200, 100));
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: vec![p(20., 50.), p(180., 50.)],
        },
        AnnotationStyle {
            line_width: 10.,
            ..AnnotationStyle::default()
        },
    )
    .unwrap();
    edit.erase_stroke(vec![p(60., 50.), p(140., 50.)], 12.)
        .unwrap();
    edit.erase_stroke(vec![p(100., 20.), p(100., 80.)], 12.)
        .unwrap();
    let image = edit.document.render_rgba().unwrap();
    for x in [70, 100, 130] {
        assert_eq!(pixel(&image, x, 50), [255; 4]);
    }
    for x in [30, 170] {
        assert_ne!(pixel(&image, x, 50), [255; 4]);
    }
}
#[test]
fn each_eraser_drag_is_one_undo_step_and_lifted_gaps_survive() {
    let mut edit = session(white(200, 100));
    let (kind, style) = arrow(6.);
    edit.add_annotation(kind, style).unwrap();
    edit.erase_stroke(vec![p(60., 50.), p(80., 50.)], 16.)
        .unwrap();
    edit.erase_stroke(vec![p(120., 50.), p(130., 50.)], 16.)
        .unwrap();
    let erased = edit.render_png().unwrap();
    let image = rgba(&erased);
    assert_ne!(pixel(&image, 100, 50), [255; 4]);
    assert_eq!(pixel(&image, 125, 50), [255; 4]);
    assert!(edit.undo());
    assert_eq!(edit.document.annotations()[0].erasures.len(), 1);
    assert_ne!(
        pixel(&edit.document.render_rgba().unwrap(), 125, 50),
        [255; 4]
    );
    assert!(edit.redo());
    assert_eq!(edit.render_png().unwrap(), erased);
    assert!(edit.undo());
    assert!(edit.undo());
    assert!(edit.document.annotations()[0].erasures.is_empty());
}
#[test]
fn erasing_nothing_does_not_make_history_or_hit_diagonal_bounding_box() {
    let mut edit = session(white(120, 120));
    edit.add_annotation(
        AnnotationKind::Arrow {
            start: p(0., 0.),
            end: p(100., 100.),
        },
        AnnotationStyle {
            line_width: 6.,
            ..AnnotationStyle::default()
        },
    )
    .unwrap();
    let revision = edit.revision();
    assert!(
        !edit
            .erase_stroke(vec![p(80., 10.), p(90., 10.)], 12.)
            .unwrap()
    );
    assert_eq!(edit.revision(), revision);
    assert!(
        edit.erase_stroke(vec![p(50., 40.), p(50., 60.)], 12.)
            .unwrap()
    );
}
#[test]
fn crop_and_visible_eraser_coordinates_preserve_holes() {
    let mut edit = session(white(200, 100));
    let (kind, style) = arrow(6.);
    edit.add_annotation(kind, style).unwrap();
    edit.erase_stroke(vec![p(100., 40.), p(100., 60.)], 20.)
        .unwrap();
    edit.apply_visible_crop(Rect::new(50., 0., 100., 100.))
        .unwrap();
    let crop = edit.document.render_rgba().unwrap();
    assert_eq!(crop.dimensions(), (100, 100));
    assert_eq!(pixel(&crop, 50, 50), [255; 4]);
    assert_ne!(pixel(&crop, 20, 50), [255; 4]);
    edit.erase_visible_stroke(vec![p(20., 40.), p(20., 60.)], 20.)
        .unwrap();
    assert_eq!(
        pixel(&edit.document.render_rgba().unwrap(), 20, 50),
        [255; 4]
    );
    assert_eq!(
        edit.document.annotations()[0].erasures[1].points[0],
        p(70., 40.)
    );
    assert_eq!(
        edit.document.visible_annotations()[0].erasures[0].points[0],
        p(50., 40.)
    );
}
#[test]
fn per_annotation_holes_do_not_erase_old_or_later_layers() {
    let mut edit = session(white(200, 100));
    let lower = edit
        .add_annotation(
            AnnotationKind::Highlight(Rect::new(80., 30., 40., 40.)),
            AnnotationStyle::highlight(),
        )
        .unwrap();
    let (kind, style) = arrow(6.);
    let upper = edit.add_annotation(kind, style).unwrap();
    // Simulate a preexisting per-annotation hole without attaching it to lower layers.
    let annotation = Arc::make_mut(&mut edit.document.annotations)
        .iter_mut()
        .find(|a| a.id == upper)
        .unwrap();
    annotation.erasures.push(EraserStroke {
        points: vec![p(100., 50.)],
        width: 20.,
    });
    let with_lower = edit.document.render_rgba().unwrap();
    assert_ne!(pixel(&with_lower, 100, 50), [255; 4]);
    edit.remove_annotation(lower).unwrap();
    assert_eq!(
        pixel(&edit.document.render_rgba().unwrap(), 100, 50),
        [255; 4]
    );
    edit.add_annotation(
        AnnotationKind::Highlight(Rect::new(80., 30., 40., 40.)),
        AnnotationStyle::highlight(),
    )
    .unwrap();
    assert_ne!(
        pixel(&edit.document.render_rgba().unwrap(), 100, 50),
        [255; 4]
    );
}
#[test]
fn mask_erasing_reveals_only_brushed_area_in_export_and_ocr() {
    let base = striped(200, 100);
    let mut edit = session(base.clone());
    edit.add_annotation(
        AnnotationKind::Blur(Rect::new(20., 20., 60., 60.)),
        AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, MaskPattern::Solid),
    )
    .unwrap();
    edit.erase_stroke(vec![p(50., 50.)], 16.).unwrap();
    for image in [
        edit.document.render_rgba().unwrap(),
        rgba(&edit.render_for_external_ocr_png().unwrap()),
    ] {
        assert_eq!(pixel(&image, 50, 50), pixel(&base, 50, 50));
        assert_eq!(pixel(&image, 25, 25), [41, 41, 41, 255]);
        assert_eq!(pixel(&image, 75, 75), [41, 41, 41, 255]);
    }
    assert_eq!(edit.document.annotations().len(), 1);
}
#[test]
fn moving_annotation_moves_eraser_holes_and_supports_undo() {
    let mut edit = session(white(160, 100));
    let id = edit
        .add_annotation(
            AnnotationKind::Blur(Rect::new(10., 10., 60., 60.)),
            AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, MaskPattern::Solid),
        )
        .unwrap();
    edit.erase_stroke(vec![p(40., 40.)], 16.).unwrap();
    let before = edit.render_png().unwrap();
    edit.move_annotation(id, 60., 0.).unwrap();
    let image = edit.document.render_rgba().unwrap();
    assert_eq!(pixel(&image, 100, 40), [255; 4]);
    assert_ne!(pixel(&image, 80, 20), [255; 4]);
    assert_eq!(pixel(&image, 20, 20), [255; 4]);
    assert_eq!(
        edit.document.annotations()[0].erasures[0].points[0],
        p(100., 40.)
    );
    edit.undo();
    assert_eq!(edit.render_png().unwrap(), before);
}
#[test]
fn hit_test_uses_paint_order_and_does_not_select_erased_holes() {
    let mut edit = session(white(100, 100));
    let mask = edit
        .add_annotation(
            AnnotationKind::Blur(Rect::new(10., 10., 60., 60.)),
            AnnotationStyle::redaction(),
        )
        .unwrap();
    let vector = edit
        .add_annotation(
            AnnotationKind::Freehand {
                points: vec![p(0., 40.), p(99., 40.)],
            },
            AnnotationStyle::default(),
        )
        .unwrap();
    assert_eq!(edit.hit_test(p(40., 40.), 1.), Some(mask));
    Arc::make_mut(&mut edit.document.annotations)[0]
        .erasures
        .push(EraserStroke {
            points: vec![p(40., 40.)],
            width: 20.,
        });
    assert_eq!(edit.hit_test(p(40., 40.), 1.), Some(vector));
    assert_eq!(edit.hit_test(p(40., 20.), 1.), Some(mask));
}
#[test]
fn invalid_edits_are_atomic_and_new_edits_clear_redo() {
    let mut edit = session(white(80, 80));
    let start = edit.revision();
    assert!(
        edit.set_crop(Some(Rect::new(f32::NAN, 0., 10., 10.)))
            .is_err()
    );
    assert_eq!(edit.revision(), start);
    assert!(!edit.can_undo());
    assert!(
        edit.add_annotation(
            AnnotationKind::Freehand {
                points: vec![p(f32::INFINITY, 2.)]
            },
            AnnotationStyle::default()
        )
        .is_err()
    );
    assert!(!edit.has_edits());
    edit.add_annotation(
        AnnotationKind::Rectangle(Rect::new(1., 1., 10., 10.)),
        AnnotationStyle::default(),
    )
    .unwrap();
    assert!(edit.has_edits());
    edit.undo();
    assert!(!edit.has_edits());
    assert!(edit.can_redo());
    edit.set_crop(Some(Rect::new(0., 0., 40., 40.))).unwrap();
    assert!(!edit.can_redo());
}
#[test]
fn history_is_bounded_and_snapshots_are_cheap_and_immutable() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RenderSnapshot>();
    let mut edit = session(white(80, 80));
    let snapshot = edit.render_snapshot();
    assert!(Arc::ptr_eq(&snapshot.base_image, &edit.document.base_image));
    assert!(Arc::ptr_eq(
        &snapshot.annotations,
        &edit.document.annotations
    ));
    for i in 0..100 {
        edit.add_annotation(
            AnnotationKind::Freehand {
                points: vec![p((i % 80) as f32, 10.)],
            },
            AnnotationStyle::default(),
        )
        .unwrap();
    }
    assert_eq!(edit.undo.len(), MAX_HISTORY_STEPS);
    assert!(snapshot.annotations().is_empty());
    assert_eq!(snapshot.render_rgba().unwrap(), white(80, 80));
    let mut count = 0;
    while edit.undo() {
        count += 1;
    }
    assert_eq!(count, MAX_HISTORY_STEPS);
    assert_eq!(edit.document.annotations().len(), 100 - MAX_HISTORY_STEPS);
}
#[test]
fn resource_bounds_fail_closed_without_partial_export() {
    assert!(validate_image_size(0, 10).is_err());
    assert!(validate_image_size(10, 60_001).is_err());
    assert!(validate_image_size(8_001, 8_000).is_err());
    let mut edit = session(white(2_000, 2_000));
    for _ in 0..65 {
        edit.add_annotation(
            AnnotationKind::Highlight(Rect::new(0., 0., 2_000., 2_000.)),
            AnnotationStyle::highlight(),
        )
        .unwrap();
    }
    assert!(edit.render_png().unwrap_err().contains("rendering budget"));
    assert!(
        edit.render_snapshot()
            .render_preview_tiles()
            .unwrap_err()
            .contains("rendering budget")
    );
    // Decorative overdraw does not block a separate redaction-only OCR image.
    assert_eq!(
        rgba(&edit.render_for_external_ocr_png().unwrap()).dimensions(),
        (2_000, 2_000)
    );
}
#[test]
fn transparent_background_is_unchanged_away_from_annotations() {
    let base = RgbaImage::from_pixel(40, 40, Rgba([19, 47, 93, 31]));
    let mut edit = session(base.clone());
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: vec![p(20., 20.)],
        },
        AnnotationStyle::default(),
    )
    .unwrap();
    let image = edit.document.render_rgba().unwrap();
    assert_eq!(pixel(&image, 0, 0), pixel(&base, 0, 0));
    assert_eq!(pixel(&image, 39, 39), pixel(&base, 39, 39));
    assert_ne!(pixel(&image, 20, 20), pixel(&base, 20, 20));
}
#[test]
fn annotations_and_erasures_remain_continuous_across_tile_boundaries() {
    let mut edit = session(white(1_400, 100));
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: vec![p(0., 50.), p(1_399., 50.)],
        },
        AnnotationStyle {
            line_width: 12.,
            ..AnnotationStyle::default()
        },
    )
    .unwrap();
    edit.erase_stroke(vec![p(500., 50.), p(540., 50.)], 6.)
        .unwrap();
    let image = edit.document.render_rgba().unwrap();
    for x in 503..538 {
        assert_eq!(pixel(&image, x, 50), [255; 4]);
        assert_ne!(pixel(&image, x, 45), [255; 4]);
    }
    for x in [490, 550, 1_020, 1_025] {
        assert_ne!(pixel(&image, x, 50), [255; 4]);
    }
}
#[test]
fn invalid_style_values_normalize_without_recursion_or_translucent_masks() {
    let style = AnnotationStyle {
        line_width: f32::NAN,
        opacity: f32::INFINITY,
        font_size: -4.,
        fill_color: Some(RgbaColor::new(f32::NAN, -2., 8., 0.1)),
        ..AnnotationStyle::default()
    }
    .normalized();
    assert_eq!(style.line_width, 4.);
    assert_eq!(style.opacity, 1.);
    assert_eq!(style.font_size, 1.);
    let mask = ScreenshotAnnotation::new(AnnotationKind::Blur(Rect::new(1., 1., 2., 2.)), style);
    assert_eq!(mask.style.mask_fill(), RgbaColor::new(0., 0., 1., 1.));
    assert_eq!(mask.style.opacity, 1.);
}
#[test]
fn text_requires_real_explicit_font_and_ocr_never_needs_font() {
    let mut edit = session(white(100, 100));
    assert!(
        edit.add_annotation(
            AnnotationKind::Text {
                text: "Hello".into(),
                origin: p(10., 10.),
                max_width: 80.
            },
            AnnotationStyle::default()
        )
        .unwrap_err()
        .contains("font")
    );
    assert!(!edit.has_edits());
    assert!(edit.set_text_font(b"not a font").is_err());
    assert!(!edit.has_text_font());
    // A malformed imported state still cannot sneak decorative text into OCR.
    Arc::make_mut(&mut edit.document.annotations).push(ScreenshotAnnotation::new(
        AnnotationKind::Text {
            text: "Sensitive decoration".into(),
            origin: p(10., 10.),
            max_width: 80.,
        },
        AnnotationStyle::default(),
    ));
    assert!(edit.render_png().is_err());
    assert!(
        edit.render_snapshot()
            .render_preview_tiles()
            .unwrap_err()
            .contains("font")
    );
    assert_eq!(
        rgba(&edit.render_for_external_ocr_png().unwrap()),
        white(100, 100)
    );
}
#[test]
fn explicit_font_wraps_text_and_masks_it_regardless_of_order() {
    let Some(font) = optional_test_font() else {
        eprintln!("Text raster integration skipped: no local test font installed");
        return;
    };
    let mut edit = session(white(140, 220));
    edit.set_text_font(&font).unwrap();
    let id = edit
        .add_annotation(
            AnnotationKind::Text {
                text: vec!["wrapped"; 28].join(" "),
                origin: p(10., 8.),
                max_width: 54.,
            },
            AnnotationStyle {
                stroke_color: RgbaColor::new(0., 0., 0., 1.),
                ..AnnotationStyle::default()
            },
        )
        .unwrap();
    let image = edit.document.render_rgba().unwrap();
    assert!((80..180).any(|y| (10..70).any(|x| pixel(&image, x, y) != [255; 4])));
    let bounds = edit
        .document
        .painted_rect(&edit.document.annotations()[0])
        .unwrap();
    assert!(bounds.height > 80.);
    edit.erase_stroke(vec![p(35., 70.)], 20.).unwrap();
    edit.move_annotation(id, 40., 0.).unwrap();
    assert_eq!(
        edit.document.annotations()[0].erasures[0].points[0],
        p(75., 70.)
    );
    edit.add_annotation(
        AnnotationKind::Blur(Rect::new(0., 0., 140., 220.)),
        AnnotationStyle::mask(AnnotationStyle::REDACTION_FILL, MaskPattern::Solid),
    )
    .unwrap();
    edit.add_annotation(
        AnnotationKind::Text {
            text: "After mask".into(),
            origin: p(10., 10.),
            max_width: 120.,
        },
        AnnotationStyle::default(),
    )
    .unwrap();
    let masked = edit.document.render_rgba().unwrap();
    assert!(masked.pixels().all(|p| p.0 == [41, 41, 41, 255]));
}
#[test]
fn unsupported_text_glyphs_error_instead_of_silent_substitution() {
    let Some(font) = optional_test_font() else {
        return;
    };
    let mut edit = session(white(100, 100));
    edit.set_text_font(&font).unwrap();
    let before = edit.revision();
    let error = edit
        .add_annotation(
            AnnotationKind::Text {
                text: "\u{10FFFF}".into(),
                origin: p(10., 10.),
                max_width: 80.,
            },
            AnnotationStyle::default(),
        )
        .unwrap_err();
    assert!(error.contains("font"));
    assert_eq!(edit.revision(), before);
}
#[test]
fn eraser_geometry_and_per_annotation_copies_are_bounded_before_allocation() {
    let mut edit = session(white(100, 100));
    let long = vec![p(10., 10.); 10_000];
    edit.add_annotation(
        AnnotationKind::Freehand {
            points: long.clone(),
        },
        AnnotationStyle::default(),
    )
    .unwrap();
    let revision = edit.revision();
    assert!(
        edit.erase_stroke(long, 10.)
            .unwrap_err()
            .contains("geometry budget")
    );
    assert_eq!(edit.revision(), revision);
    assert!(edit.document.annotations()[0].erasures.is_empty());
    let mut masks = session(white(100, 100));
    for _ in 0..20 {
        masks
            .add_annotation(
                AnnotationKind::Blur(Rect::new(0., 0., 100., 100.)),
                AnnotationStyle::redaction(),
            )
            .unwrap();
    }
    let revision = masks.revision();
    let brush = vec![p(10., 10.); 60_000];
    assert!(masks.erase_stroke(brush, 10.).unwrap_err().contains("4 MB"));
    assert_eq!(masks.revision(), revision);
    assert!(
        masks
            .document
            .annotations()
            .iter()
            .all(|a| a.erasures.is_empty())
    );
}
