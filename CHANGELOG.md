## swyh-rs Changelog

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
