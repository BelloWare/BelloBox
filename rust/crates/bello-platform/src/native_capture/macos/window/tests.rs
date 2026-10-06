//! Synthetic native metadata/ABI tests only: no SCK, TCC, screen/window catalog,
//! NSScreen, real capture, UI, clipboard, file or network calls.
use super::*;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGRectCreateDictionaryRepresentation(rect: CaptureRect) -> Id;
}
#[link(name = "objc")]
extern "C" {
    fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
    fn objc_registerClassPair(class: Id);
    fn class_addMethod(
        class: Id,
        selector: Sel,
        implementation: unsafe extern "C" fn(),
        encoding: *const c_char,
    ) -> ObjcBool;
    fn objc_disposeClassPair(class: Id);
}
unsafe fn string(value: &str) -> OwnedObject {
    let allocation = send!(class(b"NSString\0").unwrap(), b"alloc\0", () -> Id);
    OwnedObject::from_owned(send!(allocation, b"initWithBytes:length:encoding:\0",
        (*const u8 => value.as_ptr(), usize => value.len(), usize => 4) -> Id))
    .unwrap()
}
unsafe fn integer(value: i64) -> Id {
    send!(class(b"NSNumber\0").unwrap(), b"numberWithLongLong:\0", (i64 => value) -> Id)
}
unsafe fn dictionary(keys: &[Id], values: &[Id]) -> Id {
    assert_eq!(keys.len(), values.len());
    send!(class(b"NSDictionary\0").unwrap(), b"dictionaryWithObjects:forKeys:count:\0",
        (*const Id => values.as_ptr(), *const Id => keys.as_ptr(), usize => keys.len()) -> Id)
}
unsafe fn array(values: &[Id]) -> Id {
    send!(class(b"NSArray\0").unwrap(), b"arrayWithObjects:count:\0",
        (*const Id => values.as_ptr(), usize => values.len()) -> Id)
}
#[test]
fn bundle_reads_complete_utf8_and_rejects_embedded_nul_and_overflow() {
    unsafe {
        let _pool = pool().unwrap();
        for text in ["com.example.App", "界.example"] {
            let native = string(text);
            assert_eq!(read_bundle(native.ptr).unwrap(), text);
        }
        let exact = string(&"a".repeat(MAX_WINDOW_IDENTITY_BYTES));
        assert_eq!(
            read_bundle(exact.ptr).unwrap().len(),
            MAX_WINDOW_IDENTITY_BYTES
        );
        for text in [
            "app\0suffix".to_owned(),
            "app\nname".to_owned(),
            String::new(),
            "a".repeat(MAX_WINDOW_IDENTITY_BYTES + 1),
            "界".repeat(342),
        ] {
            let native = string(&text);
            assert_eq!(
                read_bundle(native.ptr),
                Err(WindowPolicyError::InvalidMetadata.into())
            );
        }
        assert_eq!(
            read_bundle(integer(7)),
            Err(WindowPolicyError::InvalidMetadata.into())
        );
    }
}
#[test]
fn cg_rows_are_owned_scalar_metadata_without_bundle_enrichment() {
    unsafe {
        let _pool = pool().unwrap();
        let bounds = OwnedCf::new(CGRectCreateDictionaryRepresentation(CaptureRect::new(
            10., 20., 80., 60.,
        )))
        .unwrap();
        let keys = [
            kCGWindowNumber,
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowAlpha,
            kCGWindowBounds,
        ];
        let values = [integer(42), integer(75), integer(0), integer(1), bounds.0];
        let rows = observations(array(&[dictionary(&keys, &values)])).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].identity.window_id, 42);
        assert_eq!(rows[0].identity.owner_process_id, 75);
        assert_eq!(rows[0].identity.owner_bundle_id, None);
        assert_eq!(
            rows[0].frame,
            WindowFrame(CaptureRect::new(10., 20., 80., 60.))
        );
        assert!(rows[0].on_screen);
        assert!(observations(array(&[integer(7)])).is_err());
        assert!(observations(array(&[dictionary(&keys[..4], &values[..4])])).is_err());
        assert!(observations(array(&vec![integer(7); MAX_WINDOW_CANDIDATES + 1])).is_err());
    }
}
unsafe extern "C" fn fixture_frame(_: Id, _: Sel) -> CaptureRect {
    CaptureRect::new(-13., 7., 123.5, 99.25)
}
unsafe extern "C" fn wrong_frame(_: Id, _: Sel) -> u64 {
    7
}
unsafe fn fixture_class(name: &std::ffi::CStr, good: bool) -> Id {
    let class = objc_allocateClassPair(class(b"NSObject\0").unwrap(), name.as_ptr(), 0);
    assert!(!class.is_null());
    let (implementation, encoding) = if good {
        (
            std::mem::transmute::<
                unsafe extern "C" fn(Id, Sel) -> CaptureRect,
                unsafe extern "C" fn(),
            >(fixture_frame),
            c"{CGRect={CGPoint=dd}{CGSize=dd}}@:",
        )
    } else {
        (
            std::mem::transmute::<unsafe extern "C" fn(Id, Sel) -> u64, unsafe extern "C" fn()>(
                wrong_frame,
            ),
            c"Q@:",
        )
    };
    assert_ne!(
        class_addMethod(
            class,
            sel_registerName(c"frame".as_ptr()),
            implementation,
            encoding.as_ptr()
        ),
        0
    );
    objc_registerClassPair(class);
    class
}
#[test]
fn checked_cgrect_imp_uses_real_struct_abi_and_rejects_wrong_encoding() {
    unsafe {
        let _pool = pool().unwrap();
        let good = fixture_class(c"BelloWindowFrameFixture", true);
        let bad = fixture_class(c"BelloWindowWrongFrameFixture", false);
        let object = OwnedObject::from_owned(send!(good, b"new\0", () -> Id)).unwrap();
        assert_eq!(
            read_frame(object.ptr).unwrap(),
            CaptureRect::new(-13., 7., 123.5, 99.25)
        );
        drop(object);
        let object = OwnedObject::from_owned(send!(bad, b"new\0", () -> Id)).unwrap();
        assert_eq!(
            read_frame(object.ptr),
            Err(WindowPolicyError::InvalidMetadata.into())
        );
        drop(object);
        objc_disposeClassPair(good);
        objc_disposeClassPair(bad);
    }
}
#[test]
fn candidate_entry_and_source_transport_are_type_checked_without_invocation() {
    // Taking the actual function address keeps the full native path compiled;
    // calling it here would enumerate/capture and is intentionally prohibited.
    let _entry: fn(
        WindowCaptureSelection,
        WindowCaptureOptions,
        Arc<WindowCaptureSession>,
        CaptureCancellation,
    ) -> WindowCaptureResult<NativeWindowCaptureSnapshot> = capture;
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<WindowEncodeTarget>();
    assert_traits::<MainContext>();
}

fn fixture() -> (
    WindowEncodeTarget,
    TopologySnapshot,
    Vec<WindowObservation<'static>>,
) {
    let token = WindowSelectionToken {
        session: 1,
        generation: 0,
    };
    let topology = TopologySnapshot {
        main_display_id: 1,
        displays: vec![WindowDisplayGeometry {
            display: CaptureDisplay {
                id: 1,
                bounds: CaptureRect::new(0., 0., 1000., 800.),
                pixels: CapturePixelSize {
                    width: 2000,
                    height: 1600,
                },
            },
            appkit_size_points: CaptureSize {
                width: 1000.,
                height: 800.,
            },
            backing_scale: 2.,
            rotation_degrees: 0.,
        }],
    };
    let observation = WindowObservation {
        identity: WindowIdentity {
            window_id: 2,
            owner_process_id: 3,
            owner_bundle_id: None,
        },
        frame: WindowFrame(CaptureRect::new(10., 20., 100., 80.)),
        layer: 0,
        alpha: 1.,
        on_screen: true,
    };
    let cancellation = CaptureCancellation::default();
    let selection =
        WindowCaptureSelection::new(observation, topology.borrowed(), 4, token, &cancellation)
            .unwrap();
    let options = WindowCaptureOptions::default();
    let submission = SubmittedWindowSource {
        identity: WindowIdentity {
            owner_bundle_id: Some("org.example.source"),
            ..observation.identity
        },
        frame: observation.frame,
        layer: 0,
        on_screen: true,
    };
    let plan = selection
        .plan(
            submission,
            &[observation],
            topology.borrowed(),
            token,
            options,
            &cancellation,
        )
        .unwrap();
    let request = WindowRequest {
        selection,
        options,
        session: Arc::new(WindowCaptureSession::new(token).unwrap()),
        cancellation,
        expected_token: token,
    };
    (
        WindowEncodeTarget {
            request,
            plan: Arc::new(plan),
        },
        topology,
        vec![observation],
    )
}
#[test]
fn final_window_validation_moves_png_once_and_binds_source_diagnostics() {
    let (target, topology, fresh) = fixture();
    let job = WindowJob::new(target.request.cancellation.clone(), Duration::from_secs(1));
    assert!(job.transition(Stage::InitialContent, Stage::Validating));
    let mut png = b"synthetic bytes, no actual image".to_vec();
    let pointer = png.as_ptr();
    complete_validated(
        &target,
        &mut png,
        target.plan.output_size(),
        &fresh,
        topology.borrowed(),
        &job,
    )
    .unwrap();
    assert!(png.is_empty());
    let result = job.wait().unwrap();
    assert_eq!(result.png.as_ptr(), pointer);
    assert_eq!(result.diagnostics.window_id, 2);
    assert_eq!(result.diagnostics.owner_process_id, 3);
    assert!(!format!("{result:?}").contains("synthetic bytes"));
    let mut duplicate = vec![1];
    assert!(complete_validated(
        &target,
        &mut duplicate,
        target.plan.output_size(),
        &fresh,
        topology.borrowed(),
        &job
    )
    .is_err());
    assert_eq!(duplicate, vec![1]);
}
#[test]
fn final_window_validation_rejects_live_generation_and_fresh_window_changes() {
    for changed_generation in [false, true] {
        let (target, topology, mut fresh) = fixture();
        let job = WindowJob::new(target.request.cancellation.clone(), Duration::from_secs(1));
        assert!(job.transition(Stage::InitialContent, Stage::Validating));
        if changed_generation {
            target
                .request
                .session
                .update(WindowSelectionToken {
                    session: 1,
                    generation: 1,
                })
                .unwrap();
        } else {
            fresh[0].identity.owner_process_id += 1;
        }
        let mut png = vec![1, 2, 3];
        let error = complete_validated(
            &target,
            &mut png,
            target.plan.output_size(),
            &fresh,
            topology.borrowed(),
            &job,
        )
        .unwrap_err();
        assert_eq!(
            error,
            WindowCaptureError::Policy(if changed_generation {
                WindowPolicyError::StaleSelection
            } else {
                WindowPolicyError::WindowChanged
            })
        );
        assert_eq!(png, vec![1, 2, 3]);
        assert!(job.complete(Err(error)));
        assert!(job.wait().is_err());
    }
}
