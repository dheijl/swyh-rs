# swyh-rs Changelog

- 1.12.14 (unreleased)

- 1.12.13 (Aug 30 2025, dheijl)
  - binaries are now built with lld as the linker using **.cargo/config.toml**:
    - on Windows :
    `[target.x86_64-pc-windows-msvc]`
    `rustflags = ["-C", "link-arg=-fuse-ld=lld"]`
    - on Linux:
    `[target.x86_64-unknown-linux-gnu]`
    `rustflags = ["-C", "link-arg=-fuse-ld=lld"]`
  - Fix for issue #221 by @DrCWO: when an RDP client (dis)connects while audio is being captured, the capture is aborted by Windows. Try to restart audio capture from the same device if it still exists. The RDP client should have audio disabled to make this work. Caveat: if you have multiple sound devices with the same name this will always pick the first one, as the sound index may have changed.

- 1.12.12 (Aug 25 2025, dheijl)
  - major refactoring of http streaming server
  - rust 1.89
  - use [ecow](https://github.com/typst/ecow) EcoString for the small strings that get cloned frequently
  - replace [strfmt](https://github.com/vitiral/strfmt) with [figura](https://github.com/saverioscagnoli/figura) for dynamic string formatting
  - improve log time display (rfc 2822 date + time in localtime)
  - also log CLI arguments
  - improve some messages
  - CLI: fix sound source index not being picked up from config if CLI argument missing (#219), and not always being stored correctly if present

- 1.12.11 (Aug 1 2025 dheijl)
  - more small refactorings
  - remove the "_Accept-Ranges: none_" HTTP header, as **MPD** does not honour the "none" value but uses ranges anyway, causing extra GET requests when using WAV or RF64. Omitting the header makes MPD behave normally. MPD behaviour observed with Moode 9.x.

- 1.12.10 (Jul 6 2025 dheijl)
  - various small refactorings
  - add the possibility to hide a player by right-clicking it's button, and unhide all hidden players by right-clicking the "UPNP rendering devices" label. Handy if you have a lot of Sonos (grouped) players, so that you do not accidentally click the wrong one breaking a stereo pair.

- 1.12.9 (Jun 25 dheijl)
  - some more refactoring
  - don't panic when unable to inject silence (issue #203)

- 1.12.8 (Jun 19 2025 dheijl)
  - rework theming and some other refactoring
  - rename the default branch to main instead of master
  - fix volume syncing (issue #202 by @dutchbreeze)

- 1.12.7 (May 9 2025 dheijl)
  - replace deprecated Stringreader with as_bytes for parsing soap xml with xml-rs
  - improve volume sync logic: only sync active players (issue #196 by @dutchbreeze)

- 1.12.6 (Apr 9 2025 dheijl)
  - get rid of the annoying restart dialog pop-up, it is replaced by a restart button that only shows when config change needs a restart to apply the changes. See also issue #171.

- 1.12.5 (Apr 4 2025 dheijl)
  - add tray icon Python code contributed by @phil2sat
  - sync volume sliders by using shift key while dragging (issue #185 by @dutchbreeze)

- 1.12.4 (Feb 20 2025 dheijl)
  - migrate ureq to version 3.0 api (with connection pooling/reuse), and give Renderer an ureq::Agent to maximize connection reuse
  - Linux: fix the nice value at -10, and document that the pipewire group has the necessary privilege to raise priority.
  - add version to HTTP User-Agent string
  - replace parking_lot RwLock with std RwLock
  - some small optimizations
  - rust 1.85, edition 2024
  
- 1.12.3 (Nov 28 2024 dheijl)
  - support multiple players at the same IP address and port (e.g. Bubble UPNP exposing multiple Chromecast devices), see issue #157. Players are no longer identified by their IP address but by their SSDP "Location".
  - built with Rust 1.83.0

- 1.12.2 (Nov 25 2024 dheijl)
  - properly handle Bubble UPNP SSDP response

- 1.12.0 (Nov 6 2024 dheijl)
  - Github CI added by @theavege, thanks!
  - replace ```once_cell::sync::Lazy``` with ```std::sync::LazyLock```, so now swyh-rs requires Rustc version **1.80** or later!!
  - swyh-rs now is compiled with Rustc 1.82.0
  - get rid of a couple of unwraps
  - update flac-bound to 0.5.0 so that
    - libflac-sys (0.3.1) and libflac (1.4.3) are now the current versions
    - swyh-rs can now use the new ```set_limit_min_bitrate()``` for the flac encoder to prevent connection loss when streaming/injecting silence.
    - This enables the "inject silence" option to work for FLAC too. Enabling the inject silence option also automatically disables the old FLAC faint white noise injection that swyh-rs inserted when no sound is being captured. Downside: silence injection will introduce a delay when sound is resumed.

- 1.11.6 (Sep 30 2024 dheijl)
  - CLI: optimize player argument parsing (-o)
  - CLI: bugfix when multiple players specified
  - CLI: bugfix in ^C handling: ask all players to stop instead of only the default player, and check that they are still playing before doing that.
  - replace some "if else if..." chains with match guards

- 1.11.5 (Sep 20 2024 dheijl)
  - replace reading the CONFIG value (rwlock!) for every captured audio frame with a simple AtomicBool read
  - CLI: allow to use the device name(s) instead of the IP address(es) with -o. A substring unique to a device name is also OK (case sensitive) (see issue #141 by @boydfields).
  - CLI: add a warning to the Readme that you should not stream to both speakers of a Sonos stereo pair, only to the master speaker, otherwise you could break the stereo pair.

- 1.11.4 (Sep 4 2024, dheijl)
  - more tweaks filling the streaming buffers:
    - replace the vec_deque pop_front() with a drain iterator
    - replace all indexing in the streaming buffers with iterators, fill the buffers "in one shot" without jumping, and zip the 2 iterators
  - show the audio capture sample format when the capture starts

- 1.11.3 (Aug 30 2024 dheijl)
  - some tweaks:
    - avoid unnecessary allocations
    - rewrite how samples are serialized in the streaming buffer, it's safer, cleaner and should be faster too

- 1.11.2 (Aug 18 2024 dheijl)
  - clean up log initializing mess
  - handle flac encoding interruption better

- 1.11.1 (Aug 16 2024 dheijl)
  - Rust 1.80.1
  - merge PR #139 by @Villardo that adds FLTK Theme support
  - add possibility to restore the theme to FLTK defaults (thanks @MoAlyousef for adding this feature to fltk-theme)
  - make theme change persistent in the config file

- 1.11.0 (Aug 10 2024 dheijl)
  - GUI: replaced the FLTK callback recursion mutex (for the choice widget popups) with an AtomicBool. See issue #10 (panic while panicking in the choice callback), but I don't know if the recursion check is still actually needed with the current FLTK version, I never could reproduce the recursion problem.
  - optimize CONFIG locking somewhat
  - make all fltk callbacks consistently follow the same pattern
  - GUI: delay logging until logging properly initialized
  - update innosetup
  - replace rand by fastrand, as rand is overkill here, it's faster and also reduces dependencies
  - some clippy

- 1.10.10 (Jul 6 2024 dheijl)
  - CLI: sanitize ssdp interval argument
  - CLI: don't ignore renderers newly discovered by ssdp
  - Windows setup: install Microsoft VC++ runtime 17 (VS 2015..2022) if not already installed, as Rust binaries compiled with the MSVC toolchain depend on it (issue #137).
  - bugfix: if the local address of the PC changes for some reason (DHCP): ignore the configured old (and wrong) value and use the current default instead.
  - refactoring: replace the three inter-thread event channels by a single enum-based channel  

- 1.10.9 (May 25 2024 dheijl)
  - optimize flac near silence injection when no sound is being captured
  - GUI: you can now **disable ssdp discovery** by setting the ssdp interval to 0.0. Note that the minimum ssdp interval if enabled stays at 0.5 (half a minute). This is equivalent to the "-x" (serve only) option in the CLI.

- 1.10.8 (May 23 2024 dheijl)
  - some code cleanup:
    - remove unused config value "use_wave_format"
    - autoreconnect now defaults to false on a fresh installation
    - GUI: changing streaming format or bits per sample no longer ask for a restart. If you change the streaming format, don't forget to change the streamsize to a working value too!

- 1.10.7 (May 22 2024 dheijl)
  - CLI: don't set autoreconnect to true if serve-only (-x) is set, it's meaningless
  - GUI: changing the StreamSize doesn't need a restart, new connections simply use the new value, active connections are not affected
  - internet radio URL: query parameters bd (bit depth) and ss (streamsize) allow to override any configured values, for example
    - `/stream/swyh.flac?bd=24&ss=nonechunked`
    - `/stream/swyh.wav?bd=16&ss=u32maxnotchunked`
    useful for CLI and/or when you don't use SSDP and the buttons in the GUI

- 1.10.6 (May 18 2024 dheijl)
  - make the default streamsize for FLAC NoneChunked
  - remove the NOISE feature flag: this enables the white noise injection for FLAC if no sound while streaming. Only works if you do **NOT**** enable "Inject Silence".
  - switch to dasp_sample for FLAC and i24

- 1.10.5 (May 9 2024 dheijl)
  - CLI: the -o option now accepts multiple player ip addresses seperated by a comma. Ignored in serve-only (-x) mode.
  - use hashbrown HashMap where possible
  - don't blow up memory if the streaming stalls, but discard samples
  - add a configurable "up-front" samples buffering before streaming actually starts, it may help to prevent stuttering with some configurations
  - fix resizing by using fltk Flex instead of Pack

- 1.10.4 (Apr 23 2024 dheijl)
  - GUI: autoreconnect now works for multiple players. When you close swyh-rs (GUI) any players still active will be remembered in the config before they are stopped. This should fix issue #129. CLI is unaffected by this change.

- 1.10.3 (Apr 8 2024 dheijl)
  - GUI: try to stop any players still streaming when the swyh-rs windows is closed. If the players don't stop streaming voluntarily exit anyway (after a 5 second time-out).

- 1.10.2 (Apr 8 2024 dheijl)
  - clip player button text

- 1.10.1 (Mar 27 2024 dheijl)
  - CLI: add ordered shutdown on CTRL-C, by telling the streamer to stop before exiting. This should fix #125.

- 1.10.0 (Mar 17 2024 dheijl)
  - CLI: allow to specify a sound source name instead of an index, it can be substring of the name, if you have duplicates append :n to the name where n is a zero-based index
  - PR #118 by @ein-shved:
    - clean up configuration.rs
    - add possibility to specify a configuration file on the commandline
    - CLI: changes to make it easier to run swyh-rs-cli as a service by @ein-shved
      - add possibility to use a minimal configuration and to make the configuration file read-only
      - add nix build files and nix module to run swyh-rs-cli as a service
      - use the local-ip-address crate so that you don't need a "dummy" internet access to Google DNS to locate the default interface for SSDP
      - new commandline switch -C (--configfile) to specify an alternative pathname of the configfile
      - switch to serve mode if no player specified

- 1.9.9 (Feb 29 2024 dheijl)
  - fix test compilation (PR #116, thanks @ein-shved)
  - add nix build recipes from @ein-shved, PR #117
  - add streamsize/chunking dropdown to swyh-rs GUI and cli argument to swyh-rs-cli. Allows one to choose a combination of streaming format an streamsize that works. See also issue #120.

- 1.9.8 (Feb 13 2024 dheijl)
  - support setting volume in swyh-rs-cli too if the renderer supports it (-v xx or --volume n, where n between 0 and 100)
  - optimize release builds (lto = "thin" and codegen-units = 1)

- 1.9.7 (Feb 8 2024 dheijl)
  - fix bug in getting/setting Sonos volume

- 1.9.6 (Feb 5 2024 dheijl)
  - don't show the volume sliders if Get/SetVolume does not work, like with recent Sonos firmware (#115)

- 1.9.5 (Feb 4 2024 dheijl)
  - add Volume Control Sliders as requested in issue #113
  - if you wish you can now compile swyh-rs with the "NOISE" feature for FLAC, where a very faint noise is sent instead of silence to keep the connection alive if there is no sound (FLAC compresses the silence away)

- 1.9.4 (Dec 4 2023 dheijl)
  - more clippy
  - introduce cargo build features "cli" and "gui", needed to build swyh-rs-cli without pulling in fltk-rs and its dependencies, and to build swyh-rs without the cli specific code. Also see the updated build information in the wiki.
  - document that recent Sonos firmware now supports FLAC format too. It solves the stuttering problem that can happen when using WAV format on some networks.
  - CLI: config sound source was ignored

- 1.9.3 (Nov 11 2023 dheijl)
  - some clippy recommendations
  - when swyh-rs or swyh-rs-cli are used as an internet radio, the URL used by the client now selects the streaming format, independent of the configured values:
    - /stream/swyh.raw => LPCM 16 bit
    - /stream/swyh.flac => FLAC 24 bit
    - /stream/swyh.wav => LPCM 16 bit with a WAV header
    - /stream/swyh.rf64 => LPCM 16 bit with a WAV/RF64 header  

- 1.9.2 (Nov 3 2023 dheijl)
  - some optimizations, use more iterators instead of loops, ...
  - cli argument "autoreconnect" removed, it's de facto **ON**
  - correct RMS value calculation
  - issue #111: introduce a new -x (--serve_only) commandline switch for the cli. If -x is present no SSDP discovery is run and playing is not started. swyh-rs-cli just sits there waiting for play requests from renderers. Some other useful options in this use case are: -f (format), -b (bits) and -s (sound source).
  - CLI: boolean options no longer need an argument, absent means false, present means true. You can still use false to disable options stored in the config. The options -h, -n and -x are not stored in the config file.

- 1.9.1 (Oct 19 2023 dheijl)
  - use WAV didl protocol info for RF64 too (instead of LPCM), should be compatible
  - add RF64 format to CLI binary too
  - use Wave HTTP header for RF64 too

- 1.9.0 (Oct 18 2023 dheijl)
  - some small fixes (cli and WAV format)
  - add support for **RF64** format, as it removes the 4 GB WAV limitation. All formats except WAV no longer have limits on the stream size.

- 1.8.7 (Oct 14 2023 dheijl)
  - a fix for LPCM (raw) audio format on Moode Audio Player by letting the URL file extension reflect the audio type.
  - make the WAV format header more correct/compatible. Note that MPD (ffmpeg/wav plugin) tries to use HTTP ranges (to parse the WAV header) which are unsupported and this leads to an extra HTTP request but it still plays the WAV.
  - reduce the HTTP response contentlength header from u64::MAX to u32::MAX. If this makes play stop after some 6 hours just enable autoresume.

- 1.8.6 (Oct 9 2023 dheijl)
  - make sure that http-tiny does not use chunking this time

- 1.8.5 (Oct 4 2023 dheijl)
  - remove "chunked transfer" config option and associated code, it's considered useless and removed from HTTP 2 anyway

- 1.8.4 (Sep 9 2023 dheijl)
  - config: make sure SoundCardIndex defaults to 0 instead of None to prevent accidentally selecting the wrong device when there are duplicate names (issue #107)

- 1.8.3 (Jul 7 2023 dheijl & joshuamegnauth54)
  - log architecture and OS environment
  - remove unnecessary thread for silence injector
  - expose the hitherto hidden "inject silence" configuration flag in the UI and in the cli commandline

- 1.8.2 (Jun 26 2023 dheijl)
  - cli: handle player ip not found (use first renderer)
  - Merge pull request #96 from joshuamegnauth54/cache_device_name:
    - get rid of some unwraps preventing possible panics
    - cache cpal sound device info
  - fix for issue #99: don't use Openhome Playlist for QPlay devices, use AVTransport instead
  
- 1.8.1 (May 6, dheijl and Joshua Megnauth @joshuamegnauth54)
  - make input devices too available for streaming, see PR #95
  - swyh-rs-cli: add a "-n" (--no-run) option. It enables a "dry-run" mode: the app exits where it would normally start streaming. Allows you to get the index of the sound sources and the ip addresses of the streamers that you need to pass as command line paremeters.

- 1.7.1 (Apr 26 2023 dheijl)
  - bugfix: update in memory shared config instance for other threads
  
- 1.7.0 (Apr 26 2023 dheijl)
  - fix shaky silence buffer generation
  - update dependencies, update rust to 1.69
  - upgrade bitflags to 2.x
  - split into a GUI binary and a new CLI binary (see issue #93)
  
- 1.6.1 (Feb 28 2023 dheijl)
  - changed SSDP interval default from 1 to 10 minutes
  - changed chunked transfer default from true to false
  - upgrade to rust 1.67.1
  - upgrade lexopt to latest version
  - upgrade Cpal to 0.15 & dasp_sample
  - clippy fixes
  - cope with Yamaha WXAD-10 having an invalid UrlBase port number in the service description (issue #89)

- 1.6.0 (Nov 6 2022 dheijl)
  - migrated from winapi to windows-rs (following cpal)
  - set the SSDP socket TTL to 2 seconds per UPNP spec
  - updated Readme mentioning that HTTP port 5901 must be open for incoming streaming requests
  - do not panck on an invalid configuration file at startup, but replace it with a new default one

- 1.5.1 (Oct 16 2022 dheijl)
  - added the possibility of having multiple configurations. This allows you to run multiple instances of swyh-rs (using an optional commandline switch:  -c  config_id or --configuration config_id), where each configuration can use a different audio source. Suggested by @cavadias, see issue #82. Each configuration gets its own config file and log file in the .swyh-rs folder in your HOME directory.
  - removed the delay when starting the streaming server as it can interfere with autoreconnect.

- 1.4.6-beta (unreleased)
  - appimage for Ubuntu 20.04 LTS and later

- 1.4.5 (Sep 8 2022 dheijl)
  - fix for pausing music with Sonos causing the Sonos to close the connection. This optionally injects silence at the music source, contributed by @genekellyjr (see issue #71), with a new "InjectSilence" boolean flag in the config.toml (not exposed in the GUI). For this to work you have to
    - check that swyh-rs uses the same output as your music source in the Windows soundmixer
    - edit your config.toml and change the InjectSilence flag from _false_ to _true_
  - flt-sys 1.3.14 builds again on Windows with MSVC, so we no longer need to use the _fltk-bundled_ feature

- 1.4.4 (Sep 1 2022 dheijl)
  - handle duplicate sound card names by storing the index too (solves issue #70)
  - make the CaptureTimeout for LPCM/WAV configurable in the config.toml, with a default of 2000 msec (as it was hardcoded before). If no sound is captured for a CaptureTimeout period, a block of slience of (CaptureTimeout / 4) msec length is sent to the receiver (was previously 250 msec hardcoded).  
  - for some reason I can no longer compile fltk on Windows with MSVC, so fltk-bundled is used for now on Windows

- 1.4.3 (Aug 3 2022 dheijl)
  - update flac-bound to official 0.3.0
  - implement "silence" sending for FLAC too, but it introduces a considerable delay due to FLAC compressing silence so well :) (issue #65), so disable this feature altogether for now

- 1.4.2 (July 18 2022 dheijl)
  - use latest flac-bound git master to build libflac-sys without OGG
  
- 1.4.1 (July 15 2022 dheijl)
  - some code cleanup and comments, and document that libflac-sys does not build on 32 bit, so no more 32 bit support
  - small ui change

- 1.4.0 (July 12 2022 dheijl)
  - add 16 bit and 24 bit FLAC support, using Flac-bound and libflac-sys
  
- 1.3.26 (June 7 2022 dheijl)
  - Fix possible exposure to CVE-2021-45707 and CVE-2022-24713 by replacing ifcfg crate with if_addrs crate.

- 1.3.25 (May 4 2022 dheijl)
  - Fix broken AVTransport (again), fixes issue #59

- 1.3.24 (April 20 2022 dheijl)
  - refactor rendering control code (pull up common OH and AV play template generation)  
  - explicit stop playing for Openhome renderers too before starting play, Moode needs it

- 1.3.23 (Feb 22 2022 dheijl)
  - fix the broken AV transport "SetAVTransportUri" DIDL-Lite template, the error was introduced with 1.3.20. Thanks again @MX10-AC2N.

- 1.3.22 (Feb 20 2022 dheijl)
  - dependency updates

- 1.3.21 (Dec 8 2021 dheijl)
  - get rid of all remaining traces of Range Headers (Linn) code  
  - fix panic when reading config after upgrade from 1.3.12 or earlier (thanks @FinalSh4re)

- 1.3.20 (Nov 24 2021 dheijl)
  - (experimental) 24 bit LPCM (audio/L24) support
  - get rid of the ini file format in favour of toml, so that I can use serde (de)serialization instead of reading and writing individual values
  - automatically migrate an exisiting config.ini to config.toml
  - update to Rust edition 2021
  - wait for the first SSDP discovery to complete before starting the streaming server
  - disable the terminal logger on Windows release build, as it panics with Rust 2021
  - add an "Accept-Ranges : none" header to HTTP responses as HTTP ranges (Linn!) are not supported
  - update dependencies

- 1.3.19 (July 6 2021 dheijl)
  - rearrange UI
  - bugfix: forgot to save the new last_network config value on first start

- 1.3.18 (July 2 2021 dheijl)
  - fix button insert position

- 1.3.17 (July 2 2021 dheijl)
  - log streaming request headers in debug log ([issue #40](https://github.com/dheijl/swyh-rs/issues/40))
  - add buildall script and 32-bit Windows build
  - add option to select the network interface (IPV4) to use and save it in the config

- 1.3.16 (May 16 2021 dheijl)
  - remove simultaneous streaming limit and reduce thread count
  - fix renderer button header and button index position

- 1.3.14 (Apr 28 2021 dheijl)
  - upgrade to fltk-rs 1.x
  - include Ubuntu (Mint 20.1) binary in release

- 1.3.13 (Apr 13 2021 dheijl)
  - update SimpleLog
  - add configurable HTTP listener port number

- 1.3.12 (Mar 23 2021 dheijl)
  - latest icon versions by @numanair

- 1.3.11 (Mar 21 20121 dheijl)
  - note-only icon for smaller icon sizes designed by @numanair

- 1.3.10 (Mar 16 2021 dheijl)
  - added icon designed by @numanair

- 1.3.9 (Mar 14 2021 dheijl)
  - clear rms meter widget values when checkbox is (un)set
  - restructure more code into modules (ui, audio), and some refactoring

- 1.3.8 (Feb 27 2021 dheijl)
  - show left and right channel RMS values

- 1.3.7 (Feb 25 2021 dheijl)
  - use ParkingLot RwLock instead of Mutex since most accesses of the locks (CLIENTS, CONFIG) are read anyway
  - clean-up configuration code
  - upgrade to rustc 1.50

- 1.3.6 (Feb 21 2021 dheijl)
  - migrate the configuration folder from `$HOME/swyh-rs` to `$HOME/.swyh-rs` so that it is hidden on Linux and comes before normal folders in Windows Explorer ([issue #32](https://github.com/dheijl/swyh-rs/issues/32))
  - add visual feedback (RMS value) for the audio capture
  - add InnoSetup Windows Setup, unsigned
  
- 1.3.5 (Feb 18 2021 dheijl)
  - changes for the new app::awake() in fltk-rs 0.14.0
  - deglob imports
  - optional support for WAV (audio/wma) file format of infinite length for renderers that do not support "naked" PCM

- 1.3.4 (Feb 03 2021 dheijl)
  - optimize GUI event loop with new fltk-rs app messages, decreasing CPU usage even more

- 1.3.3 (Jan 31 2021 dheijl)
  - remove redundant closures
  - better resizing with fltk-rs thanks @Moalyousef
  - use tiny-http crate instead of github repo (identity-encoding fix included)

- 1.3.2 (Jan 7 2020 dheijl)
  - prevent panics caused by changed ureq 2.0 error handling
  - implement a global configuration singleton (read once at startup) so that we don't have to reread it every time
  - cargo clippy
  - allow for multiple streaming connections to exist for the same renderer. This should finally fix the problems with Autoresume getting into a play/stop play loop with some renderers.

- 1.3.1 (Jan 6 2020 dheijl)
  - upgrade to rust 1.49
  - fix capture timeouts for Bubble with OpenHome Chromecast/Nest Audio

- 1.3.0 (Jan 4 2021 dheijl)
  - Removed the "SeekId" action from OpenHome control, as it is not needed and interferes with AutoResume on some renderers (Bubble)
  - adjusted the capture time-out to be smaller (15 sec) than the "no sound" time-out (30 sec) of BubbleUPNP Server
  - with the above changes Autoresume should now work reliably with OpenHome and Bubble UPNP Server
  - upgrade ureq to 2.0 (comes with breaking changes)

- 1.2.2 (Dec 29 2020 dheijl)
  - send continuous silence if no sound has been captured for 30 seconds to prevent connected renderers disconnecting
  - use official github tiny-http repo now that Equality_Reader is removed from Identity transfer

- 1.2.1 (Dec 17 2020 dheijl)
  - fix copy-and-paste bug when reading configuration file

- 1.2.0 (Dec 14 2020 dheijl)
  - slight GUI changes (BG color)
  - replace a couple of fltk handle2() events by callback2() events
  - some code cleanup

- 1.1.1 (Dec 7 2020 dheijl)
  - fix renderer button insert position

- 1.1.0 (Dec 7 2020 dheijl)
  - use good practice for Cargo.toml and Cargo.lock files (thanks @Boscop)
  - option to disable chunked transfer encoding in cases where the (AVTransport) renderer has problems with it

- 1.0.8 (Nov 27 2020 dheijl)
  - switch to parking_lot Mutex and Once, and use Ninja-Build for fltk to speed up CMake in the fltk build
  
- 1.0.7 (Nov 19 2020 dheijl)
  - upgrade to rustc 1.48, fltk-rs 0.10.11, and some small code improvements

- 1.0.6 (Nov 17 2020 dheijl)
  - implement autoconnect to the last used renderer on startup (<https://github.com/dheijl/swyh-rs/issues/19>)

- 1.0.5 (Nov 17 2020 dheijl)
  - various code improvements offered by @Boscop (<https://github.com/dheijl/swyh-rs/issues/22>)

- 1.0.4 (Nov 16 2020 dheijl)
  - bugfix for sample rate from default audio output device being advertised while sample rate of actual audio output device was used

- 1.0.3 (Nov 16 2020 dheijl)
  - SSDP now detects all OpenHome and DLNA renderers, but only uses the OpenHome device for devices that are capable of both
    - prevent panic in audio source chooser caused by vertical bar ("|") in audio source name, it too must be escaped for FLTK Menu_Item...

- 1.0.2 (Nov 15 2020 dheijl)
  - support for Chromecast as DLNA device defined in Bubble UPNP Server, thanks BubbleSoft for the assistance!

- 1.0.1 (Nov 14 2020 dheijl & MoAlyousef)
  - resizing is now usable (except for the horizontal scrollbar at the bottom that may get lost)
  - fix for '/' in the name of an output audio source

- 1.0.0 (Nov 11 2020 dheijl)
    enable windows resizing again, but it does not really work in FLTK, even when using Pack groups...

- 0.9.9  (Nov 11 2020 dheijl)
    disable resizing

- 0.9.8 (Nov 10 2020 dheijl)
    better handling of ssdp discovery change and restart button

- 0.9.7  (Nov 9 2020  dheijl)
    show a restart button after a configuration change that needs an application restart

- 0.9.6  (Nov 9 2020  dheijl)
    improve application start time

- 0.9.5  (Nov 8 2020  dheijl)
    make the SSDP discovery interval a configurable option

- 0.9.4  (Nov 6 2020  dheijl)
    simplify and unify SSDP discovery

- 0.9.3  (Oct 21 2020 dheijl)
    reduce network traffic during SSDP discovery for previously discovered renderers

- 0.9.2  (Oct 20 2020 dheijl)
    rerun SSDP discovery every minute, updating the renderers
