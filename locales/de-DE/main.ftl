# Window title
window-title = swyh-rs UPNP/DLNA Streaming V{ $version }

# Configuration panel
config-options = Konfiguration
choose-color-theme = Farbschema wählen
color-theme-label = Schema: { $name }
language-label = Sprache: { $lang }
warn-language-changed = Sprache auf { $lang } geändert, Neustart erforderlich!!
active-network = Netzwerk: { $addr }
new-network-label = Neues Netzwerk: { $name }
audio-source-label = Audioquelle: { $name }
new-audio-source-label = Neue Audioquelle: { $name }

# Checkboxes and controls
chk-autoresume = Autofortsetzung
chk-autoreconnect = Autoreconnect
ssdp-interval-label = SSDP-Intervall (Min.)
log-level-label = Log: { $level }
fmt-label = Format: { $format }
chk-24bit = 24 Bit
http-port-label = HTTP-Port:
chk-inject-silence = Stille einspeisen
strmsize-label = StrmGröße: { $size }
buffer-label = Startpuffer (ms):
chk-rms-monitor = RMS-Monitor
btn-apply-config = Konfiguration übernehmen
upnp-devices = UPNP-Geräte im Netzwerk { $addr }

# Tab-Titel
tab-audio = Audio
tab-network = Netzwerk
tab-app = App
tab-status = Status

# Status messages
status-setup-audio = Audioquellen werden eingerichtet
status-injecting-silence = Stille wird in den Ausgabestrom eingespeist
status-starting-ssdp = SSDP-Erkennung wird gestartet
status-ssdp-interval-zero = SSDP-Intervall 0 => SSDP-Erkennung wird übersprungen
status-loaded-config = Konfiguration geladen -c { $id }
status-serving-started = Server gestartet auf Port { $port }...
status-playing-to = Wiedergabe auf { $name }
status-shutting-down = { $name } wird beendet
status-dry-run-exit = dry-run - wird beendet...
status-new-renderer = Neues Gerät { $name } unter { $addr }

# Format / stream size change notifications
info-format-changed = Streaming-Format geändert auf { $format }
info-streamsize-changed = StrmGröße für { $format } geändert auf { $size }

# Warning messages (restart required)
warn-network-changed = Netzwerk auf { $name } geändert, Neustart erforderlich!!
warn-audio-changed = Audioquelle auf { $name } geändert, Neustart erforderlich!!
warn-ssdp-changed = SSDP-Intervall auf { $interval } Min. geändert, Neustart erforderlich!!
warn-log-changed = Log-Level auf { $level } geändert, Neustart erforderlich!!

# Audio capture
audio-capturing-from = Audioaufnahme von: { $name }
audio-default-config = Standard-Audio { $cfg }
audio-capture-format = Audio-Aufnahmeformat = { $fmt }
err-capture-format-stream = Fehler bei der Aufnahme des { $fmt }-Audiostroms: { $error }
err-capture-stream = Fehler { $error } bei der Aufnahme des Audioeingabestroms
audio-capture-receiving = Audioaufnahme empfängt jetzt Samples.

# FLAC encoder
err-flac-already-running = FLAC-Encoder läuft bereits!
err-flac-cant-start = FLAC-Encoder kann nicht gestartet werden
err-flac-start-error = FLAC-Encoder Startfehler { $error }
flac-encoder-end = FLAC-Encoder-Thread: Ende.
flac-encoder-silence-end = FLAC-Encoder-Thread (Stille einspeisen): Ende.
flac-encoder-exit = FLAC-Encoder-Thread beendet.
err-flac-spawn = FLAC-Encoder-Thread konnte nicht gestartet werden: { $error }.

# Silence injection
err-inject-silence-stream = Stille einspeisen: Fehler im Audioausgabestrom: { $error }
err-inject-silence-format = Stille einspeisen: Nicht unterstütztes Sample-Format: { $format }
err-inject-silence-play = Stille-Einspeisung kann nicht wiedergegeben werden.
err-inject-silence-build = Stille-Einspeisung konnte nicht erstellt werden: { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP: Kein aktives Netzwerk in der Konfiguration.
err-ssdp-parse-ip = SSDP: Lokale IP-Adresse konnte nicht analysiert werden.
err-ssdp-bind = SSDP: Socket konnte nicht gebunden werden.
err-ssdp-broadcast = SSDP: Socket konnte nicht auf Broadcast gesetzt werden.
err-ssdp-ttl = SSDP: DEFAULT_SEARCH_TTL konnte nicht am Socket gesetzt werden.
err-ssdp-oh-send = SSDP: OpenHome-Erkennungsnachricht konnte nicht gesendet werden
err-ssdp-av-send = SSDP: AV-Transport-Erkennungsnachricht konnte nicht gesendet werden

# Process priority
priority-nice = Läuft jetzt mit Nice-Wert -10
priority-above-normal = Läuft jetzt mit ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Prozesspriorität ABOVE_NORMAL konnte nicht gesetzt werden, Fehler={ $error }
err-priority-linux = Keine Berechtigung zum Erhöhen der Prozesspriorität.

# Error messages
err-no-audio-device = Kein Standard-Audiogerät gefunden!
err-no-sound-source = Keine Audioquelle in der Konfiguration!
err-capture-audio = Audio konnte nicht aufgenommen werden... Bitte Konfiguration prüfen.
err-play-stream = Audiostrom kann nicht wiedergegeben werden.
err-inject-silence = Stille kann nicht eingespeist werden!!
err-ssdp-spawn = SSDP-Erkennungs-Thread konnte nicht gestartet werden: { $error }
err-rms-spawn = RMS-Monitor-Thread konnte nicht gestartet werden: { $error }
err-server-spawn = HTTP-Streaming-Server-Thread konnte nicht gestartet werden: { $error }

# Debug build indicator
debug-build-warning = DEBUG-Build => Log-Level auf DEBUG gesetzt!

# CLI: audio source discovery
cli-found-audio-source = Audioquelle gefunden: Index = { $index }, Name = { $name }
cli-selected-audio-source-idx = Gewählte Audioquelle: { $name }[#{ $index }]
cli-selected-audio-source = Gewählte Audioquelle: { $name }
cli-selected-audio-source-pos = Gewählte Audioquelle: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Netzwerk gefunden: { $ip }
cli-available-renderer = Verfügbares Gerät #{ $n }: { $name } unter { $addr }
cli-default-renderer-ip = Standard-Gerät IP: { $ip } => { $addr }
cli-active-renderer = Aktives Gerät: { $name } => { $addr }
cli-default-player-ip = Standard-Player-IP = { $ip }
cli-no-renderers = Keine Geräte gefunden!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = ^C empfangen -> wird beendet.
cli-ctrlc-stopping = ^C: Streaming zu { $name } wird gestoppt
cli-ctrlc-no-connections = ^C: Keine aktiven HTTP-Streaming-Verbindungen
cli-ctrlc-timeout = ^C: Zeitüberschreitung beim Beenden des HTTP-Streamings - wird beendet.

# Streaming server messages
srv-listening = Streaming-Server lauscht auf http://{ $addr }/stream/swyh.wav
srv-default-streaming = Standard-Abtastrate: { $rate }, Bits pro Sample: { $bps }, Format: { $format }
srv-start-error = Fehler beim Starten des Server-Threads: { $error }
srv-thread-error = Server-Thread mit Fehler beendet { $error }
srv-streaming-request = Streaming-Anfrage { $url } von { $addr }
srv-feedback-error = HTTP-Server: Fehler beim Schreiben in den Rückkanal { $error }
srv-streaming-info = Streaming { $audio }, Eingabe-Format { $fmt }, Kanäle=2, Rate={ $rate }, bps = { $bps }, an { $addr }
srv-http-terminated = =>HTTP-Verbindung mit { $addr } beendet [{ $error }]
srv-streaming-ended = Streaming zu { $addr } beendet
srv-head-terminated = =>HTTP-HEAD-Verbindung mit { $addr } beendet [{ $error }]
srv-unsupported-method = Nicht unterstützte HTTP-Methode { $method } von { $addr }
srv-bad-request = Unbekannte Anfrage '{ $url }' von '{ $addr }'
srv-stream-terminated = =>HTTP-Streaming-Anfrage mit { $addr } beendet [{ $error }]
