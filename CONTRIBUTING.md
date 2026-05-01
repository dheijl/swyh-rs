# Contributing

Contributions are very welcome! Even if just for submitting bug fixes, or improving the documentation.

## Submission

Please fork this project, commit your changes in a branch of your fork, then start a merge request.

## Development environment setup

### Prerequisites

As this is a Rust project, you need to have standard Rust tooling installed. Please refer to the [official documentation](https://rust-lang.org/) for more information.

You also need to install prerequisites of swyh-rs' dependencies (listed in Cargo.toml). At the time of writing, on Linux this can be performed with:

```sh
# libraries for compiling fltk-rs
sudo apt-get install libx11-dev libxext-dev libxft-dev libxinerama-dev libxcursor-dev libxrender-dev libxfixes-dev libpango1.0-dev libgl1-mesa-dev libglu1-mesa-dev
# libasound2-dev for compiling the rust cpal sound library
sudo apt-get install libasound2-dev
```

### Networking

To be able to test the application, you need to allow certain types of network traffic related to DLNA/UPnP and streaming with swyh-rs. On Linux using `ufw`, this can be ensured with:

```sh
sudo ufw allow in from 192.168.0.0/24 to any port 1900 proto udp
sudo ufw allow in from fc00::/7 to any port 1900 proto udp
sudo ufw allow in from 192.168.0.0/24 to any port 32000:60000 proto udp
sudo ufw allow in from fc00::/7 to any port 32000:60000 proto udp
sudo ufw allow in from 192.168.0.0/24 to any port 5901  proto tcp
sudo ufw allow in from fc00::/7 to any port 5901 proto tcp
sudo ufw reload
```
