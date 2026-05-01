# Window title
window-title = swyh-rs UPNP/DLNA 流媒体 V{ $version }

# Configuration panel
config-options = 配置选项
choose-color-theme = 选择颜色主题
color-theme-label = 颜色主题：{ $name }
language-label = 语言：{ $lang }
warn-language-changed = 语言已更改为 { $lang }，需要重启！！
active-network = 活动网络：{ $addr }
new-network-label = 新网络：{ $name }
audio-source-label = 音频源：{ $name }
new-audio-source-label = 新音频源：{ $name }

# Checkboxes and controls
chk-autoresume = 自动恢复播放
chk-autoreconnect = 自动重连
ssdp-interval-label = SSDP 间隔（分钟）
log-level-label = 日志级别：{ $level }
fmt-label = 格式：{ $format }
chk-24bit = 24 位
http-port-label = HTTP 端口：
chk-inject-silence = 注入静音
strmsize-label = 流大小：{ $size }
buffer-label = 初始缓冲区（毫秒）：
chk-rms-monitor = RMS 监视器
btn-apply-config = 点击应用配置更改
upnp-devices = 网络 { $addr } 上的 UPNP 渲染设备

# 标签页标题
tab-audio = 音频
tab-network = 网络
tab-app = 应用
tab-status = 状态

# Status messages
status-setup-audio = 配置音频源
status-injecting-silence = 正在向输出流注入静音
status-starting-ssdp = 正在启动 SSDP 发现
status-ssdp-interval-zero = SSDP 间隔为 0 => 跳过 SSDP 发现
status-loaded-config = 已加载配置 -c { $id }
status-serving-started = 已在端口 { $port } 上启动服务...
status-playing-to = 正在播放到 { $name }
status-shutting-down = 正在关闭 { $name }
status-dry-run-exit = 演习模式 - 正在退出...
status-new-renderer = 在 { $addr } 发现新渲染器 { $name }

# Format / stream size change notifications
info-format-changed = 当前流媒体格式已更改为 { $format }
info-streamsize-changed = { $format } 的流大小已更改为 { $size }

# Warning messages (restart required)
warn-network-changed = 网络已更改为 { $name }，需要重启！！
warn-audio-changed = 音频源已更改为 { $name }，需要重启！！
warn-ssdp-changed = SSDP 间隔已更改为 { $interval } 分钟，需要重启！！
warn-log-changed = 日志级别已更改为 { $level }，需要重启！！

# Audio capture
audio-capturing-from = 正在从以下设备捕获音频：{ $name }
audio-default-config = 默认音频 { $cfg }
audio-capture-format = 音频捕获采样格式 = { $fmt }
err-capture-format-stream = 捕获 { $fmt } 音频流时出错：{ $error }
err-capture-stream = 捕获音频输入流时出错 { $error }
audio-capture-receiving = 音频捕获现在正在接收采样。

# FLAC encoder
err-flac-already-running = FLAC 编码器已在运行！
err-flac-cant-start = 无法启动 FLAC 编码器
err-flac-start-error = FLAC 编码器启动错误 { $error }
flac-encoder-end = FLAC 编码器线程：结束。
flac-encoder-silence-end = FLAC 编码器线程（注入近静音）：结束。
flac-encoder-exit = FLAC 编码器线程退出。
err-flac-spawn = 无法生成 FLAC 编码器线程：{ $error }。

# Silence injection
err-inject-silence-stream = 注入静音：输出音频流发生错误：{ $error }
err-inject-silence-format = 注入静音：不支持的采样格式：{ $format }
err-inject-silence-play = 无法播放注入静音流。
err-inject-silence-build = 无法构建注入静音流：{ $error }

# SSDP discovery errors
err-ssdp-no-network = SSDP：配置中没有活动网络。
err-ssdp-parse-ip = SSDP：无法解析本地 IP 地址。
err-ssdp-bind = SSDP：无法绑定到套接字。
err-ssdp-broadcast = SSDP：无法将套接字设置为广播模式。
err-ssdp-ttl = SSDP：无法在套接字上设置 DEFAULT_SEARCH_TTL。
err-ssdp-oh-send = SSDP：无法发送 OpenHome 发现消息
err-ssdp-av-send = SSDP：无法发送 AV Transport 发现消息

# Process priority
priority-nice = 现在以 nice 值 -10 运行
priority-above-normal = 现在以 ABOVE_NORMAL_PRIORITY_CLASS 运行
err-priority-windows = 无法将进程优先级设置为 ABOVE_NORMAL，错误 = { $error }
err-priority-linux = 抱歉，您没有提升优先级的权限...

# Error messages
err-no-audio-device = 未找到默认音频设备！
err-no-sound-source = 配置中没有声音源！
err-capture-audio = 无法捕获音频...请检查配置。
err-play-stream = 无法播放音频流。
err-inject-silence = 无法注入静音！！
err-ssdp-spawn = 无法生成 SSDP 发现线程：{ $error }
err-rms-spawn = 无法生成 RMS 监视器线程：{ $error }
err-server-spawn = 无法生成 HTTP 流媒体服务器线程：{ $error }

# Debug build indicator
debug-build-warning = 正在运行调试版本 => 日志级别已设置为 DEBUG！

# CLI: audio source discovery
cli-found-audio-source = 找到音频源：索引 = { $index }，名称 = { $name }
cli-selected-audio-source-idx = 已选择音频源：{ $name }[#{ $index }]
cli-selected-audio-source = 已选择音频源：{ $name }
cli-selected-audio-source-pos = 已选择音频源：{ $name }:{ $pos }

# CLI: network / renderer discovery
cli-found-network = 找到网络：{ $ip }
cli-available-renderer = 可用渲染器 #{ $n }：{ $name } 位于 { $addr }
cli-default-renderer-ip = 默认渲染器 IP：{ $ip } => { $addr }
cli-active-renderer = 活动渲染器：{ $name } => { $addr }
cli-default-player-ip = 默认播放器 IP = { $ip }
cli-no-renderers = 未找到渲染器！！！

# CLI: Ctrl-C shutdown
cli-received-ctrlc = 收到 ^C -> 正在退出。
cli-ctrlc-stopping = ^C：正在停止向 { $name } 的流媒体传输
cli-ctrlc-no-connections = ^C：没有活动的 HTTP 流媒体连接
cli-ctrlc-timeout = ^C：等待 HTTP 流媒体关闭超时 - 正在退出。

# Streaming server messages
srv-listening = 流媒体服务器正在监听 http://{ $addr }/stream/swyh.wav
srv-default-streaming = 默认流媒体采样率：{ $rate }，每采样位数：{ $bps }，格式：{ $format }
srv-start-error = 启动服务器线程时出错：{ $error }
srv-thread-error = 服务器线程以错误 { $error } 结束
srv-streaming-request = 来自 { $addr } 的流媒体请求 { $url }
srv-feedback-error = HTTP 服务器：写入反馈通道时出错 { $error }
srv-streaming-info = 正在流式传输 { $audio }，输入采样格式 { $fmt }，声道数=2，采样率={ $rate }，位数 = { $bps }，到 { $addr }
srv-http-terminated = =>与 { $addr } 的 HTTP 连接已终止 [{ $error }]
srv-streaming-ended = 到 { $addr } 的流媒体传输已结束
srv-head-terminated = =>与 { $addr } 的 HTTP HEAD 连接已终止 [{ $error }]
srv-unsupported-method = 来自 { $addr } 的不支持的 HTTP 方法请求 { $method }
srv-bad-request = 来自 '{ $addr }' 的无法识别的请求 '{ $url }'
srv-stream-terminated = =>与 { $addr } 的 HTTP 流媒体请求已终止 [{ $error }]
