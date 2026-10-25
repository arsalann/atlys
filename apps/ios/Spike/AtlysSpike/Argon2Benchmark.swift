import AtlysCore
import Foundation
import UIKit

/// Argon2id calibration grid (M0 task 0.6). Same grid as
/// `cargo run --release -p atlys-crypto --example argon2_bench`.
struct Argon2Benchmark: Sendable {
    static let memoryMiB: [UInt32] = [19, 46, 64, 128, 192, 256]
    static let largeMemoryMiB: [UInt32] = [384]
    static let iterations: [UInt32] = [1, 2, 3, 4, 6]

    struct Cell: Sendable, Hashable {
        let memoryMiB: UInt32
        let iterations: UInt32
        let medianMs: Double
    }

    var runs = 3
    var includeLarge = false

    var grid: [(UInt32, UInt32)] {
        let memory = Self.memoryMiB + (includeLarge ? Self.largeMemoryMiB : [])
        return memory.flatMap { m in Self.iterations.map { (m, $0) } }
    }

    /// Runs the grid off the main actor, reporting each cell as it finishes.
    func run(progress: @Sendable (Cell) async -> Void) async throws {
        for (m, t) in grid {
            try Task.checkCancellation()
            var times: [Double] = []
            for _ in 0..<runs {
                times.append(try spikeArgon2Ms(memoryKib: m * 1024, iterations: t, parallelism: 1))
            }
            times.sort()
            await progress(Cell(memoryMiB: m, iterations: t, medianMs: times[times.count / 2]))
        }
    }

    /// Markdown table for the spike report.
    @MainActor
    static func markdown(_ cells: [Cell], runs: Int) -> String {
        var out = "Device: \(deviceModel()) · iOS \(UIDevice.current.systemVersion) · "
        out += "thermal: \(thermalState()) · low power: \(ProcessInfo.processInfo.isLowPowerModeEnabled)\n"
        out += "Argon2id, p = 1, median of \(runs) runs (ms)\n\n| m \\ t |"
        out += iterations.map { " t=\($0) |" }.joined() + "\n|---|" + String(repeating: "---|", count: iterations.count) + "\n"
        let memories = Array(Set(cells.map(\.memoryMiB))).sorted()
        for m in memories {
            out += "| \(m) MiB |"
            for t in iterations {
                let value = cells.first { $0.memoryMiB == m && $0.iterations == t }.map { String(format: " %.0f |", $0.medianMs) }
                out += value ?? " – |"
            }
            out += "\n"
        }
        return out
    }

    /// Hardware model identifier, for example `iPhone11,8`. Not personal data.
    static func deviceModel() -> String {
        #if targetEnvironment(simulator)
        return "Simulator (\(ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] ?? "unknown"))"
        #else
        var info = utsname()
        uname(&info)
        return withUnsafeBytes(of: &info.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        #endif
    }

    static func thermalState() -> String {
        switch ProcessInfo.processInfo.thermalState {
        case .nominal: "nominal"
        case .fair: "fair"
        case .serious: "serious"
        case .critical: "critical"
        @unknown default: "unknown"
        }
    }
}
