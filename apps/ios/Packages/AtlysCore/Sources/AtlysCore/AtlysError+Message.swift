import Foundation

extension AtlysError {
    /// A short, user-facing explanation. Never includes key material.
    public var userMessage: String {
        switch self {
        case .WrongCredentials:
            "That didn't unlock the vault. Check it and try again."
        case .IntegrityFailure:
            "The vault's files were changed outside Atlys. Your data was not opened."
        case .WeakPassword:
            "Choose a stronger password: at least \(passwordMinLength()) characters that are hard to guess."
        case .InvalidRecoveryPhrase:
            "That recovery phrase isn't valid. Check each of the 24 words and their order."
        case .NoVault:
            "No vault was found on this iPhone."
        case .VaultExists:
            "A vault already exists on this iPhone."
        case .UpdateRequired:
            "This vault was created by a newer version of Atlys. Update the app to open it."
        case .Locked:
            "The vault is locked."
        case .WrongKeyOrTampered:
            "The data could not be decrypted."
        case .InvalidInput(let message), .Storage(let message), .Internal(let message):
            "Something went wrong: \(message)"
        }
    }
}
