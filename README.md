# swyh-rs

![CI status](https://github.com/dheijl/swyh-rs/actions/workflows/rust.yml/badge.svg)

![rs-tall](https://user-images.githubusercontent.com/2384545/112133026-8ae7c880-8bcb-11eb-835e-f6aaed25bea3.png)

## What is it

A "Stream-What-You-Hear" implementation written in Rust, MIT licensed.

### Contents

- [Why this SWYH alternative ?](#why-this-swyh-alternative-)
- [Current release and binaries](#current-release)
- [Changelog](CHANGELOG.md)
- [swyh-rs as your local internet radio station](#swyh-rs-as-your-local-internet-radio-station)
- [Todo](#todo)
- [Building (Wiki)](https://github.com/dheijl/swyh-rs/wiki)
- [Known problems](#known-problems)
- [Artwork Credits](#artwork-credits)
- [How does it work](#how-does-it-work)
- [VPN and SSDP failure](#ssdp-and-vpn)
- [The CLI binary](#the-cli-binary)
- [Windows tray-icon code (Python) by @phil2sat](https://github.com/dheijl/swyh-rs/blob/master/tray_icon/)
- [Latency, streaming format and stream duration](#latency-and-streaming-format-and-stream-duration)
- [Audio quality and Windows WasApi Loopback capture](#audio-quality-and-windows-wasapi-loopback-capture)
- [Releases](#releases)
- [Screenshots](#screenshots)

## Current Release

The current release is **[1.20.3-RC3](https://github.com/dheijl/swyh-rs/releases/tag/1.20.3-RC3)**, refer to the [Changelog](CHANGELOG.md) for more details.
This RC is released to give the new CPAL 0.18.0 development release and the new localization feature some exposure.

You can find x86/64  Windows setup and binaries and Linux (Ubuntu/Debian) appimages in [Releases](https://github.com/dheijl/swyh-rs/releases).

You can find [Arm binaries here](https://github.com/jamieduk/SWYH-ARM-64Bit-Linux/releases). They are provided by Jamie Duk (@Jay), I have not tested them. He also provides his [build recipe](https://github.com/jamieduk/SWYH-ARM-64Bit-Linux).

## Why this SWYH alternative ?

**swyh-rs** implements the idea behind the original [SWYH](https://www.streamwhatyouhear.com) (source repo <https://github.com/StreamWhatYouHear/SWYH>) written in Rust.
It allows you to stream the music you're currently playing on your PC (Windows or Linux) to an UPNP/DLNA/OpenHome compatible music player (a "Renderer").

I wrote this because

- I wanted to learn Rust
- SWYH does not work on Linux
- SWYH did not work well with Volumio (push streaming did not work)
- SWYH has a substantial memory leak due to the use of an old and unmaintained Intel .Net UPNP/DLNA library it uses.

**NOTE** swyh-rs does not support lossy mp3 or aac re-encoding, only lossless LPCM/WAV/RF64/FLAC for obvious reasons.

It has been tested with

- [MoOde audio](https://moodeaudio.org/), with Moode configured as UPNP renderer in _Openhome_ mode, and using FLAC (preferable) or LPCM (since 1.8.7) or WAV format. See [Known problems](#known-problems) for solving the 5-second delay with WAV/RF64/LPCM format. Use FLAC instead if you can.
- [Volumio](https://volumio.org/)
- Harman Kardon AV network streamers (thanks @MX10-AC2N!)
- **Denon** Heos devices. Wav Format with NoneChunked streamsize seems to work best with Denon. And if you change format or streamsize you may have to restart swyh-rs to make the Denon recognize this. See issue #183. If you select an invalid combination the Denon app will also tell you this.
- Sony AV streamers & Bravia TVs
- Goldenwave HiFiMan Serenade DAC/Headphone amlpifier using WAV/RF64/FLAC.
- **Chromecast** devices exposed as an OpenHome or DLNA devices in **Bubble UPNP Server** (thanks Bubblesoft for providing the necessary information!). If you have multiple devices in the same Bubble UPNP server, you need version 1.12.3 or later, because there are multiple devices at the same IP address. At a certain point Bubble changed the SSDP headers causing swyh-rs to no longer "see" these device. This is also fixed in 1.12.3. See issue #157 (thanks @kenkuo). Prior to 1.12.3 devices were identified by their IP address, from 1.12.3 on they are identified by their SSDP "Location".
- **Sonos** speakers/soundbars using the **WAV** format (thanks @Cunkers !). **update:** A recent update to the Sonos Play 1 also enabled **FLAC**. Depending on your network a Sonos may stutter when using WAV, if you are affected you should use FLAC if your device supports it. See issues #84 and #75. Software version "15.9 (Build 75146030)" on the Play:1 is known to support FLAC without stuttering (thanks @beWAYNE !). **Important**: if you are streaming to a stereo pair, you should only stream to the **master** of the pair, and never to both, as this can/will break the stereo pair (see issue #141)! See also [this wiki entry](https://github.com/dheijl/swyh-rs/wiki#5-Sonos-stuttering) for Sonos stuttering.
- If you want to pause music without losing the connection you can enable the  **Inject Silence** option. The InjectSilence flag is automatically added to the config file when you first start version 1.4.5 and defaults to _false_. Contributed by @genekellyjr, see issue #71, and @DanteDT. Since 1.12.0 Inject Silence should work with FLAC too. If you don't enable Inject Silence for FLAC, swyh-rs will automatically periodically inject some faint white noise in the absence of sound so that you hopefully don't loose the connection.
  - injecting silence will eat a neglegible amount of cpu cycles.
  - it seems that stuttering can occur with Sonos gear, especially with WiFi connections. You can try to set an initial buffering value to prevent this. According to @konyong installing ccproxy can also help, refer to issue #130 for more details.
- Kef Wireless LS50 II (thanks @Turbomortel via Twitter)
- Xbox 360, using Foobar2000 and entering the streaming url in foo_upnp (thanks @instinctualjealousy)
- iEast Audiocast M5 using the WAV format header (thanks @Katharsas)
- Yamaha WXAD-10 since 1.6.1 (see issue #89), and possibly other Yamaha devices?
- for QPlay devices, like the Xiaomi S12, you need version 1.8.2 or later, see issue #99. Older versions wrongly try to use Openhome instead of AVTransport.
- **Roon** with FLAC and using U32MaxChunked for streamsize (swyh-rs 1.10.5 and up). Thanks to @DrCWO for figuring this out (issue #55).
- playback on **Squeezebox** players connected to Logitech Media Server (now known as Lyrion Music Server) works by adding the swyh-rs URL as a favorite to LMS: <http://pcipaddress:5901/stream/swyh.flac>, as pointed out by @Cornelisj (issue #40).

but any OpenHome/DLNA streamer that supports FLAC (except older Sonos software versions that do not do FLAC over upnp) will probably work (since version 1.4.0).

If a device supports both OpenHome and DLNA, the OpenHome endpoint is used, and the DLNA AVTransport endpoint is ignored.

Music is streamed with the sample rate of the music source (the chosen audio output device, I personally use VBAudio HiFi Cable Input as a bit-perfect audio source).

Supported audio streaming formats:

- 16 bit or 24 bit **FLAC** (lossless compression, I'm using the lowest compression level for performance and latency reasons). It is available since version 1.4.0
- audio/wav (16 bit) with a "maximum length" (4 GB) **WAV** header, available since version 1.3.5
- uncompressed 16 bit **LPCM** format (audio/l16)
- audio/rf64 (16 bit) basically WAV with unlimited size since version 1.9.1.

Note that older libsndfile based renderers may not be able to decode the WAV format, because the stream is not "seekable".

Audio is captured using the excellent Rust [cpal library](https://github.com/RustAudio/cpal).
[fltk-rs](https://github.com/MoAlyousef/fltk-rs) is used for the GUI, as it's easy to use, and it's small, cross-platform, fast and works well.
For FLAC encoding the of use [flac-bound](https://github.com/nabijaczleweli/flac-bound) made adding FLAC encoding using the [libflac-sys Rust bindings](https://github.com/mgeier/libflac-sys/blob/master/src/bindings.rs) a breeze. This allowed me to link [libflac](https://github.com/xiph/flac) statically in the swyh-rs binary, no dlls needed!

Tested on Windows 10 and on Ubuntu 20.04 LTS and Debian Bookworm (LMDE 6) with Raspberry Pi/Hifi-Berry based devices, currently running MoodeAudio 9.x. I don't have access to a Mac, so I don't know if that also works.

Because it is written in Rust it uses almost no resources (CPU usage barely measurable, Ram usage around or below 8 MB).

## swyh-rs as your local internet radio station

You can also use swyh-rs as an internet radio station on your local network. swyh-rs is available at

- `http://{your-pc-ip}/stream/swyh.raw` when streaming "raw" LPCM format
- `http://{your-pc-ip}/stream/swyh.wav` when streaming WAV format
- `http://{your-pc-ip}/stream/swyh.rf64` when streaming RF64 format
- `http://{your-pc-ip}/stream/swyh.flac` when streaming FLAC format

You can append query parameters to the url for bits per sample (bd = bit depth, 16 or 24) and streamsize (ss: nonechunked, u32maxchunked, u64maxchunked, u32maxnotchunked, u64maxnotchunked).
The query parmeters in the query string override the configured values.

Example: `http://{your-pc-ip}/stream/swyh.flac?bd=24&ss=nonechunked`

When running the CLI with the -x option, that is effectively the only way to access the swyh-rs audio server.
This is also true when running the GUI if SSDP discovery has been disabled by setting the SSDP interval to 0.0.

### Where to get it and how to install

You can get the latest Windows binary from the [Release page](https://github.com/dheijl/swyh-rs/releases).
No install needed, no runtime, no dependencies. Just unzip the binary in a convenient place and run it.

Debug build and a release builds and a setup for Windows 64 bit are included in the release assets, I also add Linux appimages for Ubuntu 20.04 LTS and Debian Bookworm.
You would only ever need the debug build in the unlikely case rust "panics", and the program vanishes without a message. In a release build you will have a logging file in the _.swyh-rs_ folder in your home directory. But when rust "panics" you can't log it, so you will need to start the debug build from a console/terminal window. A debug build automatically raises the log level to "DEBUG". This will also allow you to catch the Rust "panic" message in the console window (release builds do not have a console on Windows). Depending on the log level you set (info/warn/debug) the release build will provide all information needed to help in troubleshooting, aside from "panics".

If you want to build swyh-rs yourself, you can find some information in the [wiki](https://github.com/dheijl/swyh-rs/wiki).

If it doesn't work for you, please open a new issue and include all the debug log level information. I will try to provide a fix ASAP.

### Todo

- I'm open to suggestions, but I definitely hate GUI programming...

### Known problems

- On a freshly installed Windows system you may get an error "**VCRUNTIME140.dll was not found**". Rust Windows binaries built with the MSVC toolchain need the Visual Studio 2015..2019 runtime. Because so much software relies on it, is almost always already present, but if not you can get the Visual C++ 2015..2019 runtime installer from [https://aka.ms/vs/16/release/vc_redist.x64.exe](https://aka.ms/vs/16/release/vc_redist.x64.exe). The current Windows installer will automatically do this if necessary.
- On linux you may have to enable **audio monitoring** with pavucontrol to make audio capture work
- make sure that your firewall or anti-virus do not block the default incoming HTTP port 5901 for streaming requests (or the port number you configured in the UI if not the default), and that outgoing UDP traffic is allowed for SSDP  
- resizing a window in fltk 1.4 is not ideal, but thanks to @MoAlyousef it is now usable in swyh-rs. But if you resize vertically to a very small window you risk losing the horizontal scrollbar in the textbox at the bottom.
- simultaneous streaming to multiple renderers is only limited by the number of renderer buttons that can be shown in the available space in the window.
- Kaspersky Antivirus can prevent audio capture, so you may have to add an exception for swyh-rs (thanks @JWolvers).
- streaming to Logitech Media Server does not work ([issue # 40]( https://github.com/dheijl/swyh-rs/issues/40))
- streaming to Linn devices does not work (due to Linn using partial requests with Range headers)
- if for some reason your config file gets corrupted/invalid it will be replaced with a default configuration at startup instead of panic-ing when deserializing.
- when using WAV/RF64/LPCM with an MPD/FFMPEG based player like MoOde, you get a 5 second delay caused by FFMPEG analyzing the input stream. You can reduce this by specifying acceptable limits for analyzing audio in the MPD config like shown below, or you can use FLAC instead (recommended).

```text
decoder {
plugin "ffmpeg"
enabled "yes"
analyzeduration "5000"
probesize "1024"
}
```

### Artwork Credits

The icon was designed by @numanair, thanks!

### How does it work?

When swyh-rs starts, it:

- captures audio from the selected audio source (WasApi on Windows, Alsa on Linux)
- starts a built-in HTTP audio streaming web server on the configured port (default 5901)
- runs **SSDP** discovery to find all UPnP/DLNA/OpenHome media renderers on the local network (takes about four seconds)
- displays a button for every discovered renderer, together with a volume slider if the renderer supports volume control — drag the slider to change the volume; hold Shift while dragging to copy the new volume to all currently active renderers
- clicking a renderer button uses the OpenHome or AvTransport protocol to instruct it to play the audio stream from the built-in web server

SSDP discovery reruns every x minutes in the background; newly found renderers are added automatically. Renderers that disappear are kept in the GUI since SSDP (UDP-based) is not guaranteed reliable.

You can also enter the web server URL directly in a renderer — for instance in MoOde as a web radio at `http://{ip_address}:5901/stream/swyh.flac` — so you can start playing from the renderer's own UI while swyh-rs is running.

On Linux silence is often automatically produced in the absence of sound. If this is not the case, you can enable the "_Inject silence_" checkbox in the audio tab. If no silence is present in the absence of sound, Swyh-rs periodically sends silence to connected renderers when no audio is playing, to prevent some renderers from disconnecting. For FLAC, a −90 dB white noise is sent instead.

The program runs at "above normal" priority to reduce the chance of stuttering when the computer is busy. On Windows this always works. On Linux you need the necessary privilege (renice): on Debian Bookworm add yourself to the `pipewire` group (`sudo usermod -a -G pipewire username`); on Ubuntu 20.04 add a `nice -10` entry to `/etc/security/limits.conf` for your user.

Since version 1.5 you can run multiple instances simultaneously, each with its own config file, using the `-c config` or `--configuration config` command-line option — e.g. `swyh-rs -c 1` or `swyh-rs --configuration vb-audio`. This lets you **stream different audio sources to different receivers simultaneously**.

You can hide a renderer button by right-clicking it. Right-click the "_UPNP rendering devices..._" label to unhide all hidden renderers.

On Windows, starting or closing an RDP session re-initialises the sound system and aborts audio capture. Since 1.12.13, swyh-rs attempts to restore capture from the original device automatically.

After one or more configuration changes that require a restart, a **Restart** button appears. Clicking it restarts swyh-rs with the new settings.

The **configuration UI** is organised into four tabs:

#### Audio tab

- **Audio source**: select the audio capture device. Changing the source requires a restart.
  On Windows, verify in the **Sound Mixer** that the chosen device is actually playing audio. On Linux, use [pavucontrol](https://freedesktop.org/software/pulseaudio/pavucontrol/) to enable the audio monitor for the capture device.
- **Streaming format**: choose between FLAC (preferred), WAV, RF64, or LPCM. FLAC is recommended — it works reliably with 24-bit audio and causes the fewest compatibility issues. WAV or LPCM should only be used when FLAC does not work with your renderer.
- **24 bit**: stream audio at 24-bit depth (FLAC/24 or LPCM audio/L24 at the source sample rate). Only works reliably with FLAC; 24-bit LPCM works with Bubble UPNP but not with most hardware streamers.
- **Stream size**: select one of five HTTP streaming size/chunking modes — choose what works best with your renderer and format:
  - _NoneChunked_: no Content-Length, chunked transfer encoding
  - _U32MaxChunked_: Content-Length = u32::MAX, chunked
  - _U64MaxChunked_: Content-Length = u64::MAX, chunked
  - _U32MaxNotChunked_: Content-Length = u32::MAX − 1, no chunking (default for WAV)
  - _U64MaxNotChunked_: Content-Length = u64::MAX − 1, no chunking
- **Initial buffering** (ms): buffer audio for this many milliseconds before streaming starts. Helps prevent stuttering on flaky WiFi networks or with renderers that have no configurable buffer.
- **Inject Silence**: continuously mix silence into the input stream as an alternative to the automatic periodic-silence mechanism, preventing some renderers from disconnecting during playback pauses. Works with FLAC since version 1.12.0. On Linux silence may be injected automatically so no need to check this in that case.

#### Network tab

- **Network interface**: select the network interface used for SSDP discovery and streaming. On a multihomed machine swyh-rs defaults to the interface that connects to the internet. Change this when a VPN or secondary interface is involved.
- **HTTP port**: the port the streaming web server listens on (default 5901). If you use a firewall, allow incoming HTTP connections on this port from your renderer(s).
- **SSDP interval** (minutes): how often SSDP discovery reruns in the background. Minimum is 0.5 minutes. Set to 0.0 to disable discovery entirely, putting swyh-rs into "_serve only_" mode — useful when running it purely as a local internet radio station.

#### App tab

- **Language / Theme**: select the UI language and FLTK colour theme.
- **Log level**: set the logging verbosity (info / debug). Log files are written to the _.swyh-rs_ folder in your home directory. Use the debug build and a console window to capture Rust "panic" messages that a release build cannot log.
- **Auto-resume**: automatically resume streaming if a renderer stops unexpectedly. Always try disabling _Chunked Transfer Encoding_ first to see if that alone fixes the problem before enabling Auto-resume.
- **Auto-reconnect**: re-activate all renderers that were active when swyh-rs was last closed.
- **RMS monitor**: enable visualisation of the RMS level (L+R channels) of the captured audio. Use this to verify that swyh-rs is actually capturing audio. Adds a negligible amount of CPU use.

#### Status tab

The Status tab is the active tab on startup. It shows a live, read-only summary of all current configuration settings and is updated automatically whenever a setting changes.

### SSDP and VPN

If you have a VPN active, like NordVPN, it's possible that SSDP will fail to find your devices, because the VPN reroutes the internal network. See [issue #247](https://github.com/dheijl/swyh-rs/issues/247).

### The CLI binary

Since 1.7.0, there is a new binary, **swyh-rs-cli**. It has no GUI, but otherwise shares all code with swyh-rs.
The GUI configuration options have all been replaced with a corresponding command line option.

This is the "Usage message" (produced by the -h or --help option):

```sh
Recognized options:
    -h (--help) : print usage
    -n (--no-run) : dry-run mode that exits just before starting to stream
    -c (--config_id) string : config_id [_cli]
    -C (--configfile) string : alternative full pathname of configfile
    -p (--server_port) u16 : server_port [5901]
    -a (--auto_reconnect) bool : auto reconnect [true]
    -r (--auto_resume) bool : auto_resume [false]
    -s (--sound_source) u16 : sound_source index or name [os default]
    -l (--log_level) string : log_level (info/debug) [info]
    -i (--ssdp_interval) i32 : ssdp_interval_mins [10]
    -b (--bits) u16 : bits_per_sample (16/24) [16]
    -f (--format) string : streaming_format (lpcm/flac/wav) [LPCM] optionally followed by a plus sign and a streamsize[LPCM+U64maxNotChunked] 
    -o (--player_ip) string : the player ip address [last used player] or the player device name (can be comma-seperated list if multiple players are selected)
    -e (--ip_address) string : ip address of the network interface [last used]
    -x (--serve_only) bool : skip ssdp discovery and start serving immediately [false]
    -u (--upfront-buffer) i32: initial audio bufferign before streaming starts [0]
```

The default values for missing options are given between square brackets. Refer to the GUI description for an explanation of the options.
Most options except -h, -n and -x are saved in the config file, so once a config is working to your liking you no longer have to provide them.

Options -h, -n and -x will ignore the optional boolean argument (true/false) if specified. Specifying the option alone is equivalent to true.
Other boolean options accept an optional true/false, because they are remembered in the config file and you should be able to change the stored value.

Hint: use the **-n (dry-run) mode** to get the index of the sound source device and the ip address of the receiver that you need to pass as commandline parameter.

You can also specify a sounde source **name** instead of an index, or a unique substring of the name. If you have multiple identically named soundcards, you can append _:n_ to the name, where n is a zero-based index in the duplicates.

For the player(s) **-o** you can also use the name(s) or a sub-string unique to the player name(s) instead of the IP address(es).

Streaming is started automatically, and you can stop and restart streaming with the remote of your player as long as the app is running.
The only way to stop the cli app is by killing it,  with "CONTROL C" or task manager or any other way you use to kill processes.
You can run as many instances simultaneously as you like as long as you start each one with its own configuration id value (-c option).
I suppose you could run it from the command line or as a scheduled task or as an autorun task in Windows or...
Since version 1.10.0 you can use Nix to build and install swyh-rs-cli as a service. The Nix files are contributed by @ein-shved. I don't use Nix myself.

When using the **-x (--serve_only)** option, no SSDP discovery is run, and playing is not started (ignoring the -o option). Instead swyh-rs-cli immediately starts listening for streaming requests from renderers until you terminate it.
If you do not specify a player swyh-rs-cli switches to serve_only mode.

### Latency and streaming format and stream duration

- For minimal latency, use LPCM (if your receiver supports it). On many devices LPCM will only work with 16 bit samples.
- WAV and RF64 have a slightly higher latency than LPCM, because it often causes an extra HTTP request at the start.
- FLAC will always have a hihger latency than LPCM/WAV/RF64 due the compression. But it causes less network traffic and has an advantage on flaky WiFi networks.
- A higher bit depth and/or sample rate will reduce latency because it will fill the buffer of the receiver faster.
- For unlimited streamsize and duration, use NoneChunked. If it doesn't work try one of the other options.
- WAV is in theory limited to 4 GB streaming, so it's possible that it only works with an u32Max streamsize. But you can try if NoneChunked works. 4 GB is only a couple of hours of streaming depending on sample size and sample rate. On MoodeAudio WAV only works with U32MaxNotChunked, but RF64 and FLAC work with anything. It depends on the decoder used in the receiver.
- On some receivers WAV and RF64 will cause an extra HTTP request, increasing latency slightly.
- If you suffer from hiccups or drop-outs caused by your WiFi network, use FLAC, as the compression increases buffering in the receiver. This makes it less likely that you will suffer from audio stuttering.

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

### Releases

The binaries I publish in [Releases](https://github.com/dheijl/swyh-rs/releases) are built on

- Windows:
  - the binaries are built on Windows 11 with the latest Rust stable version and Visual Studio 2022 Community edition MSVC
  - the setup is built with InnoSetup
  - the setup and the binaries are not digitally signed, so Smartscreen or Defender may complain.
- Linux:
  - since V12.17 the appimages are built on an Ubuntu 20.04LTS Pro VM with the latest Rust stable version, so they should run on most systems.
  - the appimages contain update information, so you can update them using the [UpdateAppImage](https://github.com/AppImageCommunity/AppImageUpdate) tool.

I do my best to keep everything up-to-date.

MAC: I'm sorry but I don't have one... If you have one and would like to contribute: please go ahead!

### Screenshots

- the App settings tab:
<img width="700" height="772" alt="app" src="https://github.com/user-attachments/assets/87cd25fc-ec38-406a-927b-99eec729890b" />

- the Audio settings tab:
<img width="700" height="772" alt="audio" src="https://github.com/user-attachments/assets/cf199c4c-439f-41e3-9141-2d1794550a1d" />

- the Network settings tab:
<img width="700" height="772" alt="netwerk" src="https://github.com/user-attachments/assets/32c8e352-2805-4dc7-a114-632761a4564c" />

- the Status tab:
<img width="700" height="772" alt="status" src="https://github.com/user-attachments/assets/b1c06acf-9c47-42be-b5fb-d1b2ec07e6cd" />
