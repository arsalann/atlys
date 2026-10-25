import AtlysCore
import Foundation

/// The directory where collectors drop sealed batches until the next unlock.
///
/// The directory uses the *Complete Until First User Authentication* file
/// protection class, which new files inherit. Background collectors can then
/// write while the phone is locked, as long as it has been unlocked once since
/// boot. See plan/v0/03-ios-app.md#the-write-only-ingest-inbox.
public struct InboxLocation: Sendable, Equatable {
    /// File protection class for the inbox and other background-writable files.
    public static let protection: FileProtectionType = .completeUntilFirstUserAuthentication

    public let url: URL

    public init(url: URL) {
        self.url = url
    }

    /// `Application Support/inbox` in the app's container.
    public static func applicationSupport(fileManager: FileManager = .default) throws -> InboxLocation {
        let base = try fileManager.url(
            for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
        )
        return InboxLocation(url: base.appending(path: "inbox", directoryHint: .isDirectory))
    }

    /// Creates the directory if needed and applies the protection class.
    public func prepare(fileManager: FileManager = .default) throws {
        try fileManager.createDirectory(
            at: url, withIntermediateDirectories: true,
            attributes: [.protectionKey: Self.protection]
        )
        try fileManager.setAttributes([.protectionKey: Self.protection], ofItemAtPath: url.path(percentEncoded: false))
    }

    /// Number of sealed batches waiting to be merged.
    public func pendingCount(fileManager: FileManager = .default) -> Int {
        let names = (try? fileManager.contentsOfDirectory(atPath: url.path(percentEncoded: false))) ?? []
        return names.filter { $0.hasSuffix(".sealed") }.count
    }

    /// A sealer that writes into this inbox using only the ingest public key.
    public func sealer(ingestPublicKey: Data) throws -> IngestSealer {
        try IngestSealer(inboxDir: url.path(percentEncoded: false), ingestPublicKey: ingestPublicKey)
    }
}
