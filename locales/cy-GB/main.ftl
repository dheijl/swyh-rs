# Window title
window-title = swyh-rs ffrydio UPNP/DLNA V{ $version }

# Configuration panel
config-options = Dewisiadau Ffurfweddu
choose-color-theme = Dewis Thema Lliw
color-theme-label = Thema lliw: { $name }
widget-style-label = Arddull teclynnau: { $style }
language-label = Iaith: { $lang }
warn-language-changed = Iaith wedi newid i { $lang }, rhaid ailgychwyn!!
warn-widget-style-changed = Arddull teclynnau wedi newid i { $style }, rhaid ailgychwyn!!
active-network = Rhwydwaith gweithredol: { $addr }
new-network-label = Rhwydwaith Newydd: { $name }
audio-source-label = Ffynhonnell Sain: { $name }
new-audio-source-label = Ffynhonnell Sain Newydd: { $name }

# Checkboxes and controls
chk-autoresume = Ailddechrau chwarae'n awtomatig
chk-autoreconnect = Ailgysylltu'n awtomatig
chk-enable-slimproto = Galluogi SlimProto
ssdp-interval-label = Cyfwng SSDP (mewn munudau)
ssdp-interval-tooltip = 0 = analluogi canfod UPnP/DLNA (rhaid ailgychwyn)
btn-ssdp-discover = Rhedeg canfod SSDP nawr
log-level-label = Lefel Log: { $level }
fmt-label = Fformat: { $format }
chk-24bit = 24 did
sample-rate-label = Cyfradd Samplu (Hz):
sr-system-default = Rhagosodiad y System ({ $rate } Hz)
http-port-label = Porth HTTP:
chk-use-dither = Dither TPDF 16-did
chk-inject-silence = Chwistrellu distawrwydd
strmsize-label = Maint ffrwd: { $size }
buffer-label = Byffer cychwynnol (msec):
latency-reduction-delay-label = Oedi lleihau hwyrni (msec):
latency-reduction-label = swm (msec):
latency-reduction-status-label = Lleihau hwyrni: { $amount } ms ar ôl { $delay } ms
chk-rms-monitor = Monitor RMS
btn-apply-config = Pwyswch i roi'r newidiadau ffurfweddu ar waith
upnp-devices = Dyfeisiau rendro UPNP ar rwydwaith { $addr }

# Tab titles
tab-audio = Sain
tab-network = Rhwydwaith
tab-app = Ap
tab-status = Statws

# Status messages
status-setup-audio = Gosod ffynonellau sain
status-injecting-silence = Yn chwistrellu distawrwydd i'r ffrwd allbwn
status-starting-ssdp = Yn cychwyn canfod SSDP
status-ssdp-interval-zero = Cyfwng SSDP 0 => Hepgor canfod SSDP
status-starting-slimproto = Yn cychwyn canfod SlimProto
status-slimproto-disabled = Canfod SlimProto wedi'i analluogi
status-loaded-config = Ffurfweddiad wedi'i lwytho -c { $id }
status-serving-started = Gweini wedi cychwyn ar borth { $port }...
status-playing-to = Yn chwarae i { $name }
status-shutting-down = Yn cau { $name }
status-dry-run-exit = rhediad prawf - yn gadael...
status-new-renderer = Rendrwr newydd { $name } yn { $addr }

# Format / stream size change notifications
info-format-changed = Fformat ffrydio cyfredol wedi newid i { $format }
info-streamsize-changed = Maint ffrwd ar gyfer { $format } wedi newid i { $size }

# Warning messages (restart required)
warn-network-changed = Rhwydwaith wedi newid i { $name }, rhaid ailgychwyn!!
warn-audio-changed = Ffynhonnell sain wedi newid i { $name }, rhaid ailgychwyn!!
warn-ssdp-changed = Cyfwng SSDP wedi newid i { $interval } munud, rhaid ailgychwyn!!
warn-slimproto-changed = Cefnogaeth SlimProto wedi newid, rhaid ailgychwyn!!
warn-log-changed = Lefel log wedi newid i { $level }, rhaid ailgychwyn!!

# Audio capture
audio-capturing-from = Yn cipio sain o: { $name }
audio-default-config = Sain ragosodedig { $cfg }
audio-capture-format = Fformat samplau cipio sain = { $fmt }
err-capture-format-stream = Gwall wrth gipio ffrwd sain { $fmt }: { $error }
err-capture-stream = Gwall { $error } wrth gipio'r ffrwd sain fewnbwn
audio-capture-receiving = Mae cipio sain bellach yn derbyn samplau.
audio-downmix = Yn cymysgu mewnbwn { $channels } sianel i lawr i stereo ar gyfer ffrydio (ITU-R BS.775)

# FLAC encoder
err-flac-already-running = Mae'r amgodiwr Flac eisoes yn rhedeg!
err-flac-cant-start = Methu cychwyn yr amgodiwr FLAC
err-flac-start-error = Gwall cychwyn yr amgodiwr Flac { $error }
flac-encoder-end = Edefyn amgodiwr Flac: diwedd.
flac-encoder-silence-end = Edefyn amgodiwr Flac (yn chwistrellu bron-ddistawrwydd): diwedd.
flac-encoder-exit = Edefyn amgodiwr Flac yn gadael.
err-flac-spawn = Methu creu edefyn amgodiwr Flac: { $error }.

# Silence injection
err-inject-silence-stream = Chwistrellu distawrwydd: digwyddodd gwall ar y ffrwd sain allbwn: { $error }
err-inject-silence-format = Chwistrellu distawrwydd: Fformat samplau heb ei gefnogi: { $format }
err-inject-silence-play = Methu chwarae'r ffrwd chwistrellu distawrwydd.
err-inject-silence-build = Methu adeiladu'r ffrwd chwistrellu distawrwydd: { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP: dim rhwydwaith gweithredol yn y ffurfweddiad.
err-ssdp-parse-ip = SSDP: Methu dosrannu'r cyfeiriad ip lleol.
err-ssdp-bind = SSDP: Methu rhwymo i'r soced.
err-ssdp-broadcast = SSDP: Methu gosod y soced i ddarlledu.
err-ssdp-ttl = SSDP: Methu gosod DEFAULT_SEARCH_TTL ar y soced.
err-ssdp-oh-send = SSDP: methu anfon neges canfod OpenHome
err-ssdp-av-send = SSDP: methu anfon neges canfod AV Transport

# Process priority
priority-nice = Bellach yn rhedeg gyda gwerth nice -10
priority-above-normal = Bellach yn rhedeg gydag ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Methu gosod blaenoriaeth y broses i ABOVE_NORMAL, gwall={ $error }
err-priority-linux = Mae'n ddrwg gennym, ond nid oes gennych ganiatâd i godi'r flaenoriaeth...

# Error messages
err-no-audio-device = Ni chanfuwyd dyfais sain ragosodedig!
warn-local-address-fallback = Ni chanfuwyd llwybr rhyngrwyd, yn defnyddio cyfeiriad rhwydwaith lleol { $addr }
err-local-address-loopback = Methu cael cyfeiriad rhwydwaith lleol! Dim ond ar { $addr } mae ffrydio ar gael
err-capture-audio = Methu cipio sain ...Gwiriwch y ffurfweddiad.
err-play-stream = Methu chwarae'r ffrwd sain.
err-inject-silence = Methu chwistrellu distawrwydd !!
err-ssdp-spawn = Methu creu edefyn canfod SSDP: { $error }
err-rms-spawn = Methu creu edefyn monitor RMS: { $error }
err-server-spawn = Methu creu edefyn y Gweinydd Ffrydio HTTP: { $error }

# Debug build indicator
debug-build-warning = Yn rhedeg adeiliad DEBUG => lefel log wedi'i gosod i DEBUG!

# CLI: audio source discovery
cli-found-audio-source = Wedi canfod Ffynhonnell Sain: mynegai = { $index }, enw = { $name }
cli-selected-audio-source-idx = Ffynhonnell sain a ddewiswyd: { $name }[#{ $index }]
cli-selected-audio-source = Ffynhonnell sain a ddewiswyd: { $name }
cli-selected-audio-source-pos = Ffynhonnell sain a ddewiswyd: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Wedi canfod rhwydwaith: { $ip }
cli-available-renderer = Rendrwr ar gael #{ $n }: { $name } yn { $addr }
cli-default-renderer-ip = Ip rendrwr rhagosodedig: { $ip } => { $addr }
cli-active-renderer = Rendrwr gweithredol: { $name } => { $addr }
cli-default-player-ip = Ip chwaraewr rhagosodedig = { $ip }
cli-no-renderers = Ni chanfuwyd unrhyw rendrwyr!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = Derbyniwyd ^C -> yn gadael.
cli-ctrlc-stopping = ^C: Yn atal ffrydio i { $name }
cli-ctrlc-no-connections = ^C: Dim cysylltiadau ffrydio HTTP gweithredol
cli-ctrlc-timeout = ^C: Terfyn amser wrth aros i ffrydio HTTP gau - yn gadael.

# Streaming server messages
srv-listening = Mae'r gweinydd ffrydio yn gwrando ar http://{ $addr }/stream/swyh.wav
srv-default-streaming = Cyfradd samplu ffrydio ragosodedig: { $rate }, didau fesul sampl: { $bps }, fformat: { $format }
srv-start-error = Gwall wrth gychwyn edefyn y gweinydd: { $error }
srv-thread-error = Daeth edefyn y gweinydd i ben gyda gwall { $error }
srv-streaming-request = Cais ffrydio { $url } o { $addr }
srv-feedback-error = Gweinydd HTTP: gwall wrth ysgrifennu i'r sianel adborth { $error }
srv-streaming-info = Yn ffrydio { $audio }, fformat samplau mewnbwn { $fmt }, sianeli=2, cyfradd={ $rate }, bps = { $bps }, i { $addr }
srv-http-terminated = =>Daeth y cysylltiad Http â { $addr } i ben [{ $error }]
srv-streaming-ended = Mae ffrydio i { $addr } wedi dod i ben
srv-head-terminated = =>Daeth y cysylltiad Http HEAD â { $addr } i ben [{ $error }]
srv-unsupported-method = Cais dull HTTP heb ei gefnogi { $method } o { $addr }
srv-bad-request = Cais anhysbys '{ $url }' o '{ $addr }'
srv-stream-terminated = =>Daeth y cais ffrydio Http â { $addr } i ben [{ $error }]

srv-range-not-satisfiable = Ni ellir bodloni cais ystod o { $addr }, yn ymateb 416

# About dialog
btn-about = Ynghylch...
about-title = Ynghylch swyh-rs
about-tagline = Ffrydiwch Beth Rydych Chi'n ei Glywed, wedi'i ysgrifennu yn Rust
about-fltk = Mae swyh-rs yn defnyddio pecyn cymorth GUI FLTK (https://www.fltk.org) drwy fltk-rs.
about-third-party = Trwyddedau trydydd parti: gweler THIRD-PARTY-LICENSES.md
