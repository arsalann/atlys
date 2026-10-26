import Foundation

/// Where the vault lives in the app's container, and its file protection.
///
/// The layout under `root` is owned by the Rust core (RFC-0002):
/// `vault/header.json`, `vault/vault.db`, `inbox/`, `quarantine/`.
///
/// File protection (verify on a device; the Simulator ignores it):
/// - `root` and everything that inherits from it (header, inbox, quarantine):
///   *Complete Until First User Authentication*, so background collectors can
///   read the ingest public key and write sealed batches while locked.
/// - `vault.db` and its WAL/SHM files: *Complete*, via ``protectDatabase()``.
public struct VaultLocation: Sendable, Equatable {
    /// Protection for the container and background-readable files.
    public static let containerProtection: FileProtectionType = .completeUntilFirstUserAuthentication
    /// Protection for the database files.
    public static let databaseProtection: FileProtectionType = .complete

    public let root: URL

    public init(root: URL) {
        self.root = root
    }

    /// `Application Support/Atlys` in the app's own container.
    public static func applicationSupport(fileManager: FileManager = .default) throws -> VaultLocation {
        let base = try fileManager.url(
            for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
        )
        return VaultLocation(root: base.appending(path: "Atlys", directoryHint: .isDirectory))
    }

    /// The root as a file system path, for the Rust core.
    public var path: String { root.path(percentEncoded: false) }

    var databaseFiles: [URL] {
        ["vault.db", "vault.db-wal", "vault.db-shm"].map {
            root.appending(path: "vault/\($0)")
        }
    }

    public var exists: Bool { vaultExists(vaultRoot: path) }

    /// Creates the container with its protection class before the core
    /// writes into it, so new files inherit the class.
    public func prepare(fileManager: FileManager = .default) throws {
        try fileManager.createDirectory(
            at: root, withIntermediateDirectories: true,
            attributes: [.protectionKey: Self.containerProtection]
        )
        try fileManager.setAttributes([.protectionKey: Self.containerProtection], ofItemAtPath: path)
    }

    /// Applies *Complete* protection to the database files that exist. Call
    /// after creating or unlocking, while the database is open.
    public func protectDatabase(fileManager: FileManager = .default) {
        for url in databaseFiles where fileManager.fileExists(atPath: url.path(percentEncoded: false)) {
            try? fileManager.setAttributes([.protectionKey: Self.databaseProtection],
                                           ofItemAtPath: url.path(percentEncoded: false))
        }
    }

    /// Removes the whole container. Test and reset use only.
    public func removeAll(fileManager: FileManager = .default) throws {
        if fileManager.fileExists(atPath: path) {
            try fileManager.removeItem(at: root)
        }
    }
}
