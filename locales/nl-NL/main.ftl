# Window title
window-title = swyh-rs UPNP/DLNA streaming V{ $version }

# Configuration panel
config-options = Configuratie Opties
choose-color-theme = Kies kleur-thema
color-theme-label = Kleur-thema: { $name }
language-label = Taal: { $lang }
warn-language-changed = Taal gewijzigd in { $lang }, herstart nodig!!
active-network = Actief netwerk: { $addr }
new-network-label = Niew Network: { $name }
audio-source-label = Audio Bron: { $name }
new-audio-source-label = Nieuwe Audio Bron: { $name }

# Checkboxes and controls
chk-autoresume = Autom. hervatten
chk-autoreconnect = Auto-connecteren
ssdp-interval-label = SSDP Interval (in minuten)
log-level-label = Log Level: { $level }
fmt-label = Formaat: { $format }
chk-24bit = 24 bit
http-port-label = HTTP Poort:
chk-inject-silence = Injecteer stilte
strmsize-label = Streamsize: { $size }
buffer-label = Startbuffer (msec):
chk-rms-monitor = RMS Monitor
btn-apply-config = Klik om de wijzigingen toe te passen
upnp-devices = UPNP toestellen op netwerk { $addr }

# Tab-titels
tab-audio = Audio
tab-network = Netwerk
tab-app = App
tab-status = Status

# Status messages
status-setup-audio = Setup audio bronnen
status-injecting-silence = Injecteer stilte in de output stream
status-starting-ssdp = Starten SSDP discovery
status-ssdp-interval-zero = SSDP interval 0 => SSDP discovery wordt overgeslagen
status-loaded-config = Laden configuratie -c { $id }
status-serving-started = Server gestart op poort { $port }...
status-playing-to = Aspelen naar { $name }
status-shutting-down = Stoppen van { $name }
status-dry-run-exit = dry-run - einde...

status-new-renderer = Nieuwe speler { $name } op { $addr }

# Format / stream size change notifications
info-format-changed = huidig streaming formaat gewijzigd naar { $format }
info-streamsize-changed = StreamSize voor { $format } gewijzigd naar { $size }

# Warning messages (restart required)
warn-network-changed = Netwerk gewijzigd naar { $name }, herstarten nodig!!
warn-audio-changed = Audio bron gewijzigd naar { $name }, herstarten nodig!!
warn-ssdp-changed = SSDP interval gwijzigd naar { $interval } minuten, herstarten nodig!!
warn-log-changed = Log level gewijzigd naar { $level }, herstarten nodig!!

# Audio capture
audio-capturing-from = Audio afvangen van: { $name }
audio-default-config = Standaard audio { $cfg }
audio-capture-format = Audio sample formaat = { $fmt }
err-capture-format-stream = Fout bij afvangen van { $fmt } audio stream: { $error }
err-capture-stream = Fout { $error } bij afvangen van audio input stream
audio-capture-receiving = Audio afvangen ontvangt nu samples.

# FLAC encoder
err-flac-already-running = Flac encoder is al actief!
err-flac-cant-start = Kan de FLAC encoder niet starten
err-flac-start-error = Flac encoder start fout { $error }
flac-encoder-end = Flac encoder thread: einde.
flac-encoder-silence-end = Flac encoder thread (injecteert bijna-stilte): einde.
flac-encoder-exit = Flac encoder thread gestopt.
err-flac-spawn = Fout bij starten van Flac encoder thread: { $error }.

# Silence injection
err-inject-silence-stream = Injecteer stilte: een fout trad op op de audio output stream: { $error }
err-inject-silence-format = Injecteer stilte: niet-ondersteund sample formaat: { $format }
err-inject-silence-play = Kan de stilte-injectie stream niet afspelen.
err-inject-silence-build = Kan de stilte-injectie stream niet aanmaken: { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP: geen actief netwerk in configuratie.
err-ssdp-parse-ip = SSDP: Kan lokaal ip-adres niet parsen.
err-ssdp-bind = SSDP: Kan niet binden aan socket.
err-ssdp-broadcast = SSDP: Kan socket niet instellen op broadcast.
err-ssdp-ttl = SSDP: Kan DEFAULT_SEARCH_TTL niet instellen op socket.
err-ssdp-oh-send = SSDP: kan OpenHome discover bericht niet verzenden
err-ssdp-av-send = SSDP: kan AV Transport discover bericht niet verzenden

# Process priority
priority-nice = Nu actief met nice waarde -10
priority-above-normal = Nu actief met ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Fout bij instellen van processprioriteit naar ABOVE_NORMAL, fout={ $error }
err-priority-linux = Geen rechten om de processprioriteit te verhogen...

# Error messages
err-no-audio-device = geen default audio bron gevonden!
err-no-sound-source = Geen audio bron in config!
err-capture-audio = Kan de audio niet afvangen ...Controleer de configuratie.
err-play-stream = Kan de audio stream niet afspelen.
err-inject-silence = Kan geen stilte injecteren !!
err-ssdp-spawn = Fout bij starten van de SSDP discovery thread: { $error }
err-rms-spawn = Fout bij starten van de RMS monitor thread: { $error }
err-server-spawn = Fout bij starten van de HTTP Streaming Server thread: { $error }

# Debug build indicator
debug-build-warning = DEBUG build actief=> log level is nu DEBUG!

# CLI: audio source discovery
cli-found-audio-source = Audio bron gevonden: index = { $index }, naam = { $name }
cli-selected-audio-source-idx = Geselecteerde audio bron: { $name }[#{ $index }]
cli-selected-audio-source = Geselecteerde audio bron: { $name }
cli-selected-audio-source-pos = Geselecteerde audio bron: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network =Netwerk gevonden: { $ip }
cli-available-renderer = Beschikbare spelers #{ $n }: { $name } op { $addr }
cli-default-renderer-ip = Standaard speler ip: { $ip } => { $addr }
cli-active-renderer = Actieve speler: { $name } => { $addr }
cli-default-player-ip = Standaard speler ip = { $ip }
cli-no-renderers = Geen spelers gevonden!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = ^C ontvangen -> Einde.
cli-ctrlc-stopping = ^C: Stop afspelen naar { $name }
cli-ctrlc-no-connections = ^C: geen HTTP streaming connecties actief
cli-ctrlc-timeout = ^C: Time-out bij het wachten op HTTP streaming afbouwen - einde.

# Streaming server berichten
srv-listening = De streaming server luistert op http://{ $addr }/stream/swyh.wav
srv-default-streaming = Standaard streaming sample rate: { $rate }, bits per sample: { $bps }, formaat: { $format }
srv-start-error = Fout bij starten van server thread: { $error }
srv-thread-error = Server thread beëindigd met fout { $error }
srv-streaming-request = Streaming verzoek { $url } van { $addr }
srv-feedback-error = HTTP Server: fout bij schrijven naar feedback kanaal { $error }
srv-streaming-info = Streaming { $audio }, input sample formaat { $fmt }, kanalen=2, rate={ $rate }, bps = { $bps }, naar { $addr }
srv-http-terminated = =>Http verbinding met { $addr } beëindigd [{ $error }]
srv-streaming-ended = Streaming naar { $addr } is beëindigd
srv-head-terminated = =>Http HEAD verbinding met { $addr } beëindigd [{ $error }]
srv-unsupported-method = Niet-ondersteunde HTTP methode { $method } van { $addr }
srv-bad-request = Onherkenbaar verzoek '{ $url }' van '{ $addr }'
srv-stream-terminated = =>Http streaming verzoek met { $addr } beëindigd [{ $error }]
