# swyh-rs

Stream What You Hear written in Rust

swyh-rs is a very basic SWYH clone ( see <https://www.streamwhatyouhear.com/>, source repo <https://github.com/StreamWhatYouHear/SWYH>), entirely written in rust.

It has only been tested with Volumio (<https://volumio.org/>) and Harman Kardon (thanks @MX10-AC2N!) streamers at this moment, but will probably support any streamer that supports the OpenHome or AVTransport protocol.

I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).

For the moment all music is streamed in uncompressed LPCM format (audio/l16) with the sample rate of the music source (the default audio output device, I personally use VBAudio HiFi Cable Input). Audio is captured using the excellent cpal (<https://github.com/RustAudio/cpal>) library.

I use fltk-rs (<https://github.com/MoAlyousef/fltk-rs>) for the GUI, as it's easy to use, is cross-platform, is fast and works well. 

Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi/HifI Berry based Volumio devices. I don't have access to a Mac, so I don't know if this also works.

You can get the latest Windows binary from the Release page (<https://github.com/dheijl/swyh-rs/releases>).
There is a debug build and a release build in the zip file. You will only need the debug build in the unlikely case rust "panics", and the program vanishes without a message. In a release build you will have a logging file in the swyh-rs folder in your home directory. But when rust "panics" you can't log it, so you will need to start the debug build from a console/terminal window. A debug build automatically raises the log level to "DEBUG". This will also allow you to catch the Rust "panic" message in the console window (release builds do not have a console on Windows). Depending on the log level you set (info/warn/debug) the release build will provide all information needed to help in troubleshooting, aside from "panics".

If it doesn't work for you, please open a new issue and include all the debug log level information. I will try to provide a fix ASAP.

### Todo:

- I'm open to suggestions, but I definitely hate GUI programming...

### Known problems:

- if your sound card has a forward slash (/) in the name, the "/" is replaced by "´´" in the sound source selection dropdown. The reason is purely technical: the FLTK MenuButton widget uses forward slashes in the text as a submenu indicator, so they have to be escaped to prevent this. 
  From the FLTK reference: _The text is split at '/' characters to automatically produce submenus (actually a totally unnecessary feature as you can now add submenu titles directly by setting FL_SUBMENU in the flags)._ Thanks go to @MoAlyousef who pointed this out to me.
- resizing a window in fltk 1.4 is not ideal, but thanks to @MoAlyousef it is now usable in swyh-rs. But if you resize vertically to a very small window you risk losing the horizontal scrollbar in the textbox at the bottom. 


### How does it work?

- first all media renderers are discoverded on the local network, this takes four seconds to complete
- then a button is shown for every renderer found
- audio is captured from the default audio device (WasApi on Windows, Alsa on Linux, not tested on Mac), but you can choose any audio source you want. Changing the sound source needs a restart of the app to take effect.
- a built-in audio streaming web server is started on port 5901.
- if you click the button for a renderer the OpenHome or AvTransport protocol is used to let the renderer play the captured audio from the webserver
- audio is always sent in audio/l16 PCM format, no matter the input source, using the sample rate of the source.
- some AVTtransport renderers will stop when detecting a pause between songs, you can use the "autoresume" checkbox if you encounter this problem.
- you can also enter the webserver url in the renderer, for instance in Volumio as a web radio: <http://{ip_address}:5901/stream/swyh.wav>, so that you can start playing from the Volumio UI if swyh-rs is already running
- the program runs at a priority "above normal" in the hope that using the computer for other stuff will not cause stuttering. 
- the SSDP discovery process is rerun every x minutes in the background, any newly discovered renderers will be automatically added to the GUI. Existing renderers that "disappear" during discovery are not deleted from the GUI, as SSDP discovery is not guaranteed to be failsafe (it uses UDP packets). The SSDP discovery interval is configurable, minimum value is 0.5 minutes, there is no maximum value.
- after a configuration change that needs a program restart, you get a "restart" button in the top right of the window. Click to restart the app.

### Audio quality and Windows WasApi Loopback capture

If you want maximum audio quality on Windows, there are a number of concerns:

- you should avoid resampling, because it affects audio quality. The sampling rate from the original audio source should be used to preserve quality. This means that you should make sure that the sampling frequency in the entire audio chain is the same (Use "Control Panel Sound" to check/change the sampling frequency). Bit depth does not really affect sound quality, and 16 bit *is* enough except if you are recording for mastering purposes in an audio lab. Deezer HiFi and Tidal HiFi use 16 bit 44100 Hz (lossless CD quality).
- on Windows, WasApi is used to capture audio. WasApi tries to capture directly from the hardware (soundcard) loopback if available, otherwise it uses the soundsource directly. In practice, this means that the soundcard loopback audio quality can be vastly inferior to the original soundsource (Realtek, Conexant, especially in laptops). Make sure all "effects" are disabled. The freeware/donationware VBAudio HiFi Cable driver (https://shop.vb-audio.com/en/win-apps/19-hifi-cable-asio-bridge.html?SubmitCurrency=1&id_currency=2) is an excellent solution to prevent this problem. Just make sure you configure it with the same sampling frequency as the default Windows audio source. You can then select HiFi Cable as the sound source in swyh-rs, and use the Windows Sound Mixer to route different apps to other sound drivers for Windows as needed (system sound etc). HiFi cable is a bit perfect pipe from the audio source to the renderer, except for the bit depth at this moment, because swyh-rs uses audio/l16 to stream to the network players, but this does not affect sound quality, it only limits the dynamic range to 96 dB which is fine for HiFi. You can also make HiFi cable the default output source, and configure other sound cards in the volume mixer for applications as needed.

### Screenshot:

![alt_tag](https://user-images.githubusercontent.com/2384545/98467438-95ce2d80-21d5-11eb-9be7-0c9f5b038a1e.PNG)

