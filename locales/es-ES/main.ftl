# Window title
window-title = swyh-rs UPNP/DLNA streaming V{ $version }

# Configuration panel
config-options = Opciones de configuración
choose-color-theme = Elegir tema de color
color-theme-label = Tema de color: { $name }
language-label = Idioma: { $lang }
warn-language-changed = Idioma cambiado a { $lang }, ¡¡se requiere reinicio!!
active-network = Red activa: { $addr }
new-network-label = Nueva red: { $name }
audio-source-label = Fuente de audio: { $name }
new-audio-source-label = Nueva fuente de audio: { $name }

# Checkboxes and controls
chk-autoresume = Reanudar automáticamente
chk-autoreconnect = Reconexión automática
ssdp-interval-label = Intervalo SSDP (minutos)
btn-ssdp-discover = Ejecutar descubrimiento SSDP ahora
log-level-label = Nivel de registro: { $level }
fmt-label = Formato: { $format }
chk-24bit = 24 bits
sample-rate-label = Frecuencia (Hz):
sr-system-default = Predeterminado del sistema ({ $rate } Hz)
http-port-label = Puerto HTTP:
chk-inject-silence = Inyectar silencio
strmsize-label = Tamaño de stream: { $size }
buffer-label = Búfer inicial (ms):
chk-rms-monitor = Monitor RMS
btn-apply-config = Pulsar para aplicar los cambios de configuración
upnp-devices = Dispositivos de renderizado UPNP en la red { $addr }

# Tab titles
tab-audio = Audio
tab-network = Red
tab-app = Aplicación
tab-status = Estado

# Status messages
status-setup-audio = Configurando fuentes de audio
status-injecting-silence = Inyectando silencio en el stream de salida
status-starting-ssdp = Iniciando descubrimiento SSDP
status-ssdp-interval-zero = Intervalo SSDP 0 => Omitiendo descubrimiento SSDP
status-loaded-config = Configuración cargada -c { $id }
status-serving-started = Servicio iniciado en el puerto { $port }...
status-playing-to = Reproduciendo en { $name }
status-shutting-down = Apagando { $name }
status-dry-run-exit = Ejecución de prueba - saliendo...
status-new-renderer = Nuevo renderizador { $name } en { $addr }

# Format / stream size change notifications
info-format-changed = El formato de streaming actual ha cambiado a { $format }
info-streamsize-changed = El tamaño de stream para { $format } ha cambiado a { $size }

# Warning messages (restart required)
warn-network-changed = Red cambiada a { $name }, ¡¡se requiere reinicio!!
warn-audio-changed = Fuente de audio cambiada a { $name }, ¡¡se requiere reinicio!!
warn-ssdp-changed = Intervalo SSDP cambiado a { $interval } minutos, ¡¡se requiere reinicio!!
warn-log-changed = Nivel de registro cambiado a { $level }, ¡¡se requiere reinicio!!

# Audio capture
audio-capturing-from = Capturando audio desde: { $name }
audio-default-config = Audio predeterminado { $cfg }
audio-capture-format = Formato de muestra de captura de audio = { $fmt }
err-capture-format-stream = Error al capturar el stream de audio { $fmt }: { $error }
err-capture-stream = Error { $error } al capturar el stream de entrada de audio
audio-capture-receiving = La captura de audio está recibiendo muestras.

# FLAC encoder
err-flac-already-running = ¡El codificador FLAC ya está en ejecución!
err-flac-cant-start = No se puede iniciar el codificador FLAC
err-flac-start-error = Error de inicio del codificador FLAC { $error }
flac-encoder-end = Hilo del codificador FLAC: fin.
flac-encoder-silence-end = Hilo del codificador FLAC (inyectando casi silencio): fin.
flac-encoder-exit = Hilo del codificador FLAC terminado.
err-flac-spawn = Error al iniciar el hilo del codificador FLAC: { $error }.

# Silence injection
err-inject-silence-stream = Inyección de silencio: ocurrió un error en el stream de audio de salida: { $error }
err-inject-silence-format = Inyección de silencio: formato de muestra no compatible: { $format }
err-inject-silence-play = No se puede reproducir el stream de inyección de silencio.
err-inject-silence-build = No se puede construir el stream de inyección de silencio: { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP: no hay red activa en la configuración.
err-ssdp-parse-ip = SSDP: no se puede analizar la dirección IP local.
err-ssdp-bind = SSDP: no se puede vincular al socket.
err-ssdp-broadcast = SSDP: no se puede configurar el socket en modo broadcast.
err-ssdp-ttl = SSDP: no se puede establecer DEFAULT_SEARCH_TTL en el socket.
err-ssdp-oh-send = SSDP: no se puede enviar el mensaje de descubrimiento OpenHome
err-ssdp-av-send = SSDP: no se puede enviar el mensaje de descubrimiento AV Transport

# Process priority
priority-nice = Ejecutándose ahora con valor nice -10
priority-above-normal = Ejecutándose ahora con ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Error al establecer la prioridad del proceso en ABOVE_NORMAL, error={ $error }
err-priority-linux = Lo sentimos, pero no tiene permisos para aumentar la prioridad...

# Error messages
err-no-audio-device = ¡No se encontró ningún dispositivo de audio predeterminado!
err-no-sound-source = ¡No hay fuente de sonido en la configuración!
err-capture-audio = No se pudo capturar el audio... Por favor, compruebe la configuración.
err-play-stream = No se puede reproducir el stream de audio.
err-inject-silence = ¡¡No se puede inyectar silencio!!
err-ssdp-spawn = No se puede iniciar el hilo de descubrimiento SSDP: { $error }
err-rms-spawn = No se puede iniciar el hilo del monitor RMS: { $error }
err-server-spawn = No se puede iniciar el hilo del servidor de streaming HTTP: { $error }

# Debug build indicator
debug-build-warning = Ejecutando compilación DEBUG => ¡nivel de registro establecido en DEBUG!

# CLI: audio source discovery
cli-found-audio-source = Fuente de audio encontrada: índice = { $index }, nombre = { $name }
cli-selected-audio-source-idx = Fuente de audio seleccionada: { $name }[#{ $index }]
cli-selected-audio-source = Fuente de audio seleccionada: { $name }
cli-selected-audio-source-pos = Fuente de audio seleccionada: { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Red encontrada: { $ip }
cli-available-renderer = Renderizador disponible #{ $n }: { $name } en { $addr }
cli-default-renderer-ip = IP del renderizador predeterminado: { $ip } => { $addr }
cli-active-renderer = Renderizador activo: { $name } => { $addr }
cli-default-player-ip = IP del reproductor predeterminado = { $ip }
cli-no-renderers = ¡¡¡No se encontraron renderizadores!!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = ^C recibido -> saliendo.
cli-ctrlc-stopping = ^C: Deteniendo el streaming a { $name }
cli-ctrlc-no-connections = ^C: No hay conexiones de streaming HTTP activas
cli-ctrlc-timeout = ^C: Tiempo de espera agotado al esperar el cierre del streaming HTTP - saliendo.

# Streaming server messages
srv-listening = El servidor de streaming está escuchando en http://{ $addr }/stream/swyh.wav
srv-default-streaming = Frecuencia de muestreo de streaming predeterminada: { $rate }, bits por muestra: { $bps }, formato: { $format }
srv-start-error = Error al iniciar el hilo del servidor: { $error }
srv-thread-error = El hilo del servidor terminó con el error { $error }
srv-streaming-request = Solicitud de streaming { $url } desde { $addr }
srv-feedback-error = Servidor HTTP: error al escribir en el canal de retroalimentación { $error }
srv-streaming-info = Transmitiendo { $audio }, formato de muestra de entrada { $fmt }, canales=2, tasa={ $rate }, bps = { $bps }, a { $addr }
srv-http-terminated = =>Conexión HTTP con { $addr } terminada [{ $error }]
srv-streaming-ended = El streaming a { $addr } ha finalizado
srv-head-terminated = =>Conexión HTTP HEAD con { $addr } terminada [{ $error }]
srv-unsupported-method = Solicitud de método HTTP no compatible { $method } desde { $addr }
srv-bad-request = Solicitud no reconocida '{ $url }' desde '{ $addr }'
srv-stream-terminated = =>Solicitud de streaming HTTP con { $addr } terminada [{ $error }]

srv-range-not-satisfiable = Solicitud de rango de { $addr } no satisfacible, respondiendo 416
