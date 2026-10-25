import AtlysCore
import CryptoKit
import Foundation
import LocalAuthentication
import Testing
@testable import AtlysSpike

/// M0 task 0.7: Secure Enclave wrap/unwrap without Face ID (tests cannot pass
/// a biometric prompt). Biometric gating and invalidation are checked
/// manually; see plan/v0/spike-report.md.
@Suite("Secure Enclave envelope", .serialized, .enabled(if: SecureEnclave.isAvailable, "no Secure Enclave"))
struct SecureEnclaveTests {
    private func vault() throws -> SpikeVault {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlys-se-\(UUID().uuidString)")
        return try SpikeVault.create(dir: dir.path(percentEncoded: false))
    }

    @Test("Wrap in the Secure Enclave, unwrap in Rust")
    func wrapAndUnwrap() throws {
        defer { SecureEnclaveSpike.reset() }
        let vault = try vault()
        let enrollment = try SecureEnclaveSpike.enroll(vault: vault, requireBiometry: false)
        #expect(enrollment.envelope.count == 203)
        #expect(try SecureEnclaveSpike.unlock(vault: vault))
    }

    @Test("An envelope for another vault does not unlock this one")
    func otherVaultRejected() throws {
        defer { SecureEnclaveSpike.reset() }
        try SecureEnclaveSpike.enroll(vault: vault(), requireBiometry: false)
        #expect(try SecureEnclaveSpike.unlock(vault: vault()) == false)
    }

    @Test("A tampered envelope fails authentication")
    func tamperedEnvelope() throws {
        let vault = try vault()
        let enclaveKey = try SecureEnclave.P256.KeyAgreement.PrivateKey()
        let ephemeral = P256.KeyAgreement.PrivateKey()
        let shared = try ephemeral.sharedSecretFromKeyAgreement(with: enclaveKey.publicKey).withUnsafeBytes { Data($0) }
        var envelope = try vault.secureEnclaveEnvelope(
            sharedSecret: shared, ephemeralPublicKey: ephemeral.publicKey.x963Representation,
            enclavePublicKey: enclaveKey.publicKey.x963Representation)
        envelope[150] ^= 0x01
        let unwrapShared = try enclaveKey.sharedSecretFromKeyAgreement(with: ephemeral.publicKey).withUnsafeBytes { Data($0) }
        #expect(throws: AtlysError.self) {
            try vault.unlockWithSecureEnclave(envelope: envelope, sharedSecret: unwrapShared)
        }
    }

    @Test("A biometry-protected key fails without authentication")
    func biometryRequired() throws {
        defer { SecureEnclaveSpike.reset() }
        let vault = try vault()
        do {
            try SecureEnclaveSpike.enroll(vault: vault, requireBiometry: true)
        } catch {
            // Expected when no biometry is enrolled on this simulator or device.
            print("biometry enrollment unavailable: \(error)")
            return
        }
        let context = LAContext()
        context.interactionNotAllowed = true
        #expect(throws: (any Error).self) { try SecureEnclaveSpike.unlock(vault: vault, context: context) }
    }
}
