@testable import AtlysUI
import Testing

@Suite("Display names")
struct DisplayNamesTests {
    @Test("HealthKit identifiers become readable names")
    func subtypes() {
        #expect(DisplayNames.subtype("HKQuantityTypeIdentifierStepCount") == "Step Count")
        #expect(DisplayNames.subtype("HKQuantityTypeIdentifierHeartRateVariabilitySDNN") == "Heart Rate Variability SDNN")
        #expect(DisplayNames.subtype("HKQuantityTypeIdentifierVO2Max") == "VO2 Max")
        #expect(DisplayNames.subtype("HKCategoryTypeIdentifierSleepAnalysis") == "Sleep Analysis")
        #expect(DisplayNames.subtype("HKQuantityTypeIdentifierForcedExpiratoryVolume1") == "Forced Expiratory Volume 1")
        #expect(DisplayNames.subtype("custom") == "custom")
    }

    @Test("Record types have names and families")
    func types() {
        #expect(DisplayNames.type("health.workout") == "Workouts")
        #expect(DisplayNames.type("future.type") == "future.type")
        #expect(DisplayNames.family("device.battery") == "Device and motion")
        #expect(DisplayNames.family("health.quantity") == "Apple Health")
    }
}
