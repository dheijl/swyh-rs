# swyh-rs

![rs-tall](https://user-images.githubusercontent.com/2384545/112133026-8ae7c880-8bcb-11eb-835e-f6aaed25bea3.png)

## What is it

A "Stream-What-You-Hear" implementation written in Rust.

**swyh-rs** implements the idea behind the original SWYH (see <https://www.streamwhatyouhear.com/>, source repo <https://github.com/StreamWhatYouHear/SWYH>) in Rust.
It allows you to stream the music you're currently playing on your PC (Windows and Linux supported) to an UPNP/DLNA/OPenHome compatible music player (a "Renderer").

It has been tested with

- Volumio devices (<https://volumio.org/>)
- Harman Kardon AV network streamers (thanks @MX10-AC2N!)
- Denon Heos devices
- Sony AV streamers & Bravia TVs
- Chromecast devices defined as an OpenHome or DLNA device in Bubble UPNP Server (thanks Bubblesoft for providing the necessary information!)
- Sonos speaker using WAV format (thanks @Cunkers !)
- Kef Wireless LS50 II (thanks @Turbomortel via Twitter)
- Xbox 360, using Foobar2000 and entering the streaming url in foo_upnp (thanks @instinctualjealousy)
- iEast Audiocast M5 using the WAV format header (thanks @Katharsas)
  
but will probably support any streamer that supports the OpenHome or AVTransport (DLNA) protocol.
If a device supports both OpenHome and DLNA, the OpenHome endpoint is used, and the DLNA AVTransport endpoint is ignored.

I wrote this because I a) wanted to learn Rust and b) SWYH does not work on Linux, does not work well with Volumio (push streaming does not work), and has a substantial memory leak in the ancient Intel .Net UPNP/DLNA library it uses.

Music is streamed in uncompressed 16 bit LPCM format (audio/l16, audio/L24,  or optionally audio/wav with an "infinite length" WAV header) with the sample rate of the music source (the chosen audio output device, I personally use VBAudio HiFi Cable Input).
Since version 1.3.5 there is also support for streaming in in !uncompressed PCM WAV file format in case your renderer does not support "naked" uncompressed PCM streams.
Note that libsndfile based renderers may not be able to decode the WAV format if they do not open the stram as a "pipe", because the stream is not "seekable".

Audio is captured using the excellent Rust cpal (<https://github.com/RustAudio/cpal>) library.
Fltk-rs (<https://github.com/MoAlyousef/fltk-rs>) is used for the GUI, as it's easy to use, is small, is cross-platform, is fast and works well.

Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi/Hifi-Berry based Volumio devices. I don't have access to a Mac, so I don't know if this also works.

Because it is written in Rust it uses almost no resources (CPU usage barely measurable, Ram usage around or below 3 MB).

### Where to get it and how to install

You can get the latest Windows binary from the Release page (<https://github.com/dheijl/swyh-rs/releases>).
No install needed, no runtime, no dependencies. Just unzip the binary in a convenient place and run it.

There is a debug build and a release build in the zip file.
You will only need the debug build in the unlikely case rust "panics", and the program vanishes without a message. In a release build you will have a logging file in the _.swyh-rs_ folder in your home directory. But when rust "panics" you can't log it, so you will need to start the debug build from a console/terminal window. A debug build automatically raises the log level to "DEBUG". This will also allow you to catch the Rust "panic" message in the console window (release builds do not have a console on Windows). Depending on the log level you set (info/warn/debug) the release build will provide all information needed to help in troubleshooting, aside from "panics".

If you want to build swyh-rs yourself, you can find some information in the [wiki](https://github.com/dheijl/swyh-rs/wiki).

If it doesn't work for you, please open a new issue and include all the debug log level information. I will try to provide a fix ASAP.

### Todo

- I'm open to suggestions, but I definitely hate GUI programming...

### Known problems

- resizing a window in fltk 1.4 is not ideal, but thanks to @MoAlyousef it is now usable in swyh-rs. But if you resize vertically to a very small window you risk losing the horizontal scrollbar in the textbox at the bottom.
- simultaneous streaming to multiple renderers is only limited by the number of renderer buttons that can be shown in the available space in the window.
- Kaspersky Antivirus can prevent audio capture, so you may have to add an exception for swyh-rs (thanks @JWolvers).
- streaming to Logitech Media Server does not work ([issue # 40]( https://github.com/dheijl/swyh-rs/issues/40))
- streaming to Linn does not work
- if for some reason your config file gets corrupted you can get a panic on startup when the config file is deserialized. The easy fix is to simply delete the config.ini (for older versions) or config.toml (for newer versions). You can find it in your home directory in the .swyh-rs folder.

### Artwork Credits

The icon was designed by @numanair, thanks!

### How does it work?

- audio is captured from the default audio device (WasApi on Windows, Alsa on Linux, not tested on Mac), but you can choose any audio source you want. Changing the sound source needs a restart of the app to take effect.
- a built-in audio streaming web server is started on port 5901.
- all media renderers are discoverded using SSDP on the local network, this takes about four seconds to complete. By default the network that connects to the internet is chosen (so that on a multihomed Windows machine the most likely interface is selected). If necessary you can choose another network from the network dropdown, for instance if you use a VPN.
- then a button is shown for every renderer found
- if you click the button for a renderer the OpenHome or AvTransport protocol is used to let the renderer play the captured audio from the webserver
- audio is always sent in audio/l16 PCM format, no matter the input source, using the sample rate of the source, unless you enable 24 bit LPCM (see below).
- some renderers will stop when detecting a pause between songs or for some other unknown reason. You can use the "*Autoresume*" checkbox if you encounter this problem. But always try to disable the "*Chunked Transfer Encoding*" first to see if this fixes the problem before you enable AutoResume. Since version 1.3.2 AutoResume should work with OpenHome renderers too (tested with Bubble UPNP Server and Chromecast/Nest Audio).
- there is an "*Autoreconnect*" checkbox, if set the last used renderer will be automatically activated on program start
- there is also a "*No Chunked Tr. Enc.*" checkbox, because some AV-Transport renderers do not support it properly (those based on the UPnP/1.0, Intel MicroStack in particular). You can safely disable chunked transfer, it's a HTTP/1.1 recommendation for streaming but it does not really matter if you do not use it.
- there is (since 1.3.5) an "*Add WAV Hdr*" checkbox, that will prepend an infinite size MS "WAV" (RIFF) header to the stream for those renderers that do not support "naked" PCM streams. It may work or not (it will not work if your renderer uses libsndfile like Volumio, because the network stream is not "seekable" and this causes the decoding of the WAV header to fail). Apparently Sonos devices do not accept raw PCM, they need the WAV header.
- there is (since 1.3.20) a check box "*24 bit*". It causes audio to be streamed in 24 bit LPCM format (audio/L24) with the sampling rate of the audio source. This does not work with older Mpd/Upmpdcli based streamers like Volumio 2.x. Not tested on Volumio 3.x yet. But BubbleUPNP correctly transcodes it to audio/L16.
- there is (since 1.3.13) an input box to select the *HTTP listener port* for the streaming server. Default is 5901. If you use a firewall, this port should allow incoming HTTP connections from your renderer(s).
- there is (since 1.3.6) an option to enable visualization of the RMS value (L+R channel) of the captured PCM audio signal. It will only add an insignificant amount of CPU use.
- you can also enter the webserver url in the renderer, for instance in Volumio as a web radio: <http://{ip_address}:5901/stream/swyh.wav>, so that you can start playing from the Volumio UI if swyh-rs is already running
- the program tries to run at a priority "above normal" in the hope that using the computer for other stuff will not cause stuttering. On Windows this always works, on Linux you need the necessary priviliges (renice).
- the SSDP discovery process is rerun every x minutes in the background, any newly discovered renderers will be automatically added to the GUI. Existing renderers that "disappear" during discovery are not deleted from the GUI, as SSDP discovery is not guaranteed to be failsafe (it uses UDP packets). The SSDP discovery interval is configurable, minimum value is 0.5 minutes, there is no maximum value.
- after a configuration change that needs a program restart, you get a "restart" popup dialog. Click "Restart" to restart the app, or "Cancel" to ignore.
- Since version 1.2.2, swyh-rs will send silence to connected renderers if no sound is being captured because no audio is currently being played. This prevents some renderers from disconnecting because they have not received any sound for some time (Bubble UPNP Server with Chromecast/Nest Audio). Apparently sending silence keeps them happy.

### Audio quality and Windows WasApi Loopback capture

If you want maximum audio quality on Windows, there are a number of concerns:

- you should avoid resampling, because it affects audio quality. The sampling rate from the original audio source should be used to preserve quality. This means that you should make sure that the sampling frequency in the entire audio chain is the same (Use "Control Panel Sound" to check/change the sampling frequency). Bit depth does not really affect sound quality, and 16 bit *is* enough except if you are recording for mastering purposes in an audio lab. Deezer HiFi and Tidal HiFi use 16 bit 44100 Hz (lossless CD quality).
- on Windows, WasApi is used to capture audio. WasApi tries to capture directly from the hardware (soundcard) loopback if available, otherwise it uses the soundsource directly. In practice, this means that the soundcard loopback audio quality can be vastly inferior to the original soundsource (Realtek, Conexant, especially in laptops). Make sure all "effects" are disabled. The freeware/donationware VBAudio HiFi Cable driver (<https://shop.vb-audio.com/en/win-apps/19-hifi-cable-asio-bridge.html?SubmitCurrency=1&id_currency=2>) is an excellent solution to prevent this problem. Just make sure you configure it with the same sampling frequency as the default Windows audio source. You can then select HiFi Cable as the sound source in swyh-rs, and use the Windows Sound Mixer to route different apps to other sound drivers for Windows as needed (system sound etc). HiFi cable is a bit perfect pipe from the audio source to the renderer, except for the bit depth at this moment, because swyh-rs uses audio/l16 to stream to the network players, but this does not affect sound quality, it only limits the dynamic range to 96 dB which is fine for HiFi. You can also make HiFi cable the default output source, and configure other sound cards in the volume mixer for applications as needed.

### Audio recording

Audio recording is not supported, as there are other and better free tools to do that, like [Audacity](https://www.audacityteam.org/).

The following pages might get you going:

- <https://manual.audacityteam.org/man/tutorial_recording_computer_playback_on_windows.html>
- <https://manual.audacityteam.org/man/recording_length.html>

See also [issue #44](https://github.com/dheijl/swyh-rs/issues/44).

### Screenshot

![Knipsel](https://user-images.githubusercontent.com/2384545/145233025-1c7145ac-abfc-4574-954a-b068036b43fe.PNG)
