import CryptoKit
import Foundation
import LocalAuthentication
import Security

/// Face ID unlock through a Secure Enclave key (RFC-0002 "Device-local
/// biometric envelope").
///
/// Swift does only what needs the Secure Enclave: P-256 ECDH. Rust derives
/// the key-encryption key and wraps or unwraps the vault key, so the vault
/// key never enters Swift. Swift sees only the ECDH shared secret, which is
/// sensitive and never stored.
///
/// The Secure Enclave key uses `.biometryCurrentSet` (adding or removing a
/// face invalidates it) and `WhenUnlockedThisDeviceOnly` (never in backups).
/// The key's opaque blob and the device envelope are stored together in one
/// Keychain item scoped by vault ID, outside the portable vault header.
public enum BiometricUnlock {
    public enum Failure: Error, Equatable {
        case unavailable
        case accessControl
        case keychain(OSStatus)
        case notEnrolled
    }

    struct Enrollment: Codable {
        let keyBlob: Data
        let envelope: Data
    }

    static let service = "org.atlys.vault.device-envelope"

    /// Whether this device can use Face ID or Touch ID with the Secure Enclave.
    /// Always false in the Simulator.
    public static var isAvailable: Bool {
        guard SecureEnclave.isAvailable else { return false }
        var error: NSError?
        return LAContext().canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    /// The biometry type name for UI text ("Face ID" or "Touch ID").
    public static var biometryName: String {
        let context = LAContext()
        _ = context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: nil)
        return context.biometryType == .touchID ? "Touch ID" : "Face ID"
    }

    /// Whether an envelope is stored for this vault.
    public static func isEnrolled(vaultID: String) -> Bool {
        (try? load(vaultID: vaultID)) != nil
    }

    /// Creates a Secure Enclave key and a device envelope for an unlocked
    /// vault, replacing any previous enrollment.
    public static func enroll(session: VaultSession) throws {
        guard SecureEnclave.isAvailable else { throw Failure.unavailable }
        let vaultID = try session.vaultId()
        guard let access = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, [.privateKeyUsage, .biometryCurrentSet], nil
        ) else { throw Failure.accessControl }

        let enclaveKey = try SecureEnclave.P256.KeyAgreement.PrivateKey(accessControl: access)
        let ephemeral = P256.KeyAgreement.PrivateKey()
        let shared = try ephemeral.sharedSecretFromKeyAgreement(with: enclaveKey.publicKey)
        let envelope = try session.deviceEnvelope(
            sharedSecret: shared.withUnsafeBytes { Data($0) },
            ephemeralPublicKey: ephemeral.publicKey.x963Representation,
            enclavePublicKey: enclaveKey.publicKey.x963Representation
        )
        try save(Enrollment(keyBlob: enclaveKey.dataRepresentation, envelope: envelope), vaultID: vaultID)
    }

    /// Prompts for Face ID, asks the Secure Enclave for the shared secret,
    /// and unlocks the vault. Throws `notEnrolled` if there is no envelope,
    /// and a CryptoKit or LocalAuthentication error if the user cancels or
    /// the key was invalidated by a biometry change.
    public static func unlock(location: VaultLocation, reason: String) throws -> VaultSession {
        let vaultID = try vaultIdUnverified(vaultRoot: location.path)
        guard let enrollment = try load(vaultID: vaultID) else { throw Failure.notEnrolled }
        let context = LAContext()
        context.localizedReason = reason
        let enclaveKey = try SecureEnclave.P256.KeyAgreement.PrivateKey(
            dataRepresentation: enrollment.keyBlob, authenticationContext: context
        )
        // The envelope must belong to this enclave key.
        guard try deviceEnvelopeEnclaveKey(deviceEnvelope: enrollment.envelope)
            == enclaveKey.publicKey.x963Representation else { throw Failure.notEnrolled }
        let ephemeralPublic = try P256.KeyAgreement.PublicKey(
            x963Representation: deviceEnvelopeEphemeralKey(deviceEnvelope: enrollment.envelope)
        )
        let shared = try enclaveKey.sharedSecretFromKeyAgreement(with: ephemeralPublic)
        return try unlockWithDevice(
            vaultRoot: location.path, deviceEnvelope: enrollment.envelope,
            sharedSecret: shared.withUnsafeBytes { Data($0) }
        )
    }

    /// Removes the enrollment for one vault. Disabling Face ID and deleting
    /// the vault both call this.
    public static func remove(vaultID: String) {
        SecItemDelete(query(vaultID: vaultID) as CFDictionary)
    }

    static func load(vaultID: String) throws -> Enrollment? {
        var q = query(vaultID: vaultID)
        q[kSecReturnData as String] = true
        var result: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else { throw Failure.keychain(status) }
        return try JSONDecoder().decode(Enrollment.self, from: data)
    }

    static func save(_ enrollment: Enrollment, vaultID: String) throws {
        remove(vaultID: vaultID)
        var item = query(vaultID: vaultID)
        item[kSecValueData as String] = try JSONEncoder().encode(enrollment)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else { throw Failure.keychain(status) }
    }

    static func query(vaultID: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: vaultID,
         kSecUseDataProtectionKeychain as String: true]
    }
}
