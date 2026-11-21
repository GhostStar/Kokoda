# Synthetic decoder fixture / 合成解码样本

`desktop-color.h264` is a single teal IDR, 98×66 pixels. Its width deliberately
requires row padding in RGBA conversion. It contains no application content.
Generated with FFmpeg 8.0.1 / libx264 using:

此文件为 98×66 青色 IDR 单帧。非对齐宽度用于回归 RGBA 转换的行填充，
不包含应用内容。使用 FFmpeg 8.0.1 / libx264 生成：

```sh
ffmpeg -f lavfi -i color=c=teal:s=98x66:r=15 -frames:v 1 \
  -c:v libx264 -preset ultrafast -tune zerolatency -x264-params keyint=1 \
  -f h264 desktop-color.h264
```

The fixture is generated data distributed under this repository's license.
样本为生成数据，按本仓库许可分发。
