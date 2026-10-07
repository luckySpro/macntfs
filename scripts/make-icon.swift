import AppKit
import Foundation
let root = URL(fileURLWithPath: CommandLine.arguments[1])
let set = root.appendingPathComponent("AppIcon.iconset")
try FileManager.default.createDirectory(at: set, withIntermediateDirectories: true)
for (name, size) in [("icon_16x16.png",16),("icon_16x16@2x.png",32),("icon_32x32.png",32),("icon_32x32@2x.png",64),("icon_128x128.png",128),("icon_128x128@2x.png",256),("icon_256x256.png",256),("icon_256x256@2x.png",512),("icon_512x512.png",512),("icon_512x512@2x.png",1024)] {
    let n = CGFloat(size)
    let image = NSImage(size:NSSize(width:n,height:n))
    image.lockFocus()
    let inset = n*0.06
    let outer = NSBezierPath(roundedRect:NSRect(x:inset,y:inset,width:n-2*inset,height:n-2*inset),xRadius:n*0.20,yRadius:n*0.20)
    NSGradient(starting:NSColor(red:0.51,green:0.40,blue:0.96,alpha:1),ending:NSColor(red:0.30,green:0.24,blue:0.68,alpha:1))!.draw(in:outer,angle:-90)
    NSColor.white.withAlphaComponent(0.13).setFill()
    NSBezierPath(roundedRect:NSRect(x:n*0.17,y:n*0.15,width:n*0.66,height:n*0.66),xRadius:n*0.16,yRadius:n*0.16).fill()
    NSColor.white.setStroke()
    let drive = NSBezierPath(roundedRect:NSRect(x:n*0.27,y:n*0.24,width:n*0.46,height:n*0.53),xRadius:n*0.065,yRadius:n*0.065)
    drive.lineWidth=n*0.032; drive.stroke()
    let line = NSBezierPath(); line.lineWidth=n*0.032; line.lineCapStyle = .round
    line.move(to:NSPoint(x:n*0.28,y:n*0.40)); line.line(to:NSPoint(x:n*0.72,y:n*0.40))
    line.move(to:NSPoint(x:n*0.35,y:n*0.32)); line.line(to:NSPoint(x:n*0.46,y:n*0.32)); line.stroke()
    NSColor.white.setFill(); NSBezierPath(ovalIn:NSRect(x:n*0.62,y:n*0.30,width:n*0.04,height:n*0.04)).fill()
    image.unlockFocus()
    let bitmap=NSBitmapImageRep(data:image.tiffRepresentation!)!
    try bitmap.representation(using:.png,properties:[:])!.write(to:set.appendingPathComponent(name))
}
let task=Process(); task.executableURL=URL(fileURLWithPath:"/usr/bin/iconutil"); task.arguments=["-c","icns",set.path,"-o",root.appendingPathComponent("AppIcon.icns").path]
try task.run(); task.waitUntilExit(); if task.terminationStatus != 0 { exit(task.terminationStatus) }
