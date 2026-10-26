import AtlysCore
import Foundation

/// Access to the spike's stand-in vault.
///
/// Background code only ever reads `ingest.pub` and builds a `SpikeSealer`
/// from it, exactly like the product will. The private keys are loaded only
/// from the foreground, to open the inbox.
enum SpikeVaultStore {
    /// Loads the spike vault, creating it on first use (foreground only).
    static func loadOrCreate() throws -> SpikeVault {
        try SpikePaths.prepare()
        let dir = SpikePaths.path(try SpikePaths.vaultDir)
        if let vault = try? SpikeVault.open(dir: dir) {
            return vault
        }
        let vault = try SpikeVault.create(dir: dir)
        // The spike key file holds unencrypted private keys: use the strongest class.
        let keyFile = try SpikePaths.vaultDir.appending(path: "spike-keys.bin")
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete],
                                              ofItemAtPath: SpikePaths.path(keyFile))
        return vault
    }

    /// Builds a sealer from the public key alone. Safe while the device is locked.
    static func sealer() throws -> SpikeSealer {
        let publicKey = try Data(contentsOf: SpikePaths.vaultDir.appending(path: "ingest.pub"))
        return try SpikePaths.inbox.spikeSealer(ingestPublicKey: publicKey)
    }
}
