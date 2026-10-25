import AtlysCollectors
import AtlysCore
import CryptoKit
import Foundation
import Testing

/// M0 task 0.5: Swift seals a batch, Rust opens it with the private key, and
/// tampered files are rejected.
@Suite("Sealed inbox round trip")
struct SealedInboxTests {
    let root = FileManager.default.temporaryDirectory.appending(path: "atlys-spike-\(UUID().uuidString)")
    var vaultDir: String { root.appending(path: "vault").path(percentEncoded: false) }
    var inbox: InboxLocation { InboxLocation(url: root.appending(path: "inbox")) }

    private func syntheticSteps(_ n: Int) -> [RecordDraft] {
        (0..<n).map { i in
            RecordDraft(recordType: "health.steps", sourceId: "synthetic-\(i)",
                        startMs: 1_700_000_000_000 + Int64(i) * 60_000, endMs: 1_700_000_060_000 + Int64(i) * 60_000,
                        dataJson: #"{"count":\#(100 + i)}"#)
        }
    }

    @Test("Swift seals through Rust; Rust opens with the private key")
    func swiftSealsRustOpens() throws {
        try inbox.prepare()
        let vault = try SpikeVault.create(dir: vaultDir)
        let sealer = try inbox.sealer(ingestPublicKey: vault.ingestPublicKey())
        _ = try sealer.seal(collector: "health", records: syntheticSteps(3))
        _ = try sealer.seal(collector: "health", records: syntheticSteps(1))
        #expect(inbox.pendingCount() == 2)

        let report = try vault.openInbox(inboxDir: inbox.url.path(percentEncoded: false), deleteOpened: true)
        #expect(report.opened == 2)
        #expect(report.rejected == 0)
        #expect(report.records == 4)
        #expect(report.batches.allSatisfy { $0.recordTypes == ["health.steps"] })
        #expect(inbox.pendingCount() == 0)
    }

    @Test("Tampered and foreign files are rejected and left in place")
    func tamperedFilesRejected() throws {
        try inbox.prepare()
        let vault = try SpikeVault.create(dir: vaultDir)
        let other = try SpikeVault.create(dir: root.appending(path: "other").path(percentEncoded: false))
        let sealer = try inbox.sealer(ingestPublicKey: vault.ingestPublicKey())

        let name = try sealer.seal(collector: "health", records: syntheticSteps(1))
        let file = inbox.url.appending(path: name)
        var bytes = try Data(contentsOf: file)
        bytes[bytes.count / 2] ^= 0x01
        try bytes.write(to: file)

        // A batch sealed to a different vault's key.
        _ = try inbox.sealer(ingestPublicKey: other.ingestPublicKey()).seal(collector: "health", records: syntheticSteps(1))

        let report = try vault.openInbox(inboxDir: inbox.url.path(percentEncoded: false), deleteOpened: true)
        #expect(report.opened == 0)
        #expect(report.rejected == 2)
        #expect(inbox.pendingCount() == 2)
    }

    @Test("Invalid records are rejected before sealing")
    func invalidRecordsRejected() throws {
        try inbox.prepare()
        let vault = try SpikeVault.create(dir: vaultDir)
        let sealer = try inbox.sealer(ingestPublicKey: vault.ingestPublicKey())
        let bad = RecordDraft(recordType: "Steps", sourceId: "x", startMs: 0, endMs: nil, dataJson: "{}")
        #expect(throws: AtlysError.self) { try sealer.seal(collector: "health", records: [bad]) }
        #expect(inbox.pendingCount() == 0)
    }

    /// The Simulator does not implement Data Protection (attributes read back
    /// as nil), so this is strict only on a device. Run it there in M0 manual steps.
    @Test("Files written by Rust inherit the inbox protection class")
    func filesInheritProtection() throws {
        try inbox.prepare()
        let vault = try SpikeVault.create(dir: vaultDir)
        let name = try inbox.sealer(ingestPublicKey: vault.ingestPublicKey()).seal(collector: "health", records: syntheticSteps(1))
        let protection = try FileManager.default.attributesOfItem(
            atPath: inbox.url.appending(path: name).path(percentEncoded: false))[.protectionKey] as? FileProtectionType
        #if targetEnvironment(simulator)
        #expect(protection == nil || protection == .completeUntilFirstUserAuthentication)
        #else
        #expect(protection == .completeUntilFirstUserAuthentication)
        #endif
    }

    /// Interop check: an inbox file built entirely with Apple CryptoKit's HPKE
    /// (an independent RFC 9180 implementation) opens in Rust. This pins the
    /// file format, info string, AAD, and padding as implementable elsewhere.
    @Test("CryptoKit HPKE output opens in Rust")
    func cryptoKitSealsRustOpens() throws {
        try inbox.prepare()
        let vault = try SpikeVault.create(dir: vaultDir)
        let recipient = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: vault.ingestPublicKey())

        let header = Data("ATLYSINB".utf8) + Data([1, 1])
        let json = Data(#"{"adm_version":"0-draft","collector":"cryptokit","created_ms":1700000000000,"records":[{"type":"health.steps","source_id":"synthetic-ck","start_ms":1700000000000,"data":{"count":7}}]}"#.utf8)
        var padded = withUnsafeBytes(of: UInt32(json.count).bigEndian) { Data($0) } + json
        padded.append(Data(count: 1024 - padded.count))

        var sender = try HPKE.Sender(recipientKey: recipient, ciphersuite: .Curve25519_SHA256_ChachaPoly,
                                     info: Data("atlys/v1/inbox".utf8))
        let ciphertext = try sender.seal(padded, authenticating: header)
        let file = header + sender.encapsulatedKey + ciphertext
        try file.write(to: inbox.url.appending(path: "01JCRYPTOKITINTEROP0000000.sealed"))

        var tampered = file
        tampered[9] = 2 // claim a different HPKE suite in the authenticated header
        try tampered.write(to: inbox.url.appending(path: "01JCRYPTOKITINTEROP0000001.sealed"))

        let report = try vault.openInbox(inboxDir: inbox.url.path(percentEncoded: false), deleteOpened: false)
        #expect(report.opened == 1)
        #expect(report.rejected == 1)
        #expect(report.batches.first?.collector == "cryptokit")
        #expect(report.batches.first?.recordTypes == ["health.steps"])
    }
}
