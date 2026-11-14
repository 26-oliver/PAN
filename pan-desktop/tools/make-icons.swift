// Produces PAN's original bread-and-play icon and its platform variants.
import AppKit

let dest = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let colors: [Character: UInt32] = [
    "s": 0x462619, "c": 0xA65324, "b": 0xD08037,
    "g": 0xF3AA54, "l": 0xFFD088, "p": 0x633322,
]
let design = [
    "........................",
    "........................",
    "........................",
    ".......ssssssssss.......",
    ".....ssccccccccccss.....",
    "....sccggggggggggccs....",
    "...scggllllllllllggcs...",
    "...scgllllllllllllgcs...",
    "...scbllllllllllllbcs...",
    "....sclllpllllllllcs....",
    "....sclllpplllllllcs....",
    "....sclllpppllllllcs....",
    "....sclllpppplllllcs....",
    "....sclllpppppllllcs....",
    "....sclllpppplllllcs....",
    "....sclllpppllllllcs....",
    "....sclllpplllllllcs....",
    "....sclllpllllllllcs....",
    "....scbllllllllllbcs....",
    "....scbbbbbbbbbbbbcs....",
    ".....sccccccccccccs.....",
    "......ssssssssssss......",
    "........................",
    "........................",
]

func rgb(_ value: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((value >> 16) & 255) / 255,
            green: CGFloat((value >> 8) & 255) / 255,
            blue: CGFloat(value & 255) / 255, alpha: 1)
}

func png(size: Int, tile: Bool) -> Data {
    let img = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: img)
    if tile {
        let edge = Double(size)
        let path = NSBezierPath(roundedRect: NSRect(x: edge * 0.08, y: edge * 0.08, width: edge * 0.84, height: edge * 0.84),
                                xRadius: edge * 0.19, yRadius: edge * 0.19)
        rgb(0x231913).setFill()
        path.fill()
    }
    let pixel = max(1, size / (tile ? 30 : 24))
    let offset = (size - pixel * 24) / 2
    for (row, line) in design.enumerated() {
        for (column, token) in line.enumerated() {
            guard let hex = colors[token] else { continue }
            rgb(hex).setFill()
            NSRect(x: offset + column * pixel, y: offset + (23 - row) * pixel, width: pixel, height: pixel).fill()
        }
    }
    NSGraphicsContext.restoreGraphicsState()
    return img.representation(using: .png, properties: [:])!
}

try png(size: 192, tile: false).write(to: dest.appendingPathComponent("pan.png"))

let iconset = dest.appendingPathComponent("PAN.iconset", isDirectory: true)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let suffix = scale == 1 ? "" : "@2x"
        try png(size: size * scale, tile: true).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)\(suffix).png"))
    }
}
let tool = Process()
tool.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
tool.arguments = ["-c", "icns", iconset.path, "-o", dest.appendingPathComponent("pan.icns").path]
try tool.run()
tool.waitUntilExit()
guard tool.terminationStatus == 0 else { fatalError("iconutil failed") }
try FileManager.default.removeItem(at: iconset)

func append<T: FixedWidthInteger>(_ number: T, to data: inout Data) {
    var little = number.littleEndian
    withUnsafeBytes(of: &little) { data.append(contentsOf: $0) }
}
let sizes = [32, 48, 128, 256]
let frames = sizes.map { png(size: $0, tile: true) }
var ico = Data([0, 0, 1, 0])
append(UInt16(sizes.count), to: &ico)
var cursor = 6 + 16 * sizes.count
for (size, frame) in zip(sizes, frames) {
    let dimension = UInt8(truncatingIfNeeded: size)
    ico.append(contentsOf: [dimension, dimension, 0, 0, 1, 0, 32, 0])
    append(UInt32(frame.count), to: &ico)
    append(UInt32(cursor), to: &ico)
    cursor += frame.count
}
for frame in frames { ico.append(frame) }
try ico.write(to: dest.appendingPathComponent("pan.ico"))
