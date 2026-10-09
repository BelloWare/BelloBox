# JSON formatter integration GUI acceptance

Run only the sealed ordinary default-feature binary for this candidate. Verify binary/source-manifest SHA256 before launch. Use isolated app config and synthetic local data. No credentials, provider/network inputs, native-Mac claim, or source mutation.

1. Open palette JSON session. Set unsorted nested input `{"z":{"b":1,"a":2},"a":[{"y":3,"x":4},{}]}`. Pretty must recursively order a before z, a before b, x before y, retaining array order. Minify must equal `{"a":[{"x":4,"y":3},{}],"z":{"a":2,"b":1}}`.
2. Numeric input `[-0,-0.0,1E2,1e2,1E+02,1E-02,1.2300,900719925474099312345678901234567890,1e999999]`. Both Pretty and Minify preserve all numeric spellings. Confirm copy/export uses actual output rather than a stale previous result.
3. Input `{"e\u0301":1,"f":2}`. Minify places f first while retaining the decomposed spelling of the other key. Input `{"\u00e9":1,"e\u0301":2}` must reject as duplicate. Validate must reject too, with copy/export disabled appropriately for failed output.
4. Valid input -> Validate: output/status is exactly the approved `Valid JSON.` (presentation may have a separate label). Then switch Pretty/Minify repeatedly, dismiss/reopen, confirm intended persisted input/mode/output behavior from the existing session contract.
5. Open the independent JSON window from session; verify durable handoff and window ownership remain intact with the numeric fixture. Replace/edit source after opening as covered by the existing GUI session contract; ensure no stale overwrite.
6. Confirm isolated output copy, transfer/export behavior, keyboard Enter/Return mode behavior, Close/Escape, and invalid-to-valid recovery do not regress. Do not modify other tool semantics.

This is an incremental formatter GUI receipt. Prior JSON session R2 receipt remains relevant history, not fresh coverage of unexercised paths. Record exact observed cases, screenshot/output evidence, binary hash, environment, cleanup, and unknowns. Native byte oracle covers17vectors on the recorded macOS/Swift/Foundation host separately; this Linux GUI does not expand that claim.
