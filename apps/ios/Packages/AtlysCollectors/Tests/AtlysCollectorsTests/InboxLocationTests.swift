import AtlysCollectors
import AtlysCore
import Foundation
import Testing

@Suite("Inbox location")
struct InboxLocationTests {
    private func temporaryInbox() -> InboxLocation {
        InboxLocation(url: FileManager.default.temporaryDirectory
            .appending(path: "atlys-tests-\(UUID().uuidString)/inbox", directoryHint: .isDirectory))
    }

    @Test("prepare() creates the directory and sets the protection class")
    func prepareSetsProtection() throws {
        let inbox = temporaryInbox()
        try inbox.prepare()
        let protection = try FileManager.default.attributesOfItem(
            atPath: inbox.url.path(percentEncoded: false))[.protectionKey] as? FileProtectionType
        #if targetEnvironment(simulator)
        // The Simulator does not implement Data Protection; attributes read back as nil.
        #expect(protection == nil || protection == InboxLocation.protection)
        #else
        #expect(protection == InboxLocation.protection)
        #endif
        #expect(inbox.pendingCount() == 0)
    }

    @Test("A sealer needs a vault")
    func sealerNeedsVault() throws {
        let inbox = temporaryInbox()
        try inbox.prepare()
        #expect(throws: AtlysError.NoVault) { try inbox.sealer() }
    }

    @Test("The vault-root initializer uses the core's layout")
    func vaultRootLayout() {
        let root = URL(filePath: "/tmp/atlys-root", directoryHint: .isDirectory)
        #expect(InboxLocation(vaultRoot: root).url.lastPathComponent == "inbox")
    }
}
