# Window title
window-title = swyh-rs UPNP/DLNA ストリーミング V{ $version }

# Configuration panel
config-options = 設定オプション
choose-color-theme = カラーテーマを選択
color-theme-label = カラーテーマ：{ $name }
language-label = 言語：{ $lang }
warn-language-changed = 言語を { $lang } に変更しました。再起動が必要です！！
active-network = アクティブネットワーク：{ $addr }
new-network-label = 新しいネットワーク：{ $name }
audio-source-label = オーディオソース：{ $name }
new-audio-source-label = 新しいオーディオソース：{ $name }

# Checkboxes and controls
chk-autoresume = 自動再開
chk-autoreconnect = 自動再接続
ssdp-interval-label = SSDP 間隔（分）
btn-ssdp-discover = SSDP 探索を今すぐ実行
log-level-label = ログレベル：{ $level }
fmt-label = フォーマット：{ $format }
chk-24bit = 24ビット
sample-rate-label = サンプルレート（Hz）：
sr-system-default = システムデフォルト（{ $rate } Hz）
http-port-label = HTTP ポート：
chk-inject-silence = 無音を挿入
strmsize-label = ストリームサイズ：{ $size }
buffer-label = 初期バッファ（ミリ秒）：
chk-rms-monitor = RMS モニター
btn-apply-config = クリックして設定変更を適用
upnp-devices = ネットワーク { $addr } 上の UPNP レンダリングデバイス

# Tab titles
tab-audio = オーディオ
tab-network = ネットワーク
tab-app = アプリ
tab-status = ステータス

# Status messages
status-setup-audio = オーディオソースを設定中
status-injecting-silence = 出力ストリームに無音を挿入中
status-starting-ssdp = SSDP 探索を開始中
status-ssdp-interval-zero = SSDP 間隔が 0 => SSDP 探索をスキップ
status-loaded-config = 設定を読み込みました -c { $id }
status-serving-started = ポート { $port } でサービスを開始しました...
status-playing-to = { $name } に再生中
status-shutting-down = { $name } をシャットダウン中
status-dry-run-exit = ドライラン - 終了中...
status-new-renderer = { $addr } で新しいレンダラー { $name } を検出

# Format / stream size change notifications
info-format-changed = 現在のストリーミングフォーマットを { $format } に変更しました
info-streamsize-changed = { $format } のストリームサイズを { $size } に変更しました

# Warning messages (restart required)
warn-network-changed = ネットワークを { $name } に変更しました。再起動が必要です！！
warn-audio-changed = オーディオソースを { $name } に変更しました。再起動が必要です！！
warn-ssdp-changed = SSDP 間隔を { $interval } 分に変更しました。再起動が必要です！！
warn-log-changed = ログレベルを { $level } に変更しました。再起動が必要です！！

# Audio capture
audio-capturing-from = オーディオをキャプチャ中：{ $name }
audio-default-config = デフォルトオーディオ { $cfg }
audio-capture-format = オーディオキャプチャサンプルフォーマット = { $fmt }
err-capture-format-stream = { $fmt } オーディオストリームのキャプチャエラー：{ $error }
err-capture-stream = オーディオ入力ストリームのキャプチャエラー { $error }
audio-capture-receiving = オーディオキャプチャがサンプルを受信中です。

# FLAC encoder
err-flac-already-running = FLAC エンコーダーはすでに実行中です！
err-flac-cant-start = FLAC エンコーダーを開始できません
err-flac-start-error = FLAC エンコーダー起動エラー { $error }
flac-encoder-end = FLAC エンコーダースレッド：終了。
flac-encoder-silence-end = FLAC エンコーダースレッド（無音近くを挿入中）：終了。
flac-encoder-exit = FLAC エンコーダースレッドを終了しました。
err-flac-spawn = FLAC エンコーダースレッドを起動できませんでした：{ $error }。

# Silence injection
err-inject-silence-stream = 無音挿入：出力オーディオストリームでエラーが発生しました：{ $error }
err-inject-silence-format = 無音挿入：サポートされていないサンプルフォーマット：{ $format }
err-inject-silence-play = 無音挿入ストリームを再生できません。
err-inject-silence-build = 無音挿入ストリームを構築できません：{ $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP：設定にアクティブなネットワークがありません。
err-ssdp-parse-ip = SSDP：ローカル IP アドレスを解析できません。
err-ssdp-bind = SSDP：ソケットにバインドできません。
err-ssdp-broadcast = SSDP：ソケットをブロードキャストに設定できません。
err-ssdp-ttl = SSDP：ソケットに DEFAULT_SEARCH_TTL を設定できません。
err-ssdp-oh-send = SSDP：OpenHome 探索メッセージを送信できません
err-ssdp-av-send = SSDP：AV Transport 探索メッセージを送信できません

# Process priority
priority-nice = nice 値 -10 で実行中
priority-above-normal = ABOVE_NORMAL_PRIORITY_CLASS で実行中
err-priority-windows = プロセス優先度を ABOVE_NORMAL に設定できませんでした。エラー={ $error }
err-priority-linux = 申し訳ありませんが、優先度を上げる権限がありません...

# Error messages
err-no-audio-device = デフォルトのオーディオデバイスが見つかりません！
err-no-sound-source = 設定にサウンドソースがありません！
err-capture-audio = オーディオをキャプチャできませんでした...設定を確認してください。
err-play-stream = オーディオストリームを再生できません。
err-inject-silence = 無音を挿入できません！！
err-ssdp-spawn = SSDP 探索スレッドを起動できません：{ $error }
err-rms-spawn = RMS モニタースレッドを起動できません：{ $error }
err-server-spawn = HTTP ストリーミングサーバースレッドを起動できません：{ $error }

# Debug build indicator
debug-build-warning = デバッグビルドを実行中 => ログレベルを DEBUG に設定しました！

# CLI: audio source discovery
cli-found-audio-source = オーディオソースを検出：インデックス = { $index }、名前 = { $name }
cli-selected-audio-source-idx = 選択されたオーディオソース：{ $name }[#{ $index }]
cli-selected-audio-source = 選択されたオーディオソース：{ $name }
cli-selected-audio-source-pos = 選択されたオーディオソース：{ $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = ネットワークを検出：{ $ip }
cli-available-renderer = 利用可能なレンダラー #{ $n }：{ $name }（{ $addr }）
cli-default-renderer-ip = デフォルトレンダラー IP：{ $ip } => { $addr }
cli-active-renderer = アクティブなレンダラー：{ $name } => { $addr }
cli-default-player-ip = デフォルトプレイヤー IP = { $ip }
cli-no-renderers = レンダラーが見つかりません！！！

# CLI: Ctrl-C shutdown
cli-received-ctrlc = ^C を受信しました -> 終了中。
cli-ctrlc-stopping = ^C：{ $name } へのストリーミングを停止中
cli-ctrlc-no-connections = ^C：アクティブな HTTP ストリーミング接続はありません
cli-ctrlc-timeout = ^C：HTTP ストリーミングのシャットダウン待機がタイムアウト - 終了中。

# Streaming server messages
srv-listening = ストリーミングサーバーが http://{ $addr }/stream/swyh.wav でリッスン中
srv-default-streaming = デフォルトストリーミングサンプルレート：{ $rate }、ビット/サンプル：{ $bps }、フォーマット：{ $format }
srv-start-error = サーバースレッドの起動エラー：{ $error }
srv-thread-error = サーバースレッドがエラー { $error } で終了しました
srv-streaming-request = { $addr } からのストリーミングリクエスト { $url }
srv-feedback-error = HTTP サーバー：フィードバックチャンネルへの書き込みエラー { $error }
srv-streaming-info = { $audio } をストリーミング中、入力サンプルフォーマット { $fmt }、チャンネル数=2、レート={ $rate }、bps = { $bps }、送信先 { $addr }
srv-http-terminated = =>{ $addr } との HTTP 接続が終了しました [{ $error }]
srv-streaming-ended = { $addr } へのストリーミングが終了しました
srv-head-terminated = =>{ $addr } との HTTP HEAD 接続が終了しました [{ $error }]
srv-unsupported-method = { $addr } からの未サポート HTTP メソッドリクエスト { $method }
srv-bad-request = '{ $addr }' からの認識できないリクエスト '{ $url }'
srv-stream-terminated = =>{ $addr } との HTTP ストリーミングリクエストが終了しました [{ $error }]
