import Foundation

@main
struct CheckMetal {
    static func main() throws {
        try shaderSource.write(toFile: CommandLine.arguments[1], atomically: true, encoding: .utf8)
    }
}
