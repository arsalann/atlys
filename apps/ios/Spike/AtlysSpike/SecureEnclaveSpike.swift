import AtlysCore
import CryptoKit
import Foundation
import LocalAuthentication
import Security

/// Face ID unlock through a Secure Enclave key (M0 task 0.7).
///
/// Swift does only what needs the Secure Enclave: P-256 ECDH. Rust derives the
/// key-encryption key and wraps or unwraps the vault key, so the vault key
/// never crosses into Swift (see core/crates/atlys-vault/src/envelope.rs).
///
/// The Secure Enclave key is created with `.biometryCurrentSet`, so adding or
/// removing a face or fingerprint invalidates it, and with
/// `WhenUnlockedThisDeviceOnly`, so it never leaves this device or its backups.
enum SecureEnclaveSpike {
    enum Failure: Error, CustomStringConvertible {
        case unavailable
        case accessControl
        case keychain(OSStatus)
        case notEnrolled

        var description: String {
            switch self {
            case .unavailable: "Secure Enclave not available"
            case .accessControl: "could not create access control"
            case .keychain(let status): "keychain error \(status)"
            case .notEnrolled: "no Face ID envelope yet"
            }
        }
    }

    struct Enrollment: Codable {
        /// Opaque, device-bound blob from CryptoKit. Only this device's Secure
        /// Enclave can use it, and only after biometric authentication.
        let keyBlob: Data
        let envelope: Data
        let requiresBiometry: Bool
    }

    private static let service = "org.atlys.spike.secure-enclave"
    private static let account = "vault-envelope"

    static var isAvailable: Bool { SecureEnclave.isAvailable }

    /// Creates a Secure Enclave key and wraps the vault key to it.
    /// `requireBiometry: false` is for automated tests, which cannot pass Face ID.
    @discardableResult
    static func enroll(vault: SpikeVault, requireBiometry: Bool = true) throws -> Enrollment {
        guard isAvailable else { throw Failure.unavailable }
        var flags: SecAccessControlCreateFlags = [.privateKeyUsage]
        if requireBiometry { flags.insert(.biometryCurrentSet) }
        guard let access = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, flags, nil
        ) else { throw Failure.accessControl }

        let enclaveKey = try SecureEnclave.P256.KeyAgreement.PrivateKey(accessControl: access)
        let ephemeral = P256.KeyAgreement.PrivateKey()
        let shared = try ephemeral.sharedSecretFromKeyAgreement(with: enclaveKey.publicKey)
        let envelope = try vault.secureEnclaveEnvelope(
            sharedSecret: shared.withUnsafeBytes { Data($0) },
            ephemeralPublicKey: ephemeral.publicKey.x963Representation,
            enclavePublicKey: enclaveKey.publicKey.x963Representation
        )
        let enrollment = Enrollment(keyBlob: enclaveKey.dataRepresentation, envelope: envelope,
                                    requiresBiometry: requireBiometry)
        try save(enrollment)
        return enrollment
    }

    /// Asks the Secure Enclave for the ECDH shared secret (prompting for Face ID
    /// if required) and lets Rust unwrap the vault key. Returns true if the
    /// envelope held this vault's key.
    static func unlock(vault: SpikeVault, context: LAContext = LAContext()) throws -> Bool {
        guard let enrollment = try load() else { throw Failure.notEnrolled }
        context.localizedReason = "Unlock your Atlys vault"
        let enclaveKey = try SecureEnclave.P256.KeyAgreement.PrivateKey(
            dataRepresentation: enrollment.keyBlob, authenticationContext: context
        )
        let ephemeralPublic = try P256.KeyAgreement.PublicKey(
            x963Representation: spikeSecureEnclaveEphemeralKey(envelope: enrollment.envelope)
        )
        let shared = try enclaveKey.sharedSecretFromKeyAgreement(with: ephemeralPublic)
        return try vault.unlockWithSecureEnclave(
            envelope: enrollment.envelope, sharedSecret: shared.withUnsafeBytes { Data($0) }
        )
    }

    static func reset() {
        SecItemDelete(baseQuery as CFDictionary)
    }

    static func load() throws -> Enrollment? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else { throw Failure.keychain(status) }
        return try JSONDecoder().decode(Enrollment.self, from: data)
    }

    private static func save(_ enrollment: Enrollment) throws {
        reset()
        var item = baseQuery
        item[kSecValueData as String] = try JSONEncoder().encode(enrollment)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else { throw Failure.keychain(status) }
    }

    private static var baseQuery: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: account]
    }
}
