## swyh-rs Changelog

- 1.02  (Nov 15 2020 dheijl)
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
