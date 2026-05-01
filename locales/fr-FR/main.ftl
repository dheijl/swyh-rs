# Window title
window-title = swyh-rs UPNP/DLNA streaming V{ $version }

# Configuration panel
config-options = Options de configuration
choose-color-theme = Choisir le thème de couleur
color-theme-label = Thème de couleur : { $name }
language-label = Langue : { $lang }
warn-language-changed = Langue changée en { $lang }, redémarrage requis !!
active-network = Réseau actif : { $addr }
new-network-label = Nouveau réseau : { $name }
audio-source-label = Source audio : { $name }
new-audio-source-label = Nouvelle source audio : { $name }

# Checkboxes and controls
chk-autoresume = Auto-reprise
chk-autoreconnect = Auto-reconnexion
ssdp-interval-label = Intervalle SSDP (min)
log-level-label = Log : { $level }
fmt-label = FMT : { $format }
chk-24bit = 24 bits
http-port-label = Port HTTP :
chk-inject-silence = Injecter du silence
strmsize-label = StrmSize : { $size }
buffer-label = Tampon initial (msec) :
chk-rms-monitor = Moniteur RMS
btn-apply-config = Cliquer pour appliquer les changements de configuration
upnp-devices = Périphériques UPNP sur le réseau { $addr }

# Titres des onglets
tab-audio = Audio
tab-network = Réseau
tab-app = App
tab-status = État

# Status messages
status-setup-audio = Configuration des sources audio
status-injecting-silence = Injection de silence dans le flux de sortie
status-starting-ssdp = Démarrage de la découverte SSDP
status-ssdp-interval-zero = Intervalle SSDP 0 => Découverte SSDP ignorée
status-loaded-config = Configuration chargée -c { $id }
status-serving-started = Serveur démarré sur le port { $port }...
status-playing-to = Lecture vers { $name }
status-shutting-down = Arrêt de { $name }
status-dry-run-exit = dry-run - fermeture...
status-new-renderer = Nouveau lecteur { $name } sur { $addr }

# Format / stream size change notifications
info-format-changed = Format de streaming actuel changé en { $format }
info-streamsize-changed = TailleFlux pour { $format } changée en { $size }

# Warning messages (restart required)
warn-network-changed = Réseau changé en { $name }, redémarrage requis !!
warn-audio-changed = Source audio changée en { $name }, redémarrage requis !!
warn-ssdp-changed = Intervalle SSDP changé en { $interval } minutes, redémarrage requis !!
warn-log-changed = Niveau de log changé en { $level }, redémarrage requis !!

# Audio capture
audio-capturing-from = Capture audio depuis : { $name }
audio-default-config = Audio par défaut { $cfg }
audio-capture-format = Format d'échantillon audio = { $fmt }
err-capture-format-stream = Erreur lors de la capture du flux audio { $fmt } : { $error }
err-capture-stream = Erreur { $error } lors de la capture du flux d'entrée audio
audio-capture-receiving = La capture audio reçoit maintenant des échantillons.

# FLAC encoder
err-flac-already-running = L'encodeur Flac est déjà en cours d'exécution !
err-flac-cant-start = Impossible de démarrer l'encodeur FLAC
err-flac-start-error = Erreur de démarrage de l'encodeur Flac { $error }
flac-encoder-end = Thread encodeur Flac : fin.
flac-encoder-silence-end = Thread encodeur Flac (injection de quasi-silence) : fin.
flac-encoder-exit = Thread encodeur Flac terminé.
err-flac-spawn = Échec du démarrage du thread encodeur Flac : { $error }.

# Silence injection
err-inject-silence-stream = Injection de silence : une erreur s'est produite sur le flux audio de sortie : { $error }
err-inject-silence-format = Injection de silence : format d'échantillon non pris en charge : { $format }
err-inject-silence-play = Impossible de lire le flux d'injection de silence.
err-inject-silence-build = Impossible de créer le flux d'injection de silence : { $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP : aucun réseau actif dans la configuration.
err-ssdp-parse-ip = SSDP : impossible d'analyser l'adresse IP locale.
err-ssdp-bind = SSDP : impossible de lier au socket.
err-ssdp-broadcast = SSDP : impossible de configurer le socket en broadcast.
err-ssdp-ttl = SSDP : impossible de définir DEFAULT_SEARCH_TTL sur le socket.
err-ssdp-oh-send = SSDP : impossible d'envoyer le message de découverte OpenHome
err-ssdp-av-send = SSDP : impossible d'envoyer le message de découverte AV Transport

# Process priority
priority-nice = Maintenant actif avec la valeur nice -10
priority-above-normal = Maintenant actif avec ABOVE_NORMAL_PRIORITY_CLASS
err-priority-windows = Échec de la définition de la priorité du processus à ABOVE_NORMAL, erreur={ $error }
err-priority-linux = Désolé, vous n'avez pas les permissions pour augmenter la priorité...

# Error messages
err-no-audio-device = Aucun périphérique audio par défaut trouvé !
err-no-sound-source = Aucune source sonore dans la configuration !
err-capture-audio = Impossible de capturer l'audio... Veuillez vérifier la configuration.
err-play-stream = Impossible de lire le flux audio.
err-inject-silence = Impossible d'injecter du silence !!
err-ssdp-spawn = Impossible de démarrer le thread de découverte SSDP : { $error }
err-rms-spawn = Impossible de démarrer le thread du moniteur RMS : { $error }
err-server-spawn = Impossible de démarrer le thread du serveur HTTP de streaming : { $error }

# Debug build indicator
debug-build-warning = Exécution en mode DEBUG => niveau de log défini sur DEBUG !

# CLI: audio source discovery
cli-found-audio-source = Source audio trouvée : index = { $index }, nom = { $name }
cli-selected-audio-source-idx = Source audio sélectionnée : { $name }[#{ $index }]
cli-selected-audio-source = Source audio sélectionnée : { $name }
cli-selected-audio-source-pos = Source audio sélectionnée : { $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = Réseau trouvé : { $ip }
cli-available-renderer = Lecteur disponible #{ $n } : { $name } sur { $addr }
cli-default-renderer-ip = IP du lecteur par défaut : { $ip } => { $addr }
cli-active-renderer = Lecteur actif : { $name } => { $addr }
cli-default-player-ip = IP du lecteur par défaut = { $ip }
cli-no-renderers = Aucun lecteur trouvé !!!

# CLI: Ctrl-C shutdown
cli-received-ctrlc = ^C reçu -> fermeture.
cli-ctrlc-stopping = ^C : Arrêt du streaming vers { $name }
cli-ctrlc-no-connections = ^C : Aucune connexion HTTP de streaming active
cli-ctrlc-timeout = ^C : Délai d'attente dépassé pour l'arrêt du streaming HTTP - fermeture.

# Streaming server messages
srv-listening = Le serveur de streaming écoute sur http://{ $addr }/stream/swyh.wav
srv-default-streaming = Fréquence d'échantillonnage par défaut : { $rate }, bits par échantillon : { $bps }, format : { $format }
srv-start-error = Erreur lors du démarrage du thread serveur : { $error }
srv-thread-error = Le thread serveur s'est terminé avec l'erreur { $error }
srv-streaming-request = Requête de streaming { $url } depuis { $addr }
srv-feedback-error = Serveur HTTP : erreur d'écriture sur le canal de retour { $error }
srv-streaming-info = Streaming { $audio }, format d'échantillon d'entrée { $fmt }, canaux=2, fréquence={ $rate }, bps = { $bps }, vers { $addr }
srv-http-terminated = =>Connexion HTTP avec { $addr } terminée [{ $error }]
srv-streaming-ended = Le streaming vers { $addr } est terminé
srv-head-terminated = =>Connexion HTTP HEAD avec { $addr } terminée [{ $error }]
srv-unsupported-method = Méthode HTTP non prise en charge { $method } depuis { $addr }
srv-bad-request = Requête non reconnue '{ $url }' depuis '{ $addr }'
srv-stream-terminated = =>Requête de streaming HTTP avec { $addr } terminée [{ $error }]
