# PAN · Play Anything Now

Reproductor de escritorio en Rust con LibVLC 3 y el logo de pan. Abre un archivo para reproducirlo al instante. Al seleccionar varios archivos, PAN inicia el primero y conserva el resto en una cola de la sesión.

## Instalar y abrir archivos

- **macOS**. Descomprime el ZIP y arrastra `PAN.app` a Aplicaciones. El paquete contiene código para Apple Silicon e Intel y lleva firma local ad hoc. Instala VLC 3 de la misma arquitectura en `/Applications/VLC.app`. En Finder, elige **Abrir con → PAN**. La barra superior usa Liquid Glass nativo en macOS 26 o posterior.
- **Windows x64**. Instala VLC 3 de 64 bits. Descomprime el ZIP y ejecuta `install.ps1` con PowerShell para registrar PAN en tu cuenta. En Explorador, elige **Abrir con → PAN**. También puedes ejecutar `pan.exe` con la ruta de un archivo.
- **Linux x86-64**. Necesitas glibc 2.31 o posterior, LibVLC 3 con sus complementos de reproducción y una sesión X11 o Wayland con OpenGL. En Debian o Ubuntu que empaqueten VLC 3, el paquete `vlc` instala el motor y los complementos. Descomprime el paquete y ejecuta `./install.sh` para registrar PAN en tu cuenta. Para usar el selector de archivos, tu escritorio necesita un portal de escritorio compatible. Arrastrar archivos o abrirlos desde el gestor también funciona.

Para usar el doble clic, elige PAN como reproductor predeterminado para cada tipo de archivo en el sistema. Las asociaciones de PAN se registran como opción de apertura.

## Controles

Espacio reproduce o pausa. Izquierda y derecha saltan 5 segundos. Arriba y abajo cambian el volumen. M silencia. F activa la pantalla completa. Ctrl+O o Cmd+O abre el selector. Los botones de la barra permiten abrir una URL y ver la cola. Los controles inferiores incluyen selección de pistas y subtítulos, velocidad y repetición.

## Compilar

Usa Rust estable y ejecuta `cargo build --release --locked` desde esta carpeta. En macOS se necesita Xcode o las Command Line Tools. El SDK de macOS 26 o posterior habilita la compilación con el aspecto Liquid Glass. Ejecuta `./packaging/macos/package.sh` para crear `dist/PAN.app`. Windows con MSVC usa el compilador de recursos del SDK para incluir el icono. Los scripts de instalación de Windows y Linux se ejecutan junto al binario y al icono del paquete correspondiente.

PAN carga LibVLC de forma dinámica. La variable `PAN_LIBVLC` permite indicar la ruta absoluta a la biblioteca de una instalación alternativa, con sus complementos compatibles. El código de PAN y los recursos de la interfaz se incluyen en el ZIP de código fuente. El motor se obtiene de una instalación de VLC o LibVLC.

## Alcance de esta versión

La interfaz usa una salida de vídeo con copia de fotogramas en memoria. La compatibilidad y el rendimiento dependen de la versión de LibVLC, sus complementos y el equipo. Esta versión se centra en reproducir archivos y emisiones, con pistas, subtítulos y cola temporal. Grabación, conversión y demás funciones avanzadas de VLC quedan fuera de esta versión.

## Referencia de REA

REA inspeccionó el ejecutable Mach-O de VLC 3.0.24 para Apple Silicon y el `Info.plist` de su paquete. Observó las dependencias de AppKit, LibVLC y LibVLCCore, las importaciones de arranque de LibVLC y las declaraciones de apertura de archivos. La inspección fue estática y la tabla de importaciones tenía cobertura parcial. PAN utiliza la API pública de LibVLC y una interfaz propia.

- Dependencias: `ev_07ff2a07cd9cdadb41933dd035694255f2799165ffd9487d09f893623a3b6835`
- Importaciones: `ev_88d479c84dffafb513e2b5aca26a1ae16c87e5b0249a4cb7592b3766a81a252b`
- Info.plist: `ev_f711c7964bb194b5c8d7f8873e7d30bd8c2941c62609e9e28b7559724ea957c2`
- [Cabecera de la API pública de LibVLC 3](https://github.com/videolan/vlc-3.0/blob/master/include/vlc/libvlc_media_player.h)
- [Liquid Glass en barras de herramientas de AppKit](https://developer.apple.com/videos/play/wwdc2025/310/)

## Biblioteca local

Abre **Biblioteca** en la barra o pulsa Cmd+L en macOS y Ctrl+L en Windows o Linux. Usa **Añadir archivos** o arrástralos a la cuadrícula. PAN copia cada archivo completo, conserva su nombre y muestra la copia cuando termina. Puedes buscar por nombre, filtrar audio o vídeo y ordenar por fecha, nombre o tamaño. Cada tarjeta reproduce su copia.

**Abrir carpeta** muestra dónde están guardados los archivos. En macOS es `~/Library/Application Support/PAN/Biblioteca`. En Windows es `%LOCALAPPDATA%\PAN\Biblioteca`. En Linux es `$XDG_DATA_HOME/PAN/Biblioteca` o `~/.local/share/PAN/Biblioteca`. Si inicias PAN con una ruta absoluta en `PAN_LIBRARY_DIR`, usará esa carpeta.

La biblioteca vive en el dispositivo. **Abrir con PAN** sigue reproduciendo de inmediato desde el original. La copia se crea al añadir el archivo a la biblioteca.

### Funciones de PAN 0.2

- **Listas**. Crea una lista y añade archivos con el menú contextual de una tarjeta. Dentro de la lista puedes reordenarlos, quitarlos, renombrar la lista o eliminarla. Las listas comparten las copias de la biblioteca.
- **Continuar la reproducción**. PAN guarda periódicamente la posición, el volumen, el silencio, la velocidad y las preferencias de subtítulos de cada archivo local. Al terminar un archivo se reinicia su posición. Puedes volver al inicio con la barra de tiempo.
- **Búsqueda y filtros**. Busca por nombre, título, artista o descripción. Filtra por tipo, duración y fecha de importación. Puedes ordenar por última reproducción o consultar el tamaño. PAN lee la duración en segundo plano cuando LibVLC puede determinarla.
- **Subtítulos y audio**. Los menús de pistas permiten abrir subtítulos externos y desplazar la sincronización en milisegundos. En Ajustes puedes elegir el idioma preferido y el tamaño del texto para el siguiente archivo que abras. El tamaño depende de las posibilidades del formato de subtítulos.
- **Fichas y carátulas**. Haz clic derecho en una tarjeta y elige «Editar ficha y carátula». Puedes añadir un JPG, PNG o WebP local, o buscar un título en Wikipedia. PAN solo envía los términos de búsqueda al pulsar «Buscar». Según el artículo, la imagen puede ser una portada, un logotipo u otra miniatura.
- **Almacenamiento y atajos**. Abre Ajustes desde la Biblioteca, con Cmd+, en Mac o Ctrl+, en Windows y Linux. Puedes ver el tamaño de los archivos, cambiar la ubicación y personalizar las teclas del reproductor. Si cambias de ubicación, se crea una carpeta «Biblioteca PAN» en el destino y se conserva la anterior.
- **Duplicados y archivos perdidos**. PAN compara el contenido durante la importación. En la Biblioteca puedes ver las copias que faltan, incluso si el disco externo está desconectado. Si el archivo vuelve a aparecer en su carpeta, PAN lo vuelve a mostrar.

## Contribuciones

[Créditos de PAN Desktop](CONTRIBUTORS.md).
