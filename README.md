# rsnes

<div align='center'>
  <img src='https://user-images.githubusercontent.com/26610181/131219139-4b2c12ca-cc3d-4a72-827c-1c83476a4401.png'
       alt='rsnes logo' width='384cm' align='center'>
</div>

A [SNES](https://en.wikipedia.org/wiki/Super_Nintendo_Entertainment_System) emulator written in [Rust](https://www.rust-lang.org/)

## Implementation Status

`Super Mario World` is already running and playing sound that only lacks sound effects and has a faulty volume setting.
Rendering already works partially (no sprites, only BGs, no color math, etc.).

- 65816 instruction implementation coverage: ≈77,5%
- SPC instruction implementation coverage: ≈70%

## Structure

This repository is a workspace consisting of two crates

- `rsnes` - the SNES backend library (located in `/rsnes/`)
- `rsnes-emulator` - a sample frontend implementation using `winit` and `wgpu` (located in `/emulator/`)

⚠️ Please note that the `rsnes` API is neither tested nor documented (well) ⚠️

## Features

This is a set of features to be implemented in the future (sorted by priority)
- [ ] Mode 7 support
- [ ] Sprite support
- [ ] S-DSP echo effect support
- [ ] S-DSP noise effect support
- [ ] Save game to files
- [ ] SA-1 support
- [ ] emulator running also on [WASM](https://webassembly.org/)
- [ ] Real Gamepad input support for `rsnes-emulator` (see [winit#944](https://github.com/rust-windowing/winit/issues/944), maybe use unstable fork or branch?)
- [ ] Improved documentation
- [ ] Tests
  - [ ] 65816 processor instruction tests
  - [ ] SPC-700 processor instruction tests
  - [ ] Audio tests
  - [ ] Video output tests
- [ ] [DSP](https://en.wikipedia.org/wiki/NEC_%C2%B5PD7720#%C2%B5PD77C25) coprocessor support
  - [ ] DSP-1, DSP-1A, DSP-1B
  - [ ] DSP-2, DSP-3, DSP-4 (low priority)
  - [ ] ST010, ST011 (very low priority)
- [ ] [GSU](https://en.wikipedia.org/wiki/Super_FX) coprocessor support (also known as Super FX)
  - [ ] GSU1
  - [ ] GSU2
- [ ] Multitap (MP5) controller support
- [ ] [SNES Mouse](https://en.wikipedia.org/wiki/Super_NES_Mouse) support
- [ ] [SNES Super Scope](https://en.wikipedia.org/wiki/Super_Scope) support
- [ ] Save States
- [ ] Capcom CX4 coprocessor support (this processor is only used in Mega Man X2 and Mega Man X3)
- [ ] SPC7110 data compression chip

## Contributing

Contributions of any kind (bug reports, feature requests, pull requests, …) are very welcome.
