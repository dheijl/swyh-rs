# Window title
window-title = swyh-rs UPNP/DLNA Streaming V{ $version }

# Configuration panel
config-options = Opzioni di configurazione
choose-color-theme = Scegli tema colori
color-theme-label = Tema: { $name }
language-label = Lingua: { $lang }
warn-language-changed = Lingua cambiata in { $lang }, riavvio richiesto!!
active-network = Rete attiva: { $addr }
new-network-label = Nuova rete: { $name }
audio-source-label = Sorgente audio: { $name }
new-audio-source-label = Nuova sorgente audio: { $name }

# Checkboxes and controls
chk-autoresume = Ripresa automatica
chk-autoreconnect = Auto-Riconnessione
ssdp-interval-label = Intervallo SSDP (minuti)
log-level-label = Log: { $level }
fmt-label = FMT: { $format }
chk-24bit = 24 bit
http-port-label = Porta HTTP:
chk-inject-silence = Inserisci silenzio
strmsize-label = DimFlusso: { $size }
buffer-label = Buffer iniziale (ms):
chk-rms-monitor = Monitor RMS
btn-apply-config = Applica modifiche alla configurazione
upnp-devices = Dispositivi UPNP sulla rete { $addr }

# Titoli delle schede
tab-audio = Audio
tab-network = Rete
tab-app = App
tab-status = Stato

# Status messages
status-setup-audio = Configurazione sorgenti audio
status-injecting-silence = Inserimento silenzio nel flusso di uscita
status-starting-ssdp = Avvio rilevamento SSDP
status-ssdp-interval-zero = Intervallo SSDP 0 => Rilevamento SSDP saltato
status-loaded-config = Configurazione caricata -c { $id }
status-serving-started = Server avviato sulla porta { $port }...
status-playing-to = Riproduzione su { $name }
status-shutting-down = Arresto di { $name }
status-dry-run-exit = dry-run - uscita in corso...
status-new-renderer = Nuovo dispositivo { $name } su { $addr }

# Format / stream size change notifications
info-format-changed = Formato di streaming cambiato in { $format }
info-streamsize-changed = DimFlusso per { $format } cambiata in { $size }

# Warning messages (restart required)
warn-network-changed = Rete cambiata in { $name }, riavvio richiesto!!
warn-audio-changed = Sorgente audio cambiata in { $name }, riavvio richiesto!!
warn-ssdp-changed = Intervallo SSDP cambiato in { $interval } minuti, riavvio richiesto!!
warn-log-changed = Livello di log cambiato in { $level }, riavvio richiesto!!

# Audio capture
audio-capturing-from = Acquisizione audio da: { $name }
audio-default-config = Audio predefinito { $cfg }
audio-capture-format = Formato di acquisizione audio = { $fmt }
err-capture-format-stream = Errore durante l'acquisizione del flusso audio { $fmt }: { $error }
err-capture-stream = Errore { $error } durante l'acquisizione del flusso audio in ingresso
audio-capture-receiving = L'acquisizione audio sta ricevendo campioni.

# FLAC encoder
err-flac-already-running = Il codificatore FLAC è già in esecuzione!
err-flac-cant-start = Impossibile avviare il codificatore FLAC
err-flac-start-error = Errore di avvio del codificatore FLAC { $error }
flac-encoder-end = Thread codificatore FLAC: fine.
flac-encoder-silence-end = Thread codificatore FLAC (inserimento silenzio): fine.
flac-encoder-exit = Thread codificatore FLAC terminato.
err-flac-spawn = Impossibile avviare il thread del codificatore FLAC: { $error }.

# Silence injection
err-inject-silence-stream = Inserimento silenzio: errore nel flusso audio di uscita: { $error }
err-inject-silence-format = Inserimento silenzio: formato campione non supportato: { $format }
err-inject-silence-play = Impossibile riprodurre il flusso di silenzio.
err-inject-silence-build = Impossibile creare il flusso di silenzio: { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP: nessuna rete attiva nella configurazione.
err-ssdp-parse-ip = SSDP: impossibile analizzare l'indirizzo IP locale.
err-ssdp-bind = SSDP: impossibile associare il socket.
err-ssdp-broadcast = SSDP: impossibile impostare il socket in modalità broadcast.
err-ssdp-ttl = SSDP: impossibile impostare DEFAULT_SEARCH_TTL sul socket.
err-ssdp-oh-send = SSDP: impossibile inviare il messaggio di rilevamento OpenHome
err-ssdp-av-send = SSDP: impossibile inviare il messaggio di rilevamento AV Transport

# Process priority
priority-nice = In esecuzione con valore nice -10
priority-above-normal = In esecuzione con ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Impossibile impostare la priorità del processo su ABOVE_NORMAL, errore={ $error }
err-priority-linux = Autorizzazioni insufficienti per aumentare la priorità del processo.

# Error messages
err-no-audio-device = Nessun dispositivo audio predefinito trovato!
err-no-sound-source = Nessuna sorgente audio nella configurazione!
err-capture-audio = Impossibile acquisire l'audio... Verificare la configurazione.
err-play-stream = Impossibile riprodurre il flusso audio.
err-inject-silence = Impossibile inserire il silenzio!!
err-ssdp-spawn = Impossibile avviare il thread di rilevamento SSDP: { $error }
err-rms-spawn = Impossibile avviare il thread del monitor RMS: { $error }
err-server-spawn = Impossibile avviare il thread del server HTTP di streaming: { $error }

# Debug build indicator
debug-build-warning = Build DEBUG in esecuzione => livello di log impostato su DEBUG!

# CLI: audio source discovery
cli-found-audio-source = Sorgente audio trovata: indice = { $index }, nome = { $name }
cli-selected-audio-source-idx = Sorgente audio selezionata: { $name }[#{ $index }]
cli-selected-audio-source = Sorgente audio selezionata: { $name }
cli-selected-audio-source-pos = Sorgente audio selezionata: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Rete trovata: { $ip }
cli-available-renderer = Dispositivo disponibile #{ $n }: { $name } su { $addr }
cli-default-renderer-ip = IP dispositivo predefinito: { $ip } => { $addr }
cli-active-renderer = Dispositivo attivo: { $name } => { $addr }
cli-default-player-ip = IP player predefinito = { $ip }
cli-no-renderers = Nessun dispositivo trovato!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = Ricevuto ^C -> uscita in corso.
cli-ctrlc-stopping = ^C: Arresto dello streaming verso { $name }
cli-ctrlc-no-connections = ^C: Nessuna connessione HTTP di streaming attiva
cli-ctrlc-timeout = ^C: Timeout in attesa dell'arresto dello streaming HTTP - uscita in corso.

# Streaming server messages
srv-listening = Il server di streaming è in ascolto su http://{ $addr }/stream/swyh.wav
srv-default-streaming = Frequenza di campionamento predefinita: { $rate }, bit per campione: { $bps }, formato: { $format }
srv-start-error = Errore durante l'avvio del thread del server: { $error }
srv-thread-error = Thread del server terminato con errore { $error }
srv-streaming-request = Richiesta di streaming { $url } da { $addr }
srv-feedback-error = Server HTTP: errore nella scrittura del canale di feedback { $error }
srv-streaming-info = Streaming { $audio }, formato ingresso { $fmt }, canali=2, frequenza={ $rate }, bps = { $bps }, verso { $addr }
srv-http-terminated = =>Connessione HTTP con { $addr } terminata [{ $error }]
srv-streaming-ended = Streaming verso { $addr } terminato
srv-head-terminated = =>Connessione HTTP HEAD con { $addr } terminata [{ $error }]
srv-unsupported-method = Metodo HTTP non supportato { $method } da { $addr }
srv-bad-request = Richiesta non riconosciuta '{ $url }' da '{ $addr }'
srv-stream-terminated = =>Richiesta di streaming HTTP con { $addr } terminata [{ $error }]
