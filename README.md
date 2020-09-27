# swyh-rs
Basic SWYH (https://www.streamwhatyouhear.com/) clone entirely written in rust.

Has only been tested with Volumio (https://volumio.org/) streamers. 

I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).

For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use HiFi Cable Input).
