import CoreGraphics
import Foundation
import simd

/// Decorations use the native Swift snow layers and the existing Metal sprite buffer.
final class HolidayScene {
    private struct Figure {
        let layer: SnowLayer
        var x: Float
        var worldX: Float
        var area: Float
        var radii: [Float]
    }
    private struct Ball {
        let layer: SnowLayer
        var x: Float
        var worldX: Float
        var area: Float = 0
    }
    private struct Plow {
        var route: [SnowLayer]
        var x: Float = -55
        var load: Float = 0
    }
    private var figures: [Figure] = []
    private var ball: Ball?
    private var plow: Plow?
    private var powder: [(position: SIMD2<Float>, life: Float)] = []
    var garlands = true
    var windows: [WindowSurface] = []
    var sculpting = false
    private(set) var sprites: [Sprite] = []
    var interactive: Bool { ball != nil || plow != nil }
    var count: Int { sprites.count }

    func clearSnow() {
        figures.removeAll(); ball = nil; plow = nil; powder.removeAll()
        sprites.removeAll()
    }

    func startPlow(layers: [SnowLayer]) {
        guard plow == nil else { return }
        placeBall()
        let windows = layers.dropFirst().filter { layer in layer.top > 0.5 || figures.contains { $0.layer === layer } }
        guard let ground = layers.first else { return }
        plow = Plow(route: [ground] + windows.sorted { $0.base.y < $1.base.y })
    }

    private func placeBall() {
        guard let b = ball else { return }
        ball = nil
        guard b.area >= .pi * 9 else {
            _ = b.layer.deposit(b.area, at: b.layer.base.x + b.x, wide: true)
            return
        }
        let radius = sqrt(b.area / .pi)
        if let i = figures.indices.filter({ figures[$0].layer === b.layer && figures[$0].radii.count < 3
            && abs(figures[$0].x - b.x) < figures[$0].radii[0] + 20 }).min(by: { abs(figures[$0].x-b.x) < abs(figures[$1].x-b.x) }) {
            figures[i].radii.append(min(radius, figures[i].radii.last! * 0.78))
            figures[i].area += b.area
        } else if figures.count < 24 {
            figures.append(Figure(layer: b.layer, x: b.x, worldX: b.worldX, area: b.area, radii: [radius]))
        } else {
            _ = b.layer.deposit(b.area, at: b.worldX, wide: true)
        }
    }

    private func sculpt(cursor: SIMD2<Float>, layers: [SnowLayer], groundSettles: Bool, width: Float, height: Float) {
        guard sculpting, plow == nil, cursor.x >= 0, cursor.x < width, cursor.y >= 0, cursor.y < height else { placeBall(); return }
        if ball == nil, figures.count < 24,
           let layer = layers.reversed().first(where: { layer in
               guard layer !== layers.first || groundSettles, let i = layer.index(at: cursor.x) else { return false }
               return layer.visible[i] && layer.heights[i] - layer.leaves[i] > 1 && abs(cursor.y-layer.surface(i)) < 28
           }) {
            ball = Ball(layer: layer, x: cursor.x-layer.base.x, worldX: cursor.x)
        }
        guard var b = ball else { return }
        let layer = b.layer
        let x = min(max(cursor.x-layer.base.x, 0), layer.width-layer.columnWidth)
        let distance = min(abs(x-b.x), 60)
        let direction: Float = b.x > x ? 1 : -1
        b.x = x; b.worldX = x+layer.base.x
        if distance >= 0.5 {
            let steps = max(1, Int(ceil(distance/layer.columnWidth)))
            for n in 0...steps {
                let i = layer.clampedIndex(at: layer.base.x+x+direction*distance*Float(n)/Float(steps))
                guard layer.visible[i] else { continue }
                let available = max(layer.heights[i]-layer.leaves[i], 0)
                let take = min(available, max((.pi*28*28-b.area)/layer.columnWidth, 0))
                layer.remove(snow: take, from: i)
                b.area += take*layer.columnWidth
            }
            layer.refreshTop()
        }
        ball = b
    }

    func step(dt: Float, cursor: SIMD2<Float>, layers: [SnowLayer], snow: Bool, groundSettles: Bool, width: Float, height: Float, time: Float) {
        guard let ground = layers.first else { return }
        let live = Set(layers.map(ObjectIdentifier.init))
        if !snow { clearSnow() }
        for i in figures.indices {
            if live.contains(ObjectIdentifier(figures[i].layer)) { figures[i].worldX = figures[i].layer.base.x+figures[i].x }
            else { _ = ground.deposit(figures[i].area, at: figures[i].worldX, wide: true) }
        }
        figures.removeAll { !live.contains(ObjectIdentifier($0.layer)) }
        if let b = ball, !live.contains(ObjectIdentifier(b.layer)) {
            _ = ground.deposit(b.area, at: b.worldX, wide: true); ball = nil
        }
        if snow { sculpt(cursor: cursor, layers: layers, groundSettles: groundSettles, width: width, height: height) }
        powder = powder.compactMap { p in
            let life = p.life-dt
            return life > 0 ? (p.position+SIMD2(70,35)*dt,life) : nil
        }
        if var p = plow {
            while let layer = p.route.last, !live.contains(ObjectIdentifier(layer)) { p.route.removeLast(); p.x = -55; p.load = 0 }
            if let layer = p.route.last {
                let old = p.x
                p.x += dt*105
                let blade = p.x+37
                var removed = false
                for i in 0..<layer.count {
                    let x = (Float(i)+0.5)*layer.columnWidth
                    if x >= old+32 && x <= blade && layer.heights[i] > 0 {
                        p.load += layer.heights[i]*layer.columnWidth; layer.set(i,0); removed = true
                    }
                }
                figures.removeAll { $0.layer === layer && $0.x <= blade && $0.x >= old-50 }
                if removed && powder.count < 80 { powder.append((layer.base+SIMD2(blade,15),0.65)) }
                layer.refreshTop()
                if p.x > layer.width+55 { p.route.removeLast(); p.x = -55; p.load = 0 }
            }
            plow = p.route.isEmpty ? nil : p
        }
        sprites.removeAll(keepingCapacity: true)
        for f in figures {
            let x = f.layer.base.x+f.x
            guard let i = f.layer.index(at: x), f.layer.visible[i] else { continue }
            var y = f.layer.surface(i)-f.radii[0]*0.15
            for (n,r) in f.radii.enumerated() {
                y += r
                let size = r*(n == 2 ? 3.6 : 2.3)
                var s = Sprite(position: SIMD2(x,y), size: size, alpha: 1, soft: 0)
                s.petal = n == 2 ? -3 : -1; s.ground = r/size; s.eyes = n == 1 ? 1 : 0
                sprites.append(s); y += r*0.70
            }
        }
        if let b = ball {
            let x = b.layer.base.x+b.x
            let r = max(sqrt(b.area / .pi),1)
            sphere(at: SIMD2(x,b.layer.surface(b.layer.clampedIndex(at: x))+r), radius: r)
        }
        if let p = plow, let layer = p.route.last {
            let x = layer.base.x+p.x
            let i = layer.clampedIndex(at: x)
            if layer.visible[i] {
                if p.load > 0 { let r = min(sqrt(p.load / .pi),21); sphere(at: SIMD2(x+42,layer.floor(i)+r*0.55),radius:r) }
                var s = Sprite(position: SIMD2(x,layer.floor(i)+23),size:105,alpha:1,soft:0)
                s.petal = -2; sprites.append(s)
            }
        }
        for p in powder { sprites.append(Sprite(position:p.position,size:12*p.life,alpha:p.life,soft:1)) }
        if garlands { lights(width: width,height: height,time: time) }
    }

    private func sphere(at position: SIMD2<Float>, radius: Float) {
        var s = Sprite(position:position,size:radius*2.3,alpha:1,soft:0)
        s.petal = -1; s.ground = 1/2.3; sprites.append(s)
    }

    private func lights(width: Float,height: Float,time: Float) {
        let colors: [SIMD3<Float>] = [SIMD3(1,0.12,0.08),SIMD3(1,0.65,0.12),SIMD3(0.12,0.9,0.32),SIMD3(0.18,0.42,1),SIMD3(0.95,0.25,0.65)]
        for w in windows.prefix(32) where !w.isDock && !w.sweeps {
            let left = max(Float(w.frame.minX)+8,0), right = min(Float(w.frame.maxX)-8,width), top = Float(w.frame.maxY)
            guard right > left, top <= height+5, top >= 12 else { continue }
            func cord(_ x: Float) -> SIMD2<Float> {
                let raw = (x-Float(w.frame.minX)-8)/156
                let t = raw-floor(raw)
                return SIMD2(x,top+2-36*t*(1-t))
            }
            func visible(_ p: SIMD2<Float>, _ margin: Float) -> Bool {
                let bounds = CGRect(x:CGFloat(p.x-margin),y:CGFloat(p.y-margin),width:CGFloat(margin*2),height:CGFloat(margin*2))
                return !(w.cover+w.blockers).contains { $0.intersects(bounds) }
            }
            for n in 0..<Int(ceil((right-left)/5)) {
                let a = cord(left+Float(n)*5), b = cord(min(left+Float(n+1)*5,right)), middle = (a+b)*0.5
                guard visible(middle,3) else { continue }
                let size = simd_length(b-a)+2
                var s = Sprite(position:middle,size:size,alpha:1,soft:0)
                s.petal = -4; s.ground = (b.x-a.x)/size*0.5; s.eyes = (b.y-a.y)/size*0.5; sprites.append(s)
            }
            let first = Int(ceil((left-Float(w.frame.minX)-15)/26)), last = Int(floor((right-Float(w.frame.minX)-15)/26))
            guard first <= last else { continue }
            for n in first...last {
                let at = cord(Float(w.frame.minX)+15+Float(n)*26)+SIMD2(0,-5)
                guard visible(at,10) else { continue }
                let group = (n%4+4)%4, color = colors[(n%5+5)%5]
                let phase = time*1.65+Float(group)*(.pi/2)+Float(w.id%97)*0.13
                let t = min(max((sin(phase)+0.45)/1.1,0),1)
                var s = Sprite(position:at,size:24,alpha:0.16+0.84*t*t*(3-2*t),soft:0)
                s.petal = -5; s.tintRed = color.x; s.tintGreen = color.y; s.tintBlue = color.z; sprites.append(s)
            }
        }
    }
}
