import CoreGraphics
import Foundation

@main
struct HolidaySmoke {
    static func main() {
        let ground = SnowLayer(origin: .zero, columns: 320, columnWidth: 2, depth: 40, opensAtEdges: false, phases: (0,0))
        for i in 0..<ground.count { ground.set(i,25) }
        ground.refreshTop()
        let scene = HolidayScene()
        scene.garlands = false
        func step(_ x: Float, held: Bool, time: Float = 0) {
            scene.sculpting = held
            scene.step(dt:1/60,cursor:SIMD2(x,25),layers:[ground],snow:true,groundSettles:true,width:640,height:480,time:time)
        }
        func roll(_ from: Float, _ to: Float) {
            for n in 0...20 { step(from+(to-from)*Float(n)/20,held:true) }
            step(to,held:false)
        }
        roll(100,180); roll(260,190); roll(350,185)
        precondition(scene.sprites.contains { $0.petal == -3 }, "Three rolled balls must become a snowman")
        precondition(scene.sprites.filter { $0.petal == -1 }.count == 2)
        scene.startPlow(layers:[ground])
        for _ in 0..<1000 { step(-100,held:false) }
        precondition(!scene.interactive)
        precondition(ground.heights.allSatisfy { $0 == 0 }, "Plow must clear its route")
        let window = WindowSurface(id:7,frame:CGRect(x:100,y:100,width:400,height:250),blockers:[],cover:[],cornerRadius:8,cornerExponent:2.5,isDock:false,sweeps:false)
        scene.windows = [window]; scene.garlands = true
        step(-100,held:false,time:0)
        let initial = scene.sprites.filter { $0.petal == -5 }.map(\.alpha)
        precondition(!initial.isEmpty)
        step(-100,held:false,time:1)
        let later = scene.sprites.filter { $0.petal == -5 }.map(\.alpha)
        precondition(zip(initial,later).contains { abs($0.0-$0.1)>0.1 }, "Bulbs must blink")
        scene.garlands = false; step(-100,held:false)
        precondition(scene.sprites.isEmpty, "Switch must remove all lights")
        print("Native snowman, plow, garland animation and switch checks passed")
    }
}
