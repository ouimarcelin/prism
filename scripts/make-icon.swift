// Draws the first, placeholder icon of Prism: a crystal of coloured facets
// on a dark rounded square. Writes a 1024 × 1024 PNG to the path given.
// Since 2026-10-18 assets/icon-1024.png holds the logo drawn by the author:
// never write this drawing over it.
//
//   swift scripts/make-icon.swift /tmp/placeholder-icon.png

import AppKit
import CoreGraphics

let size = 1024.0
let path = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon-1024.png"
let space = CGColorSpaceCreateDeviceRGB()
guard let context = CGContext(
    data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
    space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
else { fatalError("no context") }

func color(_ r: Double, _ g: Double, _ b: Double, _ a: Double = 1) -> CGColor {
    CGColor(colorSpace: space, components: [r, g, b, a])!
}

// Background: rounded square with a soft vertical gradient.
let inset = size * 0.1
let tile = CGRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let rounded = CGPath(roundedRect: tile, cornerWidth: size * 0.18, cornerHeight: size * 0.18, transform: nil)
context.saveGState()
context.addPath(rounded)
context.clip()
let background = CGGradient(colorsSpace: space, colors: [color(0.10, 0.10, 0.14), color(0.03, 0.03, 0.05)] as CFArray, locations: [0, 1])!
context.drawLinearGradient(background, start: CGPoint(x: 0, y: size), end: CGPoint(x: 0, y: 0), options: [])

// A crystal seen from the front: a hexagon cut into six facets meeting
// at a point above its center, each facet its own shade of a cool
// gradient, with light edges.
let center = CGPoint(x: size * 0.5, y: size * 0.5)
let radius = size * 0.29
let apex = CGPoint(x: center.x + size * 0.02, y: center.y + size * 0.04)
let corners = (0..<6).map { index -> CGPoint in
    let angle = Double.pi / 2 + Double(index) * Double.pi / 3
    return CGPoint(x: center.x + radius * cos(angle), y: center.y + radius * sin(angle))
}
let shades = [
    color(0.33, 0.85, 0.95), color(0.20, 0.62, 0.98), color(0.36, 0.38, 0.95),
    color(0.62, 0.32, 0.92), color(0.92, 0.36, 0.72), color(0.98, 0.62, 0.45),
]
for index in 0..<6 {
    let facet = CGMutablePath()
    facet.move(to: apex)
    facet.addLine(to: corners[index])
    facet.addLine(to: corners[(index + 1) % 6])
    facet.closeSubpath()
    context.addPath(facet)
    context.setFillColor(shades[index])
    context.fillPath()
}
// Edges of the facets, then the outline.
context.setStrokeColor(color(1, 1, 1, 0.55))
context.setLineWidth(size * 0.008)
context.setLineJoin(.round)
for corner in corners {
    context.move(to: apex)
    context.addLine(to: corner)
}
context.strokePath()
let outline = CGMutablePath()
outline.addLines(between: corners)
outline.closeSubpath()
context.addPath(outline)
context.setStrokeColor(color(1, 1, 1, 0.9))
context.setLineWidth(size * 0.014)
context.strokePath()
context.restoreGState()

guard let image = context.makeImage() else { fatalError("no image") }
let bitmap = NSBitmapImageRep(cgImage: image)
guard let data = bitmap.representation(using: .png, properties: [:]) else { fatalError("no png") }
try! data.write(to: URL(fileURLWithPath: path))
print("icon \(path)")
