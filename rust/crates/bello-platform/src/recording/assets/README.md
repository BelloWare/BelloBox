# Finite supplied recording fixture

`known-rgb.mov` contains 12 raw RGB24 frames, 96 × 64, 10 fps, 1.2 seconds, without audio. It was generated offline with the installed official FFmpeg tool; FFmpeg is not used or required at runtime. Frame pixels exactly follow `fixture.rs::pixels`; the final frame is the magenta sentinel.

Generation: write each pixel as RGB24, frame-major then top-to-bottom rows, using the finite recipe; pipe those 221,184 bytes to:

    ffmpeg -hide_banner -loglevel error -f rawvideo -pixel_format rgb24 -video_size 96x64 -framerate 10 -i pipe:0 -an -c:v rawvideo -pix_fmt rgb24 -fflags +bitexact -flags:v +bitexact -map_metadata -1 -f mov known-rgb.mov

SHA-256: `2844659c2ff59e1ab39abf13e46dfbcf0ba3cd51d97fdd1b69ca2c44949d67c9`

The injected factory only materializes these compiled-in bytes and binds the known-frame recipe to their retained finalized-file identity. It cannot accept a path or caller bytes. Linux injection is not native recording or decoding evidence. Apple tests separately verify the checked-in movie through the native reader and the actual AVAssetWriter through generated inputs.

The raw payload is checked byte-for-byte against the recipe on every platform.
Native readback follows the Swift converter's AVAssetReader BGRA output and
DeviceRGB CoreGraphics rendering; it is not a raw sample-byte extraction API.
Its separate oracle checks every pixel of all 12 frames in order, exact dimensions,
exact opaque alpha, timestamps within 1 ms, and at most one code value of RGB
rounding. This narrow fixture-specific bound reflects the observed macOS native
conversion, not H.264 loss or a promise about all codecs/devices. A larger drift
must fail and be investigated. The generated H.264 writer test remains separate.
