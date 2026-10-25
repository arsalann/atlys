import AtlysCollectors
import Foundation

/// File locations for the spike app, with their file protection classes.
///
/// - `spike/vault/`: Complete Until First User Authentication, so the ingest
///   public key (`ingest.pub`) is readable in the background while locked.
///   The spike's unencrypted private key file is downgraded to *Complete*.
/// - `inbox/`: Complete Until First User Authentication (see InboxLocation).
/// - `spike/wake-log.jsonl`, `spike/health-anchor`: same, for background writes.
enum SpikePaths {
    static var base: URL {
        get throws {
            try FileManager.default.url(
                for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
            )
        }
    }

    static var spikeDir: URL { get throws { try base.appending(path: "spike", directoryHint: .isDirectory) } }
    static var vaultDir: URL { get throws { try spikeDir.appending(path: "vault", directoryHint: .isDirectory) } }
    static var wakeLog: URL { get throws { try spikeDir.appending(path: "wake-log.jsonl") } }
    static var healthAnchor: URL { get throws { try spikeDir.appending(path: "health-anchor") } }
    static var inbox: InboxLocation { get throws { try InboxLocation.applicationSupport() } }

    static func prepare() throws {
        let fm = FileManager.default
        for dir in [try spikeDir, try vaultDir] {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true,
                                   attributes: [.protectionKey: InboxLocation.protection])
        }
        try inbox.prepare()
    }

    static func path(_ url: URL) -> String { url.path(percentEncoded: false) }
}
