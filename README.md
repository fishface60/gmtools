GMTool
======

Tools, written to aid in GMing tabletop RPGs,
with the secondary goal of becoming proficient in Rust.

Development set-up
------------------

Install rust of course.
The webui component requires wasm-pack
which is included as part of build-dependencies,
but requires the system openssl at least transitively
so libssl-dev may need to be installed.