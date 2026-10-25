import AtlysCore
import Testing

@Suite("App host")
struct AppSmokeTests {
    @Test("The app links the Rust core")
    func appLinksCore() {
        #expect(!coreInfo().version.isEmpty)
    }
}
