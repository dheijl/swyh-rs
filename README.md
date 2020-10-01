# swyh-rs
Basic SWYH (https://www.streamwhatyouhear.com/, source repo https://github.com/StreamWhatYouHear/SWYH) clone entirely written in rust.

Has only been tested with Volumio (https://volumio.org/) streamers, but will probably support any streamer that supports the OpenHome protocol (not the original DLNA).

I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).

For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use HiFi Cable Input).

I had to fork cpal (https://github.com/RustAudio/cpal) to add missing functionality, so if you want to build swyh-rs yourself you have to clone dheijl/cpal from GitHub and change the cargo.toml file accordingly.

I use fltk-rs (https://github.com/MoAlyousef/fltk-rs) for the GUI, as it's easy to use and works well.

Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi/HifI Berry based Volumio devices. I don't have access to a Mac, so I don't know if this also works.

Todo: 

- ... ?

Screenshot:

![alt_tag](https://user-images.githubusercontent.com/2384545/94679970-461c5c80-0321-11eb-8b70-ac34679f9cb3.PNG)
