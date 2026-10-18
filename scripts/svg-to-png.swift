// Renders an SVG into a square transparent PNG, centred and fitted.
// Usage: swift scripts/svg-to-png.swift assets/splash.svg target/splash.png 2048

import AppKit
let args = CommandLine.arguments
guard let image = NSImage(contentsOfFile: args[1]) else { fatalError("svg illisible") }
let side = Int(args[3])!
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: side, pixelsHigh: side, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
NSGraphicsContext.current?.imageInterpolation = .high
let s = image.size
let scale = CGFloat(side) / max(s.width, s.height)
let w = s.width * scale, h = s.height * scale
image.draw(in: NSRect(x: (CGFloat(side) - w) / 2, y: (CGFloat(side) - h) / 2, width: w, height: h))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
