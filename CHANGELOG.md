# Changelog

All notable changes to this project are documented in this file.

## [v0.5.0](https://github.com/versenilvis/clak/releases/tag/v0.5.0) - 2026-10-10

### Bug fixes

- Synthetic repeat for held transform keys on wayland ([6f3369](https://github.com/versenilvis/clak/commit/6f33696a2a6ad72cb45b5a1f90d7162ba962b7e0))
- Holding backspace not working ([ec5af4](https://github.com/versenilvis/clak/commit/ec5af4201e22914c183d25daa74efb127f05d4cb))
- Typing "speedtest" missing "e" ([b98a74](https://github.com/versenilvis/clak/commit/b98a7424ca680f9d318372dac54cb182e26e6e47))
- Closes typed as cloes ([f8d6fa](https://github.com/versenilvis/clak/commit/f8d6fa1096f5e9ba02f74e2a97b231f677490b0c))
- Address bar autocomplete with spaces ([c6c337](https://github.com/versenilvis/clak/commit/c6c33765a03baf0e8e64d53146191e5f9192d57d))

### Documentation

- Update docs ([2e6bb3](https://github.com/versenilvis/clak/commit/2e6bb3562dabef3613ae5289a1b84139289f18c3))
- Update menu section ([41012b](https://github.com/versenilvis/clak/commit/41012bfd083b0608fb14557006a1fbcbfa872ef1))

### Features

- Support steam client ([14bf58](https://github.com/versenilvis/clak/commit/14bf589c7b4dde21e47a906b7e316bc4908865ee))

### Refactors

- Seperate state into small modules ([23640f](https://github.com/versenilvis/clak/commit/23640f0b783a57bef65a8b6bd2ebfac9e6e85ae2))

## [v0.4.5](https://github.com/versenilvis/clak/releases/tag/v0.4.5) - 2026-10-08

### Documentation

- Small note about testing environment ([03a1a2](https://github.com/versenilvis/clak/commit/03a1a2beb6c7fb8194d1a397a9b0caeb9ad86661))

### Performance

- Trim down to retain only Vietnamese and necessary characters ([1a72ee](https://github.com/versenilvis/clak/commit/1a72eefa77368f5f706778388e179d77028298cc))
- Compress release archives with xz and auto-extract in installer ([05a03b](https://github.com/versenilvis/clak/commit/05a03bc7ff16983fde5e4395756e658fcd10410a))

## [v0.4.4](https://github.com/versenilvis/clak/releases/tag/v0.4.4) - 2026-10-07

### Bug fixes

- Use fcitx5 dbus controller to add clak and switch im reliably ([87a6c5](https://github.com/versenilvis/clak/commit/87a6c52eedc815ba62ab439100bac3e175c98ad4))

## [v0.4.3](https://github.com/versenilvis/clak/releases/tag/v0.4.3) - 2026-10-07

### Bug fixes

- Add doctor reminder in post_upgrade and ensure 1-click activates clak immediately ([1b6a71](https://github.com/versenilvis/clak/commit/1b6a71b9d863ed44e69e556744d73a116b09c5c9))

## [v0.4.2](https://github.com/versenilvis/clak/releases/tag/v0.4.2) - 2026-10-07

### Bug fixes

- Redesign doctor tab layout to prevent horizontal overflow ([b4ccae](https://github.com/versenilvis/clak/commit/b4ccae54bd1dbe5951c54f688a3570557ab04f7f))
- Dynamic layout for doctor console log to eliminate empty void ([d5cecd](https://github.com/versenilvis/clak/commit/d5cecdaeeaa612909a664fc34661255f46281118))

### Documentation

- Fix typos and enhance readability ([4e8b53](https://github.com/versenilvis/clak/commit/4e8b533e235d0f77afbfda58a5d83ae22b8dae5b))
- Indicate that clak menu, not its core ([d4161a](https://github.com/versenilvis/clak/commit/d4161af18b608d57a81d21381b28c2f40245846f))

### Features

- Backspace hold policy to prevent over-deletion ([6af36d](https://github.com/versenilvis/clak/commit/6af36d3787b7c74a3cc47d1272e52b6c69f1b46e))
- Binary replacement detection in engine and one-click reload in ui ([18606f](https://github.com/versenilvis/clak/commit/18606fa302131b85790fdfc5285c665bc222356f))
- Tolerance for lagging surrounding text during rapid typing ([dbb3f0](https://github.com/versenilvis/clak/commit/dbb3f09d4bbf11cb71c8e61848ae8e5b0dc4394b))
- Dynamically update subModeLabel to VI and EN ([0dd908](https://github.com/versenilvis/clak/commit/0dd9082e46a5e5b8c0bfafd07120ecea45b8b016))
- Packaging udev rules and 1-click compatibility setup in clak-gui ([049665](https://github.com/versenilvis/clak/commit/049665337eb606bc011cdfa9b66f30c7be89000c))
- Prompt user to run diagnostics in AUR install and auto-run diagnostics on gui startup ([4f3a89](https://github.com/versenilvis/clak/commit/4f3a8900b5c4d8e2b23ebd29e2e57eccce423786))

### Performance

- Optimize for input using draftjs ([cccb3c](https://github.com/versenilvis/clak/commit/cccb3c83bb542d3ce7e662dd38eb3937f59ab69b))

## [v0.4.1](https://github.com/versenilvis/clak/releases/tag/v0.4.1) - 2026-10-06

### Documentation

- Reformat ([7550ab](https://github.com/versenilvis/clak/commit/7550ab78b2c382c64f306872353a234a414c9404))
- Settings menu and feedback info ([e37c90](https://github.com/versenilvis/clak/commit/e37c909c528b85ac83e9545791847af502d1e471))

### Performance

- Trim unused font weights, disable accessibility, and enable fat LTO ([23e9d6](https://github.com/versenilvis/clak/commit/23e9d697a703893ee2798ef7440e2dc3103fd09a))

## [v0.4.0](https://github.com/versenilvis/clak/releases/tag/v0.4.0) - 2026-10-06

### Features

- Automatically diagnose system issues ([#2](https://github.com/versenilvis/clak/issues/2)) ([915456](https://github.com/versenilvis/clak/commit/915456a48f984adfdeffa9425571e2e039c860f6))

### Performance

- Maximize production performance and optimize storage caching ([820aa3](https://github.com/versenilvis/clak/commit/820aa31076ba40a5092f27a9a3dfe39c5841d3b0))

## [v0.3.2](https://github.com/versenilvis/clak/releases/tag/v0.3.2) - 2026-10-05

## [v0.3.1](https://github.com/versenilvis/clak/releases/tag/v0.3.1) - 2026-10-05

### Bug fixes

- Eliminate timing flake in rapid selection deletion test ([d6eb2a](https://github.com/versenilvis/clak/commit/d6eb2a7c7fe5302d605b637fed3b8dc1390745b6))

### Documentation

- Refine Clak feature descriptions in README ([c4ebef](https://github.com/versenilvis/clak/commit/c4ebef4bdfaa8090274985726e02bc3f2218f0dd))

### Features

- Support jetbrains apps ([f9b6db](https://github.com/versenilvis/clak/commit/f9b6db21067cb89f10d21105612e5f87b48022ac))

## [v0.3.0](https://github.com/versenilvis/clak/releases/tag/v0.3.0) - 2026-10-05

### Bug fixes

- Fix duplicate characters by restoring backspace release events and delay pacing ([270f1c](https://github.com/versenilvis/clak/commit/270f1c41e6d33577fafbd7dec0cbd7a9c791c1ae))
- Restore classic 950x700 floating window size ([a8a9b9](https://github.com/versenilvis/clak/commit/a8a9b9ba8998fa748fa16ce51194340d7af6e248))

### Documentation

- Small note ([91d3d8](https://github.com/versenilvis/clak/commit/91d3d8f6d876cd47448e1206c9b3cf2c02dd048a))
- Add image to uninstall ([9ae079](https://github.com/versenilvis/clak/commit/9ae0798878fd2656d6908b288a22e79f52610267))

## [v0.2.11](https://github.com/versenilvis/clak/releases/tag/v0.2.11) - 2026-10-05

### Bug fixes

- Using renderer-skia-opengl to render correct font ([f97b77](https://github.com/versenilvis/clak/commit/f97b776982c1c461cd7357a5d9cdd67f7cdc4449))

### Documentation

- Small note ([d2c22c](https://github.com/versenilvis/clak/commit/d2c22c83f507c77bfb04da67b00a0ac092fb16fd))

### Features

- Uninstall setting option ([5e0219](https://github.com/versenilvis/clak/commit/5e02193adc108aadeeae62cbf104d99c0ca9091b))

## [v0.2.10](https://github.com/versenilvis/clak/releases/tag/v0.2.10) - 2026-10-05

### Bug fixes

- Prevent word deletion on space and improve terminal compatibility ([#1](https://github.com/versenilvis/clak/issues/1)) ([8055aa](https://github.com/versenilvis/clak/commit/8055aa608f383df91ee533713c99f8229fdfd5fc))

## [v0.2.9](https://github.com/versenilvis/clak/releases/tag/v0.2.9) - 2026-10-05

### Bug fixes

- Resolve installer hangs, dynamic download size, and optimize install flow ([8b7325](https://github.com/versenilvis/clak/commit/8b7325670cd5126e14a973211258078f4341f2ea))
- Always restore original fcitx5 config backup on uninstall ([927c5e](https://github.com/versenilvis/clak/commit/927c5ec8e651f387639335886051de697a454d09))
- Resolve ubuntu compatibility, desktop path, and multiarch paths ([9855fa](https://github.com/versenilvis/clak/commit/9855fa32d787e1957ac25c8ee48e2f9f04d030dd))
- Enable window resizing, fix sidebar clipping and add tab scrollview ([e7ee95](https://github.com/versenilvis/clak/commit/e7ee9592de04a5200e8df68b789c3ac1bc95d0ab))
- Adjust default floating window size to 880x560 ([2e49c4](https://github.com/versenilvis/clak/commit/2e49c47000a5bd0ee2ab98f4347903c664f5de13))

### Features

- Prompt user to install fcitx5 if missing and stop if declined ([f42173](https://github.com/versenilvis/clak/commit/f42173df2d12b48d1276435376da0b8b575f7d8a))

### Refactors

- Apply safe installer enhancements and avoid invasive system changes ([1b293f](https://github.com/versenilvis/clak/commit/1b293f9b18a3dbc5417a14393e445b1960eb7c54))
- Remove dead pkg_cmd variable and streamline ensure_fcitx5 ([4f291a](https://github.com/versenilvis/clak/commit/4f291a83d1af5820764b61ad97e239605677097c))

## [v0.2.9](https://github.com/versenilvis/clak/releases/tag/v0.2.9) - 2026-10-05

### Bug fixes

- Resolve installer hangs, dynamic download size, and optimize install flow ([8b7325](https://github.com/versenilvis/clak/commit/8b7325670cd5126e14a973211258078f4341f2ea))
- Always restore original fcitx5 config backup on uninstall ([927c5e](https://github.com/versenilvis/clak/commit/927c5ec8e651f387639335886051de697a454d09))
- Resolve ubuntu compatibility, desktop path, and multiarch paths ([9855fa](https://github.com/versenilvis/clak/commit/9855fa32d787e1957ac25c8ee48e2f9f04d030dd))

### Features

- Prompt user to install fcitx5 if missing and stop if declined ([f42173](https://github.com/versenilvis/clak/commit/f42173df2d12b48d1276435376da0b8b575f7d8a))

### Refactors

- Apply safe installer enhancements and avoid invasive system changes ([1b293f](https://github.com/versenilvis/clak/commit/1b293f9b18a3dbc5417a14393e445b1960eb7c54))
- Remove dead pkg_cmd variable and streamline ensure_fcitx5 ([4f291a](https://github.com/versenilvis/clak/commit/4f291a83d1af5820764b61ad97e239605677097c))

## [v0.2.8](https://github.com/versenilvis/clak/releases/tag/v0.2.8) - 2026-10-04

### Bug fixes

- Preserve wayland environment on fcitx5 reload and enable local configuration ([eb18b6](https://github.com/versenilvis/clak/commit/eb18b61a3646af023d6d35e2b9bddf941d05d8ae))
- Import sf pro text font and increase feature title weight ([020c7a](https://github.com/versenilvis/clak/commit/020c7a3ddb016e89b61fd178fbbc48490d2f0866))

## [v0.2.7](https://github.com/versenilvis/clak/releases/tag/v0.2.7) - 2026-10-04

### Bug fixes

- Restore authentic clak logo icons with high contrast ([7e06d4](https://github.com/versenilvis/clak/commit/7e06d4ec6f1e3263e66d75c6b943723e340fcb75))

## [v0.2.6](https://github.com/versenilvis/clak/releases/tag/v0.2.6) - 2026-10-04

### Documentation

- Highlight recommendation to use only Clak with terminal colors ([d2c11a](https://github.com/versenilvis/clak/commit/d2c11a6f144601b485957a46b6d6b54051382c84))
- Sync notify-send message and guard tag existence ([e20a61](https://github.com/versenilvis/clak/commit/e20a61ecef8e0682833bb761687c945d9d93df5d))

### Features

- Show VI/EN state at input cursor and keep tray icon stable ([380e9a](https://github.com/versenilvis/clak/commit/380e9a57427db587639dcf7fed7d1d6f6246cf7a))

## [v0.2.5](https://github.com/versenilvis/clak/releases/tag/v0.2.5) - 2026-10-04

### Bug fixes

- Make icons white with black border and fix library path ([4bc5b5](https://github.com/versenilvis/clak/commit/4bc5b577863e422aa54519fb665336cc6ae96746))

## [v0.2.4](https://github.com/versenilvis/clak/releases/tag/v0.2.4) - 2026-10-04

### Bug fixes

- Refresh all icons to white contrast and update en mode indicator ([c8f7b1](https://github.com/versenilvis/clak/commit/c8f7b1153517b701259d19d06480b17626aa2ca0))
- Fix ctrl+shift toggle by preserving modifier state across reset ([ff8253](https://github.com/versenilvis/clak/commit/ff8253cf299838b7a677d7581e2df94d7dd3044a))

## [v0.2.3](https://github.com/versenilvis/clak/releases/tag/v0.2.3) - 2026-10-04

### Bug fixes

- Detect and clean residual fcitx5 profile and config in uninstall ([98f723](https://github.com/versenilvis/clak/commit/98f7238343c3286845c9449cf553fc01bfcb698c))
- Restart fcitx5 on uninstall to immediately flush tray icon ([95b416](https://github.com/versenilvis/clak/commit/95b41629e406b71c724c19c5920797365fd398e6))
- Use white fill for status icons and add post-install notification ([0511a1](https://github.com/versenilvis/clak/commit/0511a15a18b9377000756d8aefb4c4c32f012de8))
- Add subtle dark contrast border to status icons ([524e76](https://github.com/versenilvis/clak/commit/524e76a318b17f7ae745bbdc42a327ead62cff0a))

### Documentation

- Fix file path ([e62505](https://github.com/versenilvis/clak/commit/e62505cb4d8a07bde3612e1d8ea4006894efcb7e))
- Fix relative file paths from docs directory ([ed9f15](https://github.com/versenilvis/clak/commit/ed9f15129a61ae00ac777ecb22f75435d971504b))

### Features

- Add tag recipe for automated release bumping ([70ac42](https://github.com/versenilvis/clak/commit/70ac426a64c66931f29225b9db47afa7348d1223))
- Add clak-bin package and update readme ([213681](https://github.com/versenilvis/clak/commit/213681f2306ce643ffafdcc7a60847d203f26cc5))

### Refactors

- Organize recipes into logical groups ([91ca79](https://github.com/versenilvis/clak/commit/91ca793661c557db4d077deae1acd2d3840c09cd))

## [v0.2.3](https://github.com/versenilvis/clak/releases/tag/v0.2.3) - 2026-10-04

### Bug fixes

- Detect and clean residual fcitx5 profile and config in uninstall ([98f723](https://github.com/versenilvis/clak/commit/98f7238343c3286845c9449cf553fc01bfcb698c))
- Restart fcitx5 on uninstall to immediately flush tray icon ([95b416](https://github.com/versenilvis/clak/commit/95b41629e406b71c724c19c5920797365fd398e6))
- Use white fill for status icons and add post-install notification ([0511a1](https://github.com/versenilvis/clak/commit/0511a15a18b9377000756d8aefb4c4c32f012de8))
- Add subtle dark contrast border to status icons ([524e76](https://github.com/versenilvis/clak/commit/524e76a318b17f7ae745bbdc42a327ead62cff0a))

### Documentation

- Fix file path ([e62505](https://github.com/versenilvis/clak/commit/e62505cb4d8a07bde3612e1d8ea4006894efcb7e))
- Fix relative file paths from docs directory ([ed9f15](https://github.com/versenilvis/clak/commit/ed9f15129a61ae00ac777ecb22f75435d971504b))

### Features

- Add tag recipe for automated release bumping ([70ac42](https://github.com/versenilvis/clak/commit/70ac426a64c66931f29225b9db47afa7348d1223))
- Add clak-bin package and update readme ([213681](https://github.com/versenilvis/clak/commit/213681f2306ce643ffafdcc7a60847d203f26cc5))

### Refactors

- Organize recipes into logical groups ([91ca79](https://github.com/versenilvis/clak/commit/91ca793661c557db4d077deae1acd2d3840c09cd))

## [v0.2.2](https://github.com/versenilvis/clak/releases/tag/v0.2.2) - 2026-10-04

### Bug fixes

- Optimize package size and add missing app descriptions ([90d896](https://github.com/versenilvis/clak/commit/90d8966fd13e2f6d5bc6671373c16d7b56ff4284))

## [v0.2.1](https://github.com/versenilvis/clak/releases/tag/v0.2.1) - 2026-10-04

### Bug fixes

- Set _pkgname to ClakIME for archive root folder ([38c957](https://github.com/versenilvis/clak/commit/38c9578fcb1384bf67c89e8e0b90412578b6df65))

## [v0.2.0](https://github.com/versenilvis/clak/releases/tag/v0.2.0) - 2026-10-04

<div align="center">
  <img width="949" height="700" alt="image" src="https://github.com/user-attachments/assets/778c2e42-b4a5-46db-96df-2e1858077601" />
  
  <strong>NEW UI</strong>
  
</div>

### Bug fixes

- Add 50ms guard against false resets from web applications ([cadbb2](https://github.com/versenilvis/clak/commit/cadbb25c8680e8186dd03354ed916fec934f55b0))
- Handle rapid selection deletion and prevent buffering shortcut keys ([3999a3](https://github.com/versenilvis/clak/commit/3999a35d73d754a15e58de6288c35d227ce16980))
- Default excluded apps and macros to empty ([42cf4a](https://github.com/versenilvis/clak/commit/42cf4ac8161d2ebcff000b4f9026712d3e3604b7))
- Guard against stale selection in browser autocomplete detection ([751bdc](https://github.com/versenilvis/clak/commit/751bdc4e9cf138d2d1ca5a2bc570d13fbfc0882b))
- Resolve autostart disable dispatch in clak ([bde478](https://github.com/versenilvis/clak/commit/bde47837436fa705a89d0585341f090c5550019d))
- Correct clak_config_free return type to void in core.h ([bd677c](https://github.com/versenilvis/clak/commit/bd677c2d5fbfa00f90879b185d3f046439100586))
- Clean environment conf when disabling autostart ([3fbe4a](https://github.com/versenilvis/clak/commit/3fbe4a77cd70aa0dab5ed325ce3d4ae38c48b15f))
- Bound per-app state map to prevent unbounded memory growth ([3645af](https://github.com/versenilvis/clak/commit/3645af6306c9582950fce02d18f6b27131a9b100))

### Documentation

- Update installing guide ([8b671c](https://github.com/versenilvis/clak/commit/8b671c2efc3d529844ba7a2d1d4bf1d6a0996f92))
- Add wps proof ([da784b](https://github.com/versenilvis/clak/commit/da784b1558acbbe0ae874761473597d85cfdfaa3))
- Add comprehensive test documentation with test case tables ([0d1870](https://github.com/versenilvis/clak/commit/0d18708e917d25bbcf4c580107e94f728681f351))
- Enrich technical documentation with architecture diagram, invariants, dispatch paths, and API surface ([3fe2b8](https://github.com/versenilvis/clak/commit/3fe2b81fd977e37e852ba4fa15ae7feedb4aa582))
- Adjust capitalization to natural sentence casing across docs ([c42d84](https://github.com/versenilvis/clak/commit/c42d845e07b9f7bd837e13b0c4e38a3f79951894))
- Add api.md reference manual covering C-FFI surface and data structures ([782322](https://github.com/versenilvis/clak/commit/7823224ec690e326b8a494661ac68f5922724598))
- Replace absolute file URI links with relative repository paths ([695e93](https://github.com/versenilvis/clak/commit/695e93636f21a3acc2b7fd9a0ed8879166be5aff))

### Features

- Add configuration system, inotify reload, and shortcuts ([7f5830](https://github.com/versenilvis/clak/commit/7f58301f0b1c73967798ce8e449d9ee784dba9ca))
- Add clak bench CLI with group filtering and p99 regression assertion ([740950](https://github.com/versenilvis/clak/commit/74095031540ad10db5dbb1535b4739f8675c0c72))
- Reset composition on mouse click via libinput tracker ([6e8660](https://github.com/versenilvis/clak/commit/6e8660b79fa5875c311b61efa812b02ed45b33d9))
- Add clak doctor environment diagnostic utility and just recipe ([c92886](https://github.com/versenilvis/clak/commit/c9288698e428601daea3f3055f5f86f5365a4963))
- Add standalone uninstall script ([bc12ed](https://github.com/versenilvis/clak/commit/bc12ed2900b4034516d64f4b146a75367e95b5ee))
- Add wps office compatibility detection, wrappers, and benchmark tooling ([9ec566](https://github.com/versenilvis/clak/commit/9ec566467d612230218c79eac6b81d361d99c016))
- Add autostart management and update configuration ([857321](https://github.com/versenilvis/clak/commit/8573215be42a9e775821b81908bc5ac14e70ee54))
- Add autostart management subcommand and doctor check ([bbdc3c](https://github.com/versenilvis/clak/commit/bbdc3c80c66e1ea865a3ab89340c8f0b4f13ba25))
- Configure autostart on install and cleanup on uninstall ([4f9a39](https://github.com/versenilvis/clak/commit/4f9a3945abe124e116596badf3bc1d3501520822))
- Support macro expansion with case matching and enter tab triggers ([071485](https://github.com/versenilvis/clak/commit/071485ed8fab3a82f215fa076d39c474f8cbe339))
- Separate safety timeout into fast 50ms default and 250ms selection deletion ([016bc1](https://github.com/versenilvis/clak/commit/016bc15055b8fcd3037eba5cf9d79930f559be66))
- Implement auto capitalize after sentence boundary ([c6beaa](https://github.com/versenilvis/clak/commit/c6beaa98dd57ff43c28ace50dcc3c2dfa9a9031c))
- Add adaptive wait latency tracking for dynamic safety recovery ([0cf313](https://github.com/versenilvis/clak/commit/0cf31306af6a697415799f1f5b96e9a4ed4292de))
- Support fallback to fcitx program and detect terminal editor processes on gnome ([74252d](https://github.com/versenilvis/clak/commit/74252d5ef610d08ca56f43f72434ac652ea9e8c5))
- Add clak-gui Slint configuration application ([175073](https://github.com/versenilvis/clak/commit/175073131401d541e8229e29104153ddc333676c))
- Add settings action to tray icon menu ([abdbca](https://github.com/versenilvis/clak/commit/abdbcaa423f05a20f16f46b5bb1601d26c160c32))

### Performance

- Narrow mutex lock to emission and release during thread sleeps ([364888](https://github.com/versenilvis/clak/commit/364888955b3953bdee113426ae05bf0a432bbb87))

### Refactors

- Rename bin to cli ([3d1ae8](https://github.com/versenilvis/clak/commit/3d1ae8b07321f11ae10d41dcb2e119f184279952))

### Security

- Auto vuln scan ([226ade](https://github.com/versenilvis/clak/commit/226ade324032876558738d52d1567491f5397813))
- Verify sha256 checksums before archive extraction ([95c19c](https://github.com/versenilvis/clak/commit/95c19c156b8e9718110b4bbf8bd4da45b2fb3e06))

## [v0.1.0](https://github.com/versenilvis/clak/releases/tag/v0.1.0) - 2026-10-02

### Bug fixes

- Detect wayland and gtk4 terminals ([d648d1](https://github.com/versenilvis/clak/commit/d648d188d17f8fec4a567e27f3148e30d761a7a5))
- Wrong cursor detector ([41e410](https://github.com/versenilvis/clak/commit/41e4101734f87b3c91a7c0a488c9c194c00c7448))
- Accidentally triggered wrong flag ([298504](https://github.com/versenilvis/clak/commit/2985048d09bc8b7f02b0f2cd1a89f4d9bebe0ae7))
- Fix typing over selection and clippy ([b63766](https://github.com/versenilvis/clak/commit/b6376625f76c1f9d0c6ec752fcc55e8d02dbdbae))
- Set cargoRoot for engine ([cea1a3](https://github.com/versenilvis/clak/commit/cea1a3f1fa3a139d5edd71239f83f8ec396f970e))

### Documentation

- Telegram and neovim ([979166](https://github.com/versenilvis/clak/commit/9791668314056bb9bc18531ebc76eb4d2ee32d57))
- Removed test message ([02f68e](https://github.com/versenilvis/clak/commit/02f68e04b1c4ad295b1b77f70d8f2d58a5277bf6))
- Fix neovim format ([ae3159](https://github.com/versenilvis/clak/commit/ae3159f134c4b896171e560e5f9b4511b0b6245e))
- Document cursor near word and why surrounding over uinput ([4a4d0d](https://github.com/versenilvis/clak/commit/4a4d0d6008869a92cf6e670b01fac41d986eaeb8))
- Add vscode family pacing and xterm.js explanation ([92fe5c](https://github.com/versenilvis/clak/commit/92fe5c5d1744012485585c82f0ff6877f63439fc))

### Features

- Clak input method ([456376](https://github.com/versenilvis/clak/commit/4563768f17a774cde88597532dd2f678d4e97c90))
- Cursor detector ([982ba6](https://github.com/versenilvis/clak/commit/982ba676cd07b75888e9df223f1ef2135a67f3ad))
- Detect vscode family ([7ed102](https://github.com/versenilvis/clak/commit/7ed102f204e65de8ff28cd92ac8d58f5c60c008e))
- Install and update ([4650f3](https://github.com/versenilvis/clak/commit/4650f3e3ba0ff01049679c3c5d0c72ee9e3fd0db))
