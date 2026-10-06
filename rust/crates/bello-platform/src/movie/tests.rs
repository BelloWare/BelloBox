use super::*;

#[test]
fn read_range_preserves_source_pts_and_tolerance() {
    let r = MovieReadRange::new(2., 3.001, 1. / 15.).unwrap();
    assert_eq!((r.start(), r.end()), (2., 3.001));
    assert!((r.decode_duration() - (1.001 + 1. / 15.)).abs() < 1e-10);
    assert!(MovieReadRange::new(0., 120., 0.05).is_ok());
}
#[test]
fn read_range_rejects_invalid_or_unbounded_values() {
    for (a, b, c) in [
        (f64::NAN, 1., 0.1),
        (0., f64::INFINITY, 0.1),
        (-1., 1., 0.1),
        (1., 1., 0.1),
        (1., 0., 0.1),
        (0., 0.049, 0.1),
        (0., 121., 0.1),
        (0., 1., 0.01),
        (0., 1., 0.3),
    ] {
        assert!(MovieReadRange::new(a, b, c).is_err());
    }
}
#[test]
fn geometry_handles_rotation_mirror_and_origin() {
    let cases = [
        ([1., 0., 0., 1., 0., 0.], (0., 0., 4, 2)),
        ([0., 1., -1., 0., 2., 0.], (0., 0., 2, 4)),
        ([-1., 0., 0., 1., 4., 0.], (0., 0., 4, 2)),
        ([1., 0., 0., -1., 7., -3.], (7., -5., 4, 2)),
    ];
    for (transform, expected) in cases {
        let g = display_geometry(4., 2., transform).unwrap();
        assert_eq!((g.min_x, g.min_y, g.width, g.height), expected);
        assert_eq!(
            (g.extent_width, g.extent_height),
            (f64::from(g.width), f64::from(g.height))
        );
    }
}
#[test]
fn geometry_rejects_nonfinite_singular_and_excessive() {
    for t in [
        [f64::NAN, 0., 0., 1., 0., 0.],
        [0.; 6],
        [1e100, 0., 0., 1., 0., 0.],
        [1., 0., 0., 1., f64::INFINITY, 0.],
    ] {
        assert!(display_geometry(4., 2., t).is_err());
    }
    assert!(display_geometry(16385., 1., [1., 0., 0., 1., 0., 0.]).is_err());
    assert!(display_geometry(4096., 4096., [1., 0., 0., 1., 0., 0.]).is_err());
    assert!(display_geometry(3840., 2160., [1., 0., 0., 1., 0., 0.]).is_ok());
}
#[test]
fn pixel_storage_requires_entire_padded_last_row() {
    assert!(validate_pixel_storage(3, 2, 16, 32).is_ok());
    for size in [27, 28, 31] {
        assert!(validate_pixel_storage(3, 2, 16, size).is_err());
    }
    assert!(validate_pixel_storage(3, 2, 11, 24).is_err());
    assert!(validate_pixel_storage(3, 2, usize::MAX, usize::MAX).is_err());
    assert!(validate_pixel_storage(3, 2, 16, MAX_PIXEL_BUFFER_BYTES + 1).is_err());
}
#[test]
fn frame_transfer_moves_pixels_and_debug_redacts_them() {
    let rgba = vec![23, 47, 89, 255];
    let pointer = rgba.as_ptr();
    let f = MovieFrame::new(0.5, 1, 1, rgba).unwrap();
    assert!(!format!("{f:?}").contains("23, 47"));
    let (time, w, h, rgba) = f.into_parts();
    assert_eq!((time, w, h), (0.5, 1, 1));
    assert_eq!(pointer, rgba.as_ptr());
    assert_eq!(rgba, [23, 47, 89, 255]);
    assert!(MovieFrame::new(f64::NAN, 1, 1, vec![0; 4]).is_err());
    assert!(MovieFrame::new(-1., 1, 1, vec![0; 4]).is_err());
    assert!(MovieFrame::new(0., 1, 1, vec![0; 3]).is_err());
}
#[test]
fn cancellation_is_shared_atomic_state() {
    let a = MovieCancellation::default();
    let b = a.clone();
    assert!(a.check().is_ok());
    b.cancel();
    assert!(a.is_cancelled());
    assert_eq!(a.check(), Err(MovieError::Cancelled));
}
#[cfg(not(target_os = "macos"))]
#[test]
fn unavailable_open_never_reads_a_path() {
    assert!(!std::hint::black_box(NATIVE_MOVIE_READER_IMPLEMENTED));
    assert!(matches!(
        MovieAsset::open(
            Path::new("/no-such-source/no-movie"),
            MovieCancellation::default()
        ),
        Err(MovieError::Unavailable)
    ));
}
#[cfg(not(target_os = "macos"))]
#[test]
fn unavailable_reader_never_calls_the_caller() {
    let info = MovieInfo {
        duration: 1.,
        display_width: 1.,
        display_height: 1.,
        nominal_frame_rate: 15.,
    };
    let source_path = PathBuf::from("/no-such-source/no-movie");
    let mut r = MovieReader {
        info,
        source_path: source_path.clone(),
        cancellation: MovieCancellation::default(),
        _thread_bound: PhantomData,
    };
    assert!(matches!(
        r.next_frame(|| panic!("unexpected callback")),
        Err(MovieError::Unavailable)
    ));
    let a = MovieAsset {
        info,
        source_path,
        cancellation: MovieCancellation::default(),
        _thread_bound: PhantomData,
    };
    assert!(matches!(
        a.reader(MovieReadRange::new(0., 1., 0.1).unwrap()),
        Err(MovieError::Unavailable)
    ));
}
#[cfg(unix)]
mod identity {
    use super::*;
    use std::fs;
    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bellobox-movie-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn source_guard_rejects_directory_empty_and_replaced_files() {
        let d = Dir::new();
        assert!(source_file::SourceFile::open(&d.0).is_err());
        let p = d.0.join("fixture.mov");
        fs::write(&p, []).unwrap();
        assert!(source_file::SourceFile::open(&p).is_err());
        fs::write(&p, b"synthetic").unwrap();
        let s = source_file::SourceFile::open(&p).unwrap();
        assert_eq!(s.path(), p);
        s.verify().unwrap();
        let replacement = d.0.join("replacement");
        fs::write(&replacement, b"synthetic").unwrap();
        fs::rename(replacement, p).unwrap();
        assert_eq!(s.verify(), Err(MovieError::SourceChanged));
    }
    #[test]
    fn source_guard_resolves_symlink_and_detects_edits() {
        let d = Dir::new();
        let p = d.0.join("source.mov");
        let alias = d.0.join("alias.mov");
        fs::write(&p, b"movie").unwrap();
        std::os::unix::fs::symlink(&p, &alias).unwrap();
        let s = source_file::SourceFile::open(&alias).unwrap();
        assert_eq!(s.path(), p);
        fs::write(p, b"different length").unwrap();
        assert_eq!(s.verify(), Err(MovieError::SourceChanged));
    }
}
