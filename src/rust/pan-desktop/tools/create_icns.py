from pathlib import Path
import struct
import sys

iconset = Path(sys.argv[1])
output = Path(sys.argv[2])

icons = [
    (b"icp4", "icon_16x16.png"),
    (b"icp5", "icon_32x32.png"),
    (b"icp6", "icon_32x32@2x.png"),
    (b"ic07", "icon_128x128.png"),
    (b"ic08", "icon_256x256.png"),
    (b"ic09", "icon_512x512.png"),
    (b"ic10", "icon_512x512@2x.png"),
]

records = bytearray()
for kind, filename in icons:
    payload = (iconset / filename).read_bytes()
    records += kind + struct.pack(">I", len(payload) + 8) + payload

output.write_bytes(b"icns" + struct.pack(">I", len(records) + 8) + records)
