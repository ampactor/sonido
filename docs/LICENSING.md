# Licensing

Sonido is dual-licensed under MIT and Apache 2.0, at your option.

## Why dual MIT/Apache-2.0?

This is the standard licensing for the Rust embedded ecosystem. `embedded-hal`,
`embassy`, `daisy-embassy`, `cortex-m` — the entire stack Sonido targets uses
MIT or MIT/Apache-2.0. Matching that convention means no license friction when
integrating Sonido into embedded projects.

## Why not AGPL?

Sonido started under AGPL, but copyleft creates unnecessary friction for the
people most likely to use the framework: embedded developers building pedals,
plugin developers shipping DAW effects, and anyone evaluating the code for a
potential hire or acquisition.

The code is the resume. The framework demonstrates the capability — clean DSP
architecture, no_std discipline, production-grade effects. Locking it behind
copyleft doesn't protect anything worth protecting. The signature effects and
the ability to ship them are the actual value, and those live in the hands of
whoever wrote them, not in a license file.

## What this means for you

- Use Sonido in commercial products, closed-source or open-source
- Modify it, fork it, embed it in hardware
- No obligation to share your changes (though contributions are welcome)
- Choose whichever license (MIT or Apache-2.0) works for your project

See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE) for
the full license texts.
