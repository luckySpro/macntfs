import AppKit
import Foundation
let root = URL(fileURLWithPath: CommandLine.arguments[1])
let set = root.appendingPathComponent("AppIcon.iconset")
try FileManager.default.createDirectory(at: set, withIntermediateDirectories: true)
for (name, size) in [("icon_16x16.png",16),("icon_16x16@2x.png",32),("icon_32x32.png",32),("icon_32x32@2x.png",64),("icon_128x128.png",128),("icon_128x128@2x.png",256),("icon_256x256.png",256),("icon_256x256@2x.png",512),("icon_512x512.png",512),("icon_512x512@2x.png",1024)] {
    let n = CGFloat(size)
    let image = NSImage(size:NSSize(width:n,height:n))
    image.lockFocus()
    let source = root.deletingLastPathComponent().appendingPathComponent("frontend/assets/drive.png")
    guard let artwork = NSImage(contentsOf: source) else { fatalError("Missing disk artwork") }
    artwork.draw(in:NSRect(x:n*0.04,y:n*0.04,width:n*0.92,height:n*0.92),from:.zero,operation:.sourceOver,fraction:1)
    image.unlockFocus()
    let bitmap=NSBitmapImageRep(data:image.tiffRepresentation!)!
    try bitmap.representation(using:.png,properties:[:])!.write(to:set.appendingPathComponent(name))
}
let task=Process(); task.executableURL=URL(fileURLWithPath:"/usr/bin/iconutil"); task.arguments=["-c","icns",set.path,"-o",root.appendingPathComponent("AppIcon.icns").path]
try task.run(); task.waitUntilExit(); if task.terminationStatus != 0 { exit(task.terminationStatus) }
