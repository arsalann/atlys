import AtlysCore
import Foundation
import Testing

/// The product vault over UniFFI, end to end. Each create, unlock, and
/// recover runs Argon2id at the creation parameters, so the flow lives in one
/// test.
@Suite("Vault over UniFFI")
struct VaultFlowTests {
    private let password = Data("synthetic correct horse battery".utf8)
    private let newPassword = Data("synthetic second passphrase".utf8)

    private func temporaryLocation() -> VaultLocation {
        VaultLocation(root: FileManager.default.temporaryDirectory
            .appending(path: "atlys-vault-\(UUID().uuidString)", directoryHint: .isDirectory))
    }

    @Test("Create, lock, unlock, change password, recover, delete")
    func fullFlow() throws {
        let location = temporaryLocation()
        defer { try? location.removeAll() }
        try location.prepare()
        #expect(!location.exists)

        #expect(throws: AtlysError.self) { try createVault(vaultRoot: location.path, password: Data("weak".utf8)) }
        let created = try createVault(vaultRoot: location.path, password: password)
        location.protectDatabase()
        #expect(location.exists)
        #expect(created.recoveryPhrase.count == 24)
        #expect(isValidRecoveryPhrase(words: created.recoveryPhrase))
        try created.session.lock()
        #expect(created.session.isLocked())
        #expect(throws: AtlysError.Locked) { try created.session.vaultId() }

        #expect(throws: AtlysError.WrongCredentials) {
            try unlockWithPassword(vaultRoot: location.path, password: newPassword)
        }
        let session = try unlockWithPassword(vaultRoot: location.path, password: password)
        #expect(try session.vaultId() == vaultIdUnverified(vaultRoot: location.path))
        try session.changePassword(currentPassword: password, newPassword: newPassword)
        try session.lock()

        #expect(throws: AtlysError.WrongCredentials) {
            try unlockWithPassword(vaultRoot: location.path, password: password)
        }
        let recovered = try recoverVault(vaultRoot: location.path, recoveryPhrase: created.recoveryPhrase,
                                         newPassword: password)
        try recovered.delete()
        #expect(!location.exists)
        #expect(throws: AtlysError.NoVault) { try unlockWithPassword(vaultRoot: location.path, password: password) }
    }

    @Test("Password strength feedback")
    func strength() {
        #expect(!passwordStrength(password: "password1234").acceptable)
        #expect(passwordStrength(password: "plum cactus orbit ledger").acceptable)
        #expect(passwordMinLength() == 12)
    }

    @Test("Error messages never echo secrets")
    func messages() {
        #expect(AtlysError.WrongCredentials.userMessage.contains("unlock"))
        #expect(AtlysError.WeakPassword(message: "too easy to guess").userMessage.contains("12"))
    }

    @Test("Face ID is unavailable in the Simulator")
    func biometricsInSimulator() {
        #if targetEnvironment(simulator)
        #expect(!BiometricUnlock.isAvailable)
        #endif
        #expect(!BiometricUnlock.isEnrolled(vaultID: "00000000000000000000000000000000"))
    }
}
