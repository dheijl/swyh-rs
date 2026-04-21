# Window title
window-title = swyh-rs UPNP/DLNA streaming V{ $version }

# Configuration panel
config-options = Configuration Options
choose-color-theme = Choose Color Theme
color-theme-label = Color theme: { $name }
language-label = Language: { $lang }
warn-language-changed = Language changed to { $lang }, restart required!!
active-network = Active network: { $addr }
new-network-label = New Network: { $name }
audio-source-label = Audio Source: { $name }
new-audio-source-label = New Audio Source: { $name }

# Checkboxes and controls
chk-autoresume = Autoresume play
chk-autoreconnect = Autoreconnect
ssdp-interval-label = SSDP Interval (in minutes)
log-level-label = Log Level: { $level }
fmt-label = FMT: { $format }
chk-24bit = 24 bit
http-port-label = HTTP Port:
chk-inject-silence = Inject silence
strmsize-label = StrmSize: { $size }
buffer-label = Initial buffer (msec):
chk-rms-monitor = RMS Monitor
btn-apply-config = Press to apply configuration changes
upnp-devices = UPNP rendering devices on network { $addr }

# Status messages
status-setup-audio = Setup audio sources
status-injecting-silence = Injecting silence into the output stream
status-starting-ssdp = Starting SSDP discovery
status-ssdp-interval-zero = SSDP interval 0 => Skipping SSDP discovery
status-loaded-config = Loaded configuration -c { $id }
status-serving-started = Serving started on port { $port }...
status-playing-to = Playing to { $name }
status-shutting-down = Shutting down { $name }
status-dry-run-exit = dry-run - exiting...
status-new-renderer = New renderer { $name } at { $addr }

# Format / stream size change notifications
info-format-changed = Current streaming Format changed to { $format }
info-streamsize-changed = StreamSize for { $format } changed to { $size }

# Warning messages (restart required)
warn-network-changed = Network changed to { $name }, restart required!!
warn-audio-changed = Audio source changed to { $name }, restart required!!
warn-ssdp-changed = SSDP interval changed to { $interval } minutes, restart required!!
warn-log-changed = Log level changed to { $level }, restart required!!

# Error messages
err-no-audio-device = No default audio device found!
err-no-sound-source = No sound source in config!
err-capture-audio = Could not capture audio ...Please check configuration.
err-play-stream = Unable to play audio stream.
err-inject-silence = Unable to inject silence !!
err-ssdp-spawn = Unable to spawn SSDP discovery thread: { $error }
err-rms-spawn = Unable to spawn RMS monitor thread: { $error }
err-server-spawn = Unable to spawn HTTP Streaming Server thread: { $error }

# Debug build indicator
debug-build-warning = Running DEBUG build => log level set to DEBUG!

# CLI: audio source discovery
cli-found-audio-source = Found Audio Source: index = { $index }, name = { $name }
cli-selected-audio-source-idx = Selected audio source: { $name }[#{ $index }]
cli-selected-audio-source = Selected audio source: { $name }
cli-selected-audio-source-pos = Selected audio source: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Found network: { $ip }
cli-available-renderer = Available renderer #{ $n }: { $name } at { $addr }
cli-default-renderer-ip = Default renderer ip: { $ip } => { $addr }
cli-active-renderer = Active renderer: { $name } => { $addr }
cli-default-player-ip = Default player ip = { $ip }
cli-no-renderers = No renderers found!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = Received ^C -> exiting.
cli-ctrlc-stopping = ^C: Stopping streaming to { $name }
cli-ctrlc-no-connections = ^C: No HTTP streaming connections active
cli-ctrlc-timeout = ^C: Time-out waiting for HTTP streaming shutdown - exiting.
