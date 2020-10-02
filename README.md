# swyh-rs

Stream What You Hear written in Rust

Basic SWYH (https://www.streamwhatyouhear.com/, source at https://github.com/StreamWhatYouHear/SWYH) clone, entirely written in rust.

Has only been tested with Volumio (https://volumio.org/) streamers, but will probably support any streamer that supports the OpenHome protocol (not the original DLNA).

I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).

For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use HiFi Cable Input).

I had to fork cpal (https://github.com/RustAudio/cpal) to add missing functionality, so if you want to build swyh-rs yourself you have to clone dheijl/cpal from GitHub and change the cargo.toml file accordingly.

I use fltk-rs (https://github.com/MoAlyousef/fltk-rs) for the GUI, as it's easy to use, is cross-platform, is fast and works well.

Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi/HifI Berry based Volumio devices. I don't have access to a Mac, so I don't know if this also works.

Todo: 

- implement AVTransport for streamers that don't haven OpenHome support ?
- ... ?

How does it work?

- first all media renderers are discoverded on the local network, this takes four seconds to complete
- then a button is shown for every renderer found
- audio is captured from the default audio device (WasApi on Windows, Also on Linux, not tested on Mac)
- a built-in web server is started on port 5901. 
- if you click the button for a renderer the OpenHome protocol is used to let the renderer play the captured audio from the webserver
- audio is always sent in audio/l16 PCM format, no matter the input source, using the sample rate of the source
-  you can also enter the webserver url in the renderer, for instance in Volumie as a web radio: http://{ip_address}/stream/swyh.wav

Screenshot:

![alt_tag](https://user-images.githubusercontent.com/2384545/94679970-461c5c80-0321-11eb-8b70-ac34679f9cb3.PNG)
