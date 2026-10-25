import AtlysCollectors
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

    @Test("A malformed ingest public key is rejected")
    func rejectsBadPublicKey() throws {
        let inbox = temporaryInbox()
        try inbox.prepare()
        #expect(throws: (any Error).self) { try inbox.sealer(ingestPublicKey: Data(count: 5)) }
    }
}
