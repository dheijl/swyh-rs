# swyh-rs

Stream What You Hear written in Rust

swyh-rs is a very basic SWYH clone ( see <https://www.streamwhatyouhear.com/>, source repo <https://github.com/StreamWhatYouHear/SWYH>), entirely written in rust.

It has only been tested with Volumio (<https://volumio.org/>) and Harman Kardon (thanks @MX10-AC2N!) streamers at this moment, but will probably support any streamer that supports the OpenHome or AVTransport protocol.

I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).

For the moment all music is streamed in uncompressed LPCM format (audio/l16) with the sample rate of the music source (the default audio output device, I personally use VBAudio HiFi Cable Input).

I had to fork cpal (<https://github.com/RustAudio/cpal>) to add missing functionality (Windows WasApi loopback capture). I'm waiting for a cpal pull request to be merged.

I use fltk-rs (<https://github.com/MoAlyousef/fltk-rs>) for the GUI, as it's easy to use, is cross-platform, is fast and works well. I'm currently using the github repo version of fltk-rs (not the latest published crate), as it fixes a multithreading problem I had.

Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi/HifI Berry based Volumio devices. I don't have access to a Mac, so I don't know if this also works.

You can get the latest Windows binary from the Release page (<https://github.com/dheijl/swyh-rs/releases>).

If it doesn't work for you, please run the debug exe from the zip file (swyh-rs-deb.exe), this will give you a console window with loads of debug information.  Please open a new issue and include all this debug information. I will try to provide a fix ASAP.

### Todo:

- implement file logging in the release build, similar to the console log in the debug build
- probably some GUI tweaks, but I hate GUI programming


### How does it work?

- first all media renderers are discoverded on the local network, this takes four seconds to complete
- then a button is shown for every renderer found
- audio is captured from the default audio device (WasApi on Windows, Alsa on Linux, not tested on Mac), but you can choose any audio source you want. Changing the sound source needs a restart of the app to take effect.
- a built-in web server is started on port 5901.
- if you click the button for a renderer the OpenHome or AvTransport protocol is used to let the renderer play the captured audio from the webserver
- audio is always sent in audio/l16 PCM format, no matter the input source, using the sample rate of the source.
- some AVTtransport renderers will stop when detecting a pause between songs, you can use the "autoresume" checkbox if you encounter this problem.
- you can also enter the webserver url in the renderer, for instance in Volumio as a web radio: <http://{ip_address}/stream/swyh.wav>, so that you can start playing from the Volumio UI if swyh-rs is already running
- the program runs at a priority "above normal" in the hope that using the computer for other stuff will not cause stuttering

### Screenshot:

![alt_tag](https://user-images.githubusercontent.com/2384545/96004784-18183b80-0e3c-11eb-8229-8584f80569b2.PNG)

