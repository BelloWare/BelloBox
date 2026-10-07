//! Actual AVFoundation generated movies through the same Request/Controller and
//! Model used by the GPUI host. No UI launch, arbitrary-path bypass or capture.
use super::{
    model::{Model, Source},
    worker::{Controller, Request, Response},
};
use bello_platform::movie::{
    GeneratedOrientation, GeneratedTiming, MovieAsset, MovieError, generated_movie,
    generated_movie_case,
};
use bellobox_core::recording::gif::ReplacePolicy;

fn run(controller: &mut Controller, request: Request) -> Response {
    controller.submit(request);
    let (ticket, work) = controller.start().expect("single worker admission");
    let response = std::thread::spawn(move || work.run()).join().unwrap();
    controller.retire(ticket);
    response
}
fn inspect(controller: &mut Controller, model: &mut Model, source: Source) {
    let (generation, cancel) = model.load(source.clone()).unwrap();
    match run(
        controller,
        Request::Inspect {
            generation,
            source,
            cancel,
        },
    ) {
        Response::Inspect { generation, result } => model.loaded(generation, result),
        _ => panic!("wrong worker response"),
    }
    assert!(model.info.is_some(), "{}", model.status);
}
fn seek(
    controller: &mut Controller,
    model: &Model,
    seconds: f64,
) -> Result<super::model::SourceFrame, String> {
    match run(
        controller,
        Request::Seek {
            generation: model.generation,
            revision: 1,
            source: model.selected.clone().unwrap(),
            seconds,
            cancel: Default::default(),
        },
    ) {
        Response::Seek { result, .. } => result,
        _ => panic!("wrong worker response"),
    }
}
#[test]
fn generated_native_host_inspect_seek_trim_export_switch_identity_and_timing() {
    for orientation in [
        GeneratedOrientation::Landscape,
        GeneratedOrientation::Portrait,
        GeneratedOrientation::Mirrored,
    ] {
        let selected = generated_movie(orientation).unwrap();
        let original = std::fs::read(selected.path()).unwrap();
        assert!(
            matches!(
                MovieAsset::open(selected.path(), Default::default()),
                Err(MovieError::Unavailable)
            ),
            "normal native gate remains closed even with fixture feature"
        );
        let mut controller = Controller::default();
        let mut model = Model::default();
        inspect(
            &mut controller,
            &mut model,
            Source::Generated(selected.clone()),
        );
        let initial = seek(&mut controller, &model, 0.).unwrap();
        let marker = seek(&mut controller, &model, 0.2).unwrap();
        assert_ne!(initial.png, marker.png, "generated time marker changes");
        let end = seek(&mut controller, &model, model.info.unwrap().duration).unwrap();
        assert!(end.actual < model.info.unwrap().duration);
        let expected = match orientation {
            GeneratedOrientation::Portrait => (48, 64),
            _ => (64, 48),
        };
        assert_eq!(
            image::load_from_memory(&initial.png)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            expected
        );
        for loops in [false, true] {
            model.options.trim_start = 0.1;
            model.options.trim_end = Some(0.251);
            model.options.frames_per_second = 10;
            model.options.max_width = 320;
            model.options.loops = loops;
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("trim.gif");
            let work = model
                .begin(output.clone(), ReplacePolicy::RefuseExisting)
                .unwrap();
            match run(&mut controller, Request::Export(work)) {
                Response::Export { generation, result } => {
                    assert!(result.is_ok(), "{result:?}");
                    model.finished(generation, result);
                }
                _ => panic!("wrong worker response"),
            }
            let result = model.result.clone().unwrap();
            assert_eq!(result.size, (expected.0 as u16, expected.1 as u16));
            assert_eq!(result.frame_count, 2);
            assert_eq!(result.duration, 0.15);
            let bytes = std::fs::read(&output).unwrap();
            assert_eq!(bytes.windows(11).any(|s| s == b"NETSCAPE2.0"), loops);
            let mut preview = super::model::open_preview(&result).unwrap();
            let mut delays = Vec::new();
            while let Some(frame) = preview.next_frame().unwrap() {
                let rgba = image::load_from_memory(&frame.png).unwrap().to_rgba8();
                assert!(
                    rgba.pixels()
                        .all(|p| !(p[0] > 200 && p[2] > 200 && p[1] < 40)),
                    "decoded GIF must exclude final magenta source sentinel"
                );
                delays.push(frame.delay_centiseconds);
            }
            assert_eq!(delays, [10, 5]);
            // Movie/GIF review still uses the original selected owner after export.
            assert_eq!(seek(&mut controller, &model, 0.).unwrap().png, initial.png);
            let retry = model
                .begin(
                    directory.path().join("cancel.gif"),
                    ReplacePolicy::RefuseExisting,
                )
                .unwrap();
            model.cancel();
            model.cancel();
            if let Response::Export { generation, result } =
                run(&mut controller, Request::Export(retry))
            {
                model.finished(generation, result);
            }
            assert_eq!(model.result.as_ref().unwrap().path, output);
        }
        assert_eq!(std::fs::read(selected.path()).unwrap(), original);
        // Replace with exactly the same bytes and metadata-relevant movie fields.
        let replacement = selected.path().with_extension("replacement.mov");
        std::fs::write(&replacement, &original).unwrap();
        std::fs::rename(replacement, selected.path()).unwrap();
        assert!(
            seek(&mut controller, &model, 0.)
                .unwrap_err()
                .contains("changed")
        );
        let output = tempfile::tempdir().unwrap();
        let work = model
            .begin(
                output.path().join("changed.gif"),
                ReplacePolicy::RefuseExisting,
            )
            .unwrap();
        assert!(matches!(
            run(&mut controller, Request::Export(work)),
            Response::Export { result: Err(_), .. }
        ));
        assert!(!output.path().join("changed.gif").exists());
    }
}
#[test]
fn generated_native_host_short_sparse_long_and_first_following_seek_contract() {
    for timing in [
        GeneratedTiming::Short,
        GeneratedTiming::SparseLong,
        GeneratedTiming::DelayedFirst,
    ] {
        let selected = generated_movie_case(GeneratedOrientation::Landscape, timing).unwrap();
        let mut controller = Controller::default();
        let mut model = Model::default();
        inspect(&mut controller, &mut model, Source::Generated(selected));
        match timing {
            GeneratedTiming::Short => {
                assert!(model.info.unwrap().duration < 0.05);
                assert!(model.plan().is_none());
                assert_eq!(
                    seek(&mut controller, &model, model.info.unwrap().duration)
                        .unwrap()
                        .actual,
                    0.
                );
            }
            GeneratedTiming::SparseLong => {
                assert!(model.info.unwrap().duration > 120.);
                assert!(
                    seek(&mut controller, &model, 50.)
                        .unwrap_err()
                        .contains("No source frame")
                );
                assert!(seek(&mut controller, &model, 130.1).unwrap().actual >= 130.);
            }
            GeneratedTiming::DelayedFirst => {
                let frame = seek(&mut controller, &model, 0.).unwrap();
                assert!(frame.actual >= 0.099 && frame.actual <= 0.101);
            }
            GeneratedTiming::Regular => unreachable!(),
        }
    }
}
