## swyh-rs Changelog

- 1.2.3 (unreleased)
  - Removed the "SeekId" action from OpenHome control, as it is not needed and interferes with AutoResume on some renderers (Bubble) 

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
  (*__Note__: I had to use a patched fork of tiny_http to let this work, as per [this pull request](https://github.com/tiny-http/tiny-http/pull/183)*)

- 1.0.8 (Nov 27 2020 dheijl)
  - switch to parking_lot Mutex and Once, and use Ninja-Build for fltk to speed up CMake in the fltk build
  
- 1.0.7 (Nov 19 2020 dheijl)
  - upgrade to rustc 1.48, fltk-rs 0.10.11, and some small code improvements 

- 1.0.6 (Nov 17 2020 dheijl)
  - implement autoconnect to the last used renderer on startup (https://github.com/dheijl/swyh-rs/issues/19)

- 1.0.5 (Nov 17 2020 dheijl)
  - various code improvements offered by @Boscop (https://github.com/dheijl/swyh-rs/issues/22)

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
