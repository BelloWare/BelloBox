# Full-popup QR intrinsic image correction

Base: published 15197a71d81c620bf60da18f0d066505e4c0dc99.

## Preserved v2 failure

The ordinary v2 binary d1b68a3f208c603977ee532f98edeaf4bd08dcaf5050fb1b84bdff76b39edc0a was exercised through real cloud GUI input. At confirmed 400×480 client size, the QR's black modules extended below its preview into the Encoded text row and were obscured by the editor. At confirmed 520×620, the white image card protruded about19 pixels below the preview. Local captures remain local; they are not publication artifacts. The GUI worker's text receipt is in the separate popup-qr-gui evidence folder.

The v2 TestPlatform assertion inspected only the padded wrapper's bounds. It did not establish the Img child's decoded dimensions or painted extent. The original independent review is preserved verbatim in independent-review-v2.txt with a later qualification. Superseded source candidate272e2b1d404be836b836f260dca9b833535f98b3/treec65addce8d8fe06affb7d0774239c193bc279171 remains available as diagnostic source; it was never placed on rust.

## Correction

qr_image_element now uses the final allocated canvas bounds to lay out the padded card and Img as explicit squares. Side=min(width,height,296), image side=side−28. The complete image uses Contain. Hosts with side≤28 skip child creation/paint, so outer padding cannot force overflow in an initial/hidden tiny frame. It does not modify the PNG data, clipboard, save worker, or generated dimensions. Canvas prepaint retains the element for exactly one paint, with no state mutation, notification or self-scheduled frame loop. Normal GPUI image decode completion can redraw the owning view, recomputing current bounds.

Regression tests inspect both wrapper and actual Img child, repeat supported down/up sizes, and exercise an already decoded PNG with intrinsic dimensions larger than the host. Repeated non-tiny frames explicitly refresh the window. Separate fresh zero/tiny hosts verify no padded child is created. The real app minimum prevents an ordinary GUI zero-sized window; these tiny cases are synthetic layout guards.

## Test-development failure retained

app-full-v3.log recorded443 passed,1 failed,2 preexisting ignored. qr-tiny-diagnostic.log independently reproduced the one failure. It asserted absence of a debug selector after resizing a previously painted window to zero. GPUI0.2.2 Frame::clear does not clear its debug_bounds map, so the assertion read stale instrumentation entries. This was a test-oracle failure, not an observed product paint failure. The final test uses fresh tiny hosts for absence and retains resized normal hosts for current measured dimensions. Neither failing log is replaced.

qr-intrinsic-fit-r1.log records the first explicit-dimension focused pass. qr-intrinsic-fit-r2.log records10 popup tests passed before the tiny-host assertion was added. qr-intrinsic-fit-r3.log records11 popup tests passed after the final correction. Later full/default/minimal build/lint results are separate files and must be read before claiming completion.

## Limits

This correction requires a new ordinary package-clean binary and actual GUI rerun. Native macOS compilation/TestPlatform checks are separate from real AppKit/NSPasteboard/focus/sheet/IME, Retina, accessibility and scanner acceptance. No native capture/tool/vault gate, dependency, permission, credential, paid provider or release scope is changed.
