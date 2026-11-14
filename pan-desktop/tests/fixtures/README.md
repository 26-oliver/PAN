# Vídeo sintético para las pruebas

`bars.webm` contiene seis segundos de barras de color generadas por FFmpeg:

```sh
ffmpeg -f lavfi -i 'smptebars=size=96x64:rate=12' -t 6 -an -c:v libvpx -pix_fmt yuv420p -crf 35 -b:v 80k bars.webm
```
