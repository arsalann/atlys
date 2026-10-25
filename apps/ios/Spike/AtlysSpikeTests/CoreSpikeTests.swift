import AtlysCore
import Foundation
import Testing

/// M0 tasks 0.3 and 0.6 on the simulator. The same calls run on devices from
/// the spike app's Core and Argon2 tabs.
@Suite("SQLCipher and Argon2id on iOS")
struct CoreSpikeTests {
    @Test("SQLCipher opens with a raw key, rejects the wrong key, and leaves no plaintext")
    func sqlcipherSelfTest() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlys-sqlcipher-\(UUID().uuidString)")
        let report = try spikeSqlcipherSelfTest(dir: dir.path(percentEncoded: false))
        #expect(report.provider == "commoncrypto")
        #expect(report.cipherVersion.hasPrefix("4."))
        #expect(report.roundTripOk)
        #expect(report.wrongKeyRejected)
        #expect(report.headerEncrypted)
        #expect(report.plaintextAbsent)
        let repeats = try (0..<3).map { _ in try spikeSqlcipherSelfTest(dir: dir.path(percentEncoded: false)).elapsedMs }
        print("SQLCipher \(report.cipherVersion), SQLite \(report.sqliteVersion), provider \(report.provider), "
            + "self-test ms: first \(Int(report.elapsedMs)), then \(repeats.map { Int($0) })")
    }

    @Test("Argon2id runs through the FFI at the v0 default parameters")
    func argon2Default() throws {
        let ms = try spikeArgon2Ms(memoryKib: 64 * 1024, iterations: 3, parallelism: 1)
        #expect(ms > 0)
        print("Argon2id m=64MiB t=3 p=1 on this simulator host: \(String(format: "%.0f", ms)) ms")
    }
}
