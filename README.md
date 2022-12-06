# swyh-rs

![rs-tall](https://user-images.githubusercontent.com/2384545/112133026-8ae7c880-8bcb-11eb-835e-f6aaed25bea3.png)

## What is it

A "Stream-What-You-Hear" implementation written in Rust.

The current release is 1.5.1 with

- **FLAC** support (sorry but 64 bit binaries only as libflac-sys does not build on 32 bit)
- support for multiple identically named soundcards
- Sonos fix for pausing audio
- support for multiple configurations with a _-c_ commandline switch. Useful if you have multiple audiosources (suggestion by @cavadias).

**swyh-rs** implements the idea behind the original [SWYH](https://www.streamwhatyouhear.com) (source repo <https://github.com/StreamWhatYouHear/SWYH>) written in Rust.
It allows you to stream the music you're currently playing on your PC (Windows or Linux) to an UPNP/DLNA/OPenHome compatible music player (a "Renderer").

I wrote this because

- I wanted to learn Rust
- SWYH does not work on Linux
- SWYH did not work well with Volumio (push streaming did not work)
- SWYH has a substantial memory leak due to the use of an old and unmaintained Intel .Net UPNP/DLNA library it uses.

It has been tested with

- [Moode audio 8](https://moodeaudio.org/) (with Moode configured as UPNP renderer in _Openhome_ mode, and using FLAC or WAV format)
- [Volumio](https://volumio.org/)
- Harman Kardon AV network streamers (thanks @MX10-AC2N!)
- Denon Heos devices
- Sony AV streamers & Bravia TVs
- Chromecast devices defined as an OpenHome or DLNA device in Bubble UPNP Server (thanks Bubblesoft for providing the necessary information!)
- Sonos speakers/soundbars but only using the **WAV** format (thanks @Cunkers !). FLAC does _not_ work. Sonos is limited to 48KHz 16 bit anyway. If you want to pause music without losing the connection you have to change the **InjectSilence** flag to _true_ in your _{user_profile}/.swyh-rs/config.toml_ file. This flag is automatically added when you first start version 1.4.5 and defaults to _false_. Contributed by @genekellyjr, see issue #71.
- Kef Wireless LS50 II (thanks @Turbomortel via Twitter)
- Xbox 360, using Foobar2000 and entering the streaming url in foo_upnp (thanks @instinctualjealousy)
- iEast Audiocast M5 using the WAV format header (thanks @Katharsas)

but any OpenHome/DLNA streamer that supports FLAC (except Sonos) will probably work (since version 1.4.0).

If a device supports both OpenHome and DLNA, the OpenHome endpoint is used, and the DLNA AVTransport endpoint is ignored.

Music is streamed with the sample rate of the music source (the chosen audio output device, I personally use VBAudio HiFi Cable Input as a bit-perfect audio source).

Supported audio streaming formats:

- 16 bit or 24 bit **FLAC** (lossless compression, and I'm using the lowest compression level for performance and latency reasons). It is available since version 1.4.0
- audio/wav (16 bit) with an "infinite length" **WAV** header, available since version 1.3.5
- uncompressed 16 bit **LPCM** format (audio/l16)

Note that older libsndfile based renderers may not be able to decode the WAV format, because the stream is not "seekable".

Audio is captured using the excellent Rust [cpal library](https://github.com/RustAudio/cpal).
[fltk-rs](https://github.com/MoAlyousef/fltk-rs) is used for the GUI, as it's easy to use, and it's small, cross-platform, fast and works well.
For FLAC encoding the of use [flac-bound](https://github.com/nabijaczleweli/flac-bound) made adding FLAC encoding using the [libflac-sys Rust bindings](https://github.com/mgeier/libflac-sys/blob/master/src/bindings.rs) a breeze. This allowed me to link [libflac](https://github.com/xiph/flac) statically in the swyh-rs binary, no dlls needed!

Tested on Windows 10 and on Ubuntu 20.04 LTS (Mint 20) and 22.04 LTS (Mint 21) with Raspberry Pi/Hifi-Berry based devices, currently running MoodeAudio 8.x. I don't have access to a Mac, so I don't know if this also works.

Because it is written in Rust it uses almost no resources (CPU usage barely measurable, Ram usage around or below 4 MB).

### Where to get it and how to install

You can get the latest Windows binary from the [Release page](https://github.com/dheijl/swyh-rs/releases).
No install needed, no runtime, no dependencies. Just unzip the binary in a convenient place and run it.

Debug build and a release builds and a setup for Windows 64 bit are included in the release assets, I also sometimes add a Linux (Ubuntu 20.04) binary.
You would only ever need the debug build in the unlikely case rust "panics", and the program vanishes without a message. In a release build you will have a logging file in the _.swyh-rs_ folder in your home directory. But when rust "panics" you can't log it, so you will need to start the debug build from a console/terminal window. A debug build automatically raises the log level to "DEBUG". This will also allow you to catch the Rust "panic" message in the console window (release builds do not have a console on Windows). Depending on the log level you set (info/warn/debug) the release build will provide all information needed to help in troubleshooting, aside from "panics".

If you want to build swyh-rs yourself, you can find some information in the [wiki](https://github.com/dheijl/swyh-rs/wiki).

If it doesn't work for you, please open a new issue and include all the debug log level information. I will try to provide a fix ASAP.

### Todo

- I'm open to suggestions, but I definitely hate GUI programming...

### Known problems

- make sure that your firewall or anti-virus do not block the default incoming HTTP port 5901 for streaming requests (or the port number you configured in the UI if not the default)
- resizing a window in fltk 1.4 is not ideal, but thanks to @MoAlyousef it is now usable in swyh-rs. But if you resize vertically to a very small window you risk losing the horizontal scrollbar in the textbox at the bottom.
- simultaneous streaming to multiple renderers is only limited by the number of renderer buttons that can be shown in the available space in the window.
- Kaspersky Antivirus can prevent audio capture, so you may have to add an exception for swyh-rs (thanks @JWolvers).
- streaming to Logitech Media Server does not work ([issue # 40]( https://github.com/dheijl/swyh-rs/issues/40))
- streaming to Linn devices does not work
- if for some reason your config file gets corrupted/invalid it will be replaced with a default configuration at startup instead of panicking when deserializing.

### Artwork Credits

The icon was designed by @numanair, thanks!

### How does it work?

- audio is captured from the default audio device (WasApi on Windows, Alsa on Linux, not tested on Mac), but you can choose any audio source you want. Changing the sound source needs a restart of the app to take effect.
- a built-in audio streaming web server is started on port 5901.
- all media renderers are discoverded using SSDP on the local network, this takes about four seconds to complete. By default the network that connects to the internet is chosen (so that on a multihomed Windows machine the most likely interface is selected). If necessary you can choose another network from the network dropdown, for instance if you use a VPN.
- then a button is shown for every renderer found
- if you click the button for a renderer the OpenHome or AvTransport protocol is used to let the renderer play the captured audio from the webserver
- audio is always sent in audio/l16 PCM format, no matter the input source, using the sample rate of the source, unless you enable 24 bit LPCM (see below).
- some renderers will stop when detecting a pause between songs or for some other unknown reason. You can use the _*Autoresume_" checkbox if you encounter this problem. But always try to disable the "_Chunked Transfer Encoding_" first to see if this fixes the problem before you enable AutoResume. Since version 1.3.2 AutoResume should work with OpenHome renderers too (tested with Bubble UPNP Server and Chromecast/Nest Audio).
- there is an "_Autoreconnect_" checkbox, if set the last used renderer will be automatically activated on program start
- there is also a "_No Chunked Tr. Enc._" checkbox, because some AV-Transport renderers do not support it properly (those based on the UPnP/1.0, Intel MicroStack in particular). You can safely disable chunked transfer, it's a HTTP/1.1 recommendation for streaming but it does not really matter if you do not use it.
- since 1.4.0 there is a dropdown that lets you choose between FLAC, LPCM or WAV format. Preferred format is FLAC, WAV or LPCM should only be used if FLAC does not work. Also, only FLAC will work with 24 bit.
- there is (since 1.3.20) a check box "_24 bit_". It causes audio to be streamed in 24 bit LPCM format (audio/L24) with the sampling rate of the audio source. It only works reliably with the FLAC format. 24 bit works with Bubble/UPNP too with LPCM, but not with hardware streamers.
- there is (since 1.3.13) an input box to select the _HTTP listener port_ for the streaming server. Default is 5901. If you use a firewall, this port should allow incoming HTTP connections from your renderer(s).
- there is (since 1.3.6) an option to enable visualization of the RMS value (L+R channel) of the captured PCM audio signal. It will only add an insignificant amount of CPU use.
- you can also enter the webserver url in the renderer, for instance in Volumio as a web radio at <http://{ip_address}:5901/stream/swyh.wav>, so that you can start playing from the Volumio UI if swyh-rs is already running
- the program tries to run at a priority "above normal" in the hope that using the computer for other stuff will not cause stuttering. On Windows this always works, on Linux you need the necessary priviliges (renice).
- the SSDP discovery process is rerun every x minutes in the background, any newly discovered renderers will be automatically added to the GUI. Existing renderers that "disappear" during discovery are not deleted from the GUI, as SSDP discovery is not guaranteed to be failsafe (it uses UDP packets). The SSDP discovery interval is configurable, minimum value is 0.5 minutes, there is no maximum value.
- after a configuration change that needs a program restart, you get a "restart" popup dialog. Click "Restart" to restart the app, or "Cancel" to ignore.
- Since version 1.2.2, swyh-rs will send silence to connected renderers if no sound is being captured because no audio is currently being played. This prevents some renderers from disconnecting because they have not received any sound for some time (Bubble UPNP Server with Chromecast/Nest Audio). Apparently sending silence keeps them happy. I did not implement this "silence" for FLAC streaming.
- Since version 1.5 you can have multiple instances running where each instance uses a different configuration file. An optional command line parameter _-c config_ or _--configuration config_ has been added to enable this (using a shortcut or starting swyh-rs from the command line). This _config_ parameter is then used as part of the config.toml filename for the swyh-rs instance. The default _config_ is empty. Examples: _swyh-rs -c 1_ or _swyh-rs --configuration vb-audio_. This way you can **stream different audio sources** to different receivers simultaneously.

### Audio quality and Windows WasApi Loopback capture

If you want maximum audio quality on Windows, there are a number of concerns:

- you should avoid resampling, because it affects audio quality. The sampling rate from the original audio source should be used to preserve quality. This means that you should make sure that the sampling frequency in the entire audio chain is the same (Use "Control Panel Sound" to check/change the sampling frequency). Bit depth does not really affect sound quality, and 16 bit _is_ enough except if you are recording for mastering purposes in an audio lab. Deezer HiFi and Tidal HiFi use 16 bit 44100 Hz (lossless CD quality).
- on Windows, WasApi is used to capture audio. WasApi tries to capture directly from the hardware (soundcard) loopback if available, otherwise it uses the soundsource directly. In practice, this means that the soundcard loopback audio quality can be vastly inferior to the original soundsource (Realtek, Conexant, especially in laptops). Make sure all "effects" are disabled. The freeware/donationware VBAudio HiFi Cable driver (<https://shop.vb-audio.com/en/win-apps/19-hifi-cable-asio-bridge.html?SubmitCurrency=1&id_currency=2>) is an excellent solution to prevent this problem. Just make sure you configure it with the same sampling frequency as the default Windows audio source. You can then select HiFi Cable as the sound source in swyh-rs, and use the Windows Sound Mixer to route different apps to other sound drivers for Windows as needed (system sound etc). HiFi cable is a bit perfect pipe from the audio source to the renderer, except for the bit depth at this moment, because swyh-rs uses audio/l16, FLAC/16 or FLAC/24 to stream to the network players, but this does not affect sound quality, it only limits the dynamic range to 96 dB when using 16 bits which is fine for HiFi. You can also make HiFi cable the default output source, and configure other sound cards in the volume mixer for applications as needed.

### Audio recording

Audio recording is not supported, as there are other and better free tools to do that, like [Audacity](https://www.audacityteam.org/).

The following pages might get you going:

- <https://manual.audacityteam.org/man/tutorial_recording_computer_playback_on_windows.html>
- <https://manual.audacityteam.org/man/recording_length.html>

See also [issue #44](https://github.com/dheijl/swyh-rs/issues/44).

### Screenshot

![Knipsel](https://user-images.githubusercontent.com/2384545/179241258-13cce495-3875-4e1d-9115-94f80a34644a.PNG)
