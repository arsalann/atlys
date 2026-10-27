import AtlysCore
import Testing

@Suite("Rust core over UniFFI")
struct CoreInfoTests {
    @Test("Swift calls into Rust and gets a typed record back")
    func coreInfoRoundTrip() {
        let info = coreInfo()
        #expect(!info.version.isEmpty)
        #expect(info.admVersion == "0.1")
        #expect(info.displayVersion.hasPrefix("core "))
    }
}
